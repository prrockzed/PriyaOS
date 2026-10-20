//! UI screenshots (sub-phase 0.11, DS-04, QA-09): every gallery story in
//! light and dark at scale 1.0 and 2.0, compared with the baselines in
//! `ui/tests/baselines/<Component>/<state>/<theme>@<scale>.png`.
//!
//! - The gallery renders the pictures in shot mode (`ui/gallery/Main.qml
//!   --all`), one process per scale. On Linux it runs under `cage` with the
//!   headless wlroots backend, so CI and the Zenbook draw with Mesa's
//!   software OpenGL; fonts are pinned by `ui/tests/fontconfig/fonts.conf`.
//!   Elsewhere (the Mac) it opens a real window, and the pictures won't
//!   match Linux baselines.
//! - `dssim` measures each difference; `ui/tests/thresholds.toml` holds the
//!   limits (default 0.002). A mismatch writes a diff picture.
//! - Everything lands in `target/ui-shots/`: the pictures, and for each
//!   picture that differs `before/` (the baseline), `after/` and `diff/`
//!   under the same name, plus `report.md`. CI's PR report embeds them.
//!
//! `cargo xtask test ui --shots` runs it; `--update-baselines` copies the
//! pictures over the baselines (compressed with oxipng when it's
//! installed). The `ui_shots_match_baselines` test runs it in CI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::cmd::{files_under, git, read, relative};
use crate::gallery;

pub const BASELINES: &str = "ui/tests/baselines";
const THRESHOLDS: &str = "ui/tests/thresholds.toml";
const FONTS: &str = "ui/tests/fontconfig/fonts.conf";
const STORIES: &str = "ui/gallery/stories";
pub const OUT: &str = "target/ui-shots";
const SCALES: [&str; 2] = ["1", "2"];
const THEMES: [&str; 2] = ["light", "dark"];
const DEFAULT_THRESHOLD: f64 = 0.002;
/// No story may accept more difference than this.
const MAX_THRESHOLD: f64 = 0.05;
/// The pull request label with which the owner approves changed pictures.
pub const APPROVAL_LABEL: &str = "visual-change-approved";

/// One picture: a story in a theme at a scale.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Shot {
    /// `Button/hover`.
    pub story: String,
    pub theme: &'static str,
    pub scale: &'static str,
}

impl Shot {
    /// `<Component>/<state>/<theme>@<scale>.png`, under the baselines or
    /// `diff/`.
    pub fn name(&self) -> String {
        format!("{}/{}@{}.png", self.story, self.theme, self.scale)
    }

