//! Logging for priyad and the `priya` command line: to journald when running
//! under systemd, otherwise to stderr. Its own crate so the CLI doesn't
//! build the daemon to get it.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

/// Variable holding a tracing filter, such as `debug` or `priya_ai=trace,info`.
pub const LOG_ENV: &str = "PRIYA_LOG";

/// Starts logging. `default_filter` applies when `PRIYA_LOG` is unset or
/// invalid. Calling it again does nothing, so tests and binaries can both
/// call it.
pub fn init(default_filter: &str) {
    let filter =
        EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new(default_filter));
    let registry = tracing_subscriber::registry().with(filter);

    // Structured journald fields beat plain text, when stderr really is the
    // journal.
    #[cfg(target_os = "linux")]
    if stderr_is_the_journal()
        && let Ok(journald) = tracing_journald::layer()
    {
        let _ = registry
            .with(journald.with_syslog_identifier("priya".to_owned()))
            .try_init();
        return;
    }

    let _ = registry
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false),
        )
        .try_init();
}

/// systemd sets `JOURNAL_STREAM` to the `device:inode` of the stream it
/// connects to the journal. The variable is inherited by everything a
/// service starts (a terminal's shell, say), so only a matching stderr
/// counts.
#[cfg(target_os = "linux")]
fn stderr_is_the_journal() -> bool {
    std::env::var_os("JOURNAL_STREAM")
        .and_then(|value| value.into_string().ok())
        .is_some_and(|value| names(&value, std::io::stderr()))
}

/// Whether `device:inode` names the open file `file`.
#[cfg(target_os = "linux")]
fn names(stream: &str, file: impl std::os::fd::AsFd) -> bool {
    let Some((device, inode)) = stream.split_once(':') else {
        return false;
    };
    let Ok(stat) = rustix::fs::fstat(file) else {
        return false;
    };
    // The field types differ between targets; parse into whatever they are.
    device.parse().ok() == Some(stat.st_dev) && inode.parse().ok() == Some(stat.st_ino)
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-04
    // suite: unit
    #[test]
    fn init_installs_a_subscriber_and_tolerates_a_second_call() {
        init("warn");
        assert!(tracing::dispatcher::has_been_set());
        init("debug");
    }

    // covers: CORE-04
    // suite: unit
    #[cfg(target_os = "linux")]
    #[test]
    fn only_the_matching_stream_is_the_journal() {
        let file = tempfile::tempfile().unwrap();
        let stat = rustix::fs::fstat(&file).unwrap();
        assert!(names(&format!("{}:{}", stat.st_dev, stat.st_ino), &file));
        assert!(!names("0:0", &file), "a stream inherited from elsewhere");
        assert!(!names("nonsense", &file));
    }
}
