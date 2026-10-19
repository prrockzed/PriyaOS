//! Rust tests. Each lib, bin and test target is walked from its root file
//! (as cargo metadata reports it) along its `mod` declarations, so every
//! file is tied to the exact nextest binary that compiles it. In each file,
//! `syn` finds the `#[test]` / `#[tokio::test]` functions; their tags are
//! the `//` comments above their attributes (syn drops ordinary comments, so
//! those are read from the lines).

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use syn::visit::Visit;

use super::{Runner, TestCase, parse_tags};
use crate::scope::{Package, Target, Workspace};

/// A source file and the compile target it belongs to.
pub struct RustFile {
    pub path: PathBuf,
    package: String,
    /// The nextest binary id: `pkg` (library), `pkg::bin/name` or
    /// `pkg::name` (integration test).
    binary_id: String,
    /// Modules from the target's root file down to this file.
    modules: Vec<String>,
    /// A `#[cfg(…)]` (other than `cfg(test)`) on the way to this file, so it
    /// may not be built on every platform or feature set.
    conditional: bool,
}

/// Every file of every lib, bin and integration-test target, plus one
/// problem per file that doesn't parse or is reached twice by one target.
pub fn module_tree(workspace: &Workspace) -> (Vec<RustFile>, Vec<String>) {
    let mut files = Vec::new();
    let mut problems = Vec::new();
    for package in &workspace.packages {
        for target in &package.targets {
            let Some(binary_id) = binary_id(package, target) else {
                continue;
            };
            // rustc rejects a module that includes itself; report it instead
            // of walking in circles.
            let mut visited: HashSet<PathBuf> = HashSet::new();
            let mut pending = vec![Pending {
                path: target.src_path.clone(),
                modules: Vec::new(),
                owns_dir: true,
                conditional: false,
            }];
            while let Some(next) = pending.pop() {
                let Ok(source) = fs::read_to_string(&next.path) else {
                    continue; // the other candidate file for this `mod`
                };
                if !visited.insert(next.path.clone()) {
                    problems.push(format!(
                        "{}: included twice by {binary_id} (a #[path] loop?)",
                        next.path.display()
                    ));
                    continue;
                }
                match syn::parse_file(&source) {
                    Ok(syntax) => {
                        let mut decls = ModDecls::default();
                        decls.visit_file(&syntax);
                        let base = children_dir(&next.path, next.owns_dir);
                        for decl in decls.found {
                            let mut modules = next.modules.clone();
                            modules.extend(decl.inline.iter().cloned());
                            modules.push(decl.name.clone());
                            let dir = decl.inline.iter().fold(base.clone(), |d, m| d.join(m));
                            for (path, owns_dir) in candidates(&next.path, &dir, &decl) {
                                pending.push(Pending {
                                    path,
                                    modules: modules.clone(),
                                    owns_dir,
                                    conditional: next.conditional || decl.conditional,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        problems.push(format!("{}: doesn't parse: {e}", next.path.display()));
                    }
                }
                files.push(RustFile {
                    path: next.path,
                    package: package.name.clone(),
                    binary_id: binary_id.clone(),
                    modules: next.modules,
                    conditional: next.conditional,
                });
            }
        }
    }
    (files, problems)
}

/// A file waiting to be read during the module walk.
struct Pending {
    path: PathBuf,
    modules: Vec<String>,
    owns_dir: bool,
    conditional: bool,
}

fn binary_id(package: &Package, target: &Target) -> Option<String> {
    let name = &package.name;
    match target.kind.as_str() {
        "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro" => Some(name.clone()),
        "bin" => Some(format!("{name}::bin/{}", target.name)),
        "test" => Some(format!("{name}::{}", target.name)),
        _ => None, // benches, examples and build scripts aren't run as tests
    }
}

/// Where `mod x;` looks for `x`. Root files and `mod.rs` own their folder;
/// any other `a.rs` keeps its children in `a/`.
fn children_dir(file: &Path, owns_dir: bool) -> PathBuf {
    let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    if owns_dir {
        dir
    } else {
        dir.join(file.file_stem().unwrap_or_default())
    }
}

/// The files `mod name;` may refer to, each with whether it owns its
/// folder. `#[path = "…"]` is relative to the declaring file's folder, or,
/// inside an inline module, to that module's folder.
fn candidates(declaring: &Path, dir: &Path, decl: &ModDecl) -> Vec<(PathBuf, bool)> {
    if let Some(path) = &decl.path {
        let base = if decl.inline.is_empty() {
            declaring.parent().unwrap_or(Path::new(""))
        } else {
            dir
        };
        return vec![(normalize(&base.join(path)), true)];
    }
    let name = &decl.name;
    vec![
        (dir.join(format!("{name}.rs")), false),
        (dir.join(name).join("mod.rs"), true),
    ]
}

/// Resolves `.` and `..` without touching the disk, so `tests/../src/a.rs`
/// compares equal to `src/a.rs`.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// A `#[cfg(…)]` other than plain `cfg(test)`: the item may be left out on
/// some platforms or feature sets.
fn is_conditional(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg")
            && match &a.meta {
                syn::Meta::List(list) => list.tokens.to_string().trim() != "test",
                _ => true,
            }
    })
}

/// A `mod x;` declaration.
struct ModDecl {
    /// Inline modules (`mod a { … }`) around the declaration.
    inline: Vec<String>,
    name: String,
    /// The `#[path = "…"]` attribute, if any.
    path: Option<String>,
    conditional: bool,
}

#[derive(Default)]
struct ModDecls {
    inline: Vec<String>,
    /// Inside an inline module with a platform or feature cfg.
    conditional: bool,
    found: Vec<ModDecl>,
}

impl<'ast> Visit<'ast> for ModDecls {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let name = item.ident.to_string();
        if item.content.is_some() {
            let outer = self.conditional;
            self.conditional |= is_conditional(&item.attrs);
            self.inline.push(name);
            syn::visit::visit_item_mod(self, item);
            self.inline.pop();
            self.conditional = outer;
        } else {
            self.found.push(ModDecl {
                inline: self.inline.clone(),
                name,
                path: path_attribute(&item.attrs),
                conditional: self.conditional || is_conditional(&item.attrs),
            });
        }
    }

    // Modules inside function bodies aren't part of the module tree.
    fn visit_item_fn(&mut self, _: &'ast syn::ItemFn) {}
}

fn path_attribute(attrs: &[syn::Attribute]) -> Option<String> {
    let attr = attrs.iter().find(|a| a.path().is_ident("path"))?;
    let syn::Meta::NameValue(nv) = &attr.meta else {
        return None;
    };
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(path),
        ..
    }) = &nv.value
    else {
        return None;
    };
    Some(path.value())
}

