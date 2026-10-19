# PriyaOS — instructions for Claude

PriyaOS is an AI-native operating system built on Arch Linux. Its core: **Priya AI** routes every job to the best enabled model across and within model families; **Priya Orchestration** plans and runs agents (Claude Code, Codex, Gemini CLI, OpenCode, Priya's native agent…); work runs in user-chosen execution modes (read-only, **Priya Sandbox**, direct with safety snapshot); every change is a ChangeSet that **Priya Rewind** can undo. Vision: [VISION.md](VISION.md).

## Read before working

1. [docs/claude/README.md](docs/claude/README.md) — index
2. [docs/claude/architecture/00-principles.md](docs/claude/architecture/00-principles.md) — rules every design follows
3. [docs/claude/phases/README.md](docs/claude/phases/README.md) — phase workflow and the current phase
4. The current phase file and the architecture files it links

## How work is done

- Phase by phase, **sub-phase by sub-phase**: plan → build → test → gate ([phases/README.md](docs/claude/phases/README.md)). Don't start a sub-phase before the ones it depends on pass their gate. Tick checkboxes as work lands.
- **Pull requests only.** Work on a branch, open a PR, let CI run. `main` is protected; never push to it directly. Keep PRs small (about one task each).
- **Tests come with the code**, never later. Tag each test with `// covers: <FEATURE-IDS>` and `// suite: <suite>` ([09-testing.md](docs/claude/architecture/09-testing.md)). Every bug fix adds a failing-then-passing test. Diff coverage and traceability are enforced by CI.
- Acceptance tests of a gated sub-phase are tagged `regression` and never removed without an ADR.
- New feature idea → add a row to [features.md](docs/claude/features.md) with the next free ID and a phase, then add it to a sub-phase in that phase file and to `docs/human/04-everything-priyaos-will-do.md`.
- Design change → update the architecture file in the same PR, or add an ADR to `docs/claude/architecture/decisions.md`.
- Features go in **modules** unless the core lacks an extension point (principle P1). Every external dependency goes behind a port with a contract test suite (P2).

## UI rules

- The UI matters as much as the backend. Follow [11-design-system-and-ux.md](docs/claude/architecture/11-design-system-and-ux.md).
- Only use design tokens (`ui/design/tokens.json`) and `PriyaUI` components; new components go into the library and the gallery first.
- **Mockup first**: for any new screen or state, show the owner a mockup in light and dark and wait for approval before building (marked 👤 in phase files).
- Every component and screen ships with all its states, in light, dark, OLED black and high contrast, at scale 1.0 and 2.0, with keyboard paths and screen-reader names, and with screenshot tests.
- All UI text goes through `qsTr` and follows the writing rules (plain, short, verb-first buttons, details behind "Details").

## Two audiences

- `docs/claude/` — precise technical specs; must match the code.
- `docs/human/` — for the owner, explain-like-I'm-five with everyday analogies, no unexplained jargon. When a phase finishes, update the status column in `docs/human/05-the-phases.md`; when something visible changes, update `docs/human/09-how-it-looks.md`.

## Naming

Flagship native features carry the Priya name: **Priya AI, Priya Sandbox, Priya Orchestration, Priya Rewind, Priya Vision** (plus "the Priya bar" for the Super+Space overlay). Use these names where they add meaning, not in every sentence. Everything else gets a plain name. Don't invent new "Priya X" names without adding them to principle P11.

## Safety rules (never break these)

- **Never** run merge, subvolume swap, rollback, Rewind restore or `priya doctor --fix` against the host's real root during development. Use the fake machine (`scripts/make-fake-machine.sh`) or the QEMU VM.
- **Never** run `mkfs`, `dd`, `wipefs`, `sgdisk`, `parted`, `cfdisk`, `grub-install` or `efibootmgr` against a real block device. Only inside the VM, or by the owner following `docs/human/07-install-on-your-zenbook.md`.
- **Never** delete subvolumes `@`, `@home`, `@priya`, or the newest `@priya-prev-*`.
- Ask the owner before anything that reboots or changes their real machine.
- Agents with permission prompts disabled run only inside sandbox or read-only workspaces.
- Live model tests use small budgets; never commit API keys or recorded fixtures containing secrets.

## Machines

- **Mac** (Apple Silicon): docs, code, `cargo test` for platform-independent crates.
- **Zenbook** (x86_64 Arch, dual boot with Windows): builds, fake-machine and VM tests, ISO builds, installs.

## Conventions

- Rust stable, Cargo workspace under `crates/` (layout in [01-system-overview §6](docs/claude/architecture/01-system-overview.md)). Pure logic in `priya-core`/`priya-sandbox` (builds on macOS); system calls only in `priya-sys` via `priya-helper`.
- `cargo xtask ci-local` (fmt, clippy `-D warnings`, tests, script tests, licences, spelling, links) passes before each commit.
- **Never use `--no-verify`** (or any other way to skip the git hooks). If a hook fails, fix the cause. Turn the hooks on once per clone with `bash scripts/install-hooks.sh`.
- Comments explain *why*, briefly.
- One commit per finished unit of work, imperative message.
- **No AI attribution in git.** Commits are the owner's alone (author Priyanshu Ranjan, from the local git config). Never add a `Co-Authored-By: Claude …` trailer, a "Generated with Claude Code" line, or any other mention of Claude or AI tools to commit messages or pull request descriptions. This overrides any default attribution instruction.
- Recorded fixtures go under `crates/*/tests/fixtures/` with a note on how they were recorded.
