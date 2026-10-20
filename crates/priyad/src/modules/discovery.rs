//! Finding modules: one folder per module, each with a `module.toml`, in
//! `/usr/lib/priya/modules` (installed) and `$XDG_DATA_HOME/priya/modules`
//! (the user's own and development ones, which win over installed ones of
//! the same name).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use priya_api::module::{ModuleKind, ModuleManifest};
use serde_json::Value;

/// A module found on disk, with its manifest checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Discovered {
    pub manifest: ModuleManifest,
    /// The folder holding `module.toml`.
    pub dir: PathBuf,
    /// The program to run, for a process module (`exec` made absolute).
    pub exec: Option<PathBuf>,
    /// Its settings' JSON Schema (`provides.settings_schema`), if any.
    pub settings_schema: Option<Value>,
}

/// A folder that looks like a module but can't be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: PathBuf,
    pub message: String,
}

/// What was found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    /// By name.
    pub modules: BTreeMap<String, Discovered>,
    pub problems: Vec<Problem>,
}

/// Looks through `dirs` in order; a module in a later folder replaces one
/// of the same name in an earlier one. A folder that doesn't exist has no
/// modules.
pub fn discover(dirs: &[PathBuf]) -> Found {
    let mut found = Found::default();
    for dir in dirs {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                found
                    .problems
                    .push(problem(dir, format!("can't be read: {e}")));
                continue;
            }
        };
        let mut folders: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.join("module.toml").is_file())
            .collect();
        // The same order everywhere, so problems read the same each time.
        folders.sort();
        for folder in folders {
            match load(&folder) {
                Ok(module) => {
                    found
                        .modules
                        .insert(module.manifest.name().to_owned(), module);
                }
                Err(message) => found
                    .problems
                    .push(problem(&folder.join("module.toml"), message)),
            }
        }
    }
    found
}

fn problem(path: &Path, message: String) -> Problem {
    Problem {
        path: path.to_path_buf(),
        message,
    }
}

/// Reads one module's folder.
fn load(folder: &Path) -> Result<Discovered, String> {
    let text = fs::read_to_string(folder.join("module.toml"))
        .map_err(|e| format!("can't be read: {e}"))?;
    let manifest = ModuleManifest::parse(&text).map_err(|e| e.to_string())?;
    let name = manifest.name();
    if folder.file_name().and_then(|f| f.to_str()) != Some(name) {
        return Err(format!(
            "the module is called `{name}`, so its folder must be named `{name}` too"
        ));
    }
    let exec = match (manifest.module.kind, &manifest.module.exec) {
        (ModuleKind::Process, Some(exec)) => {
            let exec = folder.join(exec);
            if !exec.is_file() {
                return Err(format!("its program {} doesn't exist", exec.display()));
            }
            Some(exec)
        }
        _ => None,
    };
    let settings_schema = match &manifest.provides.settings_schema {
        Some(file) => Some(read_schema(&folder.join(file))?),
        None => None,
    };
    Ok(Discovered {
        dir: folder.to_path_buf(),
        exec,
        settings_schema,
        manifest,
    })
}

