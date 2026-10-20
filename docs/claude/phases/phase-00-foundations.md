# Phase 0 — Foundations and walking skeleton

**Goal.** Build the base that every later phase plugs into: Cargo workspace, event store, settings, public API and CLI, module host, design tokens and the PriyaUI component library. On top of that base, ship one thin usable slice (ADR-012). CI and the regression machinery go in first (ADR-013), so every change after them is protected.

**Usable at the end.** On the owner's current Arch, on any filesystem: build and install with `makepkg -si`, enable two user services and store an API key. Then Super+Space opens a minimal Priya bar. The owner asks anything and gets a streamed Markdown answer from one model: Anthropic (cloud) or a local OpenAI-compatible endpoint such as Ollama. The bar can be light or dark and follows the `ui.theme` setting live. `priya ask` does the same in a terminal. Every pull request runs lint, build, unit, contract, integration, UI screenshot and accessibility tests, plus schema, traceability and diff-coverage checks. `main` only accepts green pull requests.

**Depends on:** nothing.

**Features:** CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, AI-21, UX-02, OS-01, DS-01, DS-02, DS-03, DS-04, DS-05, DS-06, QA-01, QA-02, QA-03, QA-04, QA-05, QA-06, QA-07, QA-08, QA-09, QA-10, QA-11.

**Read first:** [CLAUDE.md](../../../CLAUDE.md) · [phases/README](README.md) (workflow, template) · [features](../features.md) · [00-principles](../architecture/00-principles.md) · [01-system-overview](../architecture/01-system-overview.md) · [02-core](../architecture/02-core.md) · [09-testing](../architecture/09-testing.md) · [11-design-system-and-ux](../architecture/11-design-system-and-ux.md) · [decisions](../architecture/decisions.md) (ADR-002, 003, 009, 011, 012, 013, 014) · [10-base-os-and-distro §4–5](../architecture/10-base-os-and-distro.md) · [04-priya-ai §2](../architecture/04-priya-ai.md) (for 0.12 only).

**Conventions in this file.**
- Every test listed below carries the tag comments defined in 0.4: `covers:`, which holds the sub-phase's feature IDs, or `infra` for tooling with no feature; `suite:`; and `acceptance: <sub-phase>`. At the sub-phase gate, `cargo xtask regression promote <sub-phase>` adds `regression: <sub-phase>`, and the test joins the permanent suite.
- Test names: Rust functions use snake_case, QML component tests start with `tst_`, and headless-compositor flows start with `journey_`.
- "Integration" in Phase 0 means priyad and its modules running in a temporary home (`TempPriyaHome`/`TestDaemon`). Fake-machine integration starts in 2.3.
- 👤 marks an action or decision by the owner.

## Sub-phase overview

| Id | Title | Depends on | Usable outcome |
|---|---|---|---|
| 0.1 | Zenbook survey and dev environments | — | Both machines build the repo. The Zenbook is recorded and the target desktop is decided |
| 0.2 | Repository and workspace scaffold | — | `cargo test --workspace` green on Mac and Zenbook |
| 0.3 | CI pipeline v1 and PR workflow | 0.2 | Pushes and PRs run CI; `main` is protected; local hooks run |
| 0.4 | Test infrastructure (testkit, tags, traceability, diff coverage, regression runner) | 0.3 | `cargo xtask trace`, `coverage` and `test regression` run in CI |
| 0.5 | Event store | 0.4 | Hash-chained log with projections, subscriptions and blobs (library) |
| 0.6 | Settings system | 0.5 | Layered, validated, hot-reloaded settings (library) |
| 0.7 | priyad, public API and CLI skeleton | 0.5, 0.6 | `priya status / settings / events tail / audit verify / doctor` work against the user service |
| 0.8 | Module host and SDK | 0.7 | Rust and Python example modules run, emit events, restart after crashes |
| 0.9 | Schema compatibility checks | 0.5–0.8 | Breaking API, event, settings or manifest changes fail CI |
| 0.10 | Design tokens and theme generator | 0.4 | Approved light/dark palette, `Theme.qml` generated, contrast checked in CI |
| 0.11 | PriyaUI base components, gallery and UI test harness | 0.4, 0.10 | Gallery of 10 components in every state; screenshot and a11y tests in CI |
| 0.12 | Walking skeleton: minimal model access and `priya ask` | 0.7, 0.9 | `priya ask "…"` streams an answer from Gemini's free tier or from Ollama |
| 0.13 | Walking skeleton: minimal Priya bar | 0.11, 0.12 | Super+Space → ask → streamed answer, light/dark |
| 0.14 | Package for any Arch and first install | 0.1, 0.13 | `makepkg -si` on the Zenbook; daily use starts |
| 0.15 | Phase gate | 0.1–0.14 | Regression suite green, owner demo, docs true |

0.10 and 0.11 can run in parallel with 0.5–0.9.

---

### 0.1 — Zenbook survey and dev environments
**Outcome:** The Zenbook's hardware and its current Arch install are recorded. The Mac and the Zenbook both have every tool the repo needs. The desktop that Phase 0 targets and the path for Phase 2 dogfooding are decided.
**Features:** infrastructure.
**Depends on:** —.

**Tasks**
- [x] `scripts/survey.sh`: read-only; runs as the user and refuses root; writes Markdown to stdout. It records:
  - **Hardware:** DMI product name (`/sys/class/dmi/id/product_name`), CPU (`lscpu`), RAM, GPU (`lspci -k`), kernel.
  - **Disks and boot:** disks (`lsblk -f`); root filesystem and options (`findmnt -no FSTYPE,SOURCE,OPTIONS /`); btrfs subvolume names from `/proc/self/mountinfo` and whether `/etc/fstab` uses `subvolid=`; `/boot` and ESP mount points and sizes; bootloader (files present in the ESP); Secure Boot state (efivars); free space.
  - **Desktop:** compositor and version (`hyprctl version`); monitors and scale (`hyprctl monitors -j`); the current binding on Super+Space (`hyprctl binds -j`); any running bar or shell (Waybar, other Quickshell configs).
  - **Software:** Secret Service provider (`busctl --user list | grep org.freedesktop.secrets`); Qt and Quickshell versions (installed, or available via `pacman -Si`); Python; Ollama.
- [x] 👤 The owner runs the script on the Zenbook. In Windows, the owner checks the BitLocker state and that the recovery key is saved. (The survey detects BitLocker itself.)
- [x] Keep the results **private**: the owner keeps the survey, and Claude keeps a summary in its local memory on the Mac. This repo is public, so it records only the decisions below, without machine details.
- [x] Record three decisions in §Decisions at the end of this file:
  - (a) Phase 0's target session: Hyprland, or another wlr-layer-shell compositor with a manual keybind. GNOME is not supported and falls back to `priya ask`.
  - (b) Phase 2's dogfooding path: the existing btrfs layout is usable after `priya doctor --fix`, or the dev installer (2.5) is needed.
  - (c) Super+Space conflict handling: free the key, or use an agreed alternative.
- [x] `scripts/dev-setup-arch.sh`: prints the package list, then asks before using sudo.
  - Arch packages: `base-devel git rustup clang mold sqlite python python-pipx shellcheck qt6-declarative qt6-tools qt6-svg quickshell cage sway grim wtype inter-font ttf-jetbrains-mono noto-fonts gitleaks typos lychee namcap`.
  - Rust: `rustup component add rustfmt clippy llvm-tools-preview`, plus `cargo install --locked cargo-nextest cargo-llvm-cov cargo-deny cargo-insta dssim`.
  - Python tools: `pipx install diff-cover`.
  - Phase 2 adds QEMU and archiso tools; they are not installed now.
- [x] `scripts/dev-setup-macos.sh`:
  - Homebrew: `rustup qt sqlite python pipx shellcheck gitleaks typos-cli lychee`. Homebrew's `rustup` is keg-only, so the script offers to add it to `PATH` in `~/.zprofile`.
  - The same cargo and pipx tools as on Arch.
  - The script states that Quickshell and Hyprland do not exist on macOS. UI work on the Mac is limited to PriyaUI components in the gallery (`qml`).
- [x] `scripts/dev-check.sh`: lists every required tool with its version and gives exit code 1 if any is missing. Run by both setup scripts and by the pre-push hook.
- [ ] 👤 The owner approves installing Ollama on the Zenbook (`pacman -S ollama`, `systemctl enable --now ollama`) and pulling one small model. The model name goes into the survey.
- [x] Root `README.md` "Develop" section: one-line setup per machine and where the docs live.

**Human in the loop:** The owner runs the survey, checks BitLocker in Windows and approves every package install on the real machine. If Super+Space is already bound, the owner decides how to handle it.

**Tests**
- Unit:
  - `survey_script_is_read_only`: `shellcheck`, plus a scan that fails on `sudo`, `mkfs`, `dd`, `wipefs`, `btrfs subvolume (create|delete|snapshot)`, `mount`, `systemctl (start|enable)` and output redirection to system paths in `scripts/survey.sh` (`covers: infra`).
  - `dev_check_reports_missing_tool`: run with a PATH that lacks `cargo-nextest` and expect exit 1 (`infra`).
- Both live in `scripts/tests/` (`test_survey_read_only.sh`, `test_dev_check.sh`) and run with `bash scripts/tests/run.sh` until `cargo xtask test` takes over in 0.4. The read-only scanner `scripts/tests/check-read-only.sh` is itself tested against `fixtures/read-only-forbidden.sh` and `fixtures/read-only-allowed.sh`.
- Regression: both.

**Done when**
- [x] The survey is recorded privately (not in this repo).
- [x] Decisions (a), (b) and (c) are recorded.
- [ ] `scripts/dev-check.sh` passes on the Mac and the Zenbook.

---

### 0.2 — Repository and workspace scaffold
**Outcome:** An empty-but-real Cargo workspace builds and tests on both machines. It has shared lints, one error model, logging, XDG paths and a secret wrapper.
**Features:** infrastructure.
**Depends on:** —.

**Tasks**
- [x] Root `Cargo.toml` workspace. It contains only the crates Phase 0 uses:
  - `crates/priya-api`, `priya-core`, `priya-store`, `priya-ai`, `priyad`, `priya-cli`, `priya-module-sdk` and `priya-testkit`;
  - `xtask/`;
  - `ui/design/generate` (crate `priya-themegen`).

  The other crates from [01 §6](../architecture/01-system-overview.md) are added by the phase that first needs them, so there is no dead code under coverage. `[workspace.package]` sets edition 2024, `rust-version` and the licence. `[workspace.dependencies]` pins `tokio`, `serde`, `serde_json`, `schemars`, `thiserror`, `tracing`, `tracing-subscriber`, `tracing-journald`, `rusqlite` (feature `bundled`), `blake3`, `zstd`, `ulid`, `time`, `toml_edit`, `clap`, `reqwest` (rustls), `insta` and `proptest`.
- [x] Toolchain and formatting files:
  - `rust-toolchain.toml`: a pinned stable version with `rustfmt`, `clippy` and `llvm-tools-preview`;
  - `rustfmt.toml`, `.editorconfig`, `.gitattributes` (PNG baselines are `binary`);
  - `.gitignore` and `_typos.toml`.
- [x] `[workspace.lints]`:
  - `unsafe_code = "forbid"`;
  - `clippy::all = "deny"`;
  - `unwrap_used = "deny"`, outside tests only;
  - `dbg_macro`, `todo`, `print_stdout` and `print_stderr` deny (logging goes through `tracing`). `priya-cli` and `xtask` allow printing at their crate root; `clippy.toml` allows unwrap, dbg and print inside tests.
- [x] `.cargo/config.toml`: alias `xtask = "run --package xtask --"`. `[workspace.metadata.priya] portable = [...]` lists the crates that must build and test on macOS: `priya-api`, `priya-core`, `priya-store`, `priya-ai`, `priya-cli`, `priya-module-sdk`, `priya-testkit` and `priya-themegen`.
- [x] Directory layout:
  - `crates/`, `modules/examples/`;
  - `ui/{design,PriyaUI,shell,gallery,tests,mockups,i18n}`;
  - `packaging/{arch,systemd}/`, `scripts/`, `data/`, `xtask/`.
- [x] 👤 The owner picks the project licence. Record it as an ADR in `decisions.md` and add `LICENSE` and `THIRD_PARTY.md`. Apache-2.0 ([ADR-022](../architecture/decisions.md)), plus `NOTICE`.
- [x] `priya-api::error`:
  - `ErrorKind`: `InvalidInput`, `NotFound`, `Conflict`, `PermissionDenied`, `Unavailable`, `Timeout`, `Cancelled`, `ProviderError` and `Internal`.
  - `ApiError { kind, retryable, user_message, detail }`.
  - Trait `UserFacing { fn kind(); fn retryable(); fn user_message() }`, implemented by every crate's `thiserror` enum.
- [x] `priya-core::paths::PriyaPaths`: config `$XDG_CONFIG_HOME/priya`, data `$XDG_DATA_HOME/priya`, runtime `$XDG_RUNTIME_DIR/priya` and state `$XDG_STATE_HOME/priya`. It uses the XDG fallbacks on macOS too. Each folder can be overridden with `PRIYA_CONFIG_DIR`, `PRIYA_DATA_DIR`, `PRIYA_RUNTIME_DIR` or `PRIYA_STATE_DIR`; relative XDG values are ignored (as the spec says) and relative overrides are an error. Without `XDG_RUNTIME_DIR` (macOS) the runtime folder is `$TMPDIR/priya-runtime-$USER`. Tests resolve paths from a map (`PriyaPaths::from_lookup`) instead of changing the process environment.
- [x] `priya-core::secret::Secret<T>`: `Debug`/`Display` print `[redacted]`, there is no `Serialize`, and the value is zeroised on drop.
- [x] `priya-core::clock::Clock` trait with `SystemClock`, so time is injectable everywhere (`FakeClock` comes in 0.4).
- [x] `priyad::logging::init()` (also used by the CLI with `--verbose`; now `priya_logging::init()`, see 0.7). It writes to journald via `tracing-journald` when `JOURNAL_STREAM` is set, and otherwise to stderr through a `tracing-subscriber` formatter. The filter comes from `PRIYA_LOG` (default `info`).
- [x] `xtask` skeleton (clap). `cargo xtask ci-local [--fast]` runs fmt, clippy, the Rust tests (nextest when installed) and `scripts/tests/run.sh`; on macOS it limits clippy and tests to the portable crates. The other subcommands arrive with their sub-phases.

**Tests**
- Unit:
  - `api_error_serializes_kind_retryable_user_message` (`CORE-04`).
  - `paths_respect_xdg_overrides` (`infra`).
  - `secret_debug_and_display_are_redacted` (`infra`).
  - `system_clock_is_monotonic_enough` (`infra`).
- Regression: all.

