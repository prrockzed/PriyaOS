# Phase 0 — Foundations

**Goal.** The skeleton every later phase plugs into: Cargo workspace, event store, module host, settings, public API, CLI shell, test harness and CI — plus a recorded picture of the owner's Zenbook.

**Depends on:** nothing.
**Features:** CORE-01, CORE-02, CORE-03, CORE-04, CORE-05.
**Read first:** [00-principles](../architecture/00-principles.md), [01-system-overview](../architecture/01-system-overview.md), [02-core](../architecture/02-core.md), [09-testing](../architecture/09-testing.md), [decisions](../architecture/decisions.md).

## Build

### 0.1 Zenbook survey (owner + Claude)
- [ ] Record model (`cat /sys/class/dmi/id/product_name`), CPU, RAM, disk layout (`lsblk -f`), root filesystem and options (`findmnt -no FSTYPE,SOURCE,OPTIONS /`), subvolumes if btrfs, bootloader (GRUB / systemd-boot), ESP size, Secure Boot state, Windows BitLocker state (owner checks in Windows)
- [ ] Write results into `docs/human/07-install-on-your-zenbook.md` §"Your laptop today" and decide: existing Arch usable for dogfooding (btrfs `@`/`@home`) or VM-only until Phase 9
- [ ] Install dev tools on the Zenbook's Arch: `base-devel git rustup qemu-full edk2-ovmf archiso arch-install-scripts btrfs-progs bubblewrap sqlite nodejs npm`, Claude Code; clone the repo

### 0.2 Workspace
- [ ] Cargo workspace with the crates from [01 §6](../architecture/01-system-overview.md) (empty where not yet needed), shared lints, `rust-toolchain.toml`
- [ ] Error type (`thiserror`) with `user_message`; `tracing` to journald/stderr

### 0.3 Event store (`priya-store`)
- [ ] Schema and migrations (`PRAGMA user_version`), append with hash chain, typed event enum in `priya-api` with versioning
- [ ] Projections framework (register projection, apply in same transaction, rebuild command)
- [ ] Subscriptions with filters and resume-from-seq
- [ ] Blob store (BLAKE3 + zstd, refcounts) — interface + local implementation
- [ ] `priya audit verify`

### 0.4 Settings (`priya-core` + `priyad`)
- [ ] Layer loading (defaults → /etc → ~/.config → project → task), typed structs with `schemars` JSON Schema export
- [ ] Hot reload via inotify; invalid file → keep last valid, emit error event
- [ ] `priya settings get/set/schema`

### 0.5 priyad, API, CLI
- [ ] `priyad` user service skeleton: socket at `$XDG_RUNTIME_DIR/priya/priyad.sock`, JSON-RPC 2.0 router, `v1.` namespace, error model
- [ ] `priya` CLI skeleton with `--json` on every read command, talking to priyad
- [ ] `priya doctor` (read-only checks: btrfs layout, fstab `subvolid`, `/boot` location, tools present, free space)

### 0.6 Module host
- [ ] `module.toml` parsing and validation, built-in and process kinds, stdio JSON-RPC with `Content-Length` framing
- [ ] Supervisor (start, health, restart with backoff, crash events), capability checks on host calls
- [ ] `priya-module-sdk` (Rust) + an `echo` example module + a Python example module to prove language independence
- [ ] `priya modules list/enable/disable`

### 0.7 Test harness and CI
- [ ] `scripts/make-fake-machine.sh` (btrfs loop image, pacstrapped minimal Arch, test user)
- [ ] CI: fmt, clippy, L0 on Linux + macOS
- [ ] Record btrfs `send --dump` fixtures on the Zenbook for Phase 1

## Tests

- L0: event hash chain, migrations, projection rebuild equals incremental, settings layering and validation, module manifest validation, JSON-RPC error mapping.
- L1: module host contract (built-in and process module behave identically).
- L2: `priya doctor` against the fake machine reports layout correctly.

## Done when

- [ ] `cargo test --workspace` green on Mac and Zenbook; CI green
- [ ] Events written by priyad survive restart; `priya audit verify` passes; tampering with a row is detected
- [ ] Echo module (Rust) and Python module both start, answer a call, emit an event, get restarted after a crash
- [ ] Settings change in a file is picked up live
- [ ] Zenbook survey recorded and path decided

**Demo.** `priya modules list`, `priya settings set ui.theme dark`, `priya audit verify`, kill the echo module and watch it restart.

**Out of scope.** Any btrfs operation, any model call.
