# PriyaOS — instructions for Claude

PriyaOS is an AI-native operating system built on Arch Linux. Its core: **Priya AI** routes every job to the best enabled model across and within model families; **Priya Orchestration** plans and runs agents (Claude Code, Codex, Gemini CLI, OpenCode, Priya's native agent…); work runs in user-chosen execution modes (read-only, **Priya Sandbox**, direct with safety snapshot); every change is a ChangeSet that **Priya Rewind** can undo. Vision: [VISION.md](VISION.md).

## Read before working

1. [docs/claude/README.md](docs/claude/README.md) — index
2. [docs/claude/architecture/00-principles.md](docs/claude/architecture/00-principles.md) — rules every design follows
3. [docs/claude/phases/README.md](docs/claude/phases/README.md) — phase workflow and the current phase
4. The current phase file and the architecture files it links

## How work is done

- Phase by phase: **plan → build → test → gate**. Don't start a phase before the previous gate passes. Tick checkboxes in the phase file as work lands.
- New feature idea → add a row to [docs/claude/features.md](docs/claude/features.md) with the next free ID and a phase, then mention it in that phase file and in `docs/human/04-everything-priyaos-will-do.md`.
- Design change → update the architecture file in the same commit, or add an ADR to `docs/claude/architecture/decisions.md`.
- Features go in **modules** unless the core lacks an extension point (principle P1). Every external dependency goes behind a port with a contract test suite (P2).

## Two audiences

- `docs/claude/` — precise technical specs; must match the code.
- `docs/human/` — for the owner, explain-like-I'm-five with everyday analogies, no unexplained jargon. When a phase finishes, update the status column in `docs/human/05-the-phases.md`.

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
- `cargo fmt` and `cargo clippy -- -D warnings` clean before each commit.
- Comments explain *why*, briefly.
- One commit per finished unit of work, imperative message, ending with the attribution line from the current session.
- Recorded fixtures go under `crates/*/tests/fixtures/` with a note on how they were recorded.