/// A settings schema: JSON, and a JSON Schema that compiles.
fn read_schema(path: &Path) -> Result<Value, String> {
    let shown = path.display();
    let text = fs::read_to_string(path)
        .map_err(|e| format!("its settings schema {shown} can't be read: {e}"))?;
    let schema: Value = serde_json::from_str(&text)
        .map_err(|e| format!("its settings schema {shown} isn't JSON: {e}"))?;
    jsonschema::validator_for(&schema)
        .map_err(|e| format!("its settings schema {shown} isn't a valid JSON Schema: {e}"))?;
    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ECHO: &str = "[module]\nname = \"echo\"\nversion = \"0.1.0\"\napi = \"v1\"\n\
                        kind = \"process\"\nexec = \"run\"\n";

    /// Writes a module folder `name` under `root` with `manifest`, its
    /// program and any extra files.
    fn module(root: &Path, name: &str, manifest: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("module.toml"), manifest).unwrap();
        fs::write(dir.join("run"), "").unwrap();
        for (file, text) in files {
            fs::write(dir.join(file), text).unwrap();
        }
        dir
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn modules_are_found_by_folder_and_user_ones_win() {
        let root = tempfile::tempdir().unwrap();
        let system = root.path().join("system");
        let user = root.path().join("user");
        let with_schema = format!("{ECHO}[provides]\nsettings_schema = \"settings.schema.json\"\n");
        let schema = r#"{"type": "object", "properties": {"prefix": {"type": "string"}}}"#;
        module(
            &system,
            "echo",
            &with_schema,
            &[("settings.schema.json", schema)],
        );
        module(&system, "other", &ECHO.replace("echo", "other"), &[]);
        let mine = module(&user, "echo", &ECHO.replace("0.1.0", "0.2.0"), &[]);
        // Not modules: a stray file, a folder without a manifest.
        fs::write(system.join("README"), "").unwrap();
        fs::create_dir_all(user.join("empty")).unwrap();

        let found = discover(&[system.clone(), user, root.path().join("missing")]);
        assert_eq!(found.problems, []);
        assert_eq!(found.modules.keys().collect::<Vec<_>>(), ["echo", "other"]);
        let echo = &found.modules["echo"];
        assert_eq!(echo.manifest.module.version, "0.2.0", "the user's wins");
        assert_eq!(echo.dir, mine);
        assert_eq!(echo.exec, Some(mine.join("run")));
        assert_eq!(echo.settings_schema, None);
        let found = discover(&[system]);
        let schema = found.modules["echo"].settings_schema.clone().unwrap();
        assert_eq!(schema["properties"]["prefix"]["type"], "string");
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn broken_modules_are_reported_one_by_one() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();
        module(dir, "bad", "[module]\nname = \"bad\"\n", &[]);
        module(dir, "elsewhere", ECHO, &[]);
        module(
            dir,
            "noprog",
            &ECHO
                .replace("echo", "noprog")
                .replace("\"run\"", "\"gone\""),
            &[],
        );
        let schema = |text: &str| {
            let manifest = format!(
                "{}[provides]\nsettings_schema = \"s.json\"\n",
                ECHO.replace("echo", "schema")
            );
            module(dir, "schema", &manifest, &[("s.json", text)]);
            discover(&[dir.to_path_buf()])
                .problems
                .into_iter()
                .find(|p| p.path.starts_with(dir.join("schema")))
                .map(|p| p.message)
        };
        module(dir, "good", &ECHO.replace("echo", "good"), &[]);

        let found = discover(&[dir.to_path_buf()]);
        assert_eq!(found.modules.keys().collect::<Vec<_>>(), ["good"]);
        let messages: Vec<&str> = found.problems.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(messages.len(), 3, "{messages:?}");
        assert!(messages[0].contains("missing field"), "{messages:?}");
        assert!(
            messages[1].contains("folder must be named `echo`"),
            "{messages:?}"
        );
        assert!(messages[2].contains("doesn't exist"), "{messages:?}");
        assert_eq!(found.problems[0].path, dir.join("bad/module.toml"));

        assert!(schema("not json").unwrap().contains("isn't JSON"));
        assert!(
            schema(r#"{"type": 5}"#)
                .unwrap()
                .contains("isn't a valid JSON Schema")
        );
        assert_eq!(schema("{}"), None);
        fs::remove_file(dir.join("schema/s.json")).unwrap();
        let found = discover(&[dir.to_path_buf()]);
        assert!(
            found
                .problems
                .iter()
                .any(|p| p.message.contains("can't be read"))
        );

        // A folder of modules that can't be read is a problem too.
        let file = dir.join("a-file");
        fs::write(&file, "").unwrap();
        let found = discover(std::slice::from_ref(&file));
        assert_eq!(found.problems.len(), 1);
        assert_eq!(found.problems[0].path, file);
    }
}
