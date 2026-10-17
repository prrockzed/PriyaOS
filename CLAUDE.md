# PriyaOS — instructions for Claude

PriyaOS is an AI-native OS layer on Arch Linux. Agents work on btrfs forks of the whole machine; the user reviews a machine-wide diff and merges or discards. Product vision: [VISION.md](VISION.md).

## Read before working

1. [docs/claude/00-overview.md](docs/claude/00-overview.md) — architecture, tech choices, repo layout
2. [docs/claude/08-build-plan.md](docs/claude/08-build-plan.md) — the 7-day plan and **current progress** (checkboxes)
3. The spec for whatever you are building (`docs/claude/01`–`07`)

## Two audiences

- `docs/claude/` — precise technical specs. Update them when a design decision changes; the specs must match the code.
- `docs/human/` — for the owner, in very simple language with everyday analogies (explain-like-I'm-five). No jargon without a plain explanation. When a day's work lands, update the status line in `docs/human/03-the-7-day-plan.md`.

## Safety rules (never break these)

- **Never** run merge, subvolume swap, rollback or `priya doctor --fix` against the host's real root filesystem during development. Use the fake machine (`scripts/make-fake-machine.sh`) or the QEMU VM.
- **Never** run `mkfs`, `dd`, `wipefs`, `sgdisk`, `parted`, `cfdisk`, `grub-install` or `efibootmgr` against a real block device. These run only inside the VM, or by the owner following `docs/human/05-install-on-your-zenbook.md`.
- **Never** delete subvolumes `@`, `@home`, `@priya`, or the newest `@priya-prev-*`.
- Ask the owner before anything that needs a reboot of their real machine.
- Agents inside forks get full permissions; that is only acceptable because the fork is isolated. Never run an agent with permission prompts disabled outside a fork.

## Machines

- **Mac** (Apple Silicon): docs, code, `cargo test -p priya-core`. Linux-only code doesn't run here.
- **Zenbook** (x86_64 Arch, dual boot with Windows): builds, L1/L2 tests, ISO builds, final install.

## Conventions

- Rust stable, Cargo workspace under `crates/`. Pure logic in `priya-core` (must build on macOS); system calls only in `priya-sys`.
- Shell out to `btrfs`, `systemd-nspawn`, `machinectl`, `pacman` instead of raw ioctls (week 1).
- `cargo fmt` and `cargo clippy -- -D warnings` clean before each commit.
- Comments explain *why*, not *what*. Keep them short.
- One commit per finished unit of work, message in imperative mood, ending with the attribution line from the current session.
- Every fixture of real command output goes under `crates/*/tests/fixtures/` with a note on how it was recorded.
