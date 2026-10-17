# 00 — Technical overview

Read this first. It is the map; the other files in `docs/claude/` are the specs.

## What we are building in week 1

A working prototype of PriyaOS on Arch Linux, ending in a bootable ISO that installs on the owner's ASUS Zenbook 14 OLED next to Windows.

Two demos must work end to end on real hardware:

1. **`priya run "<task>"`** — fork the machine (btrfs snapshots), run a coding agent (Claude Code first) inside the fork with full autonomy, show a diff of the whole machine, merge or discard.
2. **`priya update`** — run `pacman -Syu` on a fork, boot the fork in a container to check it, let an agent fix what broke, then offer the merge. Merging system changes takes effect on the next reboot; `priya rollback` undoes it.

Everything else in `VISION.md` (teach-by-showing, overnight "dreaming", phone approvals, generated mini-apps, full information-flow control) is **after week 1**. Do not start those until the week-1 build plan in [08-build-plan.md](08-build-plan.md) is fully checked off.

## Components

| Component | What it is | Spec |
|---|---|---|
| Fork engine | Creates, diffs, merges and deletes machine forks using btrfs | [01-fork-diff-merge.md](01-fork-diff-merge.md) |
| `priyad` | Root system daemon: task table, SQLite store, JSON-RPC socket | [02-daemon-and-cli.md](02-daemon-and-cli.md) |
| `priya` | CLI and review TUI for the user | [02-daemon-and-cli.md](02-daemon-and-cli.md) |
| Agent runner | Adapters for Claude Code / Codex, budgets, secrets masking, taint labels, checkers, `priya update` | [03-agents-and-policy.md](03-agents-and-policy.md) |
| Desktop | Hyprland keybind command bar, Waybar module, notifications, special workspace | [04-desktop.md](04-desktop.md) |
| Tools and memory | pacman hook that indexes installed programs, `priya mcp` server, task history search | [05-tools-and-memory.md](05-tools-and-memory.md) |
| Packaging and ISO | PKGBUILD, systemd units, archiso profile, `priya-install` | [06-packaging-and-iso.md](06-packaging-and-iso.md) |
| Tests | Unit, fake-machine integration, VM end-to-end, real laptop | [07-testing.md](07-testing.md) |

## Architecture

```
 user session (Hyprland)                                 root
┌──────────────────────────────────────┐   ┌──────────────────────────────────────────┐
│ priya CLI / review TUI               │   │ priyad (systemd service, root)            │
│ priya ask (fuzzel command bar)       │──▶│  JSON-RPC over /run/priya/priyad.sock     │
│ Waybar custom/priya module           │   │  task table + SQLite (/var/lib/priya)     │
│ priya-notify (systemd --user)        │◀──│  event stream                             │
└──────────────────────────────────────┘   │  ├─ fork engine  (btrfs CLI)              │
                                           │  ├─ runner       (systemd-nspawn)         │
                                           │  ├─ agents       (claude -p / codex exec) │
                                           │  └─ policy       (/etc/priya/policy.toml) │
                                           └──────────────────────────────────────────┘
                     Arch Linux · btrfs (@, @home, @log, @pkg, @priya) · GRUB · systemd
```

Rule: **the agent proposes, priyad decides.** The agent never runs on the live system. It only ever touches a fork, and only priyad can merge a fork into the live system, after the user approves.

## Technology choices

| Area | Choice | Why |
|---|---|---|
| Language | Rust (stable), one Cargo workspace | Systems work, one static-ish binary, good crates for TUI and async |
| Async | `tokio` | Daemon, socket, child processes |
| CLI | `clap` (derive) | Standard |
| TUI | `ratatui` + `crossterm` | Review screen |
| Text diff | `similar` | File diffs in review |
| Storage | SQLite via `rusqlite` (bundled), FTS5 for history | Simple, one file |
| IDs | ULID (`ulid` crate), shown shortened to 8 chars | Sortable |
| Config | TOML (`toml`, `serde`) | `/etc/priya/config.toml`, `/etc/priya/policy.toml` |
| IPC | JSON-RPC 2.0, newline-delimited, over a Unix socket | Easy to debug with `socat` |
| btrfs, nspawn, pacman | **Shell out to the real CLIs** (`btrfs`, `systemd-nspawn`, `machinectl`, `pacman`) | Transparent, debuggable, no unsafe ioctls in week 1 |
| Logging | `tracing` + journald | `journalctl -u priyad` |

## Repository layout (target)

```
PriyaOS/
├── CLAUDE.md                  rules for Claude (read every session)
├── VISION.md                  product vision
├── docs/
│   ├── claude/                these specs
│   └── human/                 simple explanations for the owner
├── Cargo.toml                 workspace
├── crates/
│   ├── priya-core/            pure logic: types, state machine, diff model, dump parser, policy. Builds and tests on macOS.
│   ├── priya-sys/             Linux-only wrappers: btrfs, nspawn, pacman, mounts. #[cfg(target_os = "linux")]
│   ├── priyad/                daemon binary
│   └── priya/                 CLI + TUI + `priya mcp` binary
├── packaging/
│   ├── arch/PKGBUILD
│   ├── systemd/               priyad.service, priya-notify.service (user)
│   ├── hooks/                 priya-tools.hook (pacman)
│   └── etc/                   default config.toml, policy.toml
├── desktop/                   hyprland.conf snippet, waybar module, fuzzel config
├── iso/                       archiso profile + priya-install
├── scripts/                   make-fake-machine.sh, vm-test.sh, dev helpers
└── tests/                     integration tests (Linux + root)
```

Keep everything that can be pure in `priya-core` so it is unit-testable on the Mac. `priya-sys` is the only crate that runs system commands.

## Machines

| Machine | Role |
|---|---|
| MacBook (Apple Silicon, macOS) | Planning, docs, writing code, `cargo test -p priya-core` |
| ASUS Zenbook 14 OLED (x86_64, Windows + Arch dual boot) | Builds, integration tests, QEMU VM tests, final install target |

Linux-only code cannot be run on the Mac. Integration and end-to-end tests run on the Zenbook, either by running Claude Code directly on the Zenbook's Arch install (preferred) or over SSH from the Mac. See [07-testing.md](07-testing.md).
