//! PriyaOS developer commands: `cargo xtask <command>`.
//!
//! Sub-phases add commands as they need them (test suites and coverage in
//! 0.4, schemas in 0.9, the gallery in 0.11, the ISO in 2.5).

#![allow(clippy::print_stdout, clippy::print_stderr)] // a CLI prints

mod ci_local;
mod cmd;
mod licenses;
mod pre_commit;
mod scope;
mod tags;
mod test_cmd;
#[cfg(test)]
mod workflows;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo xtask", about = "PriyaOS developer commands")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run CI's checks locally: formatting, clippy, Rust tests, script tests,
    /// licences, spelling and links. On macOS only the portable crates are
    /// built and tested.
    CiLocal {
        /// Quicker: clippy skips test code, and only unit tests run.
        #[arg(long)]
        fast: bool,
    },
    /// Check that every third-party bundle under ui/ is listed in
    /// THIRD_PARTY.md.
    Licenses,
    /// Print `-p <crate>` for every crate that must build on macOS.
    Portable,
    /// Check staged changes: formatting, clippy and unit tests of the
    /// changed crates, and secrets. Run by .githooks/pre-commit.
    PreCommit,
    /// Index every tagged test into target/trace/tests.json; fails on
    /// missing or malformed tags.
    Tags,
    /// Run tests by tag: a suite (unit, integration, …), `regression` or
    /// `all`.
    Test {
        /// A suite name, `regression` or `all`.
        selection: String,
        /// With `regression`: the unit, contract, integration and ui suites
        /// only. This is the default.
        #[arg(long, conflicts_with = "all")]
        fast: bool,
        /// With `regression`: every suite.
        #[arg(long)]
        all: bool,
        /// The nextest profile (.config/nextest.toml).
        #[arg(long, default_value = "default")]
        profile: String,
        /// Print the selected test ids instead of running them.
        #[arg(long)]
        list: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cmd::workspace_root()?;
    match cli.command {
        Cmd::CiLocal { fast } => ci_local::run_all(&root, fast),
        Cmd::Licenses => licenses::check(&root),
        Cmd::Portable => {
            println!("{}", scope::portable(&root)?.join(" "));
            Ok(())
        }
        Cmd::PreCommit => pre_commit::run_all(),
        Cmd::Tags => {
            let index = tags::build(&root, &scope::Workspace::load(&root)?)?;
            let path = tags::write(&root, &index)?;
            println!("{} tests indexed in {}", index.len(), path.display());
            Ok(())
        }
        Cmd::Test {
            selection,
            fast,
            all,
            profile,
            list,
        } => {
            let selection = test_cmd::Selection::parse(&selection, fast, all)?;
            test_cmd::run_selection(&root, &selection, &profile, list)
        }
    }
}