    fn rendered(&self, out: &Path) -> PathBuf {
        out.join(self.scale)
            .join(&self.story)
            .join(format!("{}.png", self.theme))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Same(f64),
    Changed(f64),
    /// Different width or height.
    Resized((u32, u32), (u32, u32)),
    NoBaseline,
}

/// What a run found.
#[derive(Debug, Default)]
pub struct Report {
    pub verdicts: BTreeMap<String, Verdict>,
    /// Baselines of stories, themes or scales that no longer exist.
    pub stale: Vec<String>,
}

impl Report {
    pub fn problems(&self) -> usize {
        self.stale.len()
            + self
                .verdicts
                .values()
                .filter(|v| !matches!(v, Verdict::Same(_)))
                .count()
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::from("### UI screenshots\n\n");
        let total = self.verdicts.len();
        if self.problems() == 0 {
            let _ = writeln!(md, "All {total} pictures match their baselines.");
            return md;
        }
        let _ = writeln!(
            md,
            "{} of {total} pictures differ from their baselines. Diff pictures are in the `ui-shots` artifact (`diff/`).\n",
            self.problems()
        );
        md.push_str("| Picture | Result |\n|---|---|\n");
        for (name, verdict) in &self.verdicts {
            let result = match verdict {
                Verdict::Same(_) => continue,
                Verdict::Changed(score) => format!("changed (dssim {score:.5})"),
                Verdict::Resized(was, now) => {
                    format!("size {}×{} → {}×{}", was.0, was.1, now.0, now.1)
                }
                Verdict::NoBaseline => "new: no baseline yet".to_owned(),
            };
            let _ = writeln!(md, "| `{name}` | {result} |");
        }
        for name in &self.stale {
            let _ = writeln!(md, "| `{name}` | baseline of a story that's gone |");
        }
        md
    }
}

/// Renders, compares (or updates) and writes the report. Fails on any
/// difference unless `update`.
pub fn run(root: &Path, update: bool) -> Result<Report> {
    let stories = list_stories(root)?;
    check_catalog(root, &stories)?;
    let out = root.join(OUT);
    if out.exists() {
        fs::remove_dir_all(&out).with_context(|| format!("clearing {}", out.display()))?;
    }
    let shots = all_shots(&stories);
    for scale in SCALES {
        render(root, &out, scale, &stories)?;
    }
    let missing: Vec<String> = shots
        .iter()
        .filter(|s| !s.rendered(&out).is_file())
        .map(Shot::name)
        .collect();
    if !missing.is_empty() {
        bail!(
            "the gallery didn't write {} picture(s), e.g. {}",
            missing.len(),
            missing[0]
        );
    }

    let baselines = root.join(BASELINES);
    if update {
        update_baselines(root, &out, &shots)?;
    }
    let thresholds = thresholds(&read(&root.join(THRESHOLDS))?)?;
    let mut report = Report::default();
    for shot in &shots {
        let verdict = compare(
            &baselines.join(shot.name()),
            &shot.rendered(&out),
            &out.join("diff").join(shot.name()),
            threshold(&thresholds, &shot.story),
        )?;
        if !matches!(verdict, Verdict::Same(_)) {
            keep_for_review(&baselines, &out, shot)?;
        }
        report.verdicts.insert(shot.name(), verdict);
    }
    let expected: BTreeSet<PathBuf> = shots.iter().map(|s| baselines.join(s.name())).collect();
    report.stale = files_under(std::slice::from_ref(&baselines), &[])?
        .into_iter()
        .filter(|f| f.extension().is_some_and(|e| e == "png") && !expected.contains(f))
        .map(|f| relative(&baselines, &f))
        .collect();
    if update {
        for name in &report.stale {
            fs::remove_file(baselines.join(name)).with_context(|| format!("removing {name}"))?;
        }
        report.stale.clear();
    }

    let markdown = report.to_markdown();
    fs::write(out.join("report.md"), &markdown).context("writing report.md")?;
    println!("{markdown}");
    if report.problems() > 0 {
        bail!(
            "{} picture(s) differ from the baselines; see {OUT}/report.md and {OUT}/diff/",
            report.problems()
        );
    }
    Ok(report)
}

/// Copies a differing picture and its baseline (if any) to `after/` and
/// `before/`, named like the baseline, for the PR report.
fn keep_for_review(baselines: &Path, out: &Path, shot: &Shot) -> Result<()> {
    let copies = [
        (
            baselines.join(shot.name()),
            out.join("before").join(shot.name()),
        ),
        (shot.rendered(out), out.join("after").join(shot.name())),
    ];
    for (from, to) in copies {
        if !from.is_file() {
            continue;
        }
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
        }
        fs::copy(&from, &to).with_context(|| format!("copying {}", from.display()))?;
    }
    Ok(())
}

/// Every story id (`Button/hover`), from the gallery itself.
fn list_stories(root: &Path) -> Result<Vec<String>> {
    let output = gallery::command(root, "1", &["--list".to_owned()])?
        .env("QT_QPA_PLATFORM", "offscreen")
        .stdin(Stdio::null())
        .output()
        .context("starting the gallery to list its stories")?;
    let text = String::from_utf8_lossy(&output.stderr).into_owned()
        + &String::from_utf8_lossy(&output.stdout);
    let stories = parse_story_list(&text);
    if !output.status.success() || stories.is_empty() {
        bail!("the gallery couldn't list its stories:\n{text}");
    }
    Ok(stories)
}

/// The ids in `--list` output, which prints `story: <id>` lines.
pub fn parse_story_list(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.split_once("story: ").map(|(_, id)| id.trim().to_owned()))
        .filter(|id| id.contains('/'))
        .collect()
}

