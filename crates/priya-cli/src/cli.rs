//! The commands, as clap reads them. Each API method has one (see
//! [`crate::parity`]).

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use priya_core::settings::ProviderName;

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
    /// Ask Priya something; the answer streams as it is written.
    Ask {
        /// The question; several words need no quotes.
        #[arg(required = true, num_args = 1..)]
        question: Vec<String>,
        /// Use this model for this question: <provider>/<model>.
        #[arg(long, value_name = "PROVIDER/MODEL")]
        model: Option<String>,
        /// Continue the last conversation.
        #[arg(short = 'c', long = "continue")]
        continue_last: bool,
    },
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
    /// See, turn on and off, and call modules.
    #[command(subcommand)]
    Module(ModuleCommand),
    /// Models and their keys.
    #[command(subcommand)]
    Ai(AiCommand),
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
        /// Such as light, 720 or true; JSON for lists and tables. Numbers,
        /// true and false are read as such; quote them as JSON ('"720"')
        /// for text.
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
pub enum ModuleCommand {
    /// List the modules and what each is doing.
    List,
    /// Turn a module on: it starts now and with PriyaOS from then on.
    Enable {
        /// Such as echo.
        name: String,
    },
    /// Turn a module off: it stops now and stays off.
    Disable {
        /// Such as echo.
        name: String,
    },
    /// Call a module's method: priya module call echo.say '{"text":"hi"}'.
    Call {
        /// <module>.<method>, such as echo.say.
        method: String,
        /// Its params as JSON (none means {}).
        params: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AiCommand {
    /// Show which model answers, whether its key is stored and whether it
    /// can be reached.
    Status,
    /// Store or remove a provider's API key in the keyring.
    #[command(subcommand)]
    Key(KeyCommand),
}

#[derive(Debug, Subcommand)]
pub enum KeyCommand {
    /// Store a provider's key: paste it when asked, or pipe it in.
    Set {
        /// The provider's name in your settings, such as gemini.
        #[arg(value_parser = crate::keys::provider_name)]
        provider: ProviderName,
    },
    /// Remove a provider's key from the keyring.
    Remove {
        /// Such as gemini.
        #[arg(value_parser = crate::keys::provider_name)]
        provider: ProviderName,
    },
}

#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    /// List every request the API accepts.
    Methods,
}
