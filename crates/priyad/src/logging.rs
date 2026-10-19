//! Logging for priyad and the `priya` command line: to journald when running
//! under systemd, otherwise to stderr.

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

    // systemd sets JOURNAL_STREAM when stdout/stderr go to the journal; then
    // structured journald fields beat plain text.
    #[cfg(target_os = "linux")]
    if std::env::var_os("JOURNAL_STREAM").is_some()
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