**Done when**
- [x] `cargo build --workspace` and `cargo test --workspace` pass on the Zenbook (`cargo xtask ci-local`, 2026-10-20). The portable crates build and test on the Mac.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` is clean (on macOS, and for the `x86_64-unknown-linux-gnu` target).
- [x] The licence ADR is merged (ADR-022).

---

### 0.3 — CI pipeline v1 and PR workflow
**Outcome:** Every push and pull request runs lint, licence and advisory audit, build and tests on Linux and macOS. `main` changes only through pull requests with a green `ci-pass`, and local hooks catch problems before a push.
**Features:** QA-01, QA-02, QA-03, QA-10.
**Depends on:** 0.2.

**Tasks**
- [x] `.github/workflows/ci.yml` runs on `pull_request` and on `push` to any branch. It is also callable as `workflow_call`. Settings: `concurrency: ci-${{ github.ref }}` with cancel-in-progress, top-level `permissions: contents: read`, and every third-party action pinned by commit SHA. Jobs:
  - `fmt`: `cargo fmt --all --check`.
  - `clippy`: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
  - `deny`: `cargo deny check licenses advisories bans sources` (QA-10).
  - `typos`: `typos`.
  - `links`: `lychee --offline --include-fragments '**/*.md'`.
  - `build-linux`: x86_64, all crates, `--locked`.
  - `build-macos`: `macos-14` arm64, portable crates only, read from workspace metadata.
  - `test-linux`: `cargo nextest run --workspace --profile ci`.
  - `test-macos`: `cargo nextest run` on the portable crates.
  - `secrets`: `gitleaks detect --log-opts=<base>..<head>`.
  - `ci-pass`: the aggregator. It `needs:` every required job, runs with `if: always()`, and fails if any needed job failed, was cancelled or was skipped. Later sub-phases add their jobs to `needs`: `trace`, `coverage` and `flaky` (0.4), `schemas` (0.9), `tokens` (0.10), `ui` (0.11), `package` (0.14). Branch protection therefore needs only this one check name.
  As built: `push` skips `main`, because `main.yml` calls this workflow there, so it doesn't run twice. A `scripts` job runs shellcheck and `scripts/tests/run.sh`. `secrets` uses `gitleaks git`, which replaced the deprecated `detect` command, via `ci/gitleaks-range.sh`. Shared setup lives in `.github/actions/setup-rust` and `.github/actions/install-gitleaks`. `cargo xtask portable` prints the macOS crate list. Tool versions installed by `taiki-e/install-action` are pinned; gitleaks is pinned by version and SHA-256 and bumped by hand. `scripts/lint-shell.sh` is the one shellcheck list for CI and `ci-local`.
- [x] Caching: `Swatinem/rust-cache`, keyed by `Cargo.lock` and the toolchain, on every Rust job. Target: PR pipeline wall time under 10 minutes at the end of Phase 0.
- [x] `.github/workflows/main.yml` runs on push to `main`. It calls `ci.yml`, then `cargo xtask test regression --fast` (0.4) and the package build (0.14), and uploads artifacts.
- [x] `.github/workflows/nightly.yml` runs on cron at 02:00 UTC against `main` and can also be started by hand:
  - `cargo xtask test regression --all`;
  - `cargo deny check advisories` against a freshly fetched advisory database;
  - `flaky-hunt` (0.4).
  
  A `report` job (`permissions: issues: write`) opens or updates one issue labelled `nightly-failure` and closes it after the next green night.
  As built (main and nightly): `main.yml` calls `ci.yml` and then runs `cargo xtask test regression --fast --profile ci` in the CI image (from 0.4); it calls `ci.yml` in its own concurrency group (`main-<sha>`; sharing `ci-<ref>` with the called workflow deadlocks). Until 0.4 the nightly `tests` job runs every workspace test; `regression --all` and `flaky-hunt` replace it then. A manual run with `force_failure` fails on purpose to test the report issue. Test jobs upload their JUnit reports.
- [x] `ci/Dockerfile` and `.github/workflows/ci-image.yml` build `ghcr.io/prrockzed/priyaos-ci`.
  - Contents: Arch with packages pinned to an Arch Linux Archive date; the Rust toolchain from `rust-toolchain.toml`; Qt 6, Quickshell, cage, sway, grim, wtype and dssim; the fonts from 0.1; a non-root `builder` user for `makepkg`.
  - Rebuilt weekly and whenever the Dockerfile changes. Jobs reference the image by digest; Renovate bumps the digest.

  As built: tools come from Arch packages where they exist (nextest, cargo-deny, llvm-cov, insta, gitleaks, typos, lychee); `dssim` is `cargo install`ed. Rust lives in `/usr/local/{rustup,cargo}` so root (CI jobs) and `builder` (makepkg) share it. Pull requests that touch the image build and smoke-test it without pushing; `main`, the Monday schedule and manual runs push `:<sha>` and `:latest`. The Linux jobs of `ci.yml` (fmt, clippy, deny, build, test, scripts) and the nightly tests run in it by digest (test: `job_containers_are_pinned_by_digest`); the image is private, so those workflows grant `packages: read`.
- [x] `deny.toml`:
  - Licences: an allow-list of MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0 and CC0-1.0, adjusted to the licence ADR.
  - Advisories and yanked crates: `deny`. Unmaintained crates: `warn`.
  - Sources: crates.io only. Duplicate versions: `warn`.
  As built: `unmaintained = "workspace"` (cargo-deny 0.20 has no "warn" level for it), so an unmaintained direct dependency fails and deeper ones are left to the nightly advisory run. `cargo xtask ci-local` runs licences, bans and sources offline. `deny_rejects_disallowed_license_fixture` is a script test (`scripts/tests/test_deny_rejects_proprietary.sh`, suite `integration`), because it runs the real `cargo deny` on a copied fixture workspace.
- [x] `cargo xtask licenses`: every vendored non-Rust asset under `ui/` (Lucide icons, from 0.10) must have an entry in `THIRD_PARTY.md`. Runs inside the `deny` job.
  A bundle is a folder under `ui/` with its own `LICENSE*`, `LICENCE*` or `COPYING*` file; THIRD_PARTY.md must name it in backticks. Font files must sit inside a bundle.
- [x] `.github/pull_request_template.md` asks for:
  - the sub-phase and the features covered;
  - the tests added;
  - screenshots (for UI);
  - a coverage justification, if any;
  - ADR references;
  - a safety checklist: no operation on the host root, no secrets in fixtures.
- [x] Local hooks:
  - `scripts/install-hooks.sh` runs `git config core.hooksPath .githooks`.
  - `.githooks/pre-commit`: `cargo fmt --check` on staged Rust, `clippy` and nextest unit tests on changed crates, and `gitleaks protect --staged`. `cargo xtask trace --staged` is added in 0.4, and `qmlformat`/`qmllint` on staged QML in 0.11. Budget: under 30 s on the Zenbook.
  - `.githooks/pre-push`: `scripts/dev-check.sh` and `cargo xtask ci-local --fast`.

  As built: the pre-commit logic is `cargo xtask pre-commit` (`xtask/src/pre_commit.rs`). Formatting is checked on the staged blob (rustfmt output compared with it, since `rustfmt --check` exits 0 on stdin); clippy and nextest unit tests (`kind(lib) | kind(bin)`) run on the crates owning staged files, or on every crate when a workspace-wide file changed; then `gitleaks git --pre-commit --staged`. nextest is required everywhere now; the `cargo test` fallback is gone. The three integration tests are `scripts/tests/test_pre_commit.sh`, run in a throwaway one-crate repository. Their 30 s check times the hook's own work on a tiny crate; the real budget on the Zenbook is checked by hand when the hooks are installed there. Both dev-setup scripts now run `scripts/install-hooks.sh`.
- [x] Add a "never use `--no-verify`" rule to CLAUDE.md's conventions.
- [x] `scripts/setup-branch-protection.sh` applies a ruleset to `main` with `gh api`:
  - pull request required, with 0 required approvals (single owner);
  - required status check `ci-pass`, with the branch kept up to date (strict);
  - linear history;
  - no force-push and no deletion;
  - no bypass for admins.

  A first ruleset "Protect main" (id 25430268) has been active since 2026-10-20 with everything above except `ci-pass`; the repo allows squash merges only and deletes merged branches. The script updates that ruleset in place and adds the `ci-pass` check. Claude runs it with the owner's go-ahead (2026-10-20). Applied 2026-10-20: `ci-pass` is required (strict, reported by GitHub Actions only, app id 15368) and squash is the only merge method. The repository also allows auto-merge, so `gh pr merge --auto --squash` lands a PR once `ci-pass` is green. Test: `branch_protection_requires_ci_pass` checks the ruleset JSON.
- [x] `renovate.json`: weekly, grouped cargo updates, GitHub Action digests and the CI image digest. 👤 The owner installs the Renovate GitHub app.
  Renovate is installed (2026-10-20). The CI image rule takes effect once the image exists.

**Human in the loop:** the repository (`prrockzed/PriyaOS`) is **public** (checked 2026-10-19), so branch rulesets and macOS runners are available on the free plan. If it is ever made private, branch rulesets need GitHub Pro and macOS minutes count 10×; revisit this then. 👤 The owner installs Renovate. Since 2026-10-20 the owner lets Claude merge its own PRs once `ci-pass` is green, and run the protection script.

**Tests**
- Unit:
  - `ci_pass_needs_every_required_job`: parses `ci.yml` and fails if a job is neither in `ci-pass.needs` nor on the explicit non-required list (`QA-01`).
  - `workflow_actions_pinned_by_sha` (`QA-01`).
  - `deny_rejects_disallowed_license_fixture`: `cargo deny` on a fixture workspace whose dependency has `LicenseRef-Proprietary` fails (`QA-10`).
  - `licenses_check_requires_third_party_entry` (`QA-10`).
- Integration, script tests in a temporary git repo:
  - `pre_commit_rejects_unformatted_rust` (`QA-03`).
  - `pre_commit_rejects_staged_fake_api_key`: uses a synthetic `sk-ant-…` string (`QA-03`).
  - `pre_commit_passes_clean_change_under_30s` (`QA-03`).
- Manual, recorded in the PR (👤): a PR with a failing test cannot be merged; `git push origin main` is rejected (`QA-02`).
- Regression: all automated tests above.

**Done when**
- [x] Each of these turns `ci-pass` red and blocks the merge button: a clippy warning, a failing unit test, a non-allow-listed licence, a committed fake key.
  Shown 2026-10-20 by demo PRs prrockzed/PriyaOS#23 (clippy, and coverage for the untested function), #24 (test-linux and test-macos), #25 (deny) and #26 (secrets), each closed unmerged.
- [x] Direct pushes to `main` are rejected (or option (c) above is recorded as a decision). Checked on 2026-10-20: the "Protect main" ruleset is active with no bypass actors and requires a pull request and passing checks.
  👤 The ruleset (id 25430268) requires a PR, a green strict `ci-pass` and linear history, with no bypass; Claude Code's safety check doesn't let Claude try a push to `main`, so the owner confirms once with `git push origin HEAD:main` from any branch (it must be refused).
- [x] Median PR pipeline time over 5 runs is under 10 minutes.
  Measured 2026-10-20 over 10 PR runs: median about 2 min 40 s.
- [x] Hooks are installed on the Mac and the Zenbook (Zenbook: 2026-10-20).
  👤 Installed in Claude's clones on the Mac (and `core.hooksPath` is set in the owner's checkout); the owner runs `bash scripts/install-hooks.sh` on the Zenbook.
- [x] A forced nightly failure opens the issue, and the next green night closes it.
  Shown 2026-10-20: a manual run with `force_failure` opened prrockzed/PriyaOS#29, and the next green manual run closed it ("Green again").

---

### 0.4 — Test infrastructure (testkit, tags, traceability, diff coverage, regression runner)
**Outcome:** Every test states which features it covers. CI fails in four cases: a feature claims progress without tests; changed code is under-tested; a permanent regression test disappears without an ADR; the quarantine list grows too long. `cargo xtask test regression` runs the permanent suite.
**Features:** QA-04, QA-05, QA-06, QA-07, QA-11.
**Depends on:** 0.3.

**Tasks**
- [x] Tag grammar, to be added to [09 §3](../architecture/09-testing.md). Tags are comment lines directly above a test: `#[test]`/`#[tokio::test]`, a QML `function test_*`, a shell `test_*` function, or a Python `def test_*`. Rust and QML use `//`; shell and Python use `#`. Tags:
  - `covers: <ID>[, <ID>…]` or `covers: infra`;
  - `suite: unit|contract|integration|ui|e2e|perf|chaos|security|eval|live|mutation`;
  - `acceptance: <sub-phase>` (optional);
  - `regression: <sub-phase>`, added at the gate;
  - `bug: <issue>` (optional).
- [x] `xtask/src/tags.rs`: scans `crates/**`, `modules/**`, `ui/tests/**` and `scripts/tests/**`. It builds `target/trace/tests.json`, keyed by test id (`<crate>::<module path>::<fn>`, or `<file>::<function>` for non-Rust). Malformed or missing `covers:`/`suite:` tags are errors.
- [x] `cargo xtask test <suite>`. It resolves the test list from the index into:
  - cargo-nextest filtersets (`-E 'test(=…) | …'`, chunked) for Rust;
  - `qmltestrunner -functions` for QML;
  - `pytest -k` for Python;
  - direct invocation for shell tests.
  
  `cargo xtask test regression --fast` runs tests tagged `regression:` from the unit, contract, integration and UI suites. `--all` adds every other suite. `.config/nextest.toml` defines three profiles:
  - `default`;
  - `ci`: retries 0, JUnit at `target/nextest/ci/junit.xml`, slow-timeout 60 s, terminate after 120 s;
  - `nightly`.

  As built: the scanner is `xtask/src/tags/` (`syn` for Rust module paths, line rules for QML, Python and shell). It also scans `xtask/`, whose tests are tagged too, and skips every `fixtures/` folder. A shell test file is one test, tagged in its header. Running QML and Python tests by tag fails with a clear message until their runners exist (0.11 for QML). On macOS, Rust tests of Linux-only crates are skipped with a note.
- [x] `cargo xtask trace`. Inputs: the tables in `docs/claude/features.md` (ID, phase, status), every phase file (`### N.k —` headings and their **Done when** checkboxes), the test index and the latest JUnit results. It fails when:
  1. a test references an unknown feature ID;
  2. a feature marked 🟨 or ✅ has no tagged test;
  3. a ✅ feature has no test that passed in this run;
  4. a sub-phase whose Done-when boxes are all ticked has an `acceptance:` test without `regression:`;
  5. compared with the base branch, a test tagged `regression:` was deleted or lost the tag and the PR body cites no existing `ADR-NNN`. A rename is allowed when a new test in the same PR carries the same `covers` and `acceptance` values;
  6. a test lacks `suite:`.
  
  Outputs: `target/trace/matrix.md` and `matrix.json` (feature → tests → last result), uploaded as artifacts. `--staged` mode checks only staged test files, for the pre-commit hook.
  As built (trace): `xtask/src/trace/`. Results come from `--junit` reports and `--script-results` (written by `scripts/tests/run.sh`), passed explicitly so a stale file never counts; rule 3 applies only when results are given. The base branch is exported with `git archive` into a temporary folder (nothing touches `.git`) and indexed leniently. A rename must itself carry `regression:` and excuses one removed test. A cited ADR turns removals into notes printed in the output. Instead of a `--staged` flag, the pre-commit hook runs the tag and feature-ID check whenever a test-like file is staged. CI's `trace` job needs `test-linux` and `scripts` and uploads the matrix; `ci-local` runs it without results. The end-to-end test `xtask_trace_end_to_end_on_fixture_repo` builds a small repository (catalog, phase file, Rust, QML and shell tests, JUnit and script results) in a temporary folder and compares the matrix with a golden copy.
- [x] `cargo xtask regression promote <sub-phase>`: adds `regression: <id>` under every `acceptance: <id>` tag. It is run in each sub-phase's gate PR.
  As built: 0.2's nine acceptance tests were promoted with it (0.2's Done-when boxes are all ticked).
- [x] `cargo xtask coverage` (Linux):
  - Measure: `cargo llvm-cov nextest --workspace --lcov --output-path target/coverage/lcov.info`.
  - Compare: `diff-cover` against `origin/main`, per group from `xtask/coverage.toml`. The default threshold is 85 %. `crates/priya-core/**` and `crates/priya-ai/**` need 90 %; `priya-sandbox` and `priya-policy` join at 90 % when they exist. Excluded: generated files (`ui/PriyaUI/Theme.qml`, `ui/PriyaUI/api/Api.js`, `docs/claude/api/**`), `tests/` directories and `#[cfg(test)]` modules.
  - Exception: the PR label `coverage-exception` plus a non-empty "Coverage justification" section in the PR body. The job then passes with a warning shown in the PR report.
  - QML/JS has no line-coverage gate in Phase 0; 0.11's component tests cover it.

  As built: `xtask/src/coverage.rs` computes diff coverage itself instead of calling `diff-cover`: it reads the lcov report from `cargo llvm-cov nextest`, the `git diff -U0` against the merge base (plus untracked files locally), and drops test code: files only tests compile (`#[cfg(test)] mod x;`, `#![cfg(test)]` and everything below them, found by the same module walk as the test index), and items under `#[test]`, `#[tokio::test]` or a `cfg` requiring `test` (functions, modules, impls, impl methods). Changed files without coverage data are listed as "not measured" instead of passing silently. The diff uses fixed `a/`/`b/` prefixes whatever the git config says. Groups and exclusions live in `xtask/coverage.toml`; `xtask/src/main.rs` (argument dispatch) is excluded. It also runs on macOS for the portable crates. `test-linux` runs the tests once under `cargo llvm-cov nextest` and uploads both the JUnit and lcov reports; the CI `coverage` job uses them on pull requests (other runs pass), reads the PR's labels and description from the API when it runs through `.github/actions/pr-details` (so labelling and re-running the job is enough; the `trace` job reads ADR citations the same way), and uploads `report.md` for the PR report. The PR template's section is "## Coverage justification".
- [x] Flaky-test quarantine:
  - `tests/quarantine.toml` holds entries `{ test, issue, since }`.
  - A quarantined Rust test carries `#[ignore = "flaky: #<issue>"]`; QML uses `skip("flaky: #<issue>")`.
  - `cargo xtask flaky check` fails on: an attribute without an entry or an entry without an attribute; more than 5 entries; any entry older than 7 days (it warns from day 5). It prints the list into the job summary.
  - Nightly `flaky-hunt`: runs each quarantined test 50× and the unit and contract suites 5× (`--retries 0`), then comments the pass rate on each tracking issue.

  As built: `xtask/src/flaky.rs`. Entries are `[[test]]` tables (`test`, `issue`, `since = "YYYY-MM-DD"`). The test index records each `#[ignore]` reason; any reason starting "flaky" must be exactly `flaky: #<n>` and match its entry. `flaky check` runs in its own CI job (in `ci-pass`) and in `ci-local`, and appends the list to the job summary. `flaky hunt` writes `target/trace/flaky-hunt.json`; the nightly `flaky-comments` job posts each pass rate on its issue. QML `skip("flaky: …")` joins with the QML runner in 0.11.
- [x] `priya-testkit` crate, a dev-dependency only:
  - `FakeClock`: manual `advance()`, implements `Clock`.
  - `TempPriyaHome`: temporary XDG config, data, runtime and state dirs, exported to child processes.
  - `golden`: wrappers over `insta` for JSON and text, with redactions for ULIDs, timestamps and durations.
  - `fixture(path)`: refuses a directory without `RECORDED.md`.
  - `EventLogAssert` / `assert_events!(store, ["setting.changed", …])`, with payload matchers.
  
  As built: `FakeClock`, `TempPriyaHome`, `golden` (`assert_golden_json!`, `assert_golden_text!`; ULIDs, RFC 3339 timestamps and durations (`ms`/`µs`/`ns`, or decimals with `s`/`m`/`h`) in string values are redacted; keys are not) and `fixture`. `EventLogAssert` / `assert_events!` needs the event store, so it moves to 0.5. `TempPriyaHome::apply` also gives child processes a temporary `HOME` and `XDG_*` folders. priya-core's own unit tests can't use `FakeClock` (a dev-dependency would build a second priya-core); its `tests/` folder can.

  Later sub-phases add `TestDaemon` (0.7), `FakeModuleProcess` (0.8), `FakeModelProvider`, `Cassette` and `mock-priyad` (0.12). The `FakeMachine` handle comes in 2.3 and the fake agent in 4.1.
- [x] `cargo xtask fixtures check`, run in the `secrets` job. It fails on a fixture directory without `RECORDED.md` and on secret patterns in fixtures:
  - Anthropic and OpenAI key prefixes;
  - `x-api-key` or `Authorization` headers with non-placeholder values;
  - JWTs;
  - PEM private-key headers.

  As built: fixture folders are `tests/fixtures/` under `crates/` and `modules/`; each file needs a `RECORDED.md` in its folder or above it. Every file is searched, binary or not (bytes read lossily). Keys: `sk-` plus 20+ key characters with a digit and a letter, anywhere in a line. Credential headers (`authorization`, `proxy-authorization`, `x-api-key`, `x-goog-api-key`, `api-key`, any case) fail when one of the next few tokens is a real-looking value (8+ characters, a digit and a letter, no placeholder word such as redacted, test or example as a `-`/`_` part); this covers `Header: value`, JSON objects and arrays, HAR `name`/`value` pairs and YAML values on the next line. `RECORDED.md` is not header-checked. JWTs and `BEGIN … PRIVATE KEY` lines fail. Also runs in `ci-local`; in CI it runs after the commit scan, even when that fails.
- [x] `pr-report` job (`permissions: pull-requests: write`, skipped for forks) keeps one sticky comment up to date. It shows:
  - the diff-coverage table per group;
  - traceability changes (features that gained or lost tests, newly promoted regression tests);
  - the quarantine list;
  - schema changes (from 0.9) and screenshot diffs (from 0.11).

  As built: the `pr-report` job needs `coverage`, `trace` and `flaky`, runs unless cancelled, and assembles their artifacts (`report.md`, `changes.md`, `quarantine.md`) into one comment found by a hidden marker. It is listed in `NOT_REQUIRED` (xtask/src/workflows.rs), not in `ci-pass`. `main.yml` grants `pull-requests: write` because a caller must grant what called jobs request; the job itself is skipped on `main`.
- [x] Add `trace`, `coverage` and `flaky` to `ci-pass`.
- [x] Docs: [09 §3, §5](../architecture/09-testing.md) get the exact tag grammar, the `acceptance:`/`regression:` mechanism and the coverage groups.

**Tests**
- Unit:
  - `tags_parse_rust_qml_shell_python` (`QA-05`).
  - `trace_fails_on_unknown_feature_id` (`QA-05`).
  - `trace_fails_when_in_progress_feature_has_no_tests` (`QA-05`).
  - `trace_fails_when_done_feature_has_no_passing_test` (`QA-05`).
  - `trace_fails_when_completed_subphase_has_unpromoted_acceptance_test` (`QA-05`, `QA-07`).
  - `trace_fails_when_regression_test_removed_without_adr` (`QA-07`).
  - `trace_allows_regression_test_rename` (`QA-07`).
  - `promote_adds_regression_tag_idempotently` (`QA-07`).
  - `regression_selector_picks_only_tagged_tests` (`QA-07`).
  - `coverage_groups_apply_stricter_threshold_to_core` (`QA-06`).
  - `coverage_exception_requires_label_and_justification` (`QA-06`).
  - `flaky_check_fails_over_five_entries` (`QA-11`).
  - `flaky_check_fails_on_attribute_without_entry` (`QA-11`).
  - `flaky_check_fails_after_seven_days`: uses `FakeClock` (`QA-11`).
  - `fake_clock_advances_deterministically` (`QA-04`).
  - `golden_redacts_ulids_and_timestamps` (`QA-04`).
  - `fixture_without_recorded_md_is_rejected` (`QA-04`).
  - `fixtures_check_finds_api_key_and_auth_header` (`QA-04`).