/// Every test function in `source`, or one error per malformed test.
pub fn tests_in(source: &str, file: &str, at: &RustFile) -> Vec<Result<TestCase, String>> {
    let syntax = match syn::parse_file(source) {
        Ok(syntax) => syntax,
        Err(e) => return vec![Err(format!("{file}: doesn't parse: {e}"))],
    };
    let mut finder = Finder {
        modules: at.modules.clone(),
        conditional: at.conditional,
        found: Vec::new(),
    };
    finder.visit_file(&syntax);

    let lines: Vec<&str> = source.lines().collect();
    finder
        .found
        .into_iter()
        .map(|found| {
            let Found {
                modules,
                function,
                first_line,
                conditional,
            } = found;
            let place = format!("{file}:{first_line}");
            let tags = parse_tags(&comment_block_above(&lines, first_line), &place)?;
            let name = modules
                .into_iter()
                .chain([function])
                .collect::<Vec<_>>()
                .join("::");
            Ok(TestCase {
                id: format!("{}::{name}", at.binary_id),
                file: file.to_owned(),
                line: first_line,
                conditional,
                tags,
                runner: Runner::Rust {
                    package: at.package.clone(),
                    binary_id: at.binary_id.clone(),
                    name,
                },
            })
        })
        .collect()
}

/// True when the source has test attributes; used for files outside every
/// target, whose tests would never run.
pub fn mentions_tests(source: &str) -> bool {
    source.contains("#[test]") || source.contains("#[tokio::test")
}

