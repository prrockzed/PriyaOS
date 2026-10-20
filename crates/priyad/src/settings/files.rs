//! Where settings files live, reading them, and writing one safely.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use priya_core::paths::PriyaPaths;
use priya_core::settings::{Layer, Source, project_file};

/// The folders of each settings layer (02-core §6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDirs {
    /// Shipped defaults: `/usr/share/priya/defaults`.
    pub defaults: PathBuf,
    /// The administrator's: `/etc/priya`.
    pub system: PathBuf,
    /// The owner's: `$XDG_CONFIG_HOME/priya`.
    pub user: PathBuf,
    /// Shortens paths in messages, and bounds the project-file search.
    pub home: Option<PathBuf>,
}

impl SettingsDirs {
    /// The installed locations, with the user folder from `paths`.
    pub fn installed(paths: &PriyaPaths, home: Option<PathBuf>) -> Self {
        Self {
            defaults: PathBuf::from("/usr/share/priya/defaults"),
            system: PathBuf::from("/etc/priya"),
            user: paths.config.clone(),
            home,
        }
    }

    /// The file `set` and `unset` change.
    pub fn user_file(&self) -> PathBuf {
        self.user.join("settings.toml")
    }

    fn layers(&self) -> [(Layer, &Path); 3] {
        [
            (Layer::Defaults, &self.defaults),
            (Layer::System, &self.system),
            (Layer::User, &self.user),
        ]
    }
}

/// What reading the settings folders found.
#[derive(Debug, Default)]
pub struct Read {
    pub sources: Vec<Source>,
    /// Files that exist but couldn't be read, with the reason.
    pub unreadable: Vec<(PathBuf, String)>,
}

/// Reads every `*.toml` in each layer's folder. A missing folder has no
/// settings.
pub fn read(dirs: &SettingsDirs) -> Read {
    let mut read = Read::default();
    for (layer, dir) in dirs.layers() {
        match toml_files(dir) {
            Ok(files) => {
                for file in files {
                    add(&mut read, layer, file);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => read.unreadable.push((dir.to_path_buf(), e.to_string())),
        }
    }
    read
}

/// Reads the project file for work in `cwd`, if there is one.
pub fn read_project(dirs: &SettingsDirs, cwd: &Path) -> Read {
    let mut read = Read::default();
    if let Some(home) = dirs.home.as_deref()
        && let Some(file) = project_file(cwd, home, Path::is_file)
    {
        add(&mut read, Layer::Project, file);
    }
    read
}

/// Removes temporary files that [`write_atomic`] left when it was killed
/// half way: `.<name>.tmp-<pid>-<n>`, in `dir` and beside the real file of
/// any settings symlink there.
pub fn sweep_temporary_files(dir: &Path) {
    let mut dirs = vec![dir.to_path_buf()];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_settings_file(&path)
                && entry.file_type().is_ok_and(|t| t.is_symlink())
                && let Some(real_dir) = fs::canonicalize(&path)
                    .ok()
                    .and_then(|real| real.parent().map(Path::to_path_buf))
                && !dirs.contains(&real_dir)
            {
                dirs.push(real_dir);
            }
        }
    }
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if is_ours(&entry.file_name().to_string_lossy()) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// A name `write_atomic` gives its temporary files.
fn is_ours(name: &str) -> bool {
    let Some((_, tail)) = name.strip_prefix('.').and_then(|n| n.rsplit_once(".tmp-")) else {
        return false;
    };
    let mut parts = tail.split('-');
    let numbers = parts
        .by_ref()
        .take(2)
        .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        .count();
    numbers == 2 && parts.next().is_none()
}

fn add(read: &mut Read, layer: Layer, file: PathBuf) {
    match fs::read_to_string(&file) {
        Ok(text) => read.sources.push(Source::toml(layer, file, text)),
        // Gone since the folder was listed: an editor saving, say.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => read.unreadable.push((file, e.to_string())),
    }
}

/// The `*.toml` files in `dir`, sorted. Hidden files (an editor's or our
/// own temporary ones) are skipped.
fn toml_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if is_settings_file(&path) && path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// A name the settings layers read: `*.toml`, not hidden.
pub fn is_settings_file(path: &Path) -> bool {
    let visible = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| !name.starts_with('.'));
    visible && path.extension().is_some_and(|ext| ext == "toml")
}

