//! `cargo xtask gallery [--scale 2] [-- <gallery arguments>]` (0.11): opens
//! the PriyaUI gallery, `qml ui/gallery/Main.qml`, with `ui/` on the import
//! path. Scale 2.0 is `QT_SCALE_FACTOR=2`; the gallery's own arguments
//! (shot mode: `--story`, `--all`, `--list`) follow `--`.

use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};

use crate::cmd::run;
use crate::qml::qt_tool;

pub const MAIN: &str = "ui/gallery/Main.qml";

/// `qml` set up for the gallery at `scale`, with `args` passed through.
pub fn command(root: &Path, scale: &str, args: &[String]) -> Result<Command> {
    if !matches!(scale, "1" | "1.0" | "2" | "2.0") {
        bail!("the gallery is shown at scale 1.0 or 2.0, not {scale}");
    }
    let mut command = Command::new(qt_tool("qml"));
    command
        .current_dir(root)
        .env("QT_SCALE_FACTOR", scale)
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .args(["-I", "ui", MAIN]);
    if !args.is_empty() {
        command.arg("--").args(args);
    }
    Ok(command)
}

pub fn open(root: &Path, scale: &str, args: &[String]) -> Result<()> {
    run("gallery", &mut command(root, scale, args)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: DS-04
    // suite: unit
    #[test]
    fn gallery_command_sets_scale_and_passes_arguments() {
        let root = Path::new("/repo");
        let shot = command(root, "2", &["--list".to_owned()]).unwrap();
        let args: Vec<_> = shot
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["-I", "ui", MAIN, "--", "--list"]);
        let scale = shot
            .get_envs()
            .find(|(k, _)| *k == "QT_SCALE_FACTOR")
            .and_then(|(_, v)| v);
        assert_eq!(scale, Some(std::ffi::OsStr::new("2")));
        let plain = command(root, "1.0", &[]).unwrap();
        assert_eq!(plain.get_args().count(), 3, "no -- without arguments");
        assert!(command(root, "1.5", &[]).is_err());
    }
}