/// Every stories file is in the gallery's catalog. (The other way round,
/// the gallery fails to list a catalog entry without a file.)
fn check_catalog(root: &Path, stories: &[String]) -> Result<()> {
    let listed: BTreeSet<&str> = stories
        .iter()
        .filter_map(|s| s.split_once('/').map(|(c, _)| c))
        .collect();
    let files: BTreeSet<String> = files_under(&[root.join(STORIES)], &[])?
        .iter()
        .filter_map(|f| {
            f.file_name()?
                .to_str()?
                .strip_suffix(".stories.qml")
                .map(str::to_owned)
        })
        .collect();
    let unlisted: Vec<&String> = files
        .iter()
        .filter(|f| !listed.contains(f.as_str()))
        .collect();
    if !unlisted.is_empty() {
        bail!("stories not in ui/gallery/Catalog.js: {unlisted:?}");
    }
    Ok(())
}

fn all_shots(stories: &[String]) -> Vec<Shot> {
    let mut shots = Vec::new();
    for story in stories {
        for scale in SCALES {
            for theme in THEMES {
                shots.push(Shot {
                    story: story.clone(),
                    theme,
                    scale,
                });
            }
        }
    }
    shots.sort();
    shots
}

/// One gallery run that writes every story in both themes at `scale`.
fn render(root: &Path, out: &Path, scale: &str, stories: &[String]) -> Result<()> {
    let dir = out.join(scale);
    for story in stories {
        fs::create_dir_all(dir.join(story)).context("making the picture folders")?;
    }
    let args = [
        "--all".to_owned(),
        "--out-dir".to_owned(),
        dir.display().to_string(),
    ];
    let gallery = gallery::command(root, scale, &args)?;
    let mut command = headless(root, gallery)?;
    println!("==> pictures at scale {scale}");
    let status = command
        .stdin(Stdio::null())
        .status()
        .context("starting the gallery (cage and qml must be installed)")?;
    // cage doesn't always pass on the client's status; missing pictures
    // are caught by the caller either way.
    if !status.success() {
        eprintln!("warning: the gallery exited with {status} at scale {scale}");
    }
    Ok(())
}

/// On Linux, `command` wrapped in a headless cage session with pinned
/// fonts; elsewhere, unchanged.
fn headless(root: &Path, command: Command) -> Result<Command> {
    if !cfg!(target_os = "linux") {
        return Ok(command);
    }
    let runtime = root.join("target/ui-shots-runtime");
    fs::create_dir_all(&runtime).context("making XDG_RUNTIME_DIR")?;
    set_private(&runtime)?;
    let mut cage = Command::new("cage");
    cage.current_dir(root)
        .envs(command.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("WLR_BACKENDS", "headless")
        .env("WLR_RENDERER", "pixman")
        .env("WLR_LIBINPUT_NO_DEVICES", "1")
        .env("WLR_HEADLESS_OUTPUTS", "1")
        .env("QT_QPA_PLATFORM", "wayland")
        .env("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1")
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("FONTCONFIG_FILE", root.join(FONTS))
        .arg("--")
        .arg(command.get_program())
        .args(command.get_args());
    Ok(cage)
}

#[cfg(unix)]
fn set_private(dir: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("restricting {}", dir.display()))
}

#[cfg(not(unix))]
fn set_private(_dir: &Path) -> Result<()> {
    Ok(())
}

/// Copies the pictures over the baselines, compressed when oxipng exists.
fn update_baselines(root: &Path, out: &Path, shots: &[Shot]) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("baselines come from the CI image (Linux); pictures made here would never match it");
    }
    let baselines = root.join(BASELINES);
    let mut written = Vec::new();
    for shot in shots {
        let target = baselines.join(shot.name());
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
        }
        fs::copy(shot.rendered(out), &target)
            .with_context(|| format!("copying {}", shot.name()))?;
        written.push(target);
    }
    let oxipng = Command::new("oxipng")
        .args(["--quiet", "--strip", "safe", "-o", "2"])
        .args(&written)
        .status();
    match oxipng {
        Ok(status) if status.success() => println!("Compressed {} baselines.", written.len()),
        _ => println!("oxipng isn't installed; baselines are stored uncompressed."),
    }
    Ok(())
}