/// Replaces `path` with `text` so that a crash leaves either the old file or
/// the new one, never a mix: write a hidden temporary file beside it, flush
/// it to disk, rename it over the old one, then flush the folder.
///
/// A symlinked settings file (say into a dotfiles repository) is written
/// through: the file it points to changes and the link stays. The file
/// keeps its permissions.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let target = match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    let dir = target
        .parent()
        .ok_or_else(|| io::Error::other("a settings file needs a folder"))?;
    let made_dir = !dir.exists();
    fs::create_dir_all(dir)?;
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::other("a settings file needs a name"))?;
    let temp = dir.join(format!(
        ".{}.tmp-{}-{}",
        name.to_string_lossy(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let permissions = fs::metadata(&target).ok().map(|m| m.permissions());
    let written = (|| {
        let mut file = File::options().write(true).create_new(true).open(&temp)?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, &target)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written?;
    // The rename is only durable once the folder is, and a new folder only
    // once its parent is.
    File::open(dir)?.sync_all()?;
    if made_dir && let Some(parent) = dir.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn dirs(root: &Path) -> SettingsDirs {
        SettingsDirs {
            defaults: root.join("defaults"),
            system: root.join("etc"),
            user: root.join("home/.config/priya"),
            home: Some(root.join("home")),
        }
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn reads_each_layers_toml_files_in_order() {
        let root = tempfile::tempdir().unwrap();
        let dirs = dirs(root.path());
        for (dir, name, text) in [
            (&dirs.defaults, "00-core.toml", "a = 1"),
            (&dirs.user, "20-b.toml", "b = 2"),
            (&dirs.user, "10-a.toml", "c = 3"),
            (&dirs.user, ".settings.toml.tmp-1-2", "skipped"),
            (&dirs.user, "notes.txt", "skipped"),
        ] {
            fs::create_dir_all(dir).unwrap();
            fs::write(dir.join(name), text).unwrap();
        }
        let project = root.path().join("home/code/app");
        fs::create_dir_all(&project).unwrap();
        fs::write(root.path().join("home/code/.priya.toml"), "d = 4").unwrap();

        let mut found = read(&dirs);
        found.sources.extend(read_project(&dirs, &project).sources);
        assert!(found.unreadable.is_empty());
        let found: Vec<(Layer, String)> = found
            .sources
            .iter()
            .map(|s| {
                let name = s.file.as_ref().unwrap().file_name().unwrap();
                (s.layer, name.to_string_lossy().into_owned())
            })
            .collect();
        assert_eq!(
            found,
            [
                (Layer::Defaults, "00-core.toml".into()),
                (Layer::User, "10-a.toml".into()),
                (Layer::User, "20-b.toml".into()),
                (Layer::Project, ".priya.toml".into()),
            ],
            "the missing /etc folder has no settings"
        );
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn atomic_write_replaces_the_file_and_leaves_no_temporary_files() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("new/settings.toml");
        write_atomic(&file, "a = 1\n").unwrap();
        write_atomic(&file, "a = 2\n").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "a = 2\n");
        let names: Vec<_> = fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["settings.toml"]);
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn atomic_write_goes_through_symlinks_and_keeps_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("dotfiles/priya.toml");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "a = 1\n").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = root.path().join("settings.toml");
        symlink(&real, &link).unwrap();

        write_atomic(&link, "a = 2\n").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "a = 2\n");
        let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    // covers: CORE-03
    // suite: unit
    #[test]
    fn temporary_files_left_by_a_crash_are_swept() {
        // Only names `write_atomic` makes.
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(".settings.toml.tmp-99-0"), "half").unwrap();
        fs::write(root.path().join("settings.toml"), "a = 1").unwrap();
        fs::write(root.path().join(".notes.tmp-draft"), "someone else's").unwrap();
        sweep_temporary_files(root.path());
        let mut names: Vec<_> = fs::read_dir(root.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, [".notes.tmp-draft", "settings.toml"]);
    }
}
