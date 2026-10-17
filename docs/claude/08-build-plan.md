# 08 — Week-1 build plan

Seven days, one phase per day. Each day ends with its **done when** check passing and a commit. Tick boxes in this file as work lands (this file is the progress tracker; keep `docs/human/03-the-7-day-plan.md` in sync with a one-line status per day).

If a day slips, do not skip ahead: the order matters (each day builds on the last). Cut stretch items instead.

## Day 0 — Prepare the Zenbook (owner + Claude, ~1 hour)

- [ ] Record laptop model (`cat /sys/class/dmi/id/product_name`) and current layout: `findmnt -no FSTYPE,SOURCE,OPTIONS /`, `sudo btrfs subvolume list /` (if btrfs), `lsblk -f`, `bootctl status` or GRUB presence. Write findings into `docs/human/05-install-on-your-zenbook.md` §"Your laptop today".
- [ ] Install on the Zenbook's Arch: `git base-devel rustup qemu-full edk2-ovmf archiso arch-install-scripts btrfs-progs bubblewrap sqlite nodejs npm`; `rustup default stable`; Claude Code; clone the repo.
- [ ] Decide path A (existing Arch is btrfs `@`/`@home` → use it from Day 6) or path B (ext4 or other → test only in the VM until the Day 7 ISO install).

## Day 1 — Skeleton, doctor, fake machine

- [ ] Cargo workspace with `priya-core`, `priya-sys`, `priyad`, `priya` (layout in [00](00-overview.md))
- [ ] Config loading (`--config`), tracing, error type
- [ ] `priya doctor` (read-only checks from [01 §1](01-fork-diff-merge.md) and [01 §6](01-fork-diff-merge.md))
- [ ] `scripts/make-fake-machine.sh`
- [ ] CI workflow running L0
- [ ] Record btrfs dump fixtures on the Zenbook

**Done when:** `cargo test -p priya-core` passes on Mac and Zenbook; `sudo priya doctor --config fake/config.toml` reports the fake machine correctly; CI is green.

## Day 2 — Fork and diff

- [ ] Fork create / discard ([01 §2](01-fork-diff-merge.md), [01 §7](01-fork-diff-merge.md))
- [ ] nspawn runner for commands ([01 §3](01-fork-diff-merge.md)) incl. secrets masking and fork marker
- [ ] Dump parser + classifier + ignore list ([01 §4.1](01-fork-diff-merge.md)) with fixture tests
- [ ] Summary: packages, services ([01 §4.2](01-fork-diff-merge.md))
- [ ] Temporary direct mode (no daemon yet): `sudo priya run --cmd ... --config fake/config.toml` → prints diff

**Done when:** on the fake machine, `run --cmd "pacman -S --noconfirm jq && echo hi > ~/x"` prints a correct summary (package `jq`, file `~/x`), and the live fake machine is unchanged.

## Day 3 — Daemon, task table, merge

- [ ] priyad: socket, peer-cred auth, SQLite + migrations, state machine, queue ([02](02-daemon-and-cli.md))
- [ ] CLI talks to priyad: `run --cmd`, `ps`, `logs -f`, `diff`, `merge`, `discard`, `kill`, `gc`
- [ ] Home merge with 3-way conflicts and reflinks ([01 §5.1](01-fork-diff-merge.md))
- [ ] System merge with drift check, swap, prev-roots retention; `rollback` ([01 §5.2](01-fork-diff-merge.md))
- [ ] Journal for live changes; startup recovery report
- [ ] L1 integration scenarios from [07](07-testing.md)

**Done when:** all L1 scenarios pass on the Zenbook against the fake machine.

## Day 4 — Agents, budgets, policy, update

- [ ] Claude Code adapter (record fixture first), Codex adapter
- [ ] Prompt preamble, fork-only sudoers file
- [ ] Budgets (time, USD), policy file, taint rules ([03](03-agents-and-policy.md))
- [ ] Checkers (`--check` + auto-detect)
- [ ] `priya update` with boot check and fix loop ([03 §6](03-agents-and-policy.md))
- [ ] Review TUI ([02](02-daemon-and-cli.md))

**Done when:** on the fake machine, `priya run "create a hello-world Rust CLI in ~/hello and make cargo test pass"` ends in `review` with a passing check, merges, and `~/hello` exists in live `@home`; `priya update --no-fix` produces a reviewable fork with upgraded packages and a boot-check result.

## Day 5 — Desktop

- [ ] `priya ask` with fuzzel; special workspace log window ([04](04-desktop.md))
- [ ] `priya status --waybar` + module file
- [ ] `priya-notify` user service with actions
- [ ] `priya desktop install`
- [ ] Build the first ISO (even if rough) and boot it in QEMU to shake out packaging problems early

**Done when:** inside the L2 VM (or the real Arch on path A, against the fake machine), SUPER+SPACE → prompt → notification → Waybar shows review → click opens the review TUI.

## Day 6 — Tools, memory, package, dogfood

- [ ] pacman hook + `priya tools index` + `priya tools index --all` ([05](05-tools-and-memory.md))
- [ ] `priya mcp` server with the 4 read-only tools; read-only socket for forks
- [ ] History FTS + `priya history`
- [ ] PKGBUILD, systemd units, sysusers, tmpfiles ([06 §1](06-packaging-and-iso.md))
- [ ] `priya doctor --fix` (creates `@priya`, fixes fstab)
- [ ] Path A: install on the real Zenbook Arch and run L3 items that don't need the ISO

**Done when:** `makepkg -si` + doctor works in the VM; an agent inside a fork can call `search_tools("json")` and find `jq`.

## Day 7 — ISO, installer, real install

- [ ] archiso profile + local repo + `scripts/build-iso.sh` ([06 §2](06-packaging-and-iso.md))
- [ ] `priya-install` with dual-boot safety checks ([06 §3](06-packaging-and-iso.md))
- [ ] `scripts/vm-test.sh` full L2 run passes, including fake-Windows GRUB entry, reboot, rollback
- [ ] Owner flashes USB and installs on the Zenbook following `docs/human/05-install-on-your-zenbook.md`
- [ ] L3 checklist
- [ ] Record a 2-minute demo of both demos; update README

**Done when:** the Zenbook boots PriyaOS from GRUB, Windows still boots, and both demos work on the real machine.

## After week 1 (not now)

Phone approvals (ntfy), egress filtering per fork, per-value information-flow control, opt-in activity journal, teach-by-showing, overnight "dreaming", generated mini-apps, custom shell (Quickshell), signed packages and Secure Boot, snapper-style GRUB rollback entries, multi-agent parallel forks of the same project with conflict detection.
