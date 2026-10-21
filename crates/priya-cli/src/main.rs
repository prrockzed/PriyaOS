//! `priya`, the PriyaOS command line: one command for each API method
//! (CORE-04), talking to priyad over its socket.
//!
//! Exit codes: 0 ok, 1 refused or failed, 2 usage (from clap), 3 priyad
//! isn't running, 130 stopped with Ctrl+C. Output has no colour, so `NO_COLOR` needs nothing.

#![allow(clippy::print_stdout, clippy::print_stderr)] // a CLI prints

mod ask;
mod cli;
mod keys;
#[cfg(test)]
mod parity;
mod run;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use priya_core::paths::PriyaPaths;

use crate::cli::{AiCommand, Cli, Command, KeyCommand};
use crate::run::{Failure, Out};

/// What `priya` says when priyad isn't running.
const NOT_RUNNING: &str =
    "Priya's background service isn't running. Start it with: systemctl --user start priyad";

fn main() -> ExitCode {
    let cli = Cli::parse();
    priya_logging::init(if cli.verbose { "debug" } else { "warn" });
    if let Command::Completions { shell } = cli.command {
        clap_complete::generate(shell, &mut Cli::command(), "priya", &mut std::io::stdout());
        return ExitCode::SUCCESS;
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("priya couldn't start: {e}");
            return ExitCode::from(1);
        }
    };
    let mut stdout = std::io::stdout();
    let mut out = Out::new(cli.json, &mut stdout);
    let ran = match cli.command {
        // Keys go to the keyring directly; priyad (and its socket) isn't
        // needed.
        Command::Ai(AiCommand::Key(command)) => runtime.block_on(key(command, &mut out)),
        command => {
            let socket = match cli.socket.clone().map_or_else(default_socket, Ok) {
                Ok(socket) => socket,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::from(1);
                }
            };
            runtime.block_on(run::run(command, &socket, &mut out))
        }
    };
    match ran {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Refused(message)) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
        Err(Failure::Unreachable) => {
            eprintln!("{NOT_RUNNING}");
            ExitCode::from(3)
        }
        Err(Failure::Interrupted) => {
            eprintln!("Stopped.");
            ExitCode::from(130)
        }
    }
}

async fn key(command: KeyCommand, out: &mut Out<'_>) -> Result<(), Failure> {
    let store = priya_ai::keys::SecretServiceStore;
    match command {
        KeyCommand::Set { provider } => {
            use std::io::IsTerminal;
            let stdin = std::io::stdin();
            // No lock held while typing: the hidden read takes stdin's lock
            // on its own thread, and would wait for this one forever.
            if stdin.is_terminal() {
                keys::set(&store, &provider, keys::KeyInput::Terminal, out).await
            } else {
                let mut lines = stdin.lock();
                keys::set(&store, &provider, keys::KeyInput::Lines(&mut lines), out).await
            }
        }
        KeyCommand::Remove { provider } => keys::remove(&store, &provider, out).await,
    }
}

fn default_socket() -> Result<PathBuf, String> {
    PriyaPaths::from_env()
        .map(|paths| paths.runtime.join("priyad.sock"))
        .map_err(|e| {
            use priya_api::error::UserFacing;
            e.user_message()
        })
}
