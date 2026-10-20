//! The commands, as clap reads them. Each API method has one (see
//! [`crate::parity`]).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Talk to PriyaOS from the terminal.
#[derive(Debug, Parser)]
#[command(name = "priya", version, about)]
pub struct Cli {
    /// Print the raw answer as JSON (one line per event for streams).
    #[arg(long, global = true)]
    pub json: bool,
    /// Talk to the daemon on this socket instead of the usual one.
    #[arg(long, global = true, value_name = "PATH")]
    pub socket: Option<PathBuf>,
    /// Show debug logs on stderr.
    #[arg(short, long, global = true)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show whether PriyaOS is running and what it is using.
    Status,
    /// Check that PriyaOS is healthy, and fix what it finds.
    Doctor {
        /// Fix what can be fixed (asks first).
        #[arg(long)]
        fix: bool,
        /// With --fix: only list the fixes.
        #[arg(long, requires = "fix")]
        dry_run: bool,
        /// With --fix: don't ask.
        #[arg(long, requires = "fix")]
        yes: bool,
    },
    /// Read and change settings.
    #[command(subcommand)]
    Settings(SettingsCommand),
    /// Follow what happens.
    #[command(subcommand)]
    Events(EventsCommand),
    /// Check that the history wasn't tampered with.
    #[command(subcommand)]
    Audit(AuditCommand),
    /// Look at the API itself.
    #[command(subcommand)]
    Api(ApiCommand),
    /// Print a shell completion script.
    Completions {
        /// bash, zsh, fish, elvish or powershell.
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    /// Show settings: all of them, or one key such as ui.theme.
    Get {
        key: Option<String>,
        /// Also say where each value comes from.
        #[arg(long)]
        explain: bool,
    },
    /// Change a setting in your settings file.
    Set {
        /// Such as ui.theme.
        key: String,
        /// Such as light, 720 or true; JSON for lists and tables.
        value: String,
    },
    /// Remove a setting from your settings file, so its default applies.
    Unset { key: String },
    /// Print the JSON Schema of every setting.
    Schema,
}

#[derive(Debug, Subcommand)]
pub enum EventsCommand {
    /// Show events as they happen, until Ctrl+C.
    Tail {
        /// Only these types, such as intent.* (repeatable).
        #[arg(long = "type", value_name = "PATTERN")]
        types: Vec<String>,
        /// Only streams starting with this, such as intent:.
        #[arg(long, value_name = "PREFIX")]
        stream: Option<String>,
        /// Start after this event number instead of now (0 for everything).
        #[arg(long, value_name = "SEQ")]
        from: Option<i64>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuditCommand {
    /// Check that no event was changed, removed or reordered.
    Verify {
        /// The first event to check.
        #[arg(long, value_name = "SEQ")]
        from: Option<i64>,
        /// The last event to check.
        #[arg(long, value_name = "SEQ")]
        to: Option<i64>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    /// List every request the API accepts.
    Methods,
}