/// The `//` comments directly above line `first_line` (1-based), with the
/// `//` removed. Attributes and `///` doc comments in between are passed
/// over; any other line ends the block.
fn comment_block_above<'a>(lines: &[&'a str], first_line: usize) -> Vec<&'a str> {
    let mut block = Vec::new();
    for line in lines[..first_line.saturating_sub(1)].iter().rev() {
        let line = line.trim();
        if line.starts_with("#[") || line.starts_with("///") {
            continue;
        }
        match line.strip_prefix("//") {
            Some(rest) if !rest.starts_with('!') => block.push(rest),
            _ => break,
        }
    }
    block.reverse();
    block
}

/// Collects every test function.
struct Finder {
    modules: Vec<String>,
    conditional: bool,
    found: Vec<Found>,
}

struct Found {
    modules: Vec<String>,
    function: String,
    /// The line the tags must sit above.
    first_line: usize,
    conditional: bool,
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        // `mod x;` lives in its own file, which is indexed separately.
        if item.content.is_some() {
            let outer = self.conditional;
            self.conditional |= is_conditional(&item.attrs);
            self.modules.push(item.ident.to_string());
            syn::visit::visit_item_mod(self, item);
            self.modules.pop();
            self.conditional = outer;
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item.attrs.iter().any(is_test_attribute) {
            // Tags sit above the attributes. syn treats doc comments as
            // attributes too, so those don't mark the start.
            let first_line = item
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("doc"))
                .map(|a| a.pound_token.span.start().line)
                .chain([item.sig.fn_token.span.start().line])
                .min()
                .unwrap_or(1);
            self.found.push(Found {
                modules: self.modules.clone(),
                function: item.sig.ident.to_string(),
                first_line,
                conditional: self.conditional || is_conditional(&item.attrs),
            });
        }
        // A test nested in a function body never runs, so don't look there.
    }
}

/// `#[test]`, `#[tokio::test]`, `#[tokio::test(flavor = …)]`.
fn is_test_attribute(attr: &syn::Attribute) -> bool {
    let segments: Vec<String> = attr
        .path()
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    matches!(segments.as_slice(), [one] if one == "test")
        || matches!(segments.as_slice(), [a, b] if a == "tokio" && b == "test")
}

