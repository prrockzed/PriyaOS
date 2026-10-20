//! Recorded test data (HTTP cassettes, agent streams, screenshots). Every
//! fixture folder carries a `RECORDED.md` that says how, when and with which
//! tool versions it was recorded, so it can be recorded again.

use std::path::{Path, PathBuf};

pub const RECORDED_NOTE: &str = "RECORDED.md";

/// The fixture folder at `path`. Panics, failing the test, when the folder
/// has no `RECORDED.md`.
#[track_caller]
pub fn fixture(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if !path.join(RECORDED_NOTE).is_file() {
        panic!(
            "fixture {} has no {RECORDED_NOTE}; add one saying how, when and with which tool versions it was recorded",
            path.display()
        );
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-04
    // suite: unit
    // acceptance: 0.4
    // regression: 0.4
    #[test]
    fn fixture_without_recorded_md_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let result = std::panic::catch_unwind(|| fixture(dir.path()));
        let message = *result.unwrap_err().downcast::<String>().unwrap();
        assert!(message.contains("has no RECORDED.md"), "{message}");

        std::fs::write(dir.path().join(RECORDED_NOTE), "Recorded by hand.\n").unwrap();
        assert_eq!(fixture(dir.path()), dir.path());
    }
}
