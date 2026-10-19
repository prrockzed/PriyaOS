//! PriyaOS developer commands: `cargo xtask <command>`.
//!
//! Sub-phases add commands as they need them (test suites and coverage in
//! 0.4, schemas in 0.9, the gallery in 0.11, the ISO in 2.5).

#![allow(clippy::print_stdout, clippy::print_stderr)] // a CLI prints

mod ci_local;
mod cmd;
mod licenses;
mod scope;
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
        /// Quicker: clippy skips test code, and only library unit tests run.
        #[arg(long)]
        fast: bool,
    },
    /// Check that every third-party bundle under ui/ is listed in
    /// THIRD_PARTY.md.
    Licenses,
    /// Print `-p <crate>` for every crate that must build on macOS.
    Portable,
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
    }
}