/// The set of paths in `files`, for finding files outside every target.
pub fn paths(files: &[RustFile]) -> BTreeSet<&Path> {
    files.iter().map(|f| f.path.as_path()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(binary_id: &str, modules: &[&str]) -> RustFile {
        RustFile {
            path: PathBuf::new(),
            package: "demo".into(),
            binary_id: binary_id.into(),
            modules: modules.iter().map(|m| (*m).to_owned()).collect(),
            conditional: false,
        }
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn rust_tests_get_module_path_and_tags() {
        let source = r#"
pub fn helper() {}

#[cfg(test)]
mod tests {
    /// Not a tag block: a doc comment.
    fn not_a_test() {}

    // covers: CORE-01
    // suite: unit
    /// Doc comments may sit between the tags and the attributes.
    #[test]
    #[ignore = "flaky: #3"]
    fn appends() {
        #[test]
        fn nested_never_runs() {}
    }

    mod inner {
        // covers: infra
        // suite: integration
        // acceptance: 0.5
        #[tokio::test(flavor = "multi_thread")]
        async fn runs() {}
    }

    #[test]
    fn untagged() {}
}
"#;
        let found = tests_in(
            source,
            "crates/demo/src/store.rs",
            &file("demo", &["store"]),
        );
        let ok: Vec<&TestCase> = found.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(ok.len(), 2);
        assert_eq!(ok[0].id, "demo::store::tests::appends");
        assert_eq!(ok[0].tags.covers, ["CORE-01"]);
        assert_eq!(ok[0].line, 12);
        assert_eq!(ok[1].id, "demo::store::tests::inner::runs");
        assert_eq!(ok[1].tags.acceptance.as_deref(), Some("0.5"));
        assert_eq!(
            ok[1].runner,
            Runner::Rust {
                package: "demo".into(),
                binary_id: "demo".into(),
                name: "store::tests::inner::runs".into(),
            }
        );
        let errors: Vec<&String> = found.iter().filter_map(|r| r.as_ref().err()).collect();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("crates/demo/src/store.rs:27: missing `covers:`"),
            "{}",
            errors[0]
        );
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn ids_carry_the_binary_so_lib_bin_and_test_files_never_collide() {
        let source = "// covers: infra\n// suite: unit\n#[test]\nfn works() {}\n";
        let id = |binary_id: &str| {
            tests_in(source, "x.rs", &file(binary_id, &[]))
                .remove(0)
                .unwrap()
                .id
        };
        assert_eq!(id("demo"), "demo::works");
        assert_eq!(id("demo::bin/demo"), "demo::bin/demo::works");
        assert_eq!(id("demo::alpha"), "demo::alpha::works");
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn module_tree_follows_mod_declarations_per_target() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write("src/lib.rs", "mod store;\nmod net { mod http; }\n");
        write("src/store.rs", "mod log;\n");
        write("src/store/log.rs", "");
        write("src/net/http/mod.rs", "");
        write("src/main.rs", "mod cli;\n");
        write("src/cli.rs", "");
        write("src/orphan.rs", "");
        write(
            "tests/alpha.rs",
            "#[path = \"common/util.rs\"]\nmod util;\n",
        );
        write("tests/common/util.rs", "");

        let target = |kind: &str, name: &str, src: &str| Target {
            kind: kind.into(),
            name: name.into(),
            src_path: root.join(src),
        };
        let workspace = Workspace::from_packages(vec![Package {
            name: "demo".into(),
            dir: root.to_path_buf(),
            targets: vec![
                target("lib", "demo", "src/lib.rs"),
                target("bin", "demo-cli", "src/main.rs"),
                target("test", "alpha", "tests/alpha.rs"),
            ],
        }]);
        let (files, problems) = module_tree(&workspace);
        assert!(problems.is_empty(), "{problems:?}");
        let mut seen: Vec<String> = files
            .iter()
            .map(|f| {
                format!(
                    "{} {} {}",
                    f.path.strip_prefix(root).unwrap().display(),
                    f.binary_id,
                    f.modules.join("::")
                )
            })
            .collect();
        seen.sort();
        assert_eq!(
            seen,
            [
                "src/cli.rs demo::bin/demo-cli cli",
                "src/lib.rs demo ",
                "src/main.rs demo::bin/demo-cli ",
                "src/net/http/mod.rs demo net::http",
                "src/store.rs demo store",
                "src/store/log.rs demo store::log",
                "tests/alpha.rs demo::alpha ",
                "tests/common/util.rs demo::alpha util",
            ]
        );
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn module_tree_handles_loops_parent_paths_and_cfg() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(
            "src/lib.rs",
            "#[cfg(target_os = \"linux\")]\nmod linux;\n#[path = \"lib.rs\"]\nmod again;\n",
        );
        write("src/linux.rs", "mod inner;\n");
        write("src/linux/inner.rs", "");
        write(
            "tests/alpha.rs",
            "#[path = \"../src/shared.rs\"]\nmod shared;\n",
        );
        write("src/shared.rs", "");

        let workspace = Workspace::from_packages(vec![Package {
            name: "demo".into(),
            dir: root.to_path_buf(),
            targets: vec![
                Target {
                    kind: "lib".into(),
                    name: "demo".into(),
                    src_path: root.join("src/lib.rs"),
                },
                Target {
                    kind: "test".into(),
                    name: "alpha".into(),
                    src_path: root.join("tests/alpha.rs"),
                },
            ],
        }]);
        let (files, problems) = module_tree(&workspace);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("included twice"), "{}", problems[0]);

        let find = |rel: &str| files.iter().find(|f| f.path == root.join(rel));
        assert!(
            find("src/linux/inner.rs").unwrap().conditional,
            "cfg is inherited"
        );
        assert!(!find("src/lib.rs").unwrap().conditional);
        let shared = find("src/shared.rs").expect("`..` is resolved");
        assert_eq!(shared.binary_id, "demo::alpha");
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn only_platform_and_feature_cfgs_make_a_test_conditional() {
        let source = "// covers: infra\n// suite: unit\n#[cfg(test)]\n#[test]\nfn plain() {}\n\n// covers: infra\n// suite: unit\n#[cfg(feature = \"x\")]\n#[test]\nfn gated() {}\n";
        let found: Vec<TestCase> = tests_in(source, "x.rs", &file("demo", &[]))
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        assert!(!found[0].conditional);
        assert!(found[1].conditional);
    }
}
