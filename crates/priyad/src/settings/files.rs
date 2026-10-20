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

/// Reads every `*.toml` in each layer's folder, and the project file for
/// work in `cwd`, if any. A missing folder has no settings.
pub fn read(dirs: &SettingsDirs, cwd: Option<&Path>) -> Read {
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
    if let (Some(cwd), Some(home)) = (cwd, dirs.home.as_deref())
        && let Some(file) = project_file(cwd, home, Path::is_file)
    {
        add(&mut read, Layer::Project, file);
    }
    read
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
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("a settings file needs a folder"))?;
    fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("a settings file needs a name"))?;
    let temp = dir.join(format!(
        ".{}.tmp-{}-{}",
        name.to_string_lossy(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written?;
    // The rename is only durable once the folder is.
    File::open(dir)?.sync_all()
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

        let read = read(&dirs, Some(&project));
        assert!(read.unreadable.is_empty());
        let found: Vec<(Layer, String)> = read
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
}
