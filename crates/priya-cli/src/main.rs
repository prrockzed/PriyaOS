//! `priya`, the PriyaOS command line. Everything the desktop can do will also
//! be possible here; the commands arrive in sub-phase 0.7.

#![allow(clippy::print_stdout, clippy::print_stderr)] // a CLI prints

use clap::Parser;

/// Talk to PriyaOS from the terminal.
#[derive(Debug, Parser)]
#[command(name = "priya", version, about)]
struct Cli {
    /// Show debug logs on stderr.
    #[arg(short, long, global = true)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();
    priyad::logging::init(if cli.verbose { "debug" } else { "warn" });
    tracing::debug!(?cli, "parsed arguments");
    println!(
        "priya {}: no commands yet. They arrive in sub-phase 0.7.",
        env!("CARGO_PKG_VERSION")
    );
}