/// The limits in thresholds.toml: `default` and a `[stories]` table of
/// `"Component/state" = limit`.
pub fn thresholds(text: &str) -> Result<BTreeMap<String, f64>> {
    let doc: toml_edit::DocumentMut = text.parse().context("reading thresholds.toml")?;
    let mut limits = BTreeMap::new();
    let number = |item: &toml_edit::Item, key: &str| {
        item.as_float()
            .or_else(|| item.as_integer().map(|i| i as f64))
            .with_context(|| format!("thresholds.toml: {key} must be a number"))
    };
    if let Some(default) = doc.get("default") {
        limits.insert(String::new(), number(default, "default")?);
    }
    if let Some(stories) = doc.get("stories").and_then(|s| s.as_table_like()) {
        for (key, item) in stories.iter() {
            limits.insert(key.to_owned(), number(item, key)?);
        }
    }
    // A huge or NaN limit would let any picture pass.
    if let Some((key, limit)) = limits
        .iter()
        .find(|(_, l)| !(0.0..=MAX_THRESHOLD).contains(*l))
    {
        let key = if key.is_empty() { "default" } else { key };
        bail!("thresholds.toml: {key} = {limit} is outside 0 to {MAX_THRESHOLD}");
    }
    Ok(limits)
}

fn threshold(limits: &BTreeMap<String, f64>, story: &str) -> f64 {
    limits
        .get(story)
        .or_else(|| limits.get(""))
        .copied()
        .unwrap_or(DEFAULT_THRESHOLD)
}

