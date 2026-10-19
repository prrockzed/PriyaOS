# 09 — Testing, CI and regression protection

Goal: **no change can silently break something that already works.** Every feature has tests that prove it. Every push runs them automatically. A change that breaks an earlier feature fails CI and cannot be merged. A change that adds code without tests also fails CI.

## 1. Rules

1. **Every feature has acceptance tests.** A feature in [features.md](../features.md) can only be marked ✅ when at least one test tagged with its ID passes in CI (checked automatically, §5).
2. **Every acceptance test is permanent.** Once a sub-phase passes its gate, its acceptance scenarios join the **regression suite** and run on every merge (fast ones) or nightly (slow ones) forever. Removing one requires an ADR.
3. **Every bug fix adds a test** that fails before the fix and passes after, tagged `bug:<issue-number>`.
4. **Changed code must be tested.** Diff coverage on changed lines ≥ 85 % for Rust crates (`priya-core`, `priya-sandbox`, `priya-policy`, `priya-ai` ≥ 90 %). Lower only with a written justification in the PR.
5. **Contracts don't break silently.** Public API, event, settings and module-manifest schemas are snapshotted; any breaking change fails CI unless the version is bumped and a migration exists (§6).
6. **UI is tested too.** Every component and screen has screenshot tests in light and dark (and, from sub-phase 5.11, OLED black and high contrast), plus automated accessibility checks (§7).
7. **Flaky tests are bugs.** A test that fails intermittently is quarantined within a day (`#[ignore = "flaky: <issue>"]` plus a tracking issue) and fixed within a week. The quarantine list is shown in CI output; it can't grow beyond 5.
8. **Main is always green.** All work happens on branches and lands through pull requests with required checks (§4). Direct pushes to `main` are blocked by branch protection.

## 2. Test pyramid and suites

| Suite | Tag | What | Where it runs | Speed budget |
|---|---|---|---|---|
| Unit | `unit` | Pure logic: state machines, parsers, routing math, policy evaluation, diff decisions, settings layering | Mac, Linux, CI | whole suite < 60 s |
| Contract | `contract` | Every adapter against its port's shared suite (providers with recorded HTTP cassettes, agents with recorded streams, snapshot backend, sandbox runner, notifier, STT/TTS, screen source) | Mac/Linux, CI | < 3 min |
| Integration | `integration` | Several real components together (priyad + modules + fakes); anything touching the filesystem layout runs priyad + helper against the **fake machine** (btrfs loop image with a pacstrapped Arch) | Linux (+ root for the fake machine), CI | < 10 min on PRs (sharded) |
| UI | `ui` | QML component tests (Qt Quick Test), screenshot tests per theme and scale, accessibility lint | Linux headless compositor, CI | < 5 min |
| End-to-end | `e2e` | Built ISO installed in a QEMU VM with a fake Windows partition; full user journeys across reboots | CI nightly (KVM) + Zenbook | < 60 min |
| Performance | `perf` | Budgets: fork < 2 s, ChangeSet 10k files < 10 s, bar open < 100 ms, local results in the bar < 50 ms, first answer token < 1.5 s (local fast model), Task center / Settings open < 250 ms, theme apply across the desktop < 1 s, idle priyad RSS < 80 MiB, Quickshell RSS < 150 MiB | CI nightly on fixed runner | — |
| Chaos | `chaos` | Kill priyad / helper / VM at every journaled step; assert recovery and no corruption | CI nightly | — |
| Security | `security` | Prompt-injection corpus, secret-leak checks, policy bypass attempts, malicious module | PRs (fast subset), nightly (full) | — |
| Evaluation | `eval` | Router accuracy and cost on the labelled prompt set; agent task success; undo-by-prompt ranking | Nightly + before releases | — |
| Live | `live` | Tiny real calls to each enabled provider (budget ≤ $0.50) and nightly real-agent end-to-end tasks (budget ≤ $1.50 per night); needs secrets | Nightly (main only), manual | — |
| Mutation | `mutation` | `cargo-mutants` on core crates; surviving mutants reported | Weekly | — |

The **regression suite** is not a separate set of tests: it is every test with tag `regression`, which every acceptance test gets automatically when its sub-phase passes its gate.

## 3. Test infrastructure

