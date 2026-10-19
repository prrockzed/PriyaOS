//! PriyaOS developer commands: `cargo xtask <command>`.
//!
//! Sub-phases add commands as they need them (test suites and coverage in
//! 0.4, schemas in 0.9, the gallery in 0.11, the ISO in 2.5).

#![allow(clippy::print_stdout, clippy::print_stderr)] // a CLI prints

mod ci_local;
mod cmd;
mod fixtures;
mod flaky;
mod licenses;
mod pre_commit;
mod promote;
mod scope;
mod tags;
mod test_cmd;
mod trace;
#[cfg(test)]
mod workflows;

use anyhow::{Context, Result};
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
    /// Check recorded test data: a RECORDED.md for every fixture folder,
    /// and nothing that looks like a secret.
    Fixtures {
        #[command(subcommand)]
        action: FixturesCmd,
    },
    /// Print `-p <crate>` for every crate that must build on macOS.
    Portable,
    /// Check staged changes: formatting, clippy and unit tests of the
    /// changed crates, and secrets. Run by .githooks/pre-commit.
    PreCommit,
    /// Index every tagged test into target/trace/tests.json; fails on
    /// missing or malformed tags.
    Tags,
    /// Check the test index against features.md and the phase files, and
    /// write target/trace/matrix.{md,json}.
    Trace {
        /// A JUnit report from this run (repeatable). With one, every ✅
        /// feature needs a passing test.
        #[arg(long)]
        junit: Vec<std::path::PathBuf>,
        /// Compare regression tests with this branch, e.g. origin/main.
        #[arg(long)]
        base: Option<String>,
        /// A file holding the pull request description (for ADR citations).
        #[arg(long)]
        pr_body: Option<std::path::PathBuf>,
        /// The shell tests' results from this run
        /// (target/trace/script-results.json).
        #[arg(long)]
        script_results: Option<std::path::PathBuf>,
    },
    /// The flaky-test quarantine (tests/quarantine.toml).
    Flaky {
        #[command(subcommand)]
        action: FlakyCmd,
    },
    /// Regression suite maintenance.
    Regression {
        #[command(subcommand)]
        action: RegressionCmd,
    },
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

#[derive(Subcommand)]
enum FlakyCmd {
    /// Every quarantined test is marked and listed, there are at most 5,
    /// and none is older than 7 days.
    Check,
    /// Nightly: run each quarantined test many times and the unit and
    /// contract suites a few times; write target/trace/flaky-hunt.json.
    Hunt {
        #[arg(long, default_value_t = 50)]
        runs: u32,
        #[arg(long, default_value_t = 5)]
        suite_runs: u32,
    },
}

#[derive(Subcommand)]
enum RegressionCmd {
    /// Add `regression: <sub-phase>` under every `acceptance: <sub-phase>`
    /// tag. Run in each sub-phase's gate PR.
    Promote { sub_phase: String },
}

#[derive(Subcommand)]
enum FixturesCmd {
    /// Fail on undocumented fixtures or secret-looking content.
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cmd::workspace_root()?;
    match cli.command {
        Cmd::CiLocal { fast } => ci_local::run_all(&root, fast),
        Cmd::Licenses => licenses::check(&root),
        Cmd::Fixtures {
            action: FixturesCmd::Check,
        } => fixtures::check(&root),
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
        Cmd::Trace {
            junit,
            base,
            pr_body,
            script_results,
        } => {
            let pr_body = match pr_body {
                Some(path) => std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?,
                None => String::new(),
            };
            trace::run_trace(
                &root,
                &trace::Options {
                    junit,
                    base,
                    pr_body,
                    script_results,
                },
            )
        }
        Cmd::Flaky {
            action: FlakyCmd::Check,
        } => flaky::check(&root, &priya_core::clock::SystemClock),
        Cmd::Flaky {
            action: FlakyCmd::Hunt { runs, suite_runs },
        } => flaky::hunt(&root, runs, suite_runs),
        Cmd::Regression {
            action: RegressionCmd::Promote { sub_phase },
        } => promote::promote(&root, &sub_phase),
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