- Integration:
  - `xtask_trace_end_to_end_on_fixture_repo`: a small fixture repo with `features.md`, one phase file and tests in Rust, QML and shell. The output is compared to a golden `matrix.md` (`QA-05`).
  - `xtask_coverage_on_fixture_diff` (`QA-06`).
- Regression: all of the above.

**Done when**
- [x] `trace`, `coverage` and `flaky` run in CI. The matrix artifact lists every Phase 0 feature, with ⬜ features shown as "no tests yet".
  All three are in `ci-pass`; the matrix lists every feature in features.md.
- [x] A demo PR that deletes a regression test fails `trace`. The same PR citing an existing ADR passes. The demo PR is then closed.
  Shown 2026-10-20 by prrockzed/PriyaOS#28: `trace` failed on the deleted test; after the description cited ADR-022, a re-run passed with a note. Closed unmerged.
- [x] A demo PR that adds an untested function to `priya-core` fails `coverage`.
  Shown 2026-10-20 by prrockzed/PriyaOS#23: `core | 0 | 3 | 0.0 % | needs 90 %`.
- [x] `cargo xtask test regression --fast` runs green on the Zenbook (2026-10-20: 60 passed) and in CI (the `Main` workflow on `main`).
  Green in CI (`main.yml` `regression` job after every merge). 👤 The owner runs it once on the Zenbook.
- [x] Every CI run summary shows the quarantine list (empty).
  The `flaky` job writes "Quarantined tests: None." to every run summary.

---

### 0.5 — Event store
**Outcome:** `priya-store` can do the following:
- append hash-chained events;
- keep projections in the same transaction;
- serve filtered subscriptions that resume from a sequence number;
- store large payloads as blobs;
- prove that no row was edited, deleted or reordered.

**Features:** CORE-01; QA-04 (`assert_events!`, moved here from 0.4).
**Depends on:** 0.4.

**Tasks**
- [x] `priya-api::events`:
  - `EventEnvelope { id: Ulid, ts, event_type, version: u16, stream, correlation, causation, actor: Actor, payload }`.
  - `Actor` follows [02 §1](../architecture/02-core.md): `User`, `PriyaAi`, `Agent(name)`, `Module(name)`, `Policy`, plus `System` for priyad itself.
  - A `(type, version)` registry. Each type is a Rust struct with `serde` and `schemars::JsonSchema`.
  - Phase 0 types: `setting.changed`, `settings.load_failed`, `intent.submitted`, `intent.answered`, `intent.cancelled`, `model.called`, `module.started`, `module.stopped`, `module.crashed`, `module.event`, `daemon.started`, `daemon.stopped`.
  - Add the new ones to the 02 §1.1 catalogue: `settings.load_failed`, `intent.cancelled`, `module.stopped`, `daemon.*`, and the `System` actor.
- [x] `crates/priya-store/migrations/0001_init.sql`. It contains the 02 §1 `events` table and indexes, `projection_state(name, version, last_seq)` and `blobs(hash, size, stored_size, refcount, created)`.
- [x] Migration runner on `PRAGMA user_version`, forward-only. Before each migration it makes an online backup to `priya.db.bak-v<N>`. It refuses a database newer than the binary with a plain-language message.
- [x] Connection setup:
  - pragmas: WAL, `synchronous=FULL` (P9 durability), `foreign_keys=ON`, `busy_timeout=5000`;
  - `rusqlite` bundled with FTS5;
  - one writer task owning the write connection, fed by a bounded mpsc channel;
  - a small pool of read-only connections.
- [x] `EventStore::append(NewEvent) -> EventRecord` and `append_all(Vec<NewEvent>)` write in one transaction. Each append:
  - validates the payload against its registered type;
  - offloads payloads over 8 KiB to the blob store as `{"$blob": "<blake3>", "size": n}`;
  - computes `hash = BLAKE3(prev_hash || JCS(row without hash))`, where JCS is RFC 8785 canonical JSON via `serde_jcs`;
  - uses 64 zeros as the genesis `prev_hash`.
- [x] `EventStore::verify_chain(range) -> ChainReport { ok, checked, first_bad_seq, reason }`, with reasons `hash_mismatch`, `prev_link_broken` and `seq_gap`.

  As built (append and verify): the writer is a dedicated thread owning the write connection, fed by a bounded `tokio::sync::mpsc` queue (default 256); reads run on blocking threads with a pool of read-only connections (default 4). The hash covers `seq` and every column but `hash`, as a JSON object canonicalised with `serde_jcs`, prefixed by `prev_hash`; payloads are stored as canonical JSON (the validated, normalised form), so key order never matters. A payload whose canonical JSON exceeds 8 KiB is stored as `{"$blob": "<hash>", "size": n}`; `EventStore::payload` loads it back. A batch is all-or-nothing and leaves the chain head unchanged on failure. `verify_chain` reports a gap at the missing seq, and a range with nothing stored in it checks fine. The writer resumes past the highest seq ever used (`sqlite_sequence`), so rows deleted from the end show up as a gap after the next append; until then only checkpoints (Phase 3) would notice. Whole numbers beyond ±2^53 are refused, because canonical JSON stores numbers as doubles. At most `readers` reads run at once. A panic in the writer is reported to that caller as an internal error and the writer keeps serving.
- [x] Projections:
  - Trait `Projection { NAME, VERSION, fn apply(&self, tx, &EventRecord, &full_payload) }`.
  - The registry applies every projection inside the append transaction. When `VERSION` changes, or on `rebuild(name)`, it drops the projection and replays from seq 0 with progress logging.
  - Phase 0 projections: `intents` (id, conversation id, text, answer or blob ref, model, status, timestamps; used by 0.12 follow-ups) and `modules` (name, state, restarts, last error).
- [x] Subscriptions: `subscribe(Filter { types: Vec<Glob>, stream_prefix }, from_seq) -> impl Stream<EventRecord>`.
  - It catches up from the database first, then follows live events through a broadcast signalled after commit.
  - Delivery is at-least-once and ordered per stream.
  - A lagging subscriber goes back to database catch-up; events are never dropped.

  As built (projections and subscriptions): a `Projection` has `name`, `version`, `create`, `drop_tables` and `apply(conn, record, payload)`, where `payload` is always the full payload, even when the row only links to a blob. The writer applies all of them inside each append transaction and records `last_seq`. When the store opens, `prepare` rebuilds new or re-versioned projections and replays the events a projection missed after its `last_seq`; tables of projections no longer registered are left alone. `EventStore::rebuild_projection` rebuilds on request. An event a projection can't decode is skipped with a warning; only database errors fail the append. `intents` keeps the answer inline or the link to its blob, plus the model; an answer wins over a cancellation. A subscription task holds only the store's read side, so dropping the store ends every stream; a stream also ends after its first error (resubscribe from the last seq handled). A subscription always reads from the database: after each commit the writer publishes the head seq on a `watch` channel, which wakes subscribers; so a slow one is just further behind and nothing is dropped. `subscribe(filter, after)` delivers seqs greater than `after`. The property test drives `proptest`'s `TestRunner` from a plain `#[test]`, so its tags stay readable.
- [x] `BlobStore` trait with `LocalBlobStore` at `$XDG_DATA_HOME/priya/blobs/<2 hex>/<hash>`:
  - zstd level 3;
  - each write goes to a temporary file, is fsynced, then renamed;
  - refcounts are kept in `blobs`;
  - operations: `put`, `get`, `incref`, `decref`, and garbage collection of zero-refcount blobs at startup.
  
  The `/var/lib/priya/blobs/<uid>/` location in 01 §5 needs `priya-helper`, so the store moves there in Phase 2.

  As built (database and blobs): `priya_store::db` opens connections with the pragmas above (WAL set by the writer; readers open read-only) and migrates with `MIGRATIONS`, backing up to `<file>.bak-v<N>` before each step and refusing newer schemas with `StoreError::NewerSchema`. Tables are `STRICT` with checks (64-character hashes, valid JSON payloads, versions ≥ 1, non-negative counts), `seq` is `AUTOINCREMENT` so it is never reused, and triggers make `events` append-only at the database level (compaction lifts them on purpose, ADR-019). A new database isn't backed up; later steps are. `BlobStore` methods take the caller's connection, so refcounts change in the same transaction as the events; Every hash is checked to be 64 lowercase hex characters before it becomes a path; `get` checks the content against its hash; `put` rewrites a missing file and uses a unique temporary name per write; `gc` runs at startup only, never follows symlinks, and deletes only blob-shaped files (unreferenced, or left by a crash) and temporary files. The writer task and read pool come with `EventStore::append`.