/// Compares a picture with its baseline using dssim, writing a diff picture
/// when they differ.
fn compare(baseline: &Path, shot: &Path, diff: &Path, limit: f64) -> Result<Verdict> {
    if !baseline.is_file() {
        return Ok(Verdict::NoBaseline);
    }
    let (was, now) = (png_size(baseline)?, png_size(shot)?);
    if was != now {
        return Ok(Verdict::Resized(was, now));
    }
    if let Some(dir) = diff.parent() {
        fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    let output = Command::new("dssim")
        .arg("-o")
        .arg(diff)
        .arg(baseline)
        .arg(shot)
        .output()
        .context("starting dssim")?;
    if !output.status.success() {
        bail!(
            "dssim failed on {}: {}",
            shot.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let score = parse_dssim(&String::from_utf8_lossy(&output.stdout))
        .with_context(|| format!("reading dssim's score for {}", shot.display()))?;
    if score > limit {
        return Ok(Verdict::Changed(score));
    }
    // Only differences keep a diff picture.
    let _ = fs::remove_file(diff);
    Ok(Verdict::Same(score))
}

/// Files whose change alters what the pictures show or what passes.
const GUARDED: [&str; 3] = [BASELINES, THRESHOLDS, FONTS];

/// The rule for baseline changes: a pull request that changes, renames or
/// removes a baseline, or changes the thresholds or the pinned fonts, needs
/// the owner's `visual-change-approved` label. Added baselines (new stories)
/// don't. CI's `approval-reset` job removes the label after every push to
/// a pull request that changes these files, so the label always covers
/// what the owner last saw. The
/// before and after pictures of every changed baseline are written to
/// target/ui-shots/ for the PR report either way.
pub fn guard(root: &Path, base: &str, labels: &[String]) -> Result<()> {
    let merge_base = git_text(root, &["merge-base", base, "HEAD"])?;
    let merge_base = merge_base.trim();
    let mut diff = git(root);
    diff.args([
        "diff",
        "--name-status",
        "--no-renames",
        "-z",
        merge_base,
        "HEAD",
        "--",
    ])
    .args(GUARDED);
    let changed = needs_approval(&run_git(&mut diff)?);
    if changed.is_empty() {
        println!("No baseline picture, threshold or font setting changed.");
        return Ok(());
    }
    write_review_pictures(root, merge_base, &changed)?;
    let listing: Vec<String> = changed
        .iter()
        .map(|c| format!("`{}` ({})", c.path, c.status))
        .collect();
    if !labels.iter().any(|l| l == APPROVAL_LABEL) {
        for line in &listing {
            eprintln!("  {line}");
        }
        bail!(
            "{} screenshot file(s) changed without the `{APPROVAL_LABEL}` label; the owner adds it after looking at the before, after and diff pictures",
            changed.len()
        );
    }
    println!(
        "{} screenshot file(s) changed, approved with `{APPROVAL_LABEL}`.",
        changed.len()
    );
    Ok(())
}

/// A guarded file that changed, from `git diff --name-status -z`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub status: String,
    pub path: String,
}

/// Changed or removed baseline pictures, and any change to the other
/// guarded files. NUL-separated, so no path is quoted.
pub fn needs_approval(name_status_z: &str) -> Vec<Change> {
    let fields: Vec<&str> = name_status_z
        .split('\0')
        .filter(|f| !f.is_empty())
        .collect();
    fields
        .chunks(2)
        .filter_map(|pair| match pair {
            [status, path] => Some(Change {
                status: (*status).to_owned(),
                path: (*path).to_owned(),
            }),
            _ => None,
        })
        .filter(|c| {
            if c.path.starts_with(BASELINES) {
                c.path.ends_with(".png") && !c.status.starts_with('A')
            } else {
                true
            }
        })
        .collect()
}

/// `before/` (from the merge base) and `after/` (this commit) copies of
/// each changed baseline, plus a diff picture when dssim can make one.
fn write_review_pictures(root: &Path, merge_base: &str, changed: &[Change]) -> Result<()> {
    let out = root.join(OUT);
    let mut section = String::from("\n### Screenshot files this PR changes\n\n");
    for change in changed {
        // The path goes into the PR comment: plain names only.
        if !plain_name(&change.path) {
            bail!(
                "{} isn't a plain file name (letters, digits, ._@-/)",
                change.path
            );
        }
        let _ = writeln!(section, "- `{}` ({})", change.path, change.status);
        let Some(name) = change.path.strip_prefix(&format!("{BASELINES}/")) else {
            continue;
        };
        // Separate from the harness's own before/after, which compare the
        // rendering with the new baseline.
        let before = out.join("baseline-before").join(name);
        let after = out.join("baseline-after").join(name);
        for path in [&before, &after] {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
            }
        }
        let old = git(root)
            .args(["show", &format!("{merge_base}:{}", change.path)])
            .output()
            .context("running git show")?;
        if old.status.success() {
            fs::write(&before, &old.stdout)
                .with_context(|| format!("writing {}", before.display()))?;
        }
        let new = root.join(&change.path);
        if new.is_file() {
            fs::copy(&new, &after).with_context(|| format!("copying {}", new.display()))?;
        }
        if before.is_file() && after.is_file() {
            // Best effort: a size change has no diff picture.
            let _ = compare(&before, &after, &out.join("baseline-diff").join(name), 0.0);
        }
    }
    let report = out.join("report.md");
    let mut text = fs::read_to_string(&report).unwrap_or_default();
    text.push_str(&section);
    fs::create_dir_all(&out).with_context(|| format!("making {}", out.display()))?;
    fs::write(&report, text).context("writing report.md")
}

/// Letters, digits and `._@-/`, as the PR report publishes.
fn plain_name(path: &str) -> bool {
    !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._@-/".contains(c))
}

fn git_text(root: &Path, args: &[&str]) -> Result<String> {
    run_git(git(root).args(args))
}

fn run_git(command: &mut Command) -> Result<String> {
    let output = command.output().context("running git")?;
    if !output.status.success() {
        bail!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// dssim prints `<score>\t<file>`.
pub fn parse_dssim(output: &str) -> Option<f64> {
    output.split_whitespace().next()?.parse().ok()
}

/// Width and height from a PNG's IHDR chunk.
pub fn png_size(path: &Path) -> Result<(u32, u32)> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    png_header_size(&bytes).with_context(|| format!("{} isn't a PNG", path.display()))
}

fn png_header_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.get(..8)? != SIGNATURE || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes)
    };
    Some((word(16)?, word(20)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-09
    // suite: unit
    #[test]
    fn shot_helpers_read_lists_thresholds_scores_and_sizes() {
        let listing = "qt.qpa.fonts: noise\nqml: story: Button/hover\nqml: story: Toast/kinds\nqml: story: broken\n";
        assert_eq!(parse_story_list(listing), ["Button/hover", "Toast/kinds"]);

        let limits = thresholds("default = 0.002\n[stories]\n\"Toast/kinds\" = 0.004\n").unwrap();
        assert!((threshold(&limits, "Toast/kinds") - 0.004).abs() < 1e-9);
        assert!((threshold(&limits, "Button/hover") - 0.002).abs() < 1e-9);
        assert!((threshold(&BTreeMap::new(), "x/y") - DEFAULT_THRESHOLD).abs() < 1e-9);
        assert!(thresholds("default = \"high\"").is_err());

        assert_eq!(parse_dssim("0.00123\tshot.png\n"), Some(0.00123));
        assert_eq!(parse_dssim(""), None);

        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&1152u32.to_be_bytes());
        png.extend_from_slice(&460u32.to_be_bytes());
        assert_eq!(png_header_size(&png), Some((1152, 460)));
        assert_eq!(png_header_size(b"GIF89a"), None);

        let shot = Shot {
            story: "Button/hover".into(),
            theme: "dark",
            scale: "2",
        };
        assert_eq!(shot.name(), "Button/hover/dark@2.png");
        assert_eq!(
            shot.rendered(Path::new("out")),
            Path::new("out/2/Button/hover/dark.png")
        );
        assert_eq!(
            all_shots(&["A/b".to_owned()]).len(),
            4,
            "2 themes × 2 scales"
        );
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn changed_baselines_need_the_approval_label() {
        let diff = "A\0ui/tests/baselines/New/x/light@1.png\0M\0ui/tests/baselines/Button/hover/dark@2.png\0D\0ui/tests/baselines/Old/हिंदी/light@1.png\0M\0ui/tests/baselines/README.md\0M\0ui/tests/thresholds.toml\0";
        let paths: Vec<String> = needs_approval(diff).into_iter().map(|c| c.path).collect();
        assert_eq!(
            paths,
            [
                "ui/tests/baselines/Button/hover/dark@2.png",
                "ui/tests/baselines/Old/हिंदी/light@1.png",
                "ui/tests/thresholds.toml"
            ],
            "non-ASCII paths and thresholds count; new pictures don't"
        );
        assert!(plain_name("ui/tests/baselines/A/b/dark@2.png"));
        assert!(!plain_name("ui/tests/baselines/A/`x`/dark@2.png"));

        // A real repository: adding is fine, changing needs the label.
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let sh = |args: &[&str]| {
            let status = git(repo)
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        sh(&["init", "-q", "-b", "main"]);
        sh(&["config", "user.email", "t@example.org"]);
        sh(&["config", "user.name", "T"]);
        let picture = repo.join(BASELINES).join("A/b/light@1.png");
        fs::create_dir_all(picture.parent().unwrap()).unwrap();
        fs::write(&picture, "one").unwrap();
        sh(&["add", "."]);
        sh(&["commit", "-q", "-m", "base"]);
        sh(&["checkout", "-q", "-b", "change"]);
        fs::write(picture.with_file_name("dark@1.png"), "new").unwrap();
        sh(&["add", "."]);
        sh(&["commit", "-q", "-m", "add"]);
        guard(repo, "main", &[]).unwrap();
        fs::write(&picture, "two").unwrap();
        sh(&["commit", "-q", "-am", "change"]);
        let err = guard(repo, "main", &[]).unwrap_err().to_string();
        assert!(
            err.contains("1 screenshot file(s) changed without"),
            "{err}"
        );
        guard(repo, "main", &[APPROVAL_LABEL.to_owned()]).unwrap();
        // The owner sees what changed.
        let out = repo.join(OUT);
        assert_eq!(
            fs::read_to_string(out.join("baseline-before/A/b/light@1.png")).unwrap(),
            "one"
        );
        assert_eq!(
            fs::read_to_string(out.join("baseline-after/A/b/light@1.png")).unwrap(),
            "two"
        );
        assert!(
            fs::read_to_string(out.join("report.md"))
                .unwrap()
                .contains("A/b/light@1.png` (M)")
        );
        assert!(guard(repo, "no-such-branch", &[]).is_err());
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn thresholds_must_be_small_finite_numbers() {
        assert!(
            thresholds("default = 1.0").is_err(),
            "would accept anything"
        );
        assert!(thresholds("default = nan").is_err());
        assert!(thresholds("default = -0.1").is_err());
        assert!(thresholds("[stories]\n\"A/b\" = 0.06").is_err());
        assert!(thresholds("default = 0.05").is_ok());
    }

    // Linux only: elsewhere updating is refused.
    // covers: QA-09
    // suite: unit
    #[cfg(target_os = "linux")]
    #[test]
    fn update_copies_every_picture_over_the_baselines() {
        let dir = tempfile::tempdir().unwrap();
        let (root, out) = (dir.path(), dir.path().join("out"));
        let shots = all_shots(&["A/b".to_owned()]);
        for shot in &shots {
            fs::create_dir_all(shot.rendered(&out).parent().unwrap()).unwrap();
            fs::write(shot.rendered(&out), shot.name()).unwrap();
        }
        update_baselines(root, &out, &shots).unwrap();
        for shot in &shots {
            let baseline = root.join(BASELINES).join(shot.name());
            assert_eq!(fs::read_to_string(baseline).unwrap(), shot.name());
        }
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn missing_and_resized_pictures_need_no_dssim() {
        let dir = tempfile::tempdir().unwrap();
        let png = |w: u32, h: u32| {
            let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
            bytes.extend_from_slice(b"IHDR");
            bytes.extend_from_slice(&w.to_be_bytes());
            bytes.extend_from_slice(&h.to_be_bytes());
            bytes
        };
        let (base, shot) = (dir.path().join("base.png"), dir.path().join("shot.png"));
        fs::write(&shot, png(10, 10)).unwrap();
        let diff = dir.path().join("diff.png");
        assert_eq!(
            compare(&base, &shot, &diff, 0.002).unwrap(),
            Verdict::NoBaseline
        );
        fs::write(&base, png(12, 10)).unwrap();
        assert_eq!(
            compare(&base, &shot, &diff, 0.002).unwrap(),
            Verdict::Resized((12, 10), (10, 10))
        );
        fs::write(&shot, "not a picture").unwrap();
        assert!(compare(&base, &shot, &diff, 0.002).is_err());
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn differing_pictures_are_kept_with_their_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let (baselines, out) = (dir.path().join("base"), dir.path().join("out"));
        let shot = Shot {
            story: "A/b".into(),
            theme: "light",
            scale: "1",
        };
        fs::create_dir_all(shot.rendered(&out).parent().unwrap()).unwrap();
        fs::write(shot.rendered(&out), "new").unwrap();
        keep_for_review(&baselines, &out, &shot).unwrap();
        assert!(out.join("after/A/b/light@1.png").is_file());
        assert!(
            !out.join("before/A/b/light@1.png").exists(),
            "no baseline yet"
        );
        fs::create_dir_all(baselines.join("A/b")).unwrap();
        fs::write(baselines.join("A/b/light@1.png"), "old").unwrap();
        keep_for_review(&baselines, &out, &shot).unwrap();
        assert_eq!(
            fs::read_to_string(out.join("before/A/b/light@1.png")).unwrap(),
            "old"
        );
    }

    // covers: QA-09
    // suite: unit
    #[test]
    fn shot_report_lists_every_difference() {
        let mut report = Report::default();
        report
            .verdicts
            .insert("A/b/light@1.png".into(), Verdict::Same(0.0001));
        assert_eq!(report.problems(), 0);
        assert!(report.to_markdown().contains("All 1 pictures match"));
        report
            .verdicts
            .insert("A/b/dark@1.png".into(), Verdict::Changed(0.01));
        report
            .verdicts
            .insert("A/c/dark@2.png".into(), Verdict::NoBaseline);
        report.verdicts.insert(
            "A/d/dark@2.png".into(),
            Verdict::Resized((10, 10), (12, 10)),
        );
        report.stale.push("Old/x/light@1.png".into());
        let md = report.to_markdown();
        assert_eq!(report.problems(), 4);
        assert!(
            md.contains("| `A/b/dark@1.png` | changed (dssim 0.01000) |"),
            "{md}"
        );
        assert!(md.contains("new: no baseline yet"));
        assert!(md.contains("size 10×10 → 12×10"));
        assert!(md.contains("`Old/x/light@1.png` | baseline of a story that's gone"));
        assert!(!md.contains("A/b/light@1.png"), "matches aren't listed");
    }

    // The real thing: render every story under cage and compare. Linux
    // only, like the CI image that made the baselines.
    // covers: DS-04, QA-09
    // suite: ui
    // acceptance: 0.11
    #[cfg(target_os = "linux")]
    #[test]
    fn ui_shots_match_baselines() {
        let root = crate::cmd::workspace_root().unwrap();
        let report = run(&root, false).unwrap();
        assert!(report.verdicts.len() >= 4);
    }
}
