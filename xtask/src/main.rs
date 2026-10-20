//! PriyaOS developer commands: `cargo xtask <command>`.
//!
//! Sub-phases add commands as they need them (test suites and coverage in
//! 0.4, schemas in 0.9, the gallery in 0.11, the ISO in 2.5).

#![allow(clippy::print_stdout, clippy::print_stderr)] // a CLI prints

mod ci_local;
mod cmd;
mod coverage;
mod fixtures;
mod flaky;
mod gallery;
mod i18n;
mod licenses;
mod pre_commit;
mod promote;
mod qml;
mod qml_lint;
mod schema_check;
mod schema_diff;
mod schemas;
mod scope;
mod shots;
mod tags;
mod test_cmd;
mod tokens;
mod trace;
mod ui_lint;
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
    /// Open the PriyaUI gallery (`qml ui/gallery/Main.qml`). Arguments
    /// after `--` go to the gallery (shot mode: --story, --all, --list).
    Gallery {
        /// 1.0 or 2.0, through QT_SCALE_FACTOR.
        #[arg(long, default_value = "1")]
        scale: String,
        #[arg(last = true)]
        args: Vec<String>,
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
    /// Write the JSON Schemas of the API, events, settings and module
    /// manifest under docs/claude/api, and the UI's Api.js.
    Schemas {
        /// Change nothing; fail if a generated file is out of date or a
        /// change breaks compatibility. Writes target/schemas/report.md.
        #[arg(long)]
        check: bool,
        /// With --check: classify the changes since this git ref.
        #[arg(long, requires = "check")]
        base: Option<String>,
    },
    /// Index every tagged test into target/trace/tests.json; fails on
    /// missing or malformed tags.
    Tags,
    /// Generate Theme.qml and tokens.css from ui/design/tokens.json and
    /// check colour contrast.
    Tokens {
        /// Change nothing; fail if a generated file is out of date.
        #[arg(long)]
        check: bool,
    },
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
        /// A results file of shell or QML tests from this run
        /// (target/trace/script-results.json, ui-results.json; repeatable).
        #[arg(long, alias = "script-results")]
        results: Vec<std::path::PathBuf>,
    },
    /// Diff coverage: changed Rust lines must be tested (xtask/coverage.toml).
    Coverage {
        /// The branch to compare with.
        #[arg(long, default_value = "origin/main")]
        base: String,
        /// Use this lcov report instead of running the tests.
        #[arg(long)]
        lcov: Option<std::path::PathBuf>,
        /// A pull request label (repeatable), for `coverage-exception`.
        #[arg(long)]
        label: Vec<String>,
        /// A file holding the pull request description.
        #[arg(long)]
        pr_body: Option<std::path::PathBuf>,
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
    /// PriyaUI and shell checks (0.11).
    Ui {
        #[command(subcommand)]
        action: UiCmd,
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
        /// With `ui`: only the screenshots, compared with the baselines.
        #[arg(long)]
        shots: bool,
        /// With `--shots`: replace the baselines with the new pictures.
        #[arg(long, requires = "shots")]
        update_baselines: bool,
    },
}

#[derive(Subcommand)]
enum UiCmd {
    /// Tokens only, translatable strings and spoken names in ui/PriyaUI,
    /// ui/shell and ui/gallery.
    Lint,
    /// qmllint, with no warnings, on every QML file under ui/.
    Qmllint {
        /// Fail rather than skip ui/shell where Quickshell is missing (CI).
        #[arg(long)]
        require_quickshell: bool,
    },
    /// Collect the UI's qsTr strings into ui/i18n/priya_en.ts (lupdate).
    I18n {
        /// Change nothing; fail if the file is out of date.
        #[arg(long)]
        check: bool,
    },
    /// Fail when the branch changes or removes a screenshot baseline
    /// without the `visual-change-approved` label.
    BaselinesGuard {
        /// The branch to compare with, e.g. origin/main.
        #[arg(long)]
        base: String,
        /// A pull request label (repeatable).
        #[arg(long)]
        label: Vec<String>,
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
        Cmd::Gallery { scale, args } => gallery::open(&root, &scale, &args),
        Cmd::Licenses => licenses::check(&root),
        Cmd::Fixtures {
            action: FixturesCmd::Check,
        } => fixtures::check(&root),
        Cmd::Portable => {
            println!("{}", scope::portable(&root)?.join(" "));
            Ok(())
        }
        Cmd::PreCommit => pre_commit::run_all(),
        Cmd::Schemas { check, base } => schemas::run(&root, check, base.as_deref()),
        Cmd::Tokens { check } => tokens::run(&root, check),
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
            results,
        } => {
            let pr_body = read_optional(pr_body.as_deref())?;
            trace::run_trace(
                &root,
                &trace::Options {
                    junit,
                    base,
                    pr_body,
                    results,
                },
            )
        }
        Cmd::Coverage {
            base,
            lcov,
            label,
            pr_body,
        } => coverage::run_coverage(
            &root,
            &coverage::Options {
                base,
                lcov,
                labels: label,
                pr_body: read_optional(pr_body.as_deref())?,
            },
        ),
        Cmd::Flaky {
            action: FlakyCmd::Check,
        } => flaky::check(&root, &priya_core::clock::SystemClock),
        Cmd::Flaky {
            action: FlakyCmd::Hunt { runs, suite_runs },
        } => flaky::hunt(&root, runs, suite_runs),
        Cmd::Ui {
            action: UiCmd::Lint,
        } => ui_lint::check(&root),
        Cmd::Ui {
            action: UiCmd::Qmllint { require_quickshell },
        } => qml_lint::check_all(&root, require_quickshell),
        Cmd::Ui {
            action: UiCmd::I18n { check },
        } => i18n::run(&root, check),
        Cmd::Ui {
            action: UiCmd::BaselinesGuard { base, label },
        } => shots::guard(&root, &base, &label),
        Cmd::Regression {
            action: RegressionCmd::Promote { sub_phase },
        } => promote::promote(&root, &sub_phase),
        Cmd::Test {
            selection,
            fast,
            all,
            profile,
            list,
            shots,
            update_baselines,
        } => {
            if shots {
                if selection != "ui" {
                    anyhow::bail!("--shots goes with the ui suite: cargo xtask test ui --shots");
                }
                return shots::run(&root, update_baselines).map(|_| ());
            }
            let selection = test_cmd::Selection::parse(&selection, fast, all)?;
            test_cmd::run_selection(&root, &selection, &profile, list)
        }
    }
}

/// The file's text, or empty when no file was given.
fn read_optional(path: Option<&std::path::Path>) -> Result<String> {
    path.map_or(Ok(String::new()), cmd::read)
}