- [x] Upcaster registry `priya_api::events::upcast(type, from_version, payload)`. It is empty in Phase 0, but wired now so 0.9 can enforce it.
- [x] `priya-testkit::assert_events!(events, ["type" => {payload pattern}, …])`, moved here from 0.4.

  As built: it takes any iterator of `&EventEnvelope` rather than the store (the testkit can't depend on `priya-store` without a cycle), so there is no `EventLogAssert` type. A pattern matches when every key it names has that value, nested objects alike; patterns see the payload as given, so a stored event over 8 KiB must be resolved with `EventStore::payload` first. `priya_testkit::events::setting_event(key, value)` builds valid events in bulk.

**Tests**
- Unit (all `covers: CORE-01`):
  - `append_assigns_monotonic_seq_and_links_hashes`
  - `hash_is_independent_of_json_key_order`
  - `verify_detects_edited_payload`
  - `verify_detects_deleted_row`
  - `verify_detects_reordered_rows`
  - `large_payload_goes_to_blob_and_round_trips`
  - `blob_put_is_idempotent_and_refcounted`
  - `blob_gc_removes_only_zero_refcount`
  - `projection_rebuild_equals_incremental` (proptest, 1,000 random sequences)
  - `projection_version_bump_triggers_rebuild`
  - `subscription_filters_by_type_glob_and_stream_prefix`
  - `subscription_resumes_from_seq_without_gaps`
  - `lagging_subscriber_catches_up_from_db`
  - `migration_creates_backup_and_sets_user_version`
  - `refuses_newer_schema_version_with_plain_message`
  - `unknown_event_type_rejected`
- Integration:
  - `events_survive_kill_during_append_loop`: a child process appends in a loop and is killed with SIGKILL. After reopening, the chain verifies and there is no partial row.

    As built: the child is the test binary itself, running the ignored `append_loop_child`. It prints the last seq of each acknowledged batch, and each of three rounds kills it only after three acknowledgements, so every round tests something; afterwards every acknowledged seq must be stored. SIGKILL keeps the page cache, so this proves all-or-nothing commits and that acknowledged ones outlive the process, not power-cut durability (a VM test later). The child stops when its stdin closes and is killed on drop, so it never outlives the test.
- Perf (informational, Zenbook): `append_10k_events_under_2s`. As built: one append (one transaction) per event, as priyad does; the budget is enforced only in release builds, run with `cargo nextest run --release -p priya-store -E 'binary(perf)'`.
- Regression: all unit tests and the integration test.

**Done when**
- [x] All tests are green on the Mac and on Linux. `priya-store` diff coverage is at least 85 %.
- [x] Editing any column of any row in a copy of a test database is reported by `verify_chain` at the right seq.
- [x] `intents` rebuilt from scratch is identical to incremental application across 1,000 proptest cases.

---

### 0.6 — Settings system
**Outcome:** Settings load from layered TOML files and are typed and schema-validated. An invalid file is rejected with a plain message that names the file and line, and the last valid config stays active. Edits are picked up live, and every change becomes a `setting.changed` event.
**Features:** CORE-03.
**Depends on:** 0.5.

**Tasks**
- [x] `priya-core::settings` is pure and builds on macOS. Root type: `Settings { settings_version, ui, ai, log, modules }`, with `serde` and `schemars` and `deny_unknown_fields` on core sections. Defaults are defined in code and mirrored to `/usr/share/priya/defaults/00-core.toml`. Phase 0 keys:
  - `ui.theme` (`light` | `dark`, default `dark`, because the Zenbook has an OLED screen);
  - `ui.bar.width_px` (720);
  - `ui.reduced_motion` (false);
  - `ai.strategy`: only `fixed:<provider>/<model>` is accepted; other strategy values return "arrives in Phase 1". Unset means no model is configured;
  - `ai.providers.<name> { kind: anthropic | openai_compatible, base_url?, enabled }`;
  - `ai.max_output_tokens` (2048);
  - `log.level`;
  - `modules.enabled`, plus the module sections `modules.<name>.*`.

  As built (model): the shipped defaults are `data/defaults/00-core.toml` (installed by the 0.14 package), and `shipped_defaults_match_the_code` checks it key for key against `Settings::default()`. Field doc comments become schema descriptions, so they are written for the owner. The log section is the `Logging` type, with `PRIYA_LOG` still winning. `ui.bar.width_px` must be 320–7680 and `ai.max_output_tokens` 1–1,000,000, checked when read and stated in the schema. Provider names (`ProviderName`, `[a-z0-9][a-z0-9_-]*`) are checked as map keys and in `fixed:<provider>/<model>`, whose model id may contain slashes and colons (only the first `/` ends the provider). `ai.providers.<name>.enabled` defaults to true. `best`, `balanced`, `cheapest`, `local_only` and `family:<name>`, alone or combined with `,`, `+` or spaces, are refused with "arrives in Phase 1", and so is `fixed:` followed by filters (`fixed:a/b,local_only`), which means something else in Phase 1; anything else is refused as not a strategy; values aren't trimmed. Strategy messages leave out the value, which the loader adds with the file and line. Module sections must be tables (so a misspelt `enabled` is caught) and are checked against each module's schema in 0.8. `settings_version` is checked and migrated per file, before merging.
- [x] Layer loader, from lowest to highest:
  1. shipped defaults: `/usr/share/priya/defaults/*.toml`;
  2. system: `/etc/priya/*.toml`;
  3. user: `$XDG_CONFIG_HOME/priya/*.toml`;
  4. project: `.priya.toml`, found by walking up from a given cwd and never above `$HOME`;
  5. task: `SettingsOverrides` passed with an API call.
  
  Files within a layer merge in lexical order. Tables deep-merge; arrays and scalars replace. Every leaf key keeps its provenance `{layer, file, line}`.
- [x] Validation:
  - Core sections are checked by typed deserialisation. Module sections are checked against each module's JSON Schema (`jsonschema` crate), wired in 0.8.
  - Errors carry file, line and key, plus a plain-language `user_message`, for example: "`ui.theme` must be light or dark; you wrote 'darkk' in ~/.config/priya/settings.toml line 3".
  - Unknown keys get a "did you mean" suggestion (`strsim`).

  As built (loader and validation): `settings::load(sources, home)` is pure. It takes each file's text as a `Source` (layer, path, content) or a call's overrides as JSON, so reading files is the caller's job. Sources are applied by layer and then by path, whatever order they come in. Keys are `KeyPath`s (a list of parts, printed TOML-style), so a quoted key containing a dot (`[ai.providers."my.lab"]`) stays one key. Each file's `settings_version` is checked first: a file from a newer PriyaOS is refused, and an older one is migrated in memory, keeping the lines the owner wrote for keys that survive the migration. TOML is parsed with `toml_edit`, keeping spans, and converted to JSON: dates and times become RFC 3339 strings, so module sections never see TOML-specific types. Every value, and every table written out in a file, records `{layer, file, line}`. `Layered::origin(key)` falls back to the first value below a key, then to the nearest table above it. Every unparsable file is reported at once, with its line, and common mistakes (unquoted text, a key set twice, an unclosed `[`) get plain wording. Checking walks the merged tree against `Settings::json_schema()`. That finds unknown keys, each with a "did you mean" from the same table (Damerau–Levenshtein) or the same name in another section. It also finds values where a section belongs. Then `serde_path_to_error` names each failing key; the value is dropped, so its default applies, and checking repeats, so every problem is reported in one go, sorted by layer, file and line, up to 50 (then "there are more problems; fix these first"). A table missing a field that was itself reported, or suggested for a misspelling, isn't reported again. The schema says what was expected ("light or dark", "a whole number from 320 to 7680", "a list of text"), so serde's wording never reaches the owner. Only our own types' refusals (a strategy, a provider name) pass their message on. Values are repeated back escaped and cut at 40 characters. `project_file(cwd, home, is_file)` looks for `.priya.toml` up to `home`; outside home it looks only in `cwd`; a path with `..` finds nothing. Checking module sections against each module's schema comes with the module host (0.8).
- [x] Writer `set(key, value)` and `unset(key)` act on `$XDG_CONFIG_HOME/priya/settings.toml`. They use `toml_edit` so comments and order survive, validate the merged result before writing, and write atomically (temporary file, fsync, rename).

  As built so far: the pure part. `settings::set(text, key, value)` and `unset(text, key)` edit a file's text with `toml_edit`.
  - Comments, order, spacing, CRLF line endings and a missing final newline all survive.
  - A replaced value keeps its trailing comment, and a table or inline table given new keys is edited in place.
  - New sections go at the end of the file, so they never capture the comments of the section after them.
  - `unset` drops the comments right above a key but keeps a section's comments (those before a blank line), moving them to the next key or section.
  - Keys are `KeyPath`s, so quoted parts are written back quoted.

  `settings::change(sources, Target { layer, file }, Change, home)` upgrades an older-format file, makes the edit, and checks the file with the layers up to its own, so a higher layer can't hide a bad value. It refuses only the problems the edit adds ("Not changed: `ui.theme` must be light or dark (you gave 'darkk')"), so a file with other problems can still be fixed one key at a time. Only what applies before the target counts: lower layers and earlier files of its own layer. A problem in another file names that file and line instead of "you gave". Problems are compared by key and message as shown, so a different bad value whose first 40 characters match an old one passes as old, and a new problem past the 50-problem cap can go unseen; both are rare. An unparsable file is reported with its line. Text with line breaks is written escaped on one line, so a CRLF file can't change it. The golden test edits the hand-written `crates/priya-core/tests/fixtures/settings-edit/commented.toml`. `priyad::settings::write_atomic` writes a hidden temporary file beside the target, fsyncs it, renames it over the target and fsyncs the folder; `atomic_write_survives_kill_mid_write` SIGKILLs a writing child three times and always finds one whole version.
- [x] Live reload in priyad: `notify` crate, 250 ms debounce, watching the default, system and user dirs. Project files are read fresh for each request instead of watched.
  - Valid content: swap `Arc<Settings>` and emit one `setting.changed {key, old, new, source}` per changed leaf (`file` for an edit on disk, `user` for `set`/`unset`; `module` and `natural_language` come later).
  - Invalid content: keep the last valid settings, emit `settings.load_failed {file, error}`, and log once per content hash.
- [x] `SettingsHandle::watch(prefix)` lets in-process consumers (AI gateway, theme) react to changes.

  As built (service): `priyad::settings::SettingsService` reads the default, system and user folders (`*.toml`, hidden files skipped, in path order). It keeps the last valid settings and the files they came from, and publishes them on a `watch` channel.
  - **Starting.** Temporary files a killed write left behind are swept. If a file is broken at start, the rest still apply.
  - **Changes.** Each change records one `setting.changed` per changed value, found with `settings::diff`: `source: file` for an edit on disk and `user` for `set`/`unset`. The actor is `system` for a value from the defaults or `/etc`, else `user`. Starting up records none.
  - **Broken files.** A broken or unreadable file records one `settings.load_failed { file, error }` per file and version (a hash of its text), with the path shown as `~/…`, and is logged once; the last valid settings stay.
  - **Events.** They go through an `EventSink` port; the event store implements it, checked by a contract test.
  - **Edits.** `set`/`unset` run `settings::change` on the user file, write it atomically and reload, all under one lock. Only an unreadable target blocks an edit. If another file's problems keep the result from loading, `set` says "Saved, but not in force until you fix these".
  - **Atomic writes.** `write_atomic` writes through a symlink, keeps the file's permissions, and syncs a new folder's parent.
  - **Live reload.** `watch(service, DEBOUNCE)` uses `notify` on the three folders and the user folder's parent, so a user folder that is deleted and made again is watched again. Only changes count, not reads (inotify reports opens), and rescans and watcher errors count as changes. It reloads once things are quiet for 250 ms, at most 1 s after a burst starts, and once at start for edits made before watching began. The user folder is made if missing; a missing system or defaults folder is picked up after a restart.
  - **Requests.** `effective(cwd, overrides)` builds on the last valid files plus the project file, read fresh, plus the request's overrides.
  - **Prefix watch.** `SettingsHandle::watch(prefix)` returns a `PrefixWatch` that wakes only when the values under the prefix change.
  - **Tests.** The integration tests use their own temporary folders for the defaults, system and user layers, which `TempPriyaHome` doesn't have. `priya_testkit::LoopChild` (re-run the test binary as a child, SIGKILL it) is shared by this crash test and the event store's.

- [x] `Settings::json_schema()` provides the schema export used by 0.9.
- [x] Settings migrations framework keyed on `settings_version`: `Vec<(from, fn(&mut DocumentMut))>`. It is empty in Phase 0 and exercised by synthetic tests in 0.9.

  As built: `settings::migrate(text)` returns `Current` or `Upgraded { text, from }`, editing the document with `toml_edit` so comments survive. A file without `settings_version` is version 1; a newer one is refused with a plain message. `load` runs it on every file before merging, and `shipped_migrations_have_no_gaps` checks the chain.

**Tests**
- Unit (all `covers: CORE-03`):
  - `layers_override_in_order`
  - `tables_merge_arrays_replace`
  - `project_layer_stops_at_home`
  - `task_overrides_apply_only_to_that_call`
  - `provenance_reports_file_and_line`
  - `invalid_enum_value_gives_plain_message_with_location`
  - `unknown_core_key_rejected_with_suggestion`
  - `set_preserves_comments_and_order` (golden)
  - `set_rejects_invalid_value_without_touching_file`
  - `only_fixed_strategy_accepted_in_phase0`
- Integration (temporary layer folders, in-process watcher):
  - `hot_reload_applies_valid_edit_and_emits_event`: the event arrives within 2 s.
  - `hot_reload_keeps_last_valid_on_broken_file`: exactly one `settings.load_failed`.
  - `atomic_write_survives_kill_mid_write`
- Regression: all.

**Done when**
- [x] A comment-heavy fixture file stays byte-identical after `set`, apart from the changed line.
- [x] A broken file never changes the active settings.
- [x] Changing `ui.theme` in an editor reaches a `watch` consumer within 1 s. This is checked again end to end in 0.7.

---

### 0.7 — priyad, public API and CLI skeleton
**Outcome:** `priyad` runs as a systemd user service and serves JSON-RPC on its socket. The `priya` CLI can show status, read and change settings, tail events, verify the audit chain and run `priya doctor`. Every API method has a matching CLI command.
**Features:** CORE-04, CORE-05, CORE-01, CORE-03.
**Depends on:** 0.5, 0.6.

**Tasks**
- [x] `priyad` binary (tokio):
  - a single-instance lock (`$XDG_RUNTIME_DIR/priya/priyad.lock`, flock);
  - socket directory 0700 and `priyad.sock` 0600;
  - peer-credential check: `SO_PEERCRED` uid must equal the daemon's uid (`getpeereid` on macOS);
  - `sd_notify` READY and STOPPING;
  - `daemon.started` and `daemon.stopped` events.
  
  On SIGTERM it shuts down gracefully: stop accepting connections, end in-flight streams with `cancelled`, stop modules, then flush and close the store.
- [x] Framing for surfaces: newline-delimited JSON-RPC 2.0, one object per line, at most 16 MiB. This is what Quickshell's `Socket` with `SplitParser` reads (0.13). Batch requests are supported. Document the framing in [02 §4](../architecture/02-core.md), which does not specify it yet. Module stdio keeps `Content-Length` framing (02 §5.2).
- [x] `priyad::rpc::Router`:
  - A typed method registry: a `Method` trait whose `Params` and `Result` types come from `priya-api`.
  - Per-connection server-to-client notifications.
  - Error mapping: standard codes −32700, −32600, −32601, −32602 and −32603. Application codes:
    - −32001 `not_found`, −32002 `invalid_input`, −32003 `conflict`, −32004 `permission_denied`;
    - −32005 `unavailable`, −32006 `timeout`, −32007 `cancelled`, −32008 `provider_error`.
  - Every error carries `data: {kind, retryable, user_message}`.

  As built (daemon and router):
  - **Instance and socket.** `priyad::daemon` creates the runtime folder 0700 and refuses one that is a link, a file or another user's. It takes the single-instance lock with `File::try_lock` on `priyad.lock`, replaces a stale socket, and binds `priyad.sock` with mode 0600 before opening the store. A socket path over the platform limit gets a message naming `PRIYA_RUNTIME_DIR`. Its own uid comes from `geteuid`.
  - **Peers.** Connections from another uid (tokio's `peer_cred`: `SO_PEERCRED` on Linux, `getpeereid` on macOS) are dropped unanswered.
  - **systemd.** `Systemd` reads `$NOTIFY_SOCKET` once and writes `READY=1` when serving and `STOPPING=1` as soon as the signal arrives, including to Linux abstract sockets.
  - **Router.** `priyad::rpc::Router` maps names to typed handlers: `add::<M>(|context, params, session| …)` decodes params with `decode_params`, and maps an `ApiError` to its code.
    - It answers a line, a batch or a notification per JSON-RPC.
    - Requests on one connection are answered in order.
    - A reply too big to send becomes an internal error.
    - A streaming method (one with a notification) called as a notification isn't run: nobody could recognise its stream.
  - **Server and sessions.** `rpc::server::serve` runs each connection with a `Session`: outgoing lines, subscriptions, and `after_reply()`, so a subscription's first event never precedes the reply naming it. `notify_for` queues a subscription's line under the subscriptions lock, so nothing arrives after the reply to its unsubscribe. A client that closes its sending side is done: it gets the replies it is owed, and its subscriptions end with `v1.events.ended {reason: cancelled}`. However a connection ends, even when stopped from outside, its subscription and writer tasks stop with it. A failed accept waits 100 ms.
  - **Shutdown.** On SIGTERM or SIGINT the server stops accepting connections. Each connection finishes its current request; a reply the client isn't reading is dropped. Every subscription gets `v1.events.ended {reason: cancelled}`, the queue is flushed for up to 2 s, and the connection closes. Connections still open after 5 s are stopped. A second signal exits at once. Then the settings watcher stops (waiting out a reload in progress), `daemon.stopped` is recorded, the store closes and the lock goes last.
  - **Events.** `daemon.started` and `daemon.stopped` are recorded.
  - **Settings folders.** `PRIYA_DEFAULTS_DIR` and `PRIYA_SYSTEM_DIR` override them, for tests.
- [x] Phase 0 methods and their CLI commands (CORE-04):

  | Method | CLI command |
  |---|---|
  | `v1.system.status`: version, uptime, db path, event count, modules, configured model | `priya status` |
  | `v1.system.methods`: introspection | `priya api methods` |
  | `v1.system.doctor {fix?: dry_run \| apply}` | `priya doctor [--fix [--dry-run]]` |
  | `v1.settings.get {key?, explain?}` | `priya settings get [key] [--explain]` |
  | `v1.settings.set` | `priya settings set` |
  | `v1.settings.unset` | `priya settings unset` |
  | `v1.settings.schema` | `priya settings schema` |
  | `v1.events.subscribe {types?, stream_prefix?, from_seq?}`, notifications `v1.events.event` and `v1.events.ended` | `priya events tail [--type …] [--from N]` |
  | `v1.events.unsubscribe` | Ctrl+C in `priya events tail` |
  | `v1.audit.verify` | `priya audit verify` |

  `v1.audit.verify` and `v1.system.methods` are new: add them to 02 §4. 0.8 adds `v1.module.*`; 0.12 adds `v1.intent.*` and `v1.ai.ask`.
- [x] `priya-api::methods::ALL`: a static list of every method with its params and result schema. It feeds the parity test, `v1.system.methods` and the UI helpers in 0.9.

  As built (wire): `priya_api::rpc` has the JSON-RPC 2.0 types: `Request` (an absent `id` is a notification, a null one a call), `Id` (any number, a string or null), `Response` (exactly one of `result` and `error`, checked when read), and `RpcError`, whose `data` is always an `ApiError` (made up from the code when a peer sends none). It also has `code_for`/`kind_for`, `parse_line` (a single message or a batch of at most `MAX_BATCH` = 1024) and `parse_request` (refuses scalar params, and keeps the id when it can to answer errors to it). `priya_api::framing::LineSplitter` turns bytes into lines however they arrive. It enforces `MAX_LINE` (16 MiB, not counting a CRLF's `\r`) by reporting `TooLong` once and skipping to the next newline, and `finish()` gives the last line at end of input. `encode` refuses a message over the limit. `priya_api::methods` has a `Method` trait (`NAME`, `SUMMARY`, `READ_ONLY`, `NOTIFICATION`, `Params`, `Result`), one module per group with the params and result types, `ALL`, `NOTIFICATIONS` (with schemas), and `decode_params` (absent or null means `{}`; named params only). Events travel as `StoredEvent { seq, event, prev_hash, hash }`, converted from priya-store's `EventRecord`; setting layers as `SettingLayer` (with `built_in`), converted from priya-core's `Layer`.
- [x] `priya-api::client::Client`: async, NDJSON over the Unix socket. Used by the CLI, the tests and later the Rust modules.
- [x] `priya` CLI (`priya-cli`, clap derive):
  - `--json` on every read command (NDJSON for streams) and a `--socket` override;
  - human output with the plain `user_message` on errors;
  - exit codes: 0 ok, 1 error, 2 usage, 3 priyad unreachable. Exit 3 prints "Priya's background service isn't running. Start it with: systemctl --user start priyad";
  - respects `NO_COLOR`;
  - `priya completions bash|zsh|fish`.
- [x] `priya doctor` (CORE-05), built as a check registry in priyad: `trait Check { id, title, fn run(&Ctx) -> CheckResult { status: ok|info|warn|fail, summary, details, fix: Option<Fix> } }`. Phase 0 checks:
  - `daemon.version_matches_cli`
  - `socket.permissions`
  - `store.integrity`: `PRAGMA quick_check` plus the chain check
  - `settings.valid`
  - `disk.free_space`: at least 1 GiB in the data dir
  - `fs.root_type` (info)
  - `fs.btrfs_layout` (info, read-only): subvolume names from mountinfo, whether fstab uses `subvolid=`, and whether `/boot` is inside root. It says "Priya Sandbox needs btrfs with the PriyaOS layout — arrives in Phase 2".
  
  0.12–0.14 add the AI, desktop and install checks.

  As built (doctor): `priyad::doctor` has `trait Check { id, title, run(&Context) -> Finding }` and `trait Fix { scope, description, apply(&Context) }`, `checks()` in a fixed order, and `run(context, checks, mode)`. With `fix: apply` it makes user-scope fixes and runs the check again; system-scope fixes are only listed. The checks:
  - `daemon.version_matches_cli` compares `DoctorParams.cli_version` (sent by the CLI).
  - `socket.permissions` checks for a 0700 folder and a 0600 socket owned by you; its fix restores the modes.
  - `store.integrity` runs `PRAGMA quick_check` plus `verify_chain`.
  - `settings.valid` uses `SettingsService::check_files`.
  - `disk.free_space` uses `statvfs` (via `rustix`) and warns under 1 GiB.
  - `fs.root_type` and `fs.btrfs_layout` read `/proc/self/mountinfo` and `/etc/fstab` through `priya_core::mounts`, which parses them (octal escapes included) into a `Layout`: root filesystem type, mounted subvolumes, whether `/boot` is inside root, whether fstab pins `/` to a subvolume or uses `subvolid=`, and which PriyaOS subvolumes are missing. Off Linux they report that there's nothing to check.

  The mountinfo fixtures (ext4, archinstall's btrfs, PriyaOS) are hand-written; see their `RECORDED.md`.
- [x] Doctor fixes (`Fix { scope: user|system, description, apply }`). Phase 0 implements `--fix --dry-run` (list planned fixes) and applies user-scope fixes only after interactive confirmation. System-scope fixes arrive in 2.4. During development, fixes run only inside `TempPriyaHome` (CLAUDE.md safety rules).
- [x] `packaging/systemd/priyad.service` (user unit):
  - `Type=notify`, `ExecStart=/usr/bin/priyad`;
  - `Restart=on-failure`, `RestartSec=1`;
  - `NoNewPrivileges=yes`, `LockPersonality=yes`, `RestrictRealtime=yes`, `SystemCallArchitectures=native`;
  - `WantedBy=default.target`.
  
  The unit must pass `systemd-analyze --user verify`.
- [x] `priya-testkit::TestDaemon`: starts `priyad --foreground` inside a `TempPriyaHome`, waits for the socket, hands out a typed `Client`, captures logs and kills the daemon on drop.

  As built (client, CLI, unit, test daemon):
  - **Client.** `priya_api::client::Client` (feature `client`): `connect`, typed `call::<M>`, `call_raw` and `next_notification`, which queues notifications that arrive while a reply is awaited. `ClientError` is `Unreachable` (nothing listening), `Connect` (a socket that can't be used, named in the message), `Io`, `Protocol` or `Api`.
  - **CLI.** `priya status`, `doctor [--fix [--dry-run] [--yes]]`, `settings get [KEY] [--explain] | set | unset | schema`, `events tail [--type P]… [--stream PREFIX] [--from SEQ]`, `audit verify [--from] [--to]`, `api methods` and `completions SHELL`.
    - `--json` and `--socket` apply to every command; output has no colour.
    - Exit codes: 0, 1 (a refusal, with its plain message on stderr), 2 (usage) and 3 (daemon unreachable).
    - A setting with no value prints "not set".
    - `doctor --fix` asks before user-scope fixes. With `--json` or without a terminal it needs `--yes`.
    - `events tail` follows only its own subscription. It exits 1 with the daemon's message when the stream ends on its own (`v1.events.ended`).
    - On Ctrl+C, caught from the start, it unsubscribes, waiting at most 1 s, and exits 0. It also exits 0 once its output is closed (`| head`).
    - Declining the fix prompt still exits 1 if checks fail.
    - `parity.rs` maps every method to its command; `completions` is the one local command.
  - **Unit.** `packaging/systemd/priyad.service` is as specified. `the_unit_passes_systemd_analyze_verify` runs `systemd-analyze --user verify` on Linux, with `ExecStart` pointed at the built binary.
  - **TestDaemon.** `priya_testkit::TestDaemon::start(workspace_binary("priyad"))` runs the real binary with `--foreground` in a `TempPriyaHome`. Stand-ins for `/usr/share/priya/defaults` and `/etc/priya` come through `PRIYA_DEFAULTS_DIR` and `PRIYA_SYSTEM_DIR`, `NOTIFY_SOCKET` is removed, and logs go to a file kept across restarts.
    - It is ready when priyad says `READY=1` on a notify socket of its own, which comes after the settings are loaded and watched; an accepting socket alone isn't enough. `JOURNAL_STREAM` is removed too, so logs reach the file.
    - It offers `client()`, `command(program)` for the CLI or a second daemon, `stop()` (SIGTERM, then wait) and `restart()` (same home).
    - priyad's tests use `CARGO_BIN_EXE_priyad`. `workspace_binary("priya")` finds the CLI next to the running test, so it must be built first. CI's `--workspace` runs build it, and so does `cargo xtask flaky hunt`.
  - **Logging.** journald is used only when `JOURNAL_STREAM` names the actual stderr. A shell in a terminal started by systemd inherits the variable, but `priya -v` still logs to the terminal.
    - The setup lives in its own crate, `priya-logging` (`priya_logging::init`), used by both binaries, so the CLI no longer depends on `priyad` and doesn't build the store, SQLite or `notify` (ADR-023).
    - The end-to-end tests live in `crates/priyad/tests/cli.rs`, which is Linux-scoped on CI. `cli_exit_code_3_when_daemon_missing` lives in `crates/priya-cli/tests/exit_codes.rs`, which runs on macOS too.

**Tests**
- Unit:
  - `router_maps_errors_to_jsonrpc_codes_with_user_message` (`CORE-04`)
  - `router_rejects_unknown_method_with_32601` (`CORE-04`)
  - `batch_requests_answer_every_id` (`CORE-04`)
  - `ndjson_framing_handles_partial_reads_and_16mib_limit` (`CORE-04`)
  - `doctor_results_serialize_stably` (golden, `CORE-05`)
  - `doctor_btrfs_layout_from_mountinfo_fixtures`: fixtures for ext4, plain btrfs and the PriyaOS layout (`CORE-05`)
  - `cli_exit_code_3_when_daemon_missing` (`CORE-04`)
- Contract:
  - `cli_parity_covers_every_api_method`: every entry in `methods::ALL` maps to a CLI command in `priya-cli/src/parity.rs`, and every CLI command maps back (`CORE-04`).
  - `cli_json_output_matches_result_schema`: one case per read command (`CORE-04`).
- Integration (`TestDaemon`):
  - `settings_set_via_cli_emits_event_and_persists_across_restart` (`CORE-03`)
  - `settings_file_edit_visible_via_api_within_1s` (`CORE-03`)
  - `events_tail_streams_and_resumes_from_seq` (`CORE-01`)
  - `audit_verify_passes_then_detects_tampering`: stop the daemon, edit a row, restart, verify (`CORE-01`)
  - `second_instance_refuses_to_start` (`CORE-04`)
  - `foreign_uid_connection_rejected`: Linux CI container with a second user (`CORE-04`). As built: CI runs as root, so the test opens the temporary socket's folders (0711) and the socket (0666), runs `priya --json status` as `nobody` (uid 65534, through `Command::uid`), and checks that the CLI lost the connection unanswered, that priyad logged the refusal and that its own user is still served. Run as anyone but root it passes without checking anything (its note shows with `--no-capture`); under `CI` it fails instead.
  - `sigterm_shuts_down_cleanly_and_chain_verifies` (`CORE-01`)
  - `doctor_fix_dry_run_changes_nothing` (`CORE-05`)
- Regression: all.

**Done when**
- [x] On the Zenbook, after `systemctl --user start priyad`, these all work: `priya status`, `priya settings set ui.theme light`, `priya events tail`, `priya audit verify` and `priya doctor`. Events survive `systemctl --user restart priyad`.
- [x] The parity test is green.
- [x] Idle `priyad` RSS stays under 80 MiB (09 budget), measured on the Zenbook.

  Gate run on the Zenbook (owner, 2026-10-20, Linux 7.2):
  - The regression suite passed: 60 tests.
  - The user unit passed `systemd-analyze --user verify` and started.
  - `status`, `settings set` and `--explain`, `events tail`, `audit verify` and `doctor` all worked; doctor showed 5 passes and 2 notes (ext4, no Priya Sandbox layout yet).
  - After a restart the history held all 4 events and verified.
  - Idle RSS after 30 s was 10 MiB.

---

### 0.8 — Module host and SDK
**Outcome:** priyad discovers, starts, supervises and restarts modules from `module.toml` manifests. A Rust example module and a Python example module answer calls and emit events in the same way. The Rust one also runs as a built-in with identical behaviour.
**Features:** CORE-02.
**Depends on:** 0.7.

**Tasks**
- [x] `priya-api::module::ModuleManifest` follows [02 §5.1](../architecture/02-core.md): `[module]`, `[provides]`, `[subscribes]`, `[requires]`. Validation rules:
  - `api = "v1"`;
  - name matches `[a-z][a-z0-9-]{1,31}`;
  - methods are prefixed with the module name;
  - capability strings come from a known list: `events.emit`, `settings.read:<prefix>`, `ai.role:<role>`, `network:<label>` and `api.read`;
  - `helper.*` capabilities are rejected until Phase 2;
  - unknown keys are rejected.

  As built (manifest and protocol types):
  - **Manifest.** `ModuleManifest::parse(text)` reads the TOML with every table closed (`deny_unknown_fields`); a TOML or structure mistake comes back as one sentence with its line. Everything else is then reported at once, one sentence each (`ManifestError { problems }`), bad capabilities included. Deserializing a `ModuleManifest` from any format runs the same checks (`try_from`), so an unchecked one can't exist. Beyond the rules above it refuses: the names `host`, `priya`, `system` and `v<N>`; a version that isn't `MAJOR.MINOR.PATCH[-pre]`; a process module without `exec` or a builtin with one; an `exec` with spaces around it or a relative one that leaves the module's folder; method names that aren't `<module>.<word>[.<word>…]` (`[a-z][a-z0-9_]*`); a method, capability, tool or surface id listed twice; a `settings_schema` path that leaves the folder (`./` is fine); and `default_mode = "direct_raw"`, which is never a default. `Capability` is a typed enum that parses and prints the capability strings. The host's checks use `grants(capability)` for the exact ones and `may_read_setting(key)` for settings: a module's own `modules.<name>` section, plus whatever its `settings.read:<prefix>` covers, matching whole key parts (`ui` covers `ui.theme`, not `uix`). The manifest has a JSON Schema (for 0.9).
  - **Protocol.** `priya_api::module::protocol` has the v1 messages as `Method` types: `initialize {api, host_version}` → `{api, name, version, methods}`, `ping`, `shutdown` and `event {event}`, sent one at a time so a slow module slows delivery down (host to module); `host.emit_event {custom_type, payload}` → `{seq}`, `host.settings.get {key}` → `{value}`, `host.call {method, params}` (module to host). They are listed in `MODULE_METHODS` and `HOST_METHODS`. Host-to-module params accept unknown fields, so a newer priyad can add some without breaking older modules; module-to-host params refuse them. `is_custom_type` says which event names a module may use.
  - **Framing.** `priya_api::framing::lsp`: `encode`/`encode_body` and an incremental `Decoder` that takes bytes however they arrive and returns bodies. Many small messages in one read cost one copy, and each body holds only its own bytes. Bodies are at most 16 MiB, refused from the header before the body is read. A header block may be at most 4 KiB, whatever the read boundaries. Header lines must end in CRLF, and a bare LF gets its own message. `Content-Length` must appear once, in any case; other headers are ignored. A broken stream stays broken, and `finish()` reports a stream cut off mid-message.

- [x] Discovery: `/usr/lib/priya/modules/*/module.toml` and `$XDG_DATA_HOME/priya/modules/*/module.toml` (user and dev). Which modules run is decided by `modules.enabled`.

  As built: priyad looks in `/usr/lib/priya/modules` (or `PRIYA_SYSTEM_MODULES_DIR`, for tests) and `$XDG_DATA_HOME/priya/modules`, at start, on every `v1.module.list`, and whenever `modules.enabled` changes (the settings watch calls `ModuleHost::set_enabled`). `priyad::modules::discovery::discover(dirs)` reads each `<dir>/<name>/module.toml`. A module folder in a later folder (the user's) replaces the installed one of the same name, even when it is broken, so a dev copy that doesn't load never quietly falls back to the installed one; a missing folder simply has none. A module is usable only if its manifest is valid, its folder is named after it, its `exec` (made absolute against its folder) is an executable file, and its settings schema, if any, is JSON that compiles as a JSON Schema. Anything else, including a `module.toml` that is a folder or a dangling link, is reported as a `Problem` with its path, and the other modules still load.
- [x] Stdio transport: JSON-RPC with `Content-Length` framing (`priya_api::framing::lsp`), messages capped at 16 MiB. The module's stderr goes to `tracing` under the module's name.

  As built: `priya_api::peer::Peer` (feature `peer`) is one end of a both-ways JSON-RPC conversation over two channels of whole messages, so stdio and a builtin's in-process channels share every line above the transport. `Peer::spawn(inbound, outbound, |peer| handler)` hands the handler its own peer, so it can call the other side. Requests from the other side are handled at most 64 at a time; one more is answered `unavailable` (retryable) at once, so reading never stops and a handler may wait on its own call to the other side. Answers sent without handling (busy, malformed) are bounded too: a side that keeps sending without reading them breaks the conversation. Notifications are handled one at a time, in order (requests run side by side, so a request may overtake an earlier notification); more than 1024 waiting breaks the conversation, which is why the host delivers events as calls. A handler that panics, even before returning its future, answers with an internal error. `call::<M>`/`call_raw` wait for answers (a dropped call is forgotten and its late answer ignored); `notify` sends without waiting; `close()` ends our side, and from that moment calls fail instead of being sent. A conversation ends `Closed` (the other side stopped sending, or we closed: requests in flight are still answered and queued notifications handled, for up to 5 s) or `Broken(reason)`: not JSON, a batch, an unreadable answer, an answer to an id we never used, a null-id error (it couldn't read us), a message over 16 MiB, a flood, or a transport break; work in flight is then dropped. Either way, calls waiting and calls made afterwards fail with `Closed` at once, even if the peer's task is aborted, and the outgoing channel is closed after the last answer, so a writer draining it finishes. A malformed request with an id gets `-32600`, one without is dropped. `framing::lsp::read_messages`/`write_messages` connect a peer's channels to a byte stream: a framing or read failure arrives as the last inbound item, with its reason, and `write_messages` returns why it stopped. A process module's stderr is logged line by line under target `module` with its name (any bytes; long lines in pieces), and its last 20 lines are kept for `module.crashed`.
- [x] Lifecycle:
  - **Spawn.** On Linux the module runs inside `systemd-run --user --scope --unit=priya-module-<name>` when a user manager is present; otherwise it is a plain child process.
  - **Handshake.** `initialize {api, host_version}` must return the manifest's name, version and methods within 5 s.
  - **Health.** `ping` every 30 s with a 5 s timeout.
  - **Stop.** `shutdown`, then SIGTERM, then SIGKILL after 3 s.
  
  Seccomp and Landlock profiles derived from the manifest arrive in 3.5.

  As built: `priyad::modules::launch` starts a process module in its folder, in a process group of its own, with `PRIYA_MODULE_NAME` set: through `systemd-run --user --scope --collect --quiet --unit=priya-module-<name>-<priyad pid>-<n>` when `$XDG_RUNTIME_DIR/systemd/private` exists on Linux (`Spawn::detect`), else directly. The unit name gets the pid and a counter because another priyad (a test's) may run a module of the same name and an old scope may still be going away. A task owns the child: it signals the module and its whole group, but only until it has reaped it, so a signal never reaches a reused pid; once the module has ended, anything still in its group is killed, so what a module started never outlives it. Dropping the handle kills the module. A program that is missing, can't be run or has a bad `#!` is given up on at once; other spawn failures (too many open files…) count as ordinary failures. `priyad::modules::session::Session::start` runs the handshake: `initialize` within 5 s, and an answer with API `v1` and the manifest's name, version and methods (in any order). A wrong answer is refused for good (`module.crashed {gave_up: true}`, saying what differs); no answer in time, or exiting first, is an ordinary failure. `watch` pings every 30 s (5 s to answer; a ping in flight never hides a stop or an exit) and notices an exit, a conversation that broke (garbage; reported ahead of the exit it causes), output that closed, or a module that stopped reading its input. `stop` asks `shutdown` and gives it 3 s to exit, then SIGTERM, 3 s more, then SIGKILL; a module that failed is killed at once. Events in `[subscribes] events` are delivered as `event` calls one at a time, each allowed the call time limit. `PRIYA_MODULE_TIMINGS` (`name=ms,…`, see `modules::parse_timings`) shortens every time here, for tests. `module.crashed {signal: 9}` after a ping or handshake timeout is priyad's own SIGKILL.
- [x] Supervisor:
  - exponential backoff from 1 s, capped at 30 s;
  - more than 5 crashes within 60 s stops the module and emits `module.crashed {gave_up: true}`;
  - events: `module.started`, `module.stopped`, `module.crashed {exit_status, stderr_tail}`;
  - the `modules` projection is kept up to date.

  As built: `priyad::modules::supervisor` runs one task per enabled module: start it, record `module.started {pid}`, watch it, and on failure record `module.crashed {exit_code, signal, restarts, error, stderr_tail, gave_up}` and pause before the next start (or stop for good). Each restart reads the module from disk again, so an upgraded module starts as its new self; one that is broken just then (a manifest half written) counts as an ordinary failure, and only one whose folder is gone is given up on at once. Stopping records `module.stopped {reason}` (`disabled`, `priyad is stopping`; also when the host is dropped without being stopped), but only for a copy that ran, so every `module.stopped` follows a `module.started`. A program that can't be started at all, or a builtin priyad doesn't have, is given up on at once. A supervisor that ended without stopping (it panicked) shows as failed, and enabling the module again starts it afresh. `priyad::modules::policy::Failures` answers `RestartAfter(pause)` or `GiveUp` for each failure. The pause doubles with each failure in a row, 1 s to 30 s, and a run that lasted 60 s starts it over; more than 5 failures inside 60 s gives up for good (enabling the module again starts afresh). `module.crashed` (still v1, never written before 0.8) now carries `stderr_tail` (at most 20 lines of 300 characters) and `gave_up`, and the `modules` projection (v2, rebuilt on open) shows a module the host gave up on as `failed`. `priyad::modules::access::check` decides every host call: `events.emit` for `host.emit_event`; the module's own section or a `settings.read` prefix (`ModuleManifest::may_read_setting`) for `host.settings.get`; and for `host.call`, `api.read` plus a method on an explicit list (`v1.system.status`, `v1.system.methods`, `v1.settings.schema`, and `v1.settings.get` for one key the module may read), so a module can't use the public API to get around its other limits. A refusal is `permission_denied` with a plain message naming the module; its detail names the capability that would allow it.
- [x] Host API, from module to priyad. Every call is checked against the manifest; a denial returns `permission_denied` and is logged with the module and method.
  - `host.emit_event {custom_type, payload}` becomes `module.event` with actor `module:<name>`. Needs `events.emit`.
  - `host.settings.get {key}`: a module can always read its own `modules.<name>` section; other keys need `settings.read:<prefix>`.
  - `host.call {method, params}`: only read-only public methods, and only with `api.read`.
  
  `host.ai.ask` lands in 0.12. `host.task.create` and `host.secret.get` land in Phase 3.

  As built: `priyad::modules::host_api::HostApi` answers a module's `host.*` calls after `access::check` (see Supervisor above), and logs each refusal with the module and method. `host.emit_event` refuses a `custom_type` that isn't lowercase dotted words, and records `module.event {module, name, data}` on stream `module:<name>` with actor `module:<name>`; its answer is `{seq}`. Other methods get `-32601`; notifications from a module are ignored. The host reaches the rest of priyad only through the `HostServices` port (record an event, read a setting, call a public method, subscribe to events), which the daemon fills in.
- [x] Built-in kind: the `BuiltinModule` adapter carries the same JSON messages over in-process channels. Everything above the transport is shared with process modules.

  As built: a builtin module (`kind = "builtin"`, no `exec`) is looked up by name in priyad's `Builtins` table of `BuiltinMain` entries (`priya_module_sdk::builtin::<M>`), run as a task, and given the same two channels of whole messages a process gets from its stdio; from there the peer, session, supervisor and host API are the same code. It has no pid; "stopping" it aborts its task, and a panic counts as a failure. `priyad::modules::ModuleHost` ties it together: `set_enabled` (look again, start the enabled modules, stop the rest and return once they have stopped; a copy still stopping is waited for before a new one starts), `retry` (start afresh one the host gave up on), `exists`, `call` (to the running module that lists the method, with a 30 s limit), `list` (found modules with their state, a running copy winning over what is on disk now, a copy still stopping shown as `stopping`, plus broken and missing ones; a modules folder that can't be read is logged, not listed; it looks for modules before taking the host's lock) and `running`; `stop` stops every module, waits for those already stopping, and from then on starts nothing. `crates/priyad/tests/module_host.rs` drives it with real builtin and process modules against fake services.
- [x] API and CLI: `v1.module.list|enable|disable|call {method, params}` map to `priya module list|enable|disable|call <method> [json]`.

  As built: `priya_api::methods::module` has the four methods. `v1.module.list` reports every module found, plus broken and missing ones: name, version, kind, description, methods, enabled, state (`starting`, `running`, `restarting`, `stopping`, `stopped`, `failed`, `disabled`, `broken`, `missing`), pid, restarts and the last problem as a sentence. `enable` refuses an unknown name (and says why a broken one can't be used), adds it to `modules.enabled` in the user's settings file, starts it at once, and starts afresh one the host had given up on before the call; `disable` refuses an unknown name, removes it and returns once it has stopped. Both, and the task following `modules.enabled`, hold one lock (`Daemon::module_changes`) while they read, change and apply the list, so concurrent changes aren't lost and an older list never overtakes a newer one. `call` goes to `ModuleHost::call`. `priyad::module_api::DaemonServices` fills the host's `HostServices` port with the event store, the settings in force, the router (`Router::call`, no connection needed) and store subscriptions with full payloads; it holds the daemon weakly, and `Daemon::new` builds both with `Arc::new_cyclic`. priyad starts the enabled modules before saying `READY=1`, follows `modules.enabled`, and stops every module (`module.stopped {reason: "priyad is stopping"}`) before closing the store. `v1.system.status` lists the running modules. The CLI prints `name  state  version`, restarts and the problem; `call` prints the answer as JSON. Feature `builtin-examples` builds echo into priyad as a builtin. `TestDaemon::start_with(priyad, env)` passes extra environment (such as `PRIYA_MODULE_TIMINGS`) and points `PRIYA_SYSTEM_MODULES_DIR` at a folder of its own; `modules_dir()` is the user's modules folder.
- [x] `priya-module-sdk` (Rust): `Module` trait (`initialize`, `handle(method, params)`), `serve_stdio()` and a typed `HostClient`, with rustdoc examples.

  As built: `Module` has `initialize(host, params)` (returns `InitializeResult::v1(name, version, methods)`), `handle(host, method, params)`, and optional `event` and `shutdown` hooks; methods are `async fn`s, and the trait docs say how calls run side by side (so state must be `Sync`). The SDK answers `ping` itself and refuses the module's methods and events until `initialize` has succeeded; a second `initialize` is a `conflict`. `event` calls reach the hook one at a time. `shutdown` stops taking requests, answers, and once the requests in flight are answered runs the `shutdown` hook (priyad is out of reach by then); the hook also runs when priyad goes away. Notifications aren't part of the protocol for modules: one is ignored and noted on stderr. `HostClient::{emit_event, setting, call}` return plain `ApiError`s, host refusals keeping their kind. `serve(module, inbound, outbound)` runs a module on channels; `builtin::<M>` makes the `BuiltinMain` entry priyad keeps for a builtin; `serve_stdio` (stdin read on a thread of its own, since tokio's can't be stopped) and `run_stdio` (for `main`: exit 0 when told to stop or when priyad goes away, 1 with the reason on stderr when the conversation breaks) run it as a process, writing every answer out before returning. `parse_params::<P>` and `no_such_method` give the usual errors. `testing::connect(module, host)` runs a module against a stand-in for priyad, for module authors' tests. The crate docs carry a complete example (compiled by `cargo test --doc`).
- [ ] Example modules. They are not packaged; they serve tests and as SDK documentation.
  - `modules/examples/echo` (Rust): `echo.say` and `echo.emit`, plus a settings section `modules.echo.prefix` with its own schema. It builds as a process module and, with feature `builtin-examples`, as a built-in.
  - `modules/examples/echo-py` (Python 3 standard library only): the same methods and its own framing code.

  As built (so far): `modules/examples/echo` (crate `priya-echo-example`, binary `echo-module`, `module.toml` and `settings.schema.json`) has `echo.say {text}` → `{text}` with `modules.echo.prefix` in front (read on every call), `echo.emit {text}` → records `said {text}` → `{seq}`, and a third method, `echo.setting {key}` → `{value}`, which shows what a module may read (and serves the denied-call and own-settings checks). It asks only for `events.emit`. `BUILTIN` is its builtin entry. `modules/examples/echo/tests/echo.rs` drives the real binary over stdio and the builtin over channels against a fake host, and checks that `module.toml` matches what echo says it is.
- [ ] `priya-testkit::FakeModuleProcess`: a scripted misbehaving module that can crash on the Nth call, hang, write garbage, send an oversize message or lie in its handshake.

**Tests**
- Unit (all `covers: CORE-02`):
  - `manifest_rejects_bad_name_unknown_capability_and_helper_caps`
  - `lsp_framing_round_trip_and_oversize_rejected`
  - `backoff_schedule_and_give_up_after_five_in_sixty_seconds` (`FakeClock`)
  - `capability_check_denies_undeclared_host_call`
- Contract:
  - `module_host_contract`: one macro, three instantiations (`builtin_rust_echo`, `process_rust_echo`, `process_python_echo`). It covers the handshake, call/result, error mapping, `host.emit_event` producing `module.event` with the right actor, reading the module's own settings, a denied call and shutdown. All three must produce identical golden event sequences (`CORE-02`).
- Integration (`TestDaemon`):
  - `crashed_module_restarts_and_emits_events`
  - `hung_module_killed_after_ping_timeout`
  - `garbage_output_fails_module_not_daemon`
  - `handshake_mismatch_refused`
  - `disable_stops_module_and_persists`
  - `module_settings_validated_against_module_schema` (also `CORE-03`)
- Security (fast subset):
  - `module_cannot_call_undeclared_host_method`
  - `module_cannot_read_other_modules_settings`
- Regression: all.

**Done when**
- [ ] The Rust and Python echo modules both:
  - answer `priya module call echo.say '{"text":"hi"}'`;
  - emit an event that shows up in `priya events tail`;
  - restart after `kill -9`;
  - stop after 6 rapid crashes, with `gave_up`.
- [ ] The contract suite gives identical results for all three instantiations.
- [ ] A module crash never stops priyad or another module.

---

### 0.9 — Schema compatibility checks
**Outcome:** Schemas for the public API, events, settings and module manifests are committed under `docs/claude/api/`. CI blocks a breaking change unless the version is bumped and a migration or upcaster comes with it.
**Features:** QA-08.
**Depends on:** 0.5, 0.6, 0.7, 0.8.

**Tasks**
- [x] `cargo xtask schemas` writes JSON Schema (draft 2020-12, sorted keys, stable `$id`):
  - `docs/claude/api/public/v1/<method>.json`: params and result;
  - `docs/claude/api/public/v1/errors.json`;
  - `docs/claude/api/events/<type>.v<N>.json`;
  - `docs/claude/api/settings/core.json`, plus `settings/modules/<name>.json` for first-party modules;
  - `docs/claude/api/module/manifest.v1.json`;
  - `docs/claude/api/module/host-v1/<method>.json`;
  - a generated `docs/claude/api/INDEX.md`.
  
  The helper protocol (Phase 2) and MCP tool definitions (Phase 4) use the same tool when they exist.

  As built: `xtask/src/schemas.rs`. A method file holds params and result under `$defs/params` and `$defs/result`, with the types they share defined once, plus `x-kind`, `x-direction` (`client_to_daemon`, `module_to_host` or `host_to_module`), `x-read-only` and `x-notification`. Notifications get their own files (`public/v1/v1.events.event.json`, `$defs/params`, `x-direction: daemon_to_client`). `errors.json` is the error `data` with `x-codes` (the JSON-RPC code of each kind). Event files carry `x-event-type` and `x-event-version`; a committed file of an older version that is no longer registered (its struct is gone) is kept as it is. `settings/core.json` carries `x-settings-version`, `manifest.v1.json` `x-api-version`. `$id` is the file's URL on `main`. First-party modules are the folders under `modules/` other than `examples/`; none has settings yet, and a module's own `$schema` is relabelled 2020-12. Hidden files under `docs/claude/api/` are ignored; any other file nothing generates is removed by `cargo xtask schemas` and reported by `--check`. `cargo xtask ci-local` runs `schemas --check` too.
- [ ] `cargo xtask schemas --check` regenerates into a temporary dir, diffs against the committed files and classifies every change with `xtask/src/schema_diff.rs`.
  - **Additive:** a new optional property; a new method; a new event type or version; a new enum variant in output-only positions; a loosened input constraint.
  - **Breaking:** a removed or renamed property; a type change; a new required property; a removed enum variant; a tightened input constraint; a removed method.

  Rules:
  - An additive change with an updated snapshot passes.
  - Any change without an updated snapshot fails with "run `cargo xtask schemas`".
  - A breaking change fails unless:
    - **(a) public API:** the method appears under `v2.` and `v1.` is kept;
    - **(b) events:** a new `version` file sits next to the old one and an upcaster from the old version is registered;
    - **(c) settings:** `settings_version` is bumped and a migration is registered with an old-config fixture under `crates/priya-core/tests/fixtures/settings/v<N>/`;
    - **(d) manifest:** the manifest `api` version is bumped.
- [ ] Every event type with version above 1 needs fixtures of each older version, and each must upcast to a payload valid against the current schema (checked by `--check`).
- [x] Generated UI helpers in `ui/PriyaUI/api/Api.js`: method-name constants, notification kinds and result field names, taken from the public schemas so the UI and the backend cannot drift ([11 §8](../architecture/11-design-system-and-ux.md)). Regenerated and checked together with the schemas.
  As built: a `.pragma library` file with frozen `methods` and `notifications` (camelCase names, `v1.` dropped), `fields` (each result's and notification's top-level property names) and `errorKinds`. Nothing imports it yet; the first screen that talks to priyad (0.13) does.
- [ ] Add the `schemas` job to `ci-pass`. The PR report lists each schema change with its classification.
  As built so far: the `schemas` job runs `cargo xtask schemas --check` and is in `ci-pass`; the classification and the report section come with `--check`'s rules.
- [x] Docs: link `docs/claude/api/` from `docs/claude/README.md`.

**Tests**
- Unit (all `covers: QA-08`):
  - `schema_diff_classifies_<case>`: table-driven, at least 20 cases, one per additive and breaking rule.
  - `schema_export_is_deterministic`: two runs give byte-identical output.
  - `upcast_chain_produces_valid_current_payload`: synthetic v1 to v2.
  - `old_settings_fixture_migrates_to_current`: synthetic v0 to v1.
- Integration, against a fixture copy of the workspace:
  - `check_fails_on_removed_settings_key_without_migration`
  - `check_passes_with_migration_and_fixture`
  - `check_fails_on_event_field_rename_without_new_version`
  - `check_passes_with_new_event_version_and_upcaster`
  - `check_fails_on_new_required_param_in_v1_method`
  - `check_fails_when_snapshot_not_updated`
- Regression: all.

**Done when**
- [ ] `docs/claude/api/` contains every Phase 0 method, event, settings section and the manifest.
- [ ] A demo PR that renames a field in `setting.changed` fails `schemas`. Adding `setting.changed` v2 with an upcaster makes it pass. The demo PR is then closed.
- [ ] `Api.js` regenerates with no diff in CI.

---

### 0.10 — Design tokens and theme generator
**Outcome:** One file, `ui/design/tokens.json`, generates the QML `Theme` singleton and a CSS file for mockups. Every declared text/background pair passes WCAG 2.2 AA in light and in dark, checked in CI, and the owner has approved the palette.
**Features:** DS-01, DS-02, DS-06.
**Depends on:** 0.4.

**Tasks**
- [x] `ui/design/tokens.schema.json` and `ui/design/tokens.json` contain:
  - `version`;
  - `themes`: `light` and `dark` are required. `oled`, `hc-light` and `hc-dark` are optional and not checked until 5.11;
  - `color.<token>` → `{light, dark, oled?, hc-light?, hc-dark?}` for every token in [11 §2.1](../architecture/11-design-system-and-ux.md), plus the new `border.strong`;
  - `accent.<name>` → `{accent, accent.subtle, on.accent}` per theme. Phase 0 ships Marigold only; the other accents come in 5.11;
  - `mode.{look,sandbox,direct,direct_raw}` → references to the semantic colours;
  - `font`: `ui` Inter, `mono` JetBrains Mono, `indic` Noto Sans Devanagari;
  - `type.{caption,small,body,bodyLarge,title,headline,display}` → `{size, weight, lineHeight}`;
  - `space` [2, 4, 8, 12, 16, 24, 32, 48];
  - `radius` (`control` 6, `card` 10, `panel` 16, `pill` full);
  - `motion` (`micro` 120, `standard` 200 and `panel` 320 ms; `easing.standard` and `easing.emphasised` as cubic-bezier);
  - `elevation` per theme (shadows in light, borders in dark);
  - `focus` (`width` 2, `color` → accent);
  - `contrast`: the declared pairs `{fg, bg[], kind: text | large-text | non-text}`.
- [x] Fix the starting values in 11 §2.1 that fail the check (measured during planning):
  - light `accent` `#B86E00` on `bg.base`: about 3.8:1, so it fails as body text;
  - white `on.accent` on that accent: about 4.0:1, so it fails for button labels;
  - `text.tertiary`: about 3.3:1 in light and 4.0:1 in dark;
  - `border.subtle`: about 1.3:1 in light and 1.5:1 in dark, too low to be an input's only boundary under WCAG 1.4.11.
  
  Add `border.strong` (≥ 3:1) for inputs and other focusable controls. Write the final values back into 11 §2.1.
- [x] `priya-themegen` crate at `ui/design/generate/`, a library also used by `xtask` (Phase 5 reuses it for `priya theme apply`). It validates the tokens against the schema, resolves references, rejects cycles and emits:
  - (a) `ui/PriyaUI/Theme.qml`:
    - `pragma Singleton` with `property string mode` (`"light"` | `"dark"`);
    - one palette object per theme, with camelCase names (`bgSurface1`, `textPrimary`, `accentSubtle`);
    - type tokens as `font` values with the fallback chain Inter → Noto Sans Devanagari;
    - spacing, radius and motion constants;
    - `reducedMotion`, `animationsEnabled` and `scale` properties;
    - colour transitions of `motion.standard` on mode change.
  - (b) `ui/design/build/tokens.css`: CSS custom properties per `[data-theme]`, for HTML mockups.
  - (c) `target/design/contrast.md`: the contrast report.
  
  Generated files start with a "generated from ui/design/tokens.json — do not edit" header and are committed.
- [x] `cargo xtask tokens [--check]`: regenerates the outputs; `--check` fails on any diff. Add job `tokens` to `ci-pass`.

  As built (generators):
  - **Entry point.** `priya_themegen::generate(&tokens)` returns `Outputs { theme_qml, tokens_css, contrast }`. A contrast failure is in the report rather than an error, so the report can show it.
  - **Theme.qml.**
    - A `pragma Singleton` `QtObject` with `mode` (an unknown mode shows dark), `reducedMotion`, `animationsEnabled` and `scale`.
    - An inline `component Palette` declares every palette name. Each theme is a typed `Palette` instance; a theme's unused elevation colours are transparent, with sizes 0.
    - `palette` picks one with a conditional chain, so lookups stay typed. qmllint is left with one warning: it doesn't see the qmldir singleton.
    - Colours are written as `#RRGGBB`, or `#AARRGGBB` when translucent, because QML reads alpha first.
    - Colours in use are `property color X: palette.X`, each with a `Behavior` that eases over `motionStandard` when animations are on.
    - Names are camelCase. `on.*` becomes `textOn*`, because QML reads `onX` as a signal handler.
    - Type styles are `font` values (`family: fontUi`, pixel size × `scale`, weight) plus `<style>LineHeight`. QML fonts take a single family, so Devanagari falls back through fontconfig, as 11 §2.2 says.
    - `space<n>`, `radius*` (a pill is 9999, which Qt clamps), `focusWidth`, and `motion*` (0 when movement is off).
    - `easing*` is a list for `Easing.BezierSpline`.
    - `ui/PriyaUI/qmldir` declares the singleton.
  - **tokens.css.** Colours per `[data-theme]`, light first and also `:root`. Each `--elevation-<level>-shadow` lists its layers, or `none`. Type, space, radius, motion, easing and focus are under `:root`.
  - **Contrast report.** `target/design/contrast.md`.
  - **Command.** `cargo xtask tokens` writes the files. `--check` fails if one differs. Either way it fails on any contrast failure, writing the report first.
  - **Where it runs.** `ci-local` runs the check. CI's `tokens` job (in `ci-pass`) runs it and keeps the report as an artifact; since 0.11 the `ui` job runs `tst_theme.qml`.
  - **Goldens.** The committed files are the goldens: `theme_qml_generation_golden` and `tokens_css_generation_golden` compare against them and check their structure.
  - **QML test.** `tst_theme.qml` checks real values:
    - modes, including OLED inheritance and the fallback;
    - shadow alpha;
    - easing and radii;
    - scale;
    - reduced motion, and a colour easing between modes.

    Its mode test was tagged `acceptance: 0.10` and `regression: 0.10` in 0.11, once `cargo xtask test` could run QML tests.
  - **Known gaps.** Turning `reducedMotion` on doesn't cut short a 200 ms change already running. Spacing isn't rounded at fractional scales; font sizes are.
- [x] Contrast checker `priya_themegen::contrast`: WCAG 2.2 relative luminance. Translucent colours are composited over their declared background first. Thresholds, applied to every declared pair in every required theme:
  - `text`: ≥ 4.5:1;
  - `large-text` (≥ 24 px, or ≥ 18.66 px at weight ≥ 600): ≥ 3:1;
  - `non-text` (input borders, focus ring against the adjacent surface, icons that carry meaning): ≥ 3:1;
  - `hc-*` themes: ≥ 7:1 once present.
  
  It also flags any text token that appears in no declared pair.

  As built (tokens and checker):
  - **Themes.** Each has a `name`, an optional `inherits` (OLED inherits from dark, so it lists only what differs), `checked` and `contrast` (`normal`, or `high` for WCAG AAA). Light and dark are always checked; optional themes only with `checked: true`, from 5.11.
  - **Colours.**
    - Values are `#RRGGBB`, `#RRGGBBAA` or a reference: `{color.<token>}` or `{accent.<option>.<role>}`.
    - The palette's `accent`, `accent.subtle` and `on.accent` come from `accent.default`, and are referable as `{color.accent}` and so on.
    - Every accent option is resolved in every theme, so a broken one is caught before it is offered.
    - `focus`, `mode.*` and the accent roles are reserved names.
  - **Elevation.** `elevation.card` follows the approved sheet: two shadow layers in light, a `border.subtle` border in dark. Further levels come with the screens that need them.
  - **Loading.** `priya_themegen::load` checks the file against `tokens.schema.json` (compiled in, built once) and then reads it into typed `model` structs, so every rule lives in the schema. Problems are listed with their place (`at /radius/card: …`). This needs `jsonschema`, which became possible once the owner allowed MIT-0 on 2026-10-20 (ADR-022).
  - **Palette.** `Palette::resolve` gives every theme the same palette names (`names()`) plus typed elevation (`Raised`). It refuses with a plain message:
    - a missing light or dark;
    - unknown parents, or inheritance circles;
    - a value for a theme that doesn't exist;
    - unknown references, or reference circles (naming just the circle);
    - an unknown default accent.
  - **Checker.**
    - `contrast::check` needs an opaque `bg.base`.
    - A translucent background is composited over `bg.base`, and a translucent foreground over its background.
    - Thresholds: text 4.5, large text 3, non-text 3. High contrast raises them to 7, 4.5 and 3.
    - `text.*` and `on.*` colours must appear in a pair of kind text or large text.
    - `Report::to_markdown` gives the committed report; `problems()` gives the CI message.
  - **Doc sync.** A test checks 11 §2.1's table against the palette.
- Moved to 0.11 (2026-10-20), which builds the first QML that could break it: the rule that PriyaUI and shell code use only `Theme.*`.
- Moved to 0.11 (2026-10-20), where the first components use icons (the owner allowed the download): the vendored Lucide subset.
- [x] `ui/mockups/0.10/palette.html`, built from `tokens.css`. It shows every colour token, the type scale (Latin and Devanagari samples), spacing, radius and mode colours, in light and dark, with the measured contrast next to each pair.

**UI**
- Palette sheet in light and dark. If `oled` or high-contrast values are filled in, they are shown labelled "preview — selectable in Phase 5".
- 👤 Mockup approval: the owner approves the palette sheet before 0.11 starts.

**Tests**
- Unit:
  - `tokens_json_validates_against_schema` (`DS-01`)
  - `every_color_has_light_and_dark` (`DS-02`)
  - `references_resolve_and_cycles_rejected` (`DS-01`)
  - `wcag_ratio_matches_reference_values`: #000/#FFF = 21:1 and #777/#FFF ≈ 4.48:1 (`DS-06`)
  - `contrast_check_fails_below_threshold_per_kind` (`DS-06`)
  - `alpha_colors_composited_before_measuring` (`DS-06`)
  - `undeclared_text_token_flagged` (`DS-06`)
  - `theme_qml_generation_golden` (insta, `DS-01`)
  - `tokens_css_generation_golden` (`DS-01`)
- UI:
  - `tst_theme_singleton_switches_mode`: Qt Quick Test, offscreen. Setting `Theme.mode = "light"` changes `bgBase` and `textPrimary` (`DS-02`).
- Regression: all.

**Done when**
- [x] `cargo xtask tokens --check` is green in CI. The contrast report shows every declared pair passing in light and dark.
- [x] 11 §2.1 shows the final values.
- [x] 👤 The palette is approved (owner, 2026-10-20; recorded in the PR that added `ui/mockups/0.10/palette.html`).

---

### 0.11 — PriyaUI base components, gallery and UI test harness
**Outcome:** The first PriyaUI components live in a gallery that shows every state in light and dark at scale 1.0 and 2.0. CI renders them headlessly and compares screenshots. It also checks keyboard use, screen-reader names and translatable strings, and blocks any visual change the owner has not approved.
**Features:** DS-03, DS-04, DS-05, DS-06, QA-09.
**Depends on:** 0.4, 0.10.

**Tasks**
- [x] Mockups in `ui/mockups/0.11/`: HTML using `tokens.css`, covering every component and state below, light and dark. 👤 They must be approved before building.

  As built: `ui/mockups/0.11/components.html` reads the generated `tokens.css`, adding only a pressed tint and the card depth. 👤 Approved by the owner on 2026-10-20 (in chat, recorded in the PR that added the page).
- [x] Icons (from 0.10): a vendored Lucide SVG subset in `ui/PriyaUI/icons/`, containing only the icons used, plus Lucide's ISC licence, recorded in `THIRD_PARTY.md`. Fonts come from Arch packages and are not vendored.

  As built: the 18 icons of the approved mockup from `lucide-static` 1.50.0 (npm tarball checked against the registry's sha512), with `stroke-width` changed from 2 to 1.75 (11 §2.5) and nothing else. `icons/README.md` lists each icon's use and how to add one; `tst_icon.qml` loads every one. `Icon` tints them with `MultiEffect` (brightness 1, then colorization), which needs a GPU-style renderer: the offscreen platform uses Qt's software renderer, which draws no effects, so behaviour tests check sizes and colours, and screenshots show the pixels.
- [x] Rule for 11 §8 (from 0.10): PriyaUI and shell code use only `Theme.*`. Colour literals and numeric font sizes outside `Theme.qml` are lint errors (enforced by `xtask ui lint` in 0.11).

  As built: `cargo xtask ui lint` (`xtask/src/ui_lint.rs`) checks every `.qml` and `.js` file in `ui/PriyaUI`, `ui/shell` and `ui/gallery`. Rules:
  - **Tokens only** (not in the generated `Theme.qml`): no `"#…"` hex strings, no named colour as the value of a `…color`/`…Color`/`property color` binding (`"transparent"` is allowed), no `Qt.rgba(`/`Qt.hsla(`/`Qt.hsva(`/`Qt.hsl(`/`rgba(`/`rgb(`, and no number as the value (or a conditional branch) of `pixelSize`/`pointSize`. Tints derived from tokens (`Qt.alpha(Theme.danger, 0.08)`) and `Theme.body.pixelSize * 2` are fine.
  - **Translatable** (QML outside `ui/gallery/`, whose stories hold fixed sample text): a string with letters that is the value of `text`, `placeholderText`, `Accessible.name`, `Accessible.description`, `ToolTip.text`, `label`, `title` or `body` (also `T.ToolTip.text`, `label.text`) is an error. Strings inside a call (`qsTr("…")`, `formatDateTime(d, "hh:mm")`), an index (`model["title"]`) or after a comparison (`state === "busy"`) aren't the shown value; nor are strings inside JavaScript blocks (handlers, functions, object literals).
  - **Named** (QML): `MouseArea`, `TapHandler`, `TextInput`, `TextEdit`, namespaced Qt Quick Controls (`T.Button`, `T.TextField`, …) and anything with `activeFocusOnTab` need `Accessible.name` on themselves or an enclosing object in the same file, unless they set `Accessible.ignored`. PriyaUI controls: `IconButton` needs `accessibleName`, `Button` and `Chip` need `text` (a mode `Chip` names itself), `TextField` needs `label` (or `Accessible.name`).
  
  It is a line and brace scanner, not a QML parser: comments are dropped and strings set apart first; every string ends at its line's end, so a backtick in a regex can't hide the rest of a file. A binding split over several lines is only checked on its first line. Bad fixtures live in `ui/tests/lint/fixtures/`. CI's `ui` job and `ci-local` run it.
- [ ] QML module `PriyaUI` (`ui/PriyaUI/qmldir`), on the Qt 6 version pinned in the CI image (≥ 6.5).
  - Only QtQuick, QtQuick.Controls.Basic and QtQuick.Effects. No Quickshell imports in `PriyaUI`; only `ui/shell` may import Quickshell. This keeps the components runnable in the gallery on the Mac and Linux and in offscreen tests.
  - Building blocks: `Icon` (Lucide SVG tinted with `MultiEffect` colorization; sizes 16, 20, 24) and `FocusRing` (2 px accent, shown only for keyboard focus).
- [ ] Components. Each sets `Accessible.role`, `Accessible.name` and, where useful, `Accessible.description`. Interactive ones take Tab focus. Every user-visible string goes through `qsTr`.
  - **`Button`**
    - Variants: primary, secondary, ghost, danger.
    - States: default, hover, pressed, keyboard-focused, disabled, busy (label kept, progress shimmer, no spinning wheel).
    - Activates with Enter and Space.
  - **`IconButton`**
    - `accessibleName` is required (lint-enforced).
    - Tooltip after 600 ms; the same states as Button.
  - **`TextField`**
    - Single line.
    - States: empty with placeholder, focused, filled, disabled, error (message below the field, also exposed as `Accessible.description`).
  - **`PromptInput`**
    - Multi-line and growing: 1–6 lines, then scrolls.
    - Enter submits, Shift+Enter adds a newline, Esc emits `escapePressed`, Up on empty input recalls the previous prompt.
    - IME-safe: no submit while composing pre-edit text.
    - Renders Devanagari correctly.
    - Placeholder: "Ask or tell Priya anything…".
  - **`Card`**
    - Surface1 background, card radius, optional header and footer slots.
    - Compact and expanded.
  - **`AnswerCard`**
    - Rendering: `MarkdownBlocks.js` splits the text into blocks (paragraph, heading, list, quote, fenced code). While streaming, only the last block re-renders, finished blocks never reflow, and height grows smoothly. Prose blocks use `Text { textFormat: Text.MarkdownText }`; links open only on click. `CodeBlock` uses JetBrains Mono, a language label, horizontal scroll and a Copy button. Syntax highlighting comes in 5.4.
    - States: streaming (caret), done, error mid-stream (partial text kept, inline error), long content (scrolls within a maximum height), Hindi text.
    - Footer: model label and "Copy answer".
  - **`Chip`**
    - Label and optional icon; neutral and mode variants (Look, Sandbox, Direct, Direct raw), always with icon and word, never colour alone.
    - Static or selectable.
  - **`Toast`**
    - Variants: info, success, error.
    - Auto-dismiss after 4 s, paused on hover or focus; at most 3 on screen; optional action.
    - Announced with `Accessible.AlertMessage`.
  - **`Skeleton`**
    - Shimmer lines for "thinking".
    - Static when `Theme.reducedMotion` is set.
  - **`EmptyState`**
    - Icon, title, body, optional action.
  - **`ErrorState`**
    - Title (what happened) and body (what to do next).
    - Actions "Try again" and "Details"; Details expands copyable technical text.
- [x] Gallery app in `ui/gallery/`. `cargo xtask gallery` runs `qml ui/gallery/Main.qml`.
  - Layout: a sidebar of components. Each component has `ui/gallery/stories/<Component>.stories.qml` with named states and fixed props (deterministic text, no timers).
  - Controls: theme toggle, scale (1.0 or 2.0 via `QT_SCALE_FACTOR`) and a Hindi-sample toggle.
  - Shot mode: `--story <Component>/<state> --theme <t> --scale <s> --out <png>` sets `Theme.animationsEnabled = false`, renders one story and exits.

  As built:
  - **Stories.** A `StoryBook` (`component`, `summary`, `hindi`) holds `Story` items (`name`, `title`, a scene with 16 px padding). `Catalog.js` lists the 13 books in gallery order: Icon, FocusRing, Button, IconButton, TextField, PromptInput, Card, AnswerCard, Chip, Toast, Skeleton, EmptyState, ErrorState (46 stories). Controls show fixed states through a `preview` property (`"hover"`, `"pressed"`, `"focus"`), toasts through `timeout: 0` and `previewPaused`.
  - **Controls.** Selectable chips for light/dark, text size 200 % (`Theme.scale`), Hindi samples and reduced motion. Scale 2.0 itself is a process setting: `cargo xtask gallery --scale 2` sets `QT_SCALE_FACTOR=2`.
  - **Shot mode.** `--story <C>/<state> --theme <t> --out <png>` for one picture, `--all [--theme <t>] --out-dir <dir>` for every story (as `<dir>/<C>/<state>/<theme>.png`, folders made by the caller), `--list` to print the story ids. It turns motion off before setting the theme (so no colour fade is caught), places the story, waits two frames and grabs just the story with `grabToImage` (a compositor such as cage may make the window larger; the IconButton tooltip story draws its bubble in place, since a popup sits outside the story). 92 pictures take a few seconds in one process.
  - **Tests.** `ui/tests/gallery/tst_stories.qml` loads every book; `test_a11y_audit_all_stories` runs `A11yAudit` on each story and checks that Tab reaches every control.
- [ ] Headless screenshot harness: `cargo xtask test ui --shots`, run in the CI image.
  - For every story × {light, dark} × {1.0, 2.0}: run the gallery in shot mode under `cage` (`WLR_BACKENDS=headless`, `WLR_RENDERER=pixman`, fixed output size) and capture with `grim`.
  - Fonts are pinned through `ui/tests/fontconfig/fonts.conf` (fixed hinting and antialiasing).
  - Each shot is compared to `ui/tests/baselines/<story>/<theme>@<scale>.png` with `dssim`. Thresholds live in `ui/tests/thresholds.toml` (default 0.002), and diff images are written for every mismatch.
  - `--update-baselines` rewrites the baselines (compressed with oxipng).
- [ ] Rule for baseline changes: the `ui` job fails when the PR changes any baseline file without the label `visual-change-approved`. 👤 The owner adds the label after looking at the before, after and diff images in the PR report.
- [ ] Behaviour tests: Qt Quick Test (`qmltestrunner`, `QT_QPA_PLATFORM=offscreen`) in `ui/tests/components/tst_<component>.qml`.

  As built (runner): `cargo xtask test <selection>` runs the selected QML functions with one `qmltestrunner -import ui` call per file, naming each as `<TestCase>::<function>` (from `-functions`), so only the selected ones run. A function the runner doesn't list, or a file that doesn't load, fails. Outcomes go to `target/trace/ui-results.json`; `cargo xtask trace --results` takes it with the shell results. Qt's tools come from `$PRIYA_QT_BIN`, else `/usr/lib/qt6/bin` (Arch), else PATH. CI's `ui` job (in `ci-pass`) runs `cargo xtask test ui`, and `ci-local` runs every QML test.
- [ ] Accessibility (DS-06):
  - (a) Static lint, `cargo xtask ui lint`:
    - interactive items (MouseArea, TapHandler, `activeFocusOnTab`, PriyaUI controls) must have `Accessible.name`;
    - no colour literals (`#…`, `Qt.rgba`, `rgba(`) and no numeric `font.pixelSize` outside `Theme.qml`;
    - string literals in `text:`, `placeholderText:`, `Accessible.name:` and `ToolTip.text:` must be wrapped in `qsTr` (DS-05).
  - (b) Runtime helper `ui/tests/A11yAudit.qml`, used by every component test: it walks the item tree, checks that every focusable item has a non-empty name and role, and checks that Tab order follows visual order.
  - (c) Token contrast checks from 0.10.
- [ ] i18n (DS-05):
  - `lupdate` writes `ui/i18n/priya_en.ts`; `cargo xtask ui i18n --check` fails if it is stale.
  - The UI maps the API's `error.data.kind` to translated messages and falls back to `user_message`. Daemon strings stay English until DS-20.
  - A pseudo-locale (+40 % length, accented characters) is used for clipping tests.
- [ ] Add the `ui` job (behaviour, shots, lint, i18n) to `ci-pass`. The pre-commit hook runs `qmlformat` and `qmllint` on staged QML. The PR report embeds changed shots.

**UI**
- Components as listed above, in every listed state. Long-content and Hindi-text stories exist for `TextField`, `PromptInput`, `AnswerCard`, `Toast` and `ErrorState`.
- Light and dark. OLED and high-contrast baselines are added in 5.11. Scale 1.0 and 2.0.
- Keyboard: Tab and Shift+Tab follow visual order; Enter and Space activate; Esc works in `PromptInput`; a visible focus ring appears only for keyboard focus.
- Screen-reader names: every interactive component exposes a role and a name. `AnswerCard` exposes the answer text and a "Copy answer" button. `Toast` is announced.
- 👤 Mockup approval before building. 👤 Gallery sign-off after: the owner runs `cargo xtask gallery` on the Zenbook or reviews the shot set.

**Tests**
- UI behaviour:
  - `tst_button_keyboard_activation_and_disabled` (`DS-03`, `DS-06`)
  - `tst_iconbutton_exposes_accessible_name` (`DS-06`)
  - `tst_promptinput_enter_submits_shift_enter_newline` (`DS-03`)
  - `tst_promptinput_no_submit_during_ime_preedit` (`DS-03`)
  - `tst_promptinput_up_recalls_previous` (`DS-03`)
  - `tst_answercard_streaming_does_not_reflow_finished_blocks` (`DS-03`)
  - `tst_markdownblocks_split_cases`: table of unclosed fence while streaming, nested lists, Devanagari, a link in a heading (`DS-03`)
  - `tst_codeblock_copy_puts_text_on_clipboard` (`DS-03`)
  - `tst_toast_max_three_and_pause_on_hover` (`DS-03`)
  - `tst_skeleton_static_with_reduced_motion` (`DS-06`)
  - `tst_errorstate_details_expand_and_copy` (`DS-03`)
- UI screenshots: `ui_shots_match_baselines`, every story × light/dark × 1.0/2.0 (`DS-04`, `QA-09`).
- UI accessibility:
  - `a11y_audit_all_stories` (`DS-06`, `QA-09`)
  - `ui_lint_rejects_color_literal` (`QA-09`)
  - `ui_lint_rejects_untranslated_string` (`DS-05`)
  - `ui_lint_rejects_missing_accessible_name` (`DS-06`)
  
  The lint tests run against bad fixtures.
- UI i18n:
  - `i18n_ts_file_up_to_date` (`DS-05`)
  - `pseudo_locale_does_not_clip_button_chip_errorstate` (`DS-05`)
- Regression: all behaviour, accessibility and i18n tests, plus the full screenshot suite.

**Done when**
- [ ] The gallery shows all 10 components, plus `Icon` and `FocusRing`, in every listed state, on the Zenbook and on the Mac.
- [ ] The `ui` CI job takes under 5 minutes, and the screenshots are stable across 10 reruns of the same commit.
- [ ] A demo PR that changes a colour token without `visual-change-approved` is red and shows the diffs.
- [ ] Every story can be operated with the keyboard alone.
- [ ] 👤 Gallery sign-off is recorded.

---

### 0.12 — Walking skeleton: minimal model access and `priya ask`
**Outcome:** `priya ask "…"` streams an answer in the terminal from a cloud model on a free tier (Gemini, through its OpenAI-compatible endpoint) or from a local OpenAI-compatible endpoint such as Ollama. One setting chooses which.

**Providers (owner, 2026-10-20):** testing uses free tiers, not a paid Anthropic key. Gemini, xAI (Grok), Ollama, LM Studio and llama.cpp all speak the OpenAI-compatible API, so 0.12 builds that one adapter. The Anthropic adapter moves to 1.2, which already plans it with the OpenAI one. Free tiers may use prompts to improve the provider's models, so live tests send only test questions. Every question, model call and answer is in the event log, and no key material is stored outside the Secret Service.
**Features:** AI-21, CORE-04.
**Depends on:** 0.7, 0.9.

**Tasks**
- [ ] Minimal model types in `priya-ai`. Phase 1.1 extends them without breaking changes.
  - `ModelRequest { system, messages: Vec<Message { role, text }>, max_output_tokens, temperature }`.
  - `ModelStream` yields `StreamEvent::{Started { model }, TextDelta(String), Usage { input, output, cached }, Finished { reason }}`.
  - `ProviderError { kind: auth | rate_limited | overloaded | bad_request | network | timeout | server, retryable, retry_after, user_message }`.
  - Trait `ModelProvider { fn id(); async fn complete(&self, ModelRequest) -> Result<ModelStream>; async fn health(&self) -> Health }`. This is the [04 §2](../architecture/04-priya-ai.md) trait without `list_models`, `capabilities` and `price`, which come in 1.1.
- [ ] Adapter `openai_compatible`:
  - `POST {base_url}/chat/completions` with `stream: true` and `stream_options.include_usage`.
  - Parses `data:` chunks and `[DONE]`.
  - Sends `Authorization: Bearer <key>` when a key is stored for the provider; none for a local server.
  - Works with Gemini (`https://generativelanguage.googleapis.com/v1beta/openai`), xAI (`https://api.x.ai/v1`), Ollama (`http://127.0.0.1:11434/v1`), LM Studio and the llama.cpp server.
  - HTTP 401, 403, 429 and 5xx map to `ProviderError`; `Retry-After` sets `retry_after` (free tiers rate-limit often).
  - `health` is `GET {base_url}/models` with a 2 s timeout.
- [ ] Explicit timeouts (P9): connect 5 s; first token 60 s, because local models may still be loading; 30 s idle between chunks; 5 minutes total. One retry with jitter on `overloaded` or `network`, only before the first token. No retry once streaming has started.
- [ ] Keys, minimal version (the full flow is 1.5):
  - **Store.** `priya ai key set <provider>` reads the key without echo, or from stdin. It stores the key in the Secret Service collection `priya` with attributes `{app: "priya", provider}`, using the `oo7` crate. `priya ai key remove <provider>` deletes it.
  - **Read.** priyad reads keys on demand through a `SecretStore` port: `SecretServiceStore`, or `MemorySecretStore` in tests. Keys are held as `Secret<String>`.
  - **Never stored elsewhere.** No key in settings, events or logs.
  - **Docs.** Record in [05 §3](../architecture/05-security.md) that a Secret Service provider (gnome-keyring or KeePassXC) is a runtime requirement.
- [ ] `AiGateway` in priyad, the seed of Priya AI:
  - It resolves `ai.strategy = "fixed:<provider>/<model>"` and `ai.providers.<provider>` to an adapter.
  - Shipped defaults define `ai.providers.local = {kind = "openai_compatible", base_url = "http://127.0.0.1:11434/v1"}` and `ai.providers.gemini = {kind = "openai_compatible", base_url = "https://generativelanguage.googleapis.com/v1beta/openai"}`. No strategy is set, so a fresh install is in the "no model configured" state.
  - System prompt from `/usr/share/priya/prompts/answer.md`: who Priya is, answer briefly in Markdown, today's date, say when unsure.
  - No tools and no system access in Phase 0. Read-only system tools arrive with 5.1 and Phase 4.
- [ ] API:
  - `v1.intent.submit {text, context?: {conversation_id?, cwd?}, overrides?}` returns `{intent_id, conversation_id}`. The connection then receives notifications `v1.intent.stream {intent_id, seq, kind: started | delta | usage | done | error | cancelled, text?, model?, usage?, error?}`.
  - `v1.intent.cancel {intent_id}`.
  - `v1.ai.ask {prompt, role?}`: non-streaming, for modules. The role is recorded but ignored in Phase 0.
  - `host.ai.ask` for modules that declare `ai.role:*`.
  - Add the streaming notification contract and `v1.intent.cancel` to [02 §4](../architecture/02-core.md).
- [ ] Follow-ups: a `conversation_id` brings the previous turns from the `intents` projection, at most 10 turns. Beyond 24,000 characters the oldest turns are dropped first.
- [ ] Events:
  - `intent.submitted {text, conversation_id}`.
  - `model.called {provider, model, tokens_in, tokens_out, tokens_cached?, latency_first_token_ms, latency_total_ms, outcome, role?, cost_usd?}`. `role` and `cost_usd` are absent until Phase 1.
  - `intent.answered {answer | $blob}`, `intent.cancelled`, or an error outcome.
  - Schemas are snapshotted through 0.9.
- [ ] CLI:
  - `priya ask "<question>"` streams to stdout as plain text. `--json` prints NDJSON stream events. Ctrl+C sends `v1.intent.cancel`. `--model <provider>/<model>` is a task-layer override. `-c` continues the last conversation.
  - `priya ai status` shows the provider, the model, whether a key is stored and whether the provider is reachable.
- [ ] `priya-testkit` additions:
  - `FakeModelProvider`: scripted deltas, delays, errors and usage.
  - `Cassette`: record and replay HTTP+SSE through a local `wiremock` server. Recording scrubs `x-api-key` and `Authorization`. Every cassette directory has a `RECORDED.md`.
  - The `mock-priyad` binary: replays scripted NDJSON sessions with timings, used by 0.13.
- [ ] Doctor checks:
  - `ai.configured`;
  - `ai.key_present`, for cloud providers;
  - `ai.provider_reachable`, through `health`, which never spends tokens;
  - `secret_service.available`, with a plain fix hint.
- [ ] Live smoke, manual and outside CI (CI follows with QA-13 in Phase 1): `PRIYA_LIVE=1 cargo xtask test live` makes one Gemini free-tier call with `max_output_tokens = 16` and one Ollama call.

**Human in the loop:** 👤 The owner stores the free-tier Gemini key with `priya ai key set gemini`; Claude never sees or handles key values. The owner chooses the models that `ai.strategy` points to.

**Tests**
- Contract: `model_provider_contract`. It runs against `openai_compatible` with recorded cassettes (Gemini and Ollama), and against `FakeModelProvider` (all `covers: AI-21`):
  - `streams_text_deltas_in_order`
  - `reports_usage_at_end`
  - `maps_401_to_auth_error_with_plain_message`
  - `maps_429_with_retry_after`
  - `retries_overloaded_once_before_first_token`
  - `never_retries_after_first_token`
  - `idle_timeout_yields_timeout_error`
  - `cancel_drops_connection_within_100ms`
  - `malformed_sse_line_is_error_not_panic`
- Unit:
  - `strategy_fixed_resolves_provider_and_model` (`AI-21`)
  - `no_strategy_gives_connect_hint` (`AI-21`)
  - `conversation_context_drops_oldest_first` (`AI-21`)
  - `system_prompt_includes_date` (`AI-21`)
- Integration (`TestDaemon` with `FakeModelProvider`):
  - `ask_streams_and_records_submitted_called_answered` (`AI-21`, `CORE-04`)
  - `cancel_mid_stream_records_cancelled` (`AI-21`)
  - `two_concurrent_asks_on_one_connection_are_independent` (`CORE-04`)
  - `daemon_restart_mid_stream_gives_client_error_not_hang` (`CORE-04`)
  - `cli_ask_json_matches_schema` (`CORE-04`)
  - `module_host_ai_ask_requires_capability` (`AI-21`)
- Security (fast subset):
  - `api_key_never_in_settings_events_or_logs`: run with a known fake key, then search the data dir, the event store and the captured logs (`AI-21`).
- Perf (informational, Zenbook): with a warm local small model, the first token arrives in under 1.5 s (09 budget).
- Regression: the contract suite, the unit, integration and security tests.

**Done when**
- [ ] On the Zenbook, after `priya ai key set gemini` and `priya settings set ai.strategy fixed:gemini/<free-tier model>`, `priya ask "what is btrfs?"` streams from Gemini. After `priya settings set ai.strategy fixed:local/<model>`, it streams from Ollama. Ctrl+C stops within 200 ms.
- [ ] `priya events tail --type 'intent.*' --type 'model.*'` shows three events per question. No key material exists anywhere under `~/.local/share/priya` or `~/.config/priya`.
- [ ] The contract suite is green for the adapter (Gemini and Ollama cassettes) and the fake.

---

### 0.13 — Walking skeleton: minimal Priya bar
**Outcome:** On the owner's desktop:
- Super+Space opens the Priya bar.
- A question plus Enter streams the answer into an AnswerCard.
- Esc stops the answer, and a second Esc closes the bar.
- The bar follows `ui.theme` live in light and dark.

**Features:** UX-02, DS-02, DS-03, DS-06.
**Depends on:** 0.11, 0.12.

**Tasks**
- [ ] Mockups in `ui/mockups/0.13/`: every bar state listed under UI, light and dark, at the Zenbook's resolution and scale. 👤 They must be approved before building.
- [ ] `PriyaClient` (`ui/PriyaUI/client/PriyaClient.qml` and `PriyaClient.js`): JSON-RPC over NDJSON.
  - The transport is injected: Quickshell `Socket` with `SplitParser` in the shell, and a mock transport in tests. `PriyaUI` stays free of Quickshell.
  - It handles request ids and routes notifications by `intent_id`.
  - Reconnect backoff goes from 250 ms to 5 s, and the connection state is `connecting`, `connected` or `unavailable`.
  - Method names come from the generated `Api.js`.
- [ ] Quickshell config `ui/shell/priya/shell.qml`, installed to `/usr/share/priya/shell/`. It runs as its own instance (`qs -p /usr/share/priya/shell`), next to Waybar or any other Quickshell config.
  - `PriyaBar.qml` is a `PanelWindow` on `WlrLayershell` layer `Overlay`, with exclusive keyboard focus while visible.
  - Position and size: top-centre, width `min(ui.bar.width_px, 60 % of the screen)`, radius 16.
  - It is created at startup and only shown or hidden, to meet the < 100 ms open budget.
  - `IpcHandler { target: "priya-bar" }` with `toggle()`, `open()` and `close()`.
- [ ] Bar content, built from PriyaUI components only:
  - the Priya mark (a static glyph with a state tint) and `PromptInput`;
  - a result area showing `Skeleton` (thinking), `AnswerCard` (streaming and done, footer "Answered by <model>" plus Copy), `ErrorState` or `EmptyState`;
  - footer key hints: Enter ask · Shift+Enter new line · Esc stop/close;
  - maximum height 70 % of the screen, with scrolling inside.
  
  Mode and model chips, attachments, mic and local results belong to Phase 5 (UX-17).
- [ ] Behaviour:
  - **Open.** Super+Space toggles the bar. Opening focuses the input; the previous answer stays until a new question.
  - **Ask.** Enter calls `v1.intent.submit` with the bar's `conversation_id`. A new conversation starts after 10 minutes idle or on Ctrl+N.
  - **Stop and close.** Esc while streaming calls `v1.intent.cancel`; the partial answer stays and gets a "Stopped" chip. Esc when idle closes the bar and focus returns to the previous window. Clicking outside closes the bar; a running stream continues and is shown on reopen.
  - **Copy.** Ctrl+Shift+C copies the answer.
- [ ] Theme: on connect the bar reads `ui.theme` with `v1.settings.get` and subscribes to `setting.changed` for `ui.*` (`v1.events.subscribe`). Changes switch `Theme.mode` live with a 200 ms colour transition.
- [ ] Desktop integration:
  - **Hyprland snippet** `/usr/share/priya/desktop/hyprland.conf`:
    - `bind = SUPER, SPACE, exec, qs -p /usr/share/priya/shell ipc call priya-bar toggle`;
    - `exec-once = systemctl --user import-environment WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP && systemctl --user start priya-shell`;
    - a layer rule that turns blur off.
    
    The owner adds a single `source = …` line to `~/.config/hypr/hyprland.conf`. This is also a user-scope doctor fix that asks for confirmation.
  - **`packaging/systemd/priya-shell.service`** (user unit): `PartOf=graphical-session.target`, `ExecStart=/usr/bin/qs -p /usr/share/priya/shell`, `Restart=on-failure`.
  - **Other wlr-layer-shell compositors:** the docs give the equivalent keybind.
- [ ] Doctor checks:
  - `desktop.quickshell_installed`: version at least the pinned one;
  - `desktop.hyprland_snippet_sourced`;
  - `desktop.priya_shell_running`;
  - `desktop.shortcut_free`: reads `hyprctl binds -j` and reports a conflicting Super+Space binding (for example Omarchy's launcher) with the alternative from decision 0.1(c);
  - `desktop.fonts_present`: `fc-list` finds Inter, JetBrains Mono and Noto Sans Devanagari.
- [ ] Journey harness in `ui/tests/journeys/`. It runs the real shell config under headless Hyprland, with headless `sway` (which also supports wlr-layer-shell) as a fallback. `PriyaClient` connects to `mock-priyad`. Actions are driven with `qs … ipc call` and `wtype`, and screens are captured with `grim`.
- [ ] Gallery stories `PriyaBarContent/<state>`: the bar's content without layer-shell, so every state gets screenshot baselines under 0.11's harness.
- [ ] Perf instrumentation: at debug level the bar logs `open_requested → first_frame` and `submit → first_delta_rendered`.

**UI**
- States:
  - hidden;
  - idle: placeholder and key hints;
  - typing;
  - thinking: Skeleton shimmer;
  - streaming;
  - done: AnswerCard, model label, Copy;
  - stopped: partial answer and a "Stopped" chip;
  - provider error, for example "Anthropic is busy right now" with Try again and Details;
  - no model configured: EmptyState "Connect a model" showing the two commands;
  - priyad unavailable: ErrorState "Priya's background service isn't running" with the start command;
  - offline: a cloud provider is unreachable, with a hint to switch to the local model;
  - long content: scrolls;
  - Hindi question and answer.
- Light and dark. OLED and high contrast come in 5.11. Scale 1.0 and 2.0, and the owner's fractional scale on the Zenbook OLED is checked by hand.
- Keyboard path: Super+Space → type → Enter → Esc to stop if needed → Tab to Copy → Esc to close. No mouse needed, and focus returns to the previous window.
- Screen-reader names: the input is "Ask Priya". The answer is announced once when complete, not per token. The buttons are "Copy answer", "Try again" and "Details".
- 👤 Mockup approval before building. 👤 Sign-off on the recorded journey and on real use on the Zenbook.

**Tests**
- UI behaviour (mock transport):
  - `tst_bar_enter_submits_and_shows_thinking` (`UX-02`)
  - `tst_bar_stream_deltas_render_in_order` (`UX-02`)
  - `tst_bar_esc_cancels_then_closes` (`UX-02`)
  - `tst_bar_error_try_again_resubmits` (`UX-02`)
  - `tst_bar_no_model_shows_connect_empty_state` (`UX-02`)
  - `tst_bar_daemon_unavailable_then_reconnects` (`UX-02`)
  - `tst_bar_theme_follows_setting_changed` (`DS-02`)
  - `tst_priyaclient_routes_notifications_by_intent_id` (`UX-02`)
  - `tst_priyaclient_reconnect_backoff` (`UX-02`)
- UI screenshots: `PriyaBarContent` per state × light/dark × 1.0/2.0 (`UX-02`, `DS-02`, `DS-03`).
- UI journeys (headless compositor with `mock-priyad`):
  - `journey_ask_stream_copy_close` (`UX-02`)
  - `journey_ask_cancel_mid_stream` (`UX-02`)
  - `journey_theme_switch_while_open` (`DS-02`)
  - `journey_keyboard_only`: no pointer events (`DS-06`)
- Accessibility: `a11y_audit_bar_states` (`DS-06`). Optional 👤 Orca smoke test on the Zenbook: the input and the answer are read out.
- Perf (manual on the Zenbook, recorded in the PR):
  - bar open p95 < 100 ms over 20 opens;
  - first token rendered in under 1.5 s with a warm local small model;
  - idle CPU of the shell instance ≈ 0 % over 60 s.
- Regression: all behaviour, screenshot, journey and accessibility tests.

**Done when**
- [ ] On the Zenbook, Super+Space → question → streamed answer works with Anthropic and with Ollama. Esc stops, and a second Esc closes.
- [ ] `priya settings set ui.theme light` restyles the open bar within 1 s, without a restart.
- [ ] The perf numbers are recorded and within budget.
- [ ] 👤 Mockup approval and journey sign-off are recorded.

---

### 0.14 — Package for any Arch and first install
**Outcome:** `makepkg -si` from the repo installs Phase 0 on any Arch install, whatever the filesystem. After enabling two user services, storing a key and adding one Hyprland line, the owner uses the Priya bar every day.
**Features:** OS-01, CORE-05.
**Depends on:** 0.1, 0.13.

**Tasks**
- [ ] `packaging/arch/PKGBUILD`, a split package built from the local checkout. `pkgver()` comes from `git describe`, as `0.0.<commits>.g<hash>`. `prepare()` runs `cargo fetch --locked`; `build()` runs `cargo build --release --frozen`. Packages:
  - **`priya`**
    - Files: `/usr/bin/priyad`, `/usr/bin/priya`, `/usr/lib/systemd/user/priyad.service`, `/usr/share/priya/{defaults,prompts,desktop}/`, shell completions, licence.
    - Optional dependencies: `ollama` (local model), and `gnome-keyring` or `keepassxc` (Secret Service).
  - **`priya-shell`**
    - Files: `/usr/share/priya/shell/` (the Quickshell config and the `PriyaUI` module), `/usr/lib/systemd/user/priya-shell.service`.
    - Dependencies: `priya`, `quickshell`, `qt6-declarative`, `qt6-svg`, `inter-font`, `ttf-jetbrains-mono`, `noto-fonts`.
- [ ] `priya.install`: only prints a post-install message with the next commands. It enables no services and edits no user files.
- [ ] `package` job in `ci.yml`, added to `ci-pass`. It builds in the CI image as the `builder` user and runs `namcap`. Then, in a fresh container, it:
  - installs the package and runs `pacman -Qkk`;
  - runs `priya --version`;
  - starts `priyad --foreground` with a temporary runtime dir and runs `priya doctor --json` (everything ok or info, apart from the desktop checks);
  - installs the next build over it to test the upgrade;
  - removes everything with `pacman -Rns` and checks that no files remain under `/usr`.
  
  On `main` it uploads the `.pkg.tar.zst` as an artifact.
- [ ] Doctor checks: `install.paths` (binaries and units present; priyad and CLI versions match) and `systemd.user_units` (whether `priyad` and `priya-shell` are enabled and active).
- [ ] Upgrade safety: a newer build keeps `~/.local/share/priya/priya.db` and migrates it with the backup from 0.5.
- [ ] Owner guide: a section "Try the Priya bar (Phase 0)" in `docs/human/07-install-on-your-zenbook.md`, in plain words. It covers building and installing, enabling the services, adding the key, adding the Hyprland line, pressing Super+Space, and removing everything again.
- [ ] 👤 First install on the Zenbook: the owner follows the guide while Claude reads the `priya doctor` output. Problems become issues and are fixed in normal PRs; the guide is corrected until it works without extra help.
- [ ] Daily use for at least a week, or as long as the owner chooses. Feedback goes into one GitHub issue checklist; fixes land as normal PRs.

**Human in the loop:** 👤 The owner runs `makepkg -si` (sudo), enables `priyad` and `priya-shell`, stores the key and adds the Hyprland line. Everything can be removed with `pacman -Rns priya priya-shell`. User data under `~/.local/share/priya` and `~/.config/priya` stays until the owner deletes it, as the guide explains.

**Tests**
- Integration (CI container):
  - `package_installs_and_doctor_passes` (`OS-01`, `CORE-05`)
  - `package_upgrade_keeps_and_migrates_db` (`OS-01`)
  - `package_removal_leaves_no_system_files` (`OS-01`)
  - `namcap_clean` (`OS-01`)
- Unit:
  - `pkgver_from_git_describe`: a shell test (`OS-01`).
- E2E: an L4 checklist, run by hand on the Zenbook and recorded in the PR:
  - install from clean;
  - Super+Space works after login with no manual start;
  - it still works after a reboot;
  - after suspend and resume, the bar reconnects;
  - with the network off, the offline state shows and the local model still answers;
  - remove and reinstall.
- Regression: the four integration tests and the unit test.

**Done when**
- [ ] The package builds on every merge to `main` and passes the install, upgrade and removal tests.
- [ ] The owner installed it on the Zenbook using only the guide.
- [ ] The L4 checklist is ticked.

---

### 0.15 — Phase gate
**Outcome:** Phase 0 is proven in CI and in the owner's hands, and the docs describe what was built.
**Features:** every Phase 0 feature (gate).
**Depends on:** 0.1–0.14.

**Tasks**
- [ ] Every sub-phase is promoted (`cargo xtask regression promote 0.k`) and `cargo xtask trace` is green.
- [ ] `cargo xtask test regression --all` is green on `main`, and nightly is green three nights in a row. The quarantine list is empty.
- [ ] Results recorded in §Results below:
  - overall line coverage per Phase 0 crate;
  - bar open p95;
  - first token, local and cloud;
  - idle priyad RSS;
  - median PR pipeline time.
- [ ] Docs made true:
  - [01 §6](../architecture/01-system-overview.md): add `priya-testkit`, `xtask` and `priya-themegen`; mark which crates exist.
  - [02](../architecture/02-core.md): new events and actor; new methods; NDJSON framing; the intent streaming contract.
  - [05 §3](../architecture/05-security.md): the Secret Service requirement.
  - [09](../architecture/09-testing.md): tag grammar, job names, coverage groups, the baseline approval label.
  - [11](../architecture/11-design-system-and-ux.md): final tokens, `border.strong`, and the rule that PriyaUI has no Quickshell imports.
  - [10 §4](../architecture/10-base-os-and-distro.md): the exact Zenbook model.
  - ADRs for the licence and the surface framing.
- [ ] `features.md`: all 25 Phase 0 features set to ✅, which `trace` verifies.
- [ ] `docs/human/05-the-phases.md`: Phase 0 marked done with a plain-language summary, and the table reordered to the current numbering (it still lists the old order: Sandbox as 1, Priya AI as 3).
- [ ] 👤 Owner demo on the Zenbook, following the script below.
- [ ] Short retrospective. Any change to the workflow goes into [phases/README](README.md).

**Demo script**
1. A PR with a deliberately failing test: `ci-pass` is red and merging is blocked. Fix it: green.
2. `priya status`, `priya doctor`, `priya audit verify`.
3. Super+Space, then a question in English: a streamed answer from Anthropic. Press Esc mid-answer.
4. `priya settings set ui.theme light` while the bar is open.
5. `priya settings set ai.strategy fixed:local/<model>`, then a question in Hindi: the answer comes from Ollama.
6. `priya module call echo.say` (dev module); `kill -9` the module and watch it restart in `priya events tail`.
7. `cargo xtask gallery`, toggling dark and light.

**Done when**
- [ ] Every item in the Phase gate below is ticked.

---

## Phase gate

- [ ] The full regression suite (`cargo xtask test regression --all`) is green in CI on `main` and in three consecutive nightly runs. The quarantine list is empty.
- [ ] `cargo xtask trace` is green, and every acceptance test of 0.1–0.14 carries `regression:`.
- [ ] 👤 Owner demo of the walking skeleton on the Zenbook (0.15 script) is done and signed off.
- [ ] Architecture docs and ADRs are updated as listed in 0.15. CLAUDE.md is updated if the workflow changed.
- [ ] Feature statuses in [features.md](../features.md): all 25 Phase 0 features are ✅.
- [ ] Simple status updated in `docs/human/05-the-phases.md`.
- [ ] CI required checks are enabled on `main`: the ruleset requires `ci-pass`, and `ci-pass` needs fmt, clippy, deny, typos, links, build-linux, build-macos, test-linux, test-macos, secrets, trace, coverage, flaky, schemas, tokens, ui and package. If the owner chose option (c) in 0.3, the decision and its risk are recorded instead.

## Decisions (filled during 0.1)

- (a) Phase 0 target session: **Hyprland**.
- (b) Phase 2 dogfooding path: **Path C, PriyaOS as a third system** (2.5). The owner's Arch is their daily system and isn't btrfs, so it's neither converted nor replaced. The owner shrinks the unused Windows by about 200 GB from Windows' own Disk Management, and the dev installer puts PriyaOS in that free space. The boot menu then offers PriyaOS, Arch and Windows and remembers the last choice.
- (c) Super+Space conflict handling: **no conflict** on the owner's machine; all planned Super keybinds are free. The conflict check in `priya desktop install` is still built for other machines.
- GitHub plan and visibility: public (checked 2026-10-19), plan _tbd_ · Licence: Apache-2.0 (ADR-022)

## Results (filled during 0.15)

- Coverage per crate: _tbd_ · Bar open p95: _tbd_ · First token (local / cloud): _tbd_ · Idle priyad RSS: 10 MiB (Zenbook, 2026-10-20) · Median PR pipeline: _tbd_

## Out of scope

- Any btrfs operation, snapshot, fork, sandbox, merge or rollback, and any `priya-helper` or root component (Phase 2). Doctor reports the filesystem but changes nothing system-wide.
- The model catalog, discovery, roles, strategies other than `fixed:`, routing, costs and prices, token optimization, the local proxy, Google/xAI/OpenAI-native adapters and the "Connect a provider" UI (Phase 1).
- Tasks, plans, policy (Cedar), capability sandboxing of modules (seccomp/Landlock), approvals, secrets brokering, kill switch (Phase 3).
- Agents, tools, read-only system tool runs, `priya mcp` (Phases 4–5). Phase 0 answers come from the model alone.
- Mode and model chips, attachments, local results, the launcher, cards beyond the answer card, notifications, Settings app, syntax highlighting, OLED-black, high-contrast, accent choice, auto light/dark, whole-desktop theming (Phases 3–5).
- Hindi UI translation (Phase 10). Phase 0 only renders Hindi input and answers correctly.
- Signed repository, ISO, installer, first-boot setup (Phase 9). Phase 0 installs from a local `makepkg` build.
- Fake machine, VM end-to-end, live provider tests in CI, mutation testing, performance budgets as blocking checks (Phases 1–2). Phase 0 records perf numbers by hand.
- GNOME and other compositors without wlr-layer-shell. They can use `priya ask` only.