- **`priya-testkit` crate**: fixture loading, golden-file assertions (`insta` snapshots), fake clock, fake model provider, fake agent binary that replays recorded streams, fake notifier, temp settings layers, event-log assertions, `FakeMachine` handle (create, run priyad + helper against it, tear down).
- **Tags in code**: every test declares what it covers in comment lines directly above it. `xtask/src/tags/` parses them into `target/trace/tests.json`; a missing or malformed tag fails CI.
  ```rust
  // covers: SBX-04, SBX-03
  // suite: integration
  // acceptance: 2.11
  #[test]
  fn home_merge_conflict_keeps_live_file() { … }
  ```
  - **Where tags go.** Rust: above the attributes of a `#[test]` or `#[tokio::test]` function; other attributes (`#[ignore]`) and `///` doc comments may sit between. Tests nested inside function bodies are ignored, since they never run. QML (`tst_*.qml`): above each `function test_*`. Python (`test_*.py`): above each `def test_*` or `async def test_*`, decorators allowed between. Shell (`scripts/tests/test_*.sh`): the whole file is one test, tagged in the header comment under `#!`. Rust and QML use `//`, shell and Python `#`.
  - **Tags.** `covers: <ID>[, <ID>…]` (IDs from [features.md](../features.md), like `CORE-01`) or `covers: infra` for tooling; `suite: unit|contract|integration|ui|e2e|perf|chaos|security|eval|live|mutation`; optional `acceptance: <sub-phase>` (e.g. `0.5`); `regression: <sub-phase>`, added at the gate; optional `bug: <issue>`. `covers` and `suite` are required; each tag appears at most once. Other comment lines in the block are ignored.
  - **Test ids.** Rust: `<nextest binary id>::<module path>::<fn>`, where the binary id is `<package>` for the library, `<package>::bin/<name>` for a binary and `<package>::<test file>` for an integration test (so `priya-core::paths::tests::…`, `xtask::bin/xtask::tags::tests::…`). Files are tied to targets by walking each target's `mod` declarations from its root file, as cargo metadata reports it. Others: `<file>::<function>`, or `<file>` for a shell test.
  - **Never silently skipped.** A `.rs` file with tests that no target compiles, a `proptest!` block (its tests can't carry tags) and a duplicate test id are errors. Before running, `cargo xtask test` checks that nextest knows every indexed Rust test; a test behind a platform or feature `#[cfg]` that isn't built on this machine gets a note instead of an error. `#[ignore]`d (quarantined) tests are known but skipped.
  - **Scanned folders**: `crates/`, `modules/`, `ui/`, `scripts/tests/` and `xtask/`, skipping `target/` and every `fixtures/` folder.
  - **Running by tag**: `cargo xtask test <suite>`, `cargo xtask test regression [--fast|--all]` (`--fast`: unit, contract, integration and ui suites) and `cargo xtask test all`. Rust tests go to nextest with an exact filter per test; shell tests run with bash. `--list` prints the selection.
  - **Diff coverage.** `cargo xtask coverage` compares the lines a change adds or edits against the lcov report of the CI test run. Groups live in `xtask/coverage.toml`: 85 % by default, 90 % for `crates/priya-core/**` and `crates/priya-ai/**` (`priya-sandbox` and `priya-policy` join when they exist). Test code, `tests/` folders, generated files and xtask's argument dispatch don't count. A PR labelled `coverage-exception` with a "Coverage justification" section passes with a warning.
  - **Quarantine.** A flaky test is never retried: it gets `#[ignore = "flaky: #<issue>"]` and an entry in `tests/quarantine.toml`. `cargo xtask flaky check` allows at most 5 entries, none older than 7 days (warning from day 5); the nightly flaky hunt posts each quarantined test's pass rate on its issue.
  - **Regression mechanism.** Acceptance tests carry `acceptance: <sub-phase>`. At the sub-phase gate, `cargo xtask regression promote <sub-phase>` adds `regression: <sub-phase>` under each of them, which puts them in the permanent regression suite.
- **Recorded fixtures**: provider HTTP cassettes, agent streams, `btrfs receive --dump` outputs, screenshots, audio. Each fixture directory has a `RECORDED.md` (how, when, tool versions). Secrets are scrubbed by a pre-commit check.
- **Fake machine**: `scripts/make-fake-machine.sh` builds a 6 GiB btrfs loop image with `@`, `@home`, `@log`, `@pkg`, `@priya`, `@priya-forks`, `@priya-timeline`, a pacstrapped minimal Arch and a test user; cached as a CI artifact keyed by script hash and Arch archive date. Arch packages come from a pinned Arch Linux Archive date so runs are reproducible.
- **VM harness**: `scripts/vm-test.sh <iso> <suite>` creates a qcow2 disk with a fake Windows layout (260 MiB ESP + NTFS "Windows"), boots OVMF, installs unattended, then runs suites over SSH with snapshots between journeys.
- **Headless UI harness**: Hyprland in headless mode (or `cage` for single-surface tests) with `WLR_BACKENDS=headless`; Quickshell loads components from the gallery; `grim` captures; perceptual diff (`dssim`) with per-component thresholds; runs at scale 1.0 and 2.0.
- **`cargo xtask`**: one entry point — `xtask test unit|contract|integration|ui|e2e|regression|all`, `xtask coverage`, `xtask trace`, `xtask schemas`, `xtask gallery`, `xtask fake-machine`.

## 4. CI pipeline (GitHub Actions)

```
 push to branch / PR ──▶ ① checks (all must pass to merge)
                          fmt · clippy -D warnings · cargo-deny (licenses, advisories) · typos · markdown link check
                          build: linux x86_64 (all crates) + macOS arm64 (portable crates)
                          unit + contract (Linux + macOS)
                          security fast subset
                          schema compatibility (§6)
                          traceability (§5)
                          diff coverage (§1.4)
                          integration (fake machine, sharded) ── ubuntu runner with sudo, loop devices, privileged archlinux container
                          ui (headless compositor): component tests, screenshots vs baselines, a11y lint
                          PR comment: coverage diff, screenshot diffs (images), traceability changes, perf deltas
 merge to main ─────────▶ ② same as ① + full regression suite + build packages (artifacts)
 nightly (main) ────────▶ ③ e2e in VM (KVM) · perf · chaos · security full · eval · live (budgeted)
                          results to a dashboard issue; any failure opens/updates an issue automatically
 weekly ────────────────▶ ④ mutation testing · dependency updates PR (renovate) · fixture freshness report
 tag v* ────────────────▶ ⑤ release: everything in ①–③ green on the tagged commit → signed packages → repo → ISO → checksums → release notes
```

- **Runners**: GitHub-hosted Ubuntu runners for ①–②; nightly e2e on GitHub-hosted runners with KVM where available, otherwise a **self-hosted runner on the Zenbook** (labels `self-hosted, linux, kvm, zenbook`), which also runs the L4-adjacent hardware smoke tests on demand. Because the Zenbook is the owner's daily machine, its runner uses a dedicated `priya-ci` user **without sudo** that can only start VMs (KVM group) and read hardware state; privileged fake-machine jobs never run on it. Self-hosted runners only run workflows from this repository's branches, never from forks.
- **Caching**: cargo (sccache), fake-machine image, Arch package cache, Qt/Quickshell build.
- **Branch protection on `main`**: required checks from ①, linear history, PRs required (Claude opens PRs; the owner or an approval rule merges).
- **Local pre-commit hook** (`scripts/install-hooks.sh`): fmt, clippy on changed crates, unit tests of changed crates, secret scan, `covers:` tag presence on new tests. It's fast enough to never be skipped.

## 5. Traceability

`cargo xtask trace` checks the test index against [features.md](../features.md) and the phase files, and writes `target/trace/matrix.md` and `matrix.json` (feature → tests → last result), attached to every CI run. It fails when:

1. a test covers a feature ID that features.md doesn't list;
2. a feature marked 🟨 or ✅ has no tagged test;
3. a ✅ feature has no test that passed in this run (when `--junit` / `--script-results` are given, as in CI);
4. a sub-phase whose **Done when** boxes are all ticked has an `acceptance:` test without `regression:` (fix: `cargo xtask regression promote <sub-phase>`);
5. compared with the base branch (`--base`), a `regression:` test was deleted or lost its tag, unless the PR description (`--pr-body`) cites an existing `ADR-NNN` (the removal is then printed as a note) or a new `regression:` test with the same `covers` and `acceptance` replaces it, one for one.

A test without `suite:` already fails when the index is built. This is what turns "did my change break an old feature?" into a mechanical answer: the old feature's tests are in the regression suite and run on the PR.

## 6. Schema and compatibility checks

`cargo xtask schemas` exports JSON Schemas for: public API (`v1.*`), events (every type and version), settings (all sections incl. modules), module manifest, helper protocol, MCP tool definitions. They're committed under `docs/claude/api/`. CI regenerates and compares:

- additive changes (new optional field, new method, new event type) → allowed, snapshot updated in the same PR;
- breaking changes (removed/renamed field, type change, new required field) → fail unless the API version is bumped (`v2.` namespace kept alongside `v1.`) or, for events, a new event version with an upcaster from the old version is added;
- settings: removed keys need a migration that rewrites user files (tested with fixtures of old configs).

## 7. UI testing

- **Component tests** (Qt Quick Test) for behaviour: keyboard navigation, focus order, states, signals.
- **Screenshot tests**: every component and screen in every state listed in its spec (empty, loading, streaming, success, error, offline, long text, Hindi text), in **light, dark, OLED black, high-contrast**, at scale 1.0 and 2.0. Baselines live in `ui/tests/baselines/`; updating them requires the PR to show before/after images, reviewed by the owner for visual changes.
- **Accessibility**: token contrast checker (every text/background token pair ≥ WCAG 2.2 AA, 4.5:1; large text 3:1); QML lint requiring `Accessible.name`/`role` on interactive items; keyboard-only journey tests; reduced-motion snapshot variants.
- **Journeys**: scripted UI flows against a mock priyad (deterministic events) — e.g. "ask → streaming answer → approve → review → merge" — asserting screens and timings.
- **Design review**: every sub-phase with UI ends with the owner viewing the component gallery or a recorded journey and approving (human gate), see [11-design-system-and-ux.md](11-design-system-and-ux.md).

## 8. Levels on real hardware

After CI is green, phases with hardware impact run an **L4 checklist** on the Zenbook (listed in the phase file). Results are recorded in the PR or the phase file. Hardware smoke tests that can be automated (suspend/resume, audio device present, Wi-Fi connects, external display) run on the self-hosted Zenbook runner on demand.

## 9. Safety rules for testing

- Development never merges, swaps, rolls back, restores or runs `doctor --fix` on the host's real root. Fake machine or VM only.
- No `mkfs`, `dd`, `wipefs`, `sgdisk`, `parted`, `cfdisk`, `grub-install`, `efibootmgr` on real devices outside the VM or the owner's guided install.
- Live model tests always run with hard budgets; CI secrets are scoped to the nightly live job only.
