# 07 — Testing

Four levels. Each feature must pass the lowest level that can test it before moving up.

| Level | Where | What | Command |
|---|---|---|---|
| L0 Unit | Mac or Linux | Pure logic in `priya-core`: state machine, dump parser, classifier, 3-way merge decisions, policy parsing, Waybar JSON, agent stream parsers | `cargo test -p priya-core` |
| L1 Fake machine | Zenbook (Linux, root) or CI | Fork/diff/merge against a btrfs **loop image**, never the real disk | `sudo scripts/test-fake-machine.sh` |
| L2 VM | QEMU/KVM on the Zenbook | Built ISO → install into a VM disk → scripted scenarios incl. reboot and rollback | `scripts/vm-test.sh out/priyaos-*.iso` |
| L3 Real laptop | Zenbook, after L2 passes | Dogfooding on the real install | manual checklist below |

## L0 — Unit tests

- Record real `btrfs receive --dump` outputs on the Zenbook (create files, renames, deletes, symlinks, chmods in a loop image) and commit them as fixtures. Parser tests use those fixtures.
- Agent parsers: record real `claude -p ... --output-format stream-json --verbose` and `codex exec --json` output once, commit as fixtures (strip anything personal).
- 3-way merge decision table from [01 §5.1](01-fork-diff-merge.md) as a table-driven test.

## L1 — Fake machine

`scripts/make-fake-machine.sh <dir>`:
1. `truncate -s 6G <dir>/disk.img`, `mkfs.btrfs`, loop-mount at `<dir>/top`.
2. Create `@`, `@home`, `@priya`, `@priya-forks`.
3. Fill `@` with a real minimal Arch via `pacstrap -c <dir>/top/@ base` (Arch host) so nspawn and pacman work; create a user `tester` with a home in `@home`.
4. Write `<dir>/config.toml` pointing `top_mount`, subvolume names and the db path at the fake machine.

priyad and the CLI accept `--config <path>` so tests run a private priyad on a private socket against the fake machine.

Scenarios (Rust integration tests in `tests/`, `#[ignore]` unless `PRIYA_IT=1` and root):
- `run --cmd "touch ~/a && echo hi > ~/b"` → diff shows 2 added home files → merge → files exist in live `@home`.
- Conflict: fork edits `~/x`, then the test edits live `~/x` → merge reports conflict, live file untouched.
- `run --cmd "pacman -S --noconfirm jq"` (uses host pkg cache) → summary shows package `jq` → system merge swaps `@`, `@priya-prev-*` exists → rollback swaps back.
- Drift: change a file in live `@` after fork → system merge refused.
- Discard deletes all fork subvolumes.
- Secrets: `~/.ssh/id_test` exists → not visible inside the fork.
- Budget: `run --cmd "sleep 600" --budget 5s` → failed with `time budget`.

CI (`.github/workflows/ci.yml`): L0 on every push (ubuntu-latest and macos-latest). L1 subset on ubuntu-latest with sudo, using a busybox-based fake root instead of pacstrap (skip pacman scenarios).

## L2 — VM

`scripts/vm-test.sh <iso>` on the Zenbook:
- Requires `qemu-full`, `edk2-ovmf`, KVM.
- Creates a 40 GiB qcow2 disk **with a pre-made fake Windows layout** (ESP 260 MiB vfat + an NTFS partition labelled `Windows`) so the dual-boot path of `priya-install` is exercised.
- Boots the ISO with OVMF (UEFI), runs `priya-install` unattended via `PRIYA_INSTALL_ANSWERS=<file>` (an env var only honored in the live ISO), reboots into the installed system.
- Over SSH (port-forward 2222) runs: `priya doctor`, the L1 scenarios against the real root, `priya update --no-fix` (if network), merge, reboot, verify, `priya rollback`, reboot, verify.
- Checks the GRUB menu lists the fake Windows entry (os-prober output in `grub.cfg`).

## L3 — Real laptop checklist

Only after L2 passes. Before starting, a manual snapshot: `sudo btrfs subvolume snapshot -r / /.priya-manual-<date>` (if already on btrfs) and the user has their BitLocker recovery key.

- [ ] `priya doctor` all green
- [ ] SUPER+SPACE → "install htop and add an alias top=htop to my zshrc" → review → merge → works after reboot
- [ ] A real coding task in a real repo, merged
- [ ] `priya update` → reboot → system works → `priya rollback` → reboot → previous state
- [ ] Windows still boots from the GRUB menu
- [ ] Wi-Fi, audio, brightness, suspend/resume still work

## Safety rules for testing (also in CLAUDE.md)

- Never run merge, swap, rollback or `doctor --fix` against the **host** root while developing. Use the fake machine or the VM.
- Never run `mkfs`, `dd`, `wipefs`, `sgdisk`, `parted` or `grub-install` on a real device. These only ever run inside the VM or the ISO installer, and on the real laptop only by the user.
