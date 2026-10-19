# Phase 2 — Priya Sandbox

**Goal.** Make every change reviewable and reversible. The tiny root helper forks the whole machine (system + home) with btrfs snapshots, runs any command inside the fork, computes a machine-wide ChangeSet, and merges it (home live, system on reboot) with three-way conflict handling and rollback. Read-only and direct (safety-snapshot) modes use the same code path. The owner's Zenbook moves onto the PriyaOS disk layout, tested first in a VM with a fake Windows install.

**Usable at the end.**
- The Zenbook runs the PriyaOS disk layout, reached either by `priya doctor --fix` on an existing suitable btrfs Arch or by the dev installer (minimal ISO, tested in a VM with a fake Windows partition, then installed by the owner next to Windows following the guide). Windows still boots.
- `priya sandbox run --cmd "…" --mode read_only|sandbox|direct` works for any command; `priya sandbox shell` opens an interactive shell inside a fork.
- `priya review` shows the ChangeSet in a terminal UI (plain summary, groups, diffs, conflicts); the same diff components are in the component gallery.
- Merge (home live, system on reboot), conflict resolution, direct-mode keep/revert, `priya rollback`.
- CI runs fake-machine integration tests on every PR; nightly VM end-to-end, performance and chaos suites run.

**Depends on:** Phase 1 gate (event store, blob store, settings, `priya doctor` read-only checks, testkit and fake-machine handle, component gallery from Phase 0; Priya AI from Phase 1 for "ask Priya to merge the text").

**Features:** CORE-06, SBX-01, SBX-02, SBX-03, SBX-04, SBX-05, SBX-06, SBX-08, SBX-09, SEC-01, UX-01, OS-15, QA-15, QA-16, QA-17, QA-18, QA-19.

**Read first:**
- [03 — Execution modes and Priya Sandbox](../architecture/03-execution-and-sandbox.md) (primary)
- [01 — System overview §2 processes, §5 storage, §6 crates](../architecture/01-system-overview.md)
- [02 — Core §1 events, §4 API](../architecture/02-core.md)
- [05 — Security §1 threat model, §3 secrets (static mask list)](../architecture/05-security.md)
- [09 — Testing](../architecture/09-testing.md) (fake machine, VM harness, perf, chaos, self-hosted runner, §9 safety rules)
- [10 — Base OS and distro §3 installer, §4 Zenbook](../architecture/10-base-os-and-distro.md)
- [11 — Design system §4 Review, §7 accessibility, §9 process](../architecture/11-design-system-and-ux.md)
- [decisions.md](../architecture/decisions.md): ADR-004, ADR-005, ADR-008, ADR-013
- [docs/human/07-install-on-your-zenbook.md](../../human/07-install-on-your-zenbook.md)

> **Safety (CLAUDE.md).** During development nothing in this phase runs merge, swap, rollback or `doctor --fix` against the host's real root, and no `mkfs`/`dd`/`wipefs`/`sgdisk`/`parted`/`grub-install`/`efibootmgr` touches a real block device. Everything is built and tested on the fake machine or in the QEMU VM. Steps marked 👤 are performed by the owner on the Zenbook, following the guide, after the VM tests are green.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 2.1 | Root helper | Phase 1 | `priya-helper` runs as a hardened root service; `priya doctor` shows it healthy |
| 2.2 | Snapshot backend | Phase 1 | `SnapshotBackend` btrfs adapter passes its contract suite on a loop image |
| 2.3 | Fake machine in CI | 2.1, 2.2 | Every PR runs integration tests against a pacstrapped btrfs fake machine; Zenbook runner online |
| 2.4 | Disk-layout doctor and fixes | 2.2, 2.3 | `priya doctor` explains exactly what the layout lacks; `--fix --dry-run` shows the plan |
| 2.5 | Dev installer and moving the Zenbook onto the layout | 2.4 | Dev ISO installs PriyaOS next to (fake) Windows in a VM; then the owner moves the Zenbook 👤 |
| 2.6 | Fork lifecycle | 2.1, 2.2, 2.3 | `priya sandbox fork create/list/discard` creates a whole-machine fork in < 2 s |
| 2.7 | Sandbox runner | 2.6 | `priya sandbox run --cmd` and `priya sandbox shell` run inside a fork |
| 2.8 | Booted runs (boot check) | 2.7 | `priya sandbox boot-check` boots a fork and reports new failed units |
| 2.9 | ChangeSet engine I (paths, classification, blobs) | 2.6, 2.7 | Every sandbox run produces an exact list of changed paths with before/after blobs |
| 2.10 | ChangeSet engine II (summaries) | 2.9 | `priya change show` prints "Installed jq. Changed 2 files in project api." |
| 2.11 | Home merge and conflicts | 2.10 | Home changes merge live; conflicts resolved as mine / agent's / both / ask Priya |
| 2.12 | System merge and rollback | 2.8, 2.11 | System changes apply on reboot; `priya rollback` returns to the previous root |
| 2.13 | Read-only and direct modes | 2.7, 2.9, 2.11 | `--mode read_only` can't write; `--mode direct` keeps or reverts from a safety snapshot |
| 2.14 | Review TUI and diff components | 2.10, 2.11, 2.12, 2.13 | `priya review` in the terminal; diff components in the gallery |
| 2.15 | Chaos and performance suites | 2.5, 2.12, 2.13 | Nightly VM e2e, perf budgets and crash-at-every-step chaos tests |
| 2.16 | Phase gate | all | Owner demo on the Zenbook |

---

### 2.1 — Root helper
**Outcome:** `priya-helper` runs as the only root component with no network, accepts a fixed set of validated operations from priyad over a Unix socket, journals every live-changing step, and `priya doctor` reports it healthy.
**Features:** CORE-06, SEC-01.
**Depends on:** Phase 1 gate.

**Tasks**
- [ ] Helper protocol types in `priya-api::helper`: JSON-RPC 2.0, `helper.hello {protocol_version}` handshake, one method per operation (`helper.fork.*`, `helper.run.*`, `helper.changeset.scan`, `helper.merge.*`, `helper.revert.apply`, `helper.system.*`, `helper.doctor.fix`, `helper.journal.*`); every request carries `op_id` (ULID) for idempotency; schemas exported by `cargo xtask schemas` into `docs/claude/api/helper/`.
- [ ] `priya-helper` binary skeleton: socket-activated `priya-helper.socket` (`/run/priya/helper.sock`, `SocketMode=0660`, group `priya`) + `priya-helper.service`; `SO_PEERCRED` authentication; request size limit (1 MiB), per-uid concurrency limit, structured errors with `user_message`.
- [ ] systemd hardening in the unit: `PrivateNetwork=yes`, `RestrictAddressFamilies=AF_UNIX`, `IPAddressDeny=any`, `NoNewPrivileges=yes`, `PrivateMounts=yes` (the top-level mount at `/run/priya/top` is invisible to users), `ProtectKernelModules=yes`, `ProtectClock=yes`, minimal `CapabilityBoundingSet=` (CAP_SYS_ADMIN, CAP_DAC_OVERRIDE, CAP_FOWNER, CAP_CHOWN, CAP_SETUID, CAP_SETGID, CAP_KILL), `SystemCallArchitectures=native`. A unit test parses the shipped unit file and fails if any hardening line is removed.
- [ ] Invariants module (pure, in `priya-sandbox::invariants`, builds on macOS): never delete `@`, `@home`, `@priya`, `@priya-forks` (the container), or the newest `@priya-prev-*`; home operations only inside the caller's home; fork ids must belong to the caller (`meta.json` uid); system operations only for members of `priya-admin`; path canonicalisation with `openat2(RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS)` semantics and rejection of `..`, absolute escapes and symlink escapes; free-space floor (refuse new forks below 5 %).
- [ ] Helper journal in `/var/lib/priya/system.db` (SQLite WAL, root 0600): `journal(op_id, op, params_json, step, state: started|step_done|done|failed, ts)`; `JournalStep` enum lists every live-changing step so chaos tests can enumerate them; write before and after each step.
- [ ] Startup recovery: mount the btrfs top level (subvolid 5) at `/run/priya/top` inside the helper's mount namespace; read unfinished journal entries and expose them via `helper.journal.unfinished`; **report, never auto-retry**; priyad turns them into a plain-language notice in `priya doctor` and a `helper.recovery_needed` event.
- [ ] Development target guard: `/etc/priya/helper.toml` has `target = "host" | "<mountpoint>"`. Debug builds and test builds refuse `target = "host"` unless `/etc/priya/allow-host-target` exists; the dev installer (2.5) and `doctor --fix` (2.4) create it only on the owner's confirmed machine or in the VM. Integration tests point the helper at the mounted fake machine.
- [ ] `priyad` side: `HelperClient` (typed, timeouts, reconnect, version check) and a fake helper in `priya-testkit` that records calls, so priyad logic is tested on macOS.
- [ ] Packaging: `sysusers.d/priya.conf` (groups `priya`, `priya-admin`), `tmpfiles.d/priya.conf` (`/var/lib/priya` 0755 root, `/run/priya`), unit files in the `priya` package; `priya doctor` checks helper reachable, protocol version, unit hardening present, caller's group membership.
- [ ] Docs: helper operations table and the `priya` group added to [01 §2](../architecture/01-system-overview.md); new event `helper.recovery_needed` in [02 §1.1](../architecture/02-core.md).

**Tests**
- Unit: invariant table (every protected subvolume × every delete/rename op → refused); path canonicaliser against a corpus of escapes (`../`, `//`, symlink chains, `/proc/self/root`, NUL bytes, overlong paths); journal state machine; request-size and concurrency limits; unit-file hardening parser.
- Contract: helper protocol round-trip for every method with the fake helper and the real helper (schema-checked).
- Integration (fake machine): peer-credential auth (uid not in `priya` → refused; not in `priya-admin` → system ops refused); helper has no network (`curl` from within the helper's namespace fails; `getsockopt` shows no AF_INET); target guard refuses `host` in a debug build; kill the helper after `step_done` of a fake two-step op → restart reports exactly one unfinished entry and does nothing else.
- Security: fuzz the JSON-RPC decoder (`cargo fuzz`, 10 min nightly); request to delete `@home` via every op → refused with `user_message`; request touching another user's home → refused.
- Regression: `helper_refuses_protected_subvolume_delete`, `helper_refuses_other_users_home`, `helper_has_no_network`, `helper_reports_unfinished_journal_without_retry`, `helper_unit_hardening_present`.

**Done when**
- [ ] No privileged operation exists outside `priya-helper` (grep-based CI check: `priya-sys` is a dependency only of `priya-helper` and test crates)
- [ ] All invariant and security tests pass; fuzzing runs nightly with no crash
- [ ] `priya doctor` shows "Helper: running, protocol v1, hardened" on the fake machine

---

### 2.2 — Snapshot backend
**Outcome:** a `SnapshotBackend` port with a btrfs adapter and an in-memory fake, both passing the same contract suite; recorded dump fixtures for the ChangeSet engine.
**Features:** infrastructure (serves SBX-01, SBX-02, SBX-05).
**Depends on:** Phase 1 gate.

**Tasks**
- [ ] `SnapshotBackend` trait in `priya-sandbox::ports`: `create_snapshot(src, dst, readonly)`, `delete(subvol)`, `generation(subvol)`, `subvol_id(path)`, `list(prefix)`, `set_readonly(subvol, bool)`, `rename(src, dst)`, `find_new(subvol, since_gen) -> Vec<PathBuf>`, `send_dump(parent, child) -> impl Read`, `free_space() -> {bytes, ratio}`.
- [ ] btrfs adapter in `priya-sys::btrfs` wrapping `btrfs-progs` (`btrfs subvolume snapshot|delete|show|list|find-new`, `btrfs property set ro`, `btrfs send --no-data -p … | btrfs receive --dump`, `btrfs filesystem usage -b`); output parsers are pure functions in `priya-sandbox` with fixtures.
- [ ] `FakeSnapshotBackend` in `priya-testkit` (in-memory tree with generations) so fork/ChangeSet logic is unit-tested on macOS.
- [ ] Contract suite macro `snapshot_backend_contract!` (create/delete/rename/readonly/generation monotonicity/find-new/dump ordering/error mapping) run against both adapters.
- [ ] Record `btrfs receive --dump` fixtures in a loop image on the Zenbook (never the real root): add/modify/delete, rename (file and dir), hardlink, symlink, xattr/ACL, chmod/chown, sparse file, large file, file↔dir type change, special characters and Devanagari in names; each fixture dir gets `RECORDED.md` (kernel, btrfs-progs versions, script used).
- [ ] Error mapping to `user_message` ("Not enough free space to make a safe copy (3 % free, need 5 %)").

**Tests**
- Unit: output parsers for `subvolume show/list`, `find-new`, `filesystem usage`; fake backend semantics.
- Contract: `snapshot_backend_contract!` on `FakeSnapshotBackend` (Mac + Linux) and btrfs on a loop image (Linux CI).
- Integration (fake machine): snapshot of a 2 GiB `@` completes < 1 s; read-only snapshot rejects writes.
- Regression: the full contract suite for both adapters.

**Done when**
- [ ] Both adapters pass the identical contract suite in CI
- [ ] Fixtures committed with `RECORDED.md`; no fixture contains secrets (pre-commit scan green)

---

### 2.3 — Fake machine in CI
**Outcome:** every pull request runs sharded integration tests against a pacstrapped btrfs fake machine with the full PriyaOS layout; the Zenbook is registered as a self-hosted runner for KVM and hardware jobs.
**Features:** QA-15, QA-19.
**Depends on:** 2.1, 2.2.

**Tasks**
- [ ] Extend `scripts/make-fake-machine.sh` to the full [03 §2](../architecture/03-execution-and-sandbox.md) layout: `@`, `@home`, `@log`, `@pkg`, `@priya`, `@priya-forks`, `@priya-timeline`; `/boot` inside `@` with GRUB files installed into the image (no `grub-install` to any real device); fstab with `subvol=` only; packages from a pinned Arch Linux Archive date; test user `tester` in groups `priya`, `priya-admin`; seeded home files including `~/.ssh/id_test` (dummy key) for mask tests.
- [ ] Pristine snapshots `@pristine-root` and `@pristine-home` inside the image; `FakeMachine::fresh()` in `priya-testkit` restores them by snapshot (< 1 s) so tests are isolated without re-copying the 6 GiB image.
- [ ] `FakeMachine` handle: mount the image via loop device, start `priya-helper` with `target = <mountpoint>` and `priyad` for the test user, `boot()` the image with `systemd-nspawn -b` for reboot simulation, tear down; logs attached to failures.
- [ ] `cargo xtask fake-machine build|shell|boot|clean` and `cargo xtask test integration --shard N/M`.
- [ ] CI job: GitHub-hosted Ubuntu runner, privileged `archlinux` container with loop devices; image cached as an artifact keyed by script hash + archive date; 4 shards; total < 10 min; failure artifacts (helper journal, priyad log, `btrfs subvolume list`).
- [ ] Self-hosted runner on the Zenbook (QA-19): dedicated Unix user `priya-ci` (member of `kvm`, **no sudo**), labels `self-hosted, linux, kvm, zenbook`; restricted to this repository's branches (no fork PRs); privileged jobs run only inside QEMU VMs, never against the host. 👤 Owner approves the runner registration token step.
- [ ] Runner runbook in `docs/claude/` (CI section of [09 §4](../architecture/09-testing.md)): how to re-register after reinstall (needed after 2.5), how to take it offline.

**Tests**
- Integration (fake machine): `fake_machine_layout_matches_spec` (subvolumes, mounts, fstab without `subvolid=`), `fake_machine_fresh_restores_pristine`, `fake_machine_boots_to_multi_user`.
- Contract: `FakeMachine` handle used by both the helper and priyad test harnesses.
- Regression: the three tests above.

**Done when**
- [ ] Integration job is a required check on `main`; median PR integration time < 10 min
- [ ] Image cache hit rate > 90 % on PRs that don't touch the script
- [ ] A manual workflow dispatch runs `scripts/vm-test.sh --smoke` on the Zenbook runner successfully

---

### 2.4 — Disk-layout doctor and fixes
**Outcome:** `priya doctor` explains in plain language whether this machine can run Priya Sandbox and what's missing; `priya doctor --fix` shows a step-by-step plan (dry run by default) and applies safe fixes through the helper.
**Features:** SBX-01 (layout prerequisite), SEC-01.
**Depends on:** 2.2, 2.3.

**Tasks**
- [ ] Layout checks (pure decision logic in `priya-sandbox::layout`, probes in `priya-sys`): btrfs root; `/` mounted with `subvol=@`; fstab and kernel cmdline without `subvolid=`; presence and mounts of `@home`, `@log`, `@pkg`, `@priya` (at `/var/lib/priya`); existence of `@priya-forks`, `@priya-timeline`; `/boot` inside `@` vs on the ESP (warn: kernels excluded from sandboxed system changes); GRUB vs systemd-boot (warn: kernel rollback unsupported, ADR-008); free space ≥ 10 %.
- [ ] Hazard checks: active swapfile inside `@` (btrfs can't snapshot a subvolume holding an active swapfile → propose `@swap`); nested subvolumes under `@` or `@home` (e.g. `/var/lib/machines`, `/var/lib/docker`, `/var/lib/portables`) that snapshots would silently skip; snapper / Timeshift / grub-btrfs present (may rename or prune subvolumes); `@` not being the default-mounted subvolume; compression and `noatime` options.
- [ ] Fix catalogue, each a typed `Fix {id, description, commands_preview, needs_reboot, reversible}` applied via `helper.doctor.fix {fix_id}` with journaling: create `@priya`, `@priya-forks`, `@priya-timeline`; move existing `/var/lib/priya` contents into `@priya` and add its fstab line; rewrite fstab removing `subvolid=` (backup `/etc/fstab.priya-bak`); set `rootflags=subvol=@` in `/etc/default/grub` and regenerate `grub.cfg`. Fixes that need moving data (`@log`, `@pkg`, `@swap`, `/boot` into `@`) are **guided only**: doctor prints the owner's commands, it never moves them itself.
- [ ] `priya doctor --fix` defaults to `--dry-run` output (plan with every command); applying needs `--apply` plus typed confirmation of the hostname; refuses while any fork, run or merge is active.
- [ ] The fixer refuses `target = host` unless `/etc/priya/allow-host-target` exists (2.1 guard); the guide (2.5) tells the owner when to create it.
- [ ] `v1.system.doctor {fix?}` API wired; `--json` output for scripts and the VM harness.
- [ ] Plain-language report format: "Your disk is ready for Priya Sandbox" / "2 things to fix before Priya Sandbox can work" with each item explained.

**Human in the loop**
- The owner sees every fix as a plain sentence plus the exact commands before anything changes, and confirms by typing the hostname. Guided fixes are never automated. Backup advice (manual read-only snapshot `@before-priya`) is printed before the first fix.

**Tests**
- Unit: layout decision table from fixture probes (archinstall default layout, Omarchy layout, ext4 root, systemd-boot + ESP `/boot`, swapfile in `@`, nested subvolumes, `subvolid=` in fstab and cmdline).
- Integration (fake machine): broken variants generated from the pristine image (`subvolid=` added, `@priya` missing, swapfile in `@`) → doctor reports exactly those; `--fix --apply` repairs the auto-fixable ones; a second run reports clean; kill the helper mid-fix → recovery report, fstab either old or new (never partial).
- E2E (VM): doctor `--fix` on a VM installed with the archinstall default btrfs layout → reboot → doctor clean → sandbox smoke run.
- Security: doctor refuses host target in debug builds; refuses while a run is active.
- Regression: `doctor_detects_subvolid_in_fstab`, `doctor_detects_swapfile_in_root`, `doctor_detects_nested_subvolumes`, `doctor_fix_is_dry_run_by_default`, `doctor_fix_repairs_fake_machine`.

**Done when**
- [ ] Every check in the catalogue has a passing positive and negative fixture
- [ ] `doctor --fix --apply` turns the archinstall-default VM into a clean PriyaOS layout and the VM reboots fine
- [ ] Owner reads one doctor report and one dry-run plan and finds them understandable 👤

---

### 2.5 — Dev installer and moving the Zenbook onto the layout
**Outcome:** a minimal dev ISO installs Arch + the PriyaOS disk layout + `priya` next to Windows and other Linux systems, either into free space or onto a chosen partition. It is proven in a VM with a fake Windows and a fake existing Arch, then the owner adds PriyaOS to the Zenbook as a third system (Path C, decided in 0.1) following the guide.
**Features:** OS-15, QA-16.
**Depends on:** 2.4.

**Tasks — installer and ISO**
- [ ] Installer planning logic in a new crate `priya-install` (pure `plan` module builds on macOS): input = `lsblk -J -O` + `blkid` + ESP listing fixtures; output = `InstallPlan {target, esp, steps, refusals}`.
- [ ] Windows and safety detection: ESP (type GUID `C12A7328-…`), MSR (`E3C9E316-…`), NTFS, BitLocker (`blkid TYPE=BitLocker` / `-FVE-FS-` signature), Windows recovery (`DE94BBA4-…`), `EFI/Microsoft/Boot/bootmgfw.efi` on the ESP, the installer USB itself, mounted partitions.
- [ ] Existing Linux detection: every Linux filesystem is mounted read-only in a private mount namespace and its `/etc/os-release` read, so existing systems are shown by name ("Arch Linux, 500 GiB") and **locked** like Windows. Their swap partitions are never added to the new fstab (they may hold another system's hibernation image).
- [ ] Free-space targets: GPT gaps of at least 40 GiB are listed as targets. Installing there creates one new partition (`sgdisk --new` inside the gap, type Linux root x86-64) and nothing else. The installer never moves, resizes or deletes a partition; making room (for example shrinking Windows from Windows' own Disk Management) is a guided step the user does before installing.
- [ ] Refusal rules (each with a plain-language reason): target is ESP, MSR, NTFS, BitLocker, Windows recovery, the boot USB, mounted, or < 40 GiB; ESP has < 16 MiB free; more than one ESP and none chosen explicitly; target is a detected Linux system, or of unknown type, and not explicitly confirmed with `--erase-existing-linux <name>`.
- [ ] ESP choice when there are several: prefer the ESP that already holds a Linux loader (`EFI/GRUB`, `EFI/arch`, `EFI/systemd`), so Windows' ESP is left alone; ask when it's ambiguous. Only `EFI/PriyaOS/` is ever written; other loaders' folders are never touched.
- [ ] Installer TUI/CLI `priya-install`: text picture of the disk with Windows, other Linux systems and EFI marked **locked**; free space shown as **new partition here**; an existing partition target shown as **will be erased**; ESP shown as **shared, not erased**; `--dry-run` prints the full plan with every command and makes zero writes; applying requires typing the target exactly (the partition name, e.g. `nvme0n1p5`, or `free-1` for a free-space target).
- [ ] Execution (journaled to `/var/log/priya-install.log`, copied into the new system): `mkfs.btrfs -L priyaos` on the target only; create `@`, `@home`, `@log`, `@pkg`, `@containers`, `@machines`, `@priya`, `@priya-forks`, `@priya-timeline`; zram swap (no swapfile in `@`); mount with `compress=zstd:1,noatime,subvol=`; `pacstrap` base set + `priya`, `priya-helper` and Phase 0–1 packages from a bundled local repo; `genfstab -U` post-processed to remove `subvolid=`, and `subvol=` from the `/` entry (root comes from the kernel's `rootflags`, so previous systems can boot — see 2.12); ESP mounted at `/efi` (never formatted); `grub-install --target=x86_64-efi --efi-directory=/efi --bootloader-id=PriyaOS`; `GRUB_DISABLE_OS_PROBER=false` so the menu lists Windows and other Linux systems; `GRUB_DEFAULT=saved` + `GRUB_SAVEDEFAULT=true` so the menu starts the system chosen last time; `grub-mkconfig`; user in `wheel`, `priya`, `priya-admin`; enable `NetworkManager`, `priya-helper.socket`, `priyad` (user); minimal Hyprland + kitty + Priya bar; `/etc/priya/allow-host-target` created (this is now the owner's PriyaOS machine).
- [ ] Unattended mode for tests: `priya-install --unattended install.toml` is refused unless `systemd-detect-virt` reports a VM **and** the target disk serial matches the TOML.
- [ ] archiso profile `iso/dev/` (based on `releng`) with the local `[priyaos-dev]` repo, `priya-install`, and a rescue command `priya rollback --offline --device <partition>` (mounts the top level, swaps `@` with the newest `@priya-prev-*`; the last-resort rescue even after the previous-system boot entry lands in 2.12). `cargo xtask iso dev` builds `priyaos-dev-<date>-x86_64.iso` on the Zenbook or CI.

**Tasks — VM proof (before any real hardware)**
- [ ] `scripts/vm-test.sh` fake-Windows disk: GPT qcow2 with 260 MiB ESP containing `EFI/Microsoft/Boot/bootmgfw.efi` (dummy), 16 MiB MSR, NTFS "Windows" with sentinel files, a partition carrying a BitLocker `-FVE-FS-` header, Windows recovery partition, a 60 GiB ext4 "existing Arch" with a bootable kernel, its own GRUB in `EFI/GRUB` and sentinel files, and 60 GiB of free space; an OVMF NVRAM entry "Windows Boot Manager" created inside the VM.
- [ ] Install journey (nightly, QA-16): boot ISO in QEMU/OVMF → unattended install into the free space → reboot → GRUB shows PriyaOS, Arch and Windows Boot Manager → PriyaOS boots → `priya doctor --json` all green → `priya ask` works with the fake provider → reboot, pick Arch, it boots → reboot, the menu preselects Arch (saved default).
- [ ] Replace journey: unattended install onto the existing Arch partition with `--erase-existing-linux` (Path B, for other users).
- [ ] Untouched assertions: SHA-256 of the MSR, NTFS, BitLocker, recovery **and existing Arch** partitions identical before/after the free-space install; ESP tree identical except the new `EFI/PriyaOS/`; NVRAM still contains the Windows Boot Manager and Arch GRUB entries; the partition table differs only by the one new partition.
- [ ] Negative journeys: choosing ESP, NTFS, BitLocker, recovery or the USB → refused with zero writes (whole-disk hash unchanged); wrong confirmation text → aborted with zero writes; power cut (QMP `quit`) after `mkfs` → re-run of the installer from the start succeeds.

**Tasks — moving the Zenbook (👤 owner-performed)**
- [x] Path decision 👤, made in 0.1 from the private survey. Paths: **A** (existing Arch is btrfs and doctor shows only auto-fixable or guided items) → `priya doctor --fix`; **B** → dev installer replacing an old Linux partition; **C** → dev installer into free space as an extra system. **The owner takes Path C**: their Arch is their daily system and stays untouched; they shrink the unused Windows by about 200 GB and PriyaOS goes into that space.
- [ ] Update the guide `docs/human/07-install-on-your-zenbook.md` with a new "Dev install (Phase 2)" section in plain language (the Phase 9 section stays), covering Path C in full and Paths A and B briefly. Owner reads it end to end and approves before any step 👤.
- [ ] Guide checklist, every item 👤:
  1. Back up everything needed from the current Arch (projects not on GitHub, `~/.config`, documents) to a USB drive or cloud, and check the backup opens on another machine. Path C doesn't erase Arch, but it changes the partition table and the shared ESP, so a backup is still required.
  2. If Windows uses BitLocker: get the **recovery key** from account.microsoft.com/devices/recoverykey, write it down and photograph it.
  3. In Windows: turn off Fast Startup; if BitLocker is on, **suspend protection** for one restart. Path C: in **Disk Management**, right-click C: → **Shrink Volume** by about 200 GB (204800 MB), leaving the space **unallocated** (don't create a partition in it). If Windows offers less, temporarily turn off hibernation (`powercfg /h off`) and System Protection, restart and try again.
  4. Save `lsblk -f` and `efibootmgr -v` output (phone photo or USB) as the "before" picture.
  5. Secure Boot off in the ASUS firmware (F2 → F7 → Security).
  6. Path A: take a manual read-only snapshot of `@` at the btrfs top level named `@before-priya` (exact commands in the guide), install the `priya` package, run `sudo priya doctor`, read the plan, then `sudo priya doctor --fix --apply`, reboot.
     Paths B and C: flash the dev ISO (balenaEtcher on the Mac), boot it (Esc), connect Wi-Fi, run `priya-install --dry-run`, **review the plan together with Claude** (Path C: the target is the free space, Arch and Windows show as locked), then `priya-install`, type the target name, reboot.
  7. Boot Windows once from GRUB and once from the firmware menu; enter the BitLocker key if asked; protection resumes automatically after the restart. Path C: boot Arch from the new menu and check it's unchanged.
  8. Boot PriyaOS, run `priya doctor`; restore dotfiles from the backup.
- [ ] Recovery section in the guide: Windows always reachable via Esc → Windows Boot Manager, and Arch via Esc → its own GRUB entry, even if PriyaOS's menu breaks; if PriyaOS won't boot → boot the dev USB → `priya rollback --offline`; removing PriyaOS entirely = delete its partition and `EFI/PriyaOS/` (steps in the guide), which leaves Arch and Windows as before.
- [ ] Re-register the self-hosted runner (2.3) on the new system 👤.

**Human in the loop**
- The owner decides the path, approves the guide, performs every disk-changing step, types the target name, and confirms Windows still boots. Claude never runs a disk-changing command on the Zenbook; Claude may walk through `--dry-run` output with the owner.

**Tests**
- Unit: `InstallPlan` decision table over `lsblk` fixtures (single NVMe with Windows + Arch; Windows + Arch + free space; Windows + BitLocker; two ESPs, one with a Linux loader; ESP nearly full; USB-only; target too small; free gap too small; unknown filesystem; existing Linux chosen without `--erase-existing-linux`); fstab post-processor removes `subvolid=`; confirmation matcher is exact (no trimming, no case folding).
- E2E (VM): `install_into_free_space_next_to_arch_and_windows`, `existing_arch_unchanged_and_boots`, `boot_menu_remembers_last_choice`, `install_replacing_old_linux`, `windows_partitions_unchanged_after_install`, `esp_only_gains_priyaos_dir`, `nvram_keeps_windows_entry`, `grub_menu_lists_windows`, `installer_refuses_esp_target`, `installer_refuses_bitlocker_target`, `installer_refuses_boot_usb`, `wrong_confirmation_writes_nothing`, `rerun_after_power_cut_succeeds`, `offline_rollback_from_rescue_iso`.
- Chaos: QMP power cut at each installer journal step → either the target is untouched or the re-run succeeds; Windows partitions unchanged in every case.
- Regression: all E2E scenarios above (nightly).

**Done when**
- [ ] All VM journeys pass three nights in a row
- [ ] Guide approved by the owner 👤
- [ ] The Zenbook runs PriyaOS as a third system: `priya doctor` all green on hardware; Arch boots unchanged; Windows boots from GRUB and the firmware menu; BitLocker key not lost if used 👤
- [ ] L4 hardware items in the phase gate checklist pass after the move

---

### 2.6 — Fork lifecycle
**Outcome:** `priya sandbox fork create|list|discard` makes and removes a whole-machine copy-on-write fork in under 2 seconds, with free-space and concurrency limits.
**Features:** SBX-01.
**Depends on:** 2.1, 2.2, 2.3.

**Tasks**
- [ ] Domain types in `priya-core`: `ForkId`, `ForkMeta {run_id, uid, base_root_gen, base_home_gen, created_at, shared_with: Option<StepId>, state}`; `WorkspaceShare::{Own, SharedWith}` stored now, used by Phase 12.
- [ ] `helper.fork.create {run_id}`: under `@priya-forks/<run-id>/` create `base-root`, `base-home` (read-only) and `root`, `home` (writable) snapshots plus `meta.json`; journaled.
- [ ] `helper.fork.discard {run_id}` (deletes all four subvolumes and meta; never touches anything outside `@priya-forks/<run-id>/`), `helper.fork.list {uid}`.
- [ ] Limits: warn below 10 % free, refuse below 5 % (`user_message` explains); max concurrent forks (setting `sandbox.max_forks`, default 4) with a FIFO queue in priyad.
- [ ] GC: failed/discarded forks older than 24 h removed by a daily timer in priyad (`helper.fork.discard`); orphaned forks (meta without a known run) reported in `priya doctor`, never auto-deleted.
- [ ] Events `fork.created`, `fork.discarded` (added to [02 §1.1](../architecture/02-core.md)); `forks` projection.
- [ ] Debug CLI `priya sandbox fork create|list|discard` (`--json`).

**Tests**
- Unit: queue and limit logic; GC selection (never selects active or <24 h forks); meta serialisation.
- Integration (fake machine): `fork_create_and_discard_leaves_no_subvolumes`; `fork_refused_below_5_percent_free` (fill image); `fork_queue_respects_max_forks`; `fork_of_other_users_run_refused`; kill helper between snapshots → recovery report, partial fork removable by `discard`.
- Perf: `fork_create_under_2s` on the fake machine (budget enforced from 2.15).
- Regression: all integration tests above.

**Done when**
- [ ] Fork create p95 < 2 s on the fake machine and on the Zenbook
- [ ] Discard leaves zero subvolumes behind (checked by `btrfs subvolume list`)

---

### 2.7 — Sandbox runner
**Outcome:** the owner can run `priya sandbox run --cmd "sudo pacman -S --noconfirm jq && echo hi > ~/x"` or open `priya sandbox shell`, and everything happens inside a fork while the live machine stays untouched.
**Features:** SBX-06, SBX-09, SBX-14.
**Depends on:** 2.6.

**Tasks**
- [ ] `SandboxRunner` trait in `priya-sandbox::ports`: `launch(LaunchSpec) -> RunHandle`, `signal(run, Stop|Kill)`, `freeze/thaw(run)`, `wait(run) -> ExitStatus`, `stream(run) -> Output`.
- [ ] `LaunchSpec` (pure, unit-tested builder): fork paths, uid, mapped cwd, env, binds, masks, cgroup limits, timeout; rendered to nspawn arguments per [03 §4](../architecture/03-execution-and-sandbox.md): `--directory=<fork>/root`, `--bind=<fork>/home:/home`, `--bind=/var/cache/pacman/pkg`, `--user`, `--chdir`, `--setenv=PRIYA_RUN=<id>`, `--resolv-conf=bind-host`, `--timezone=off`, `--link-journal=no`, `--register=no`.
- [ ] nspawn adapter in `priya-sys::nspawn`, launched by the helper inside transient scope `priya-run-<run-id>.scope` with `CPUQuota`, `MemoryMax`, `TasksMax` from settings (`sandbox.limits.*`).
- [ ] Fork-only setup written into the fork's root before launch and on the ChangeSet ignore list: `/run/priya/fork` marker, `/etc/sudoers.d/90-priya-fork` (passwordless sudo inside the fork only).
- [ ] Other users' homes masked inside the fork (tmpfs over `/home/<other>`); static mask list from settings `sandbox.mask` (default list from [05 §3](../architecture/05-security.md)) applied as tmpfs / `--bind-ro=/dev/null:<file>`; agent auth allow-list kept visible. Cedar-driven masks replace this in 3.5.
- [ ] Output streaming: helper → priyad → `run.output` events (payloads > 8 KiB go to blobs, [02 §1.4](../architecture/02-core.md)); `run.started`, `run.heartbeat`, `run.finished {exit_code, duration}`.
- [ ] Timeouts and stop: `sandbox.default_timeout`, `priya sandbox stop <run>` (SIGTERM, 5 s, SIGKILL on the scope).
- [ ] Temporary CLI (replaced by the task CLI in 3.2): `priya sandbox run --cmd "<cmd>" [--mode sandbox] [--cwd] [--timeout]` and `priya sandbox shell` (interactive, `--console=interactive`); on exit both print "N changes ready: run `priya review <id>`".
- [ ] `priya sandbox shell` opens an interactive shell inside a fresh fork (same masking and limits); leaving it produces a normal ChangeSet to review

**Tests**
- Unit: `LaunchSpec` → argv golden files (insta); cwd mapping (inside home, outside home, non-existent in fork); mask rendering.
- Contract: `SandboxRunner` suite (launch, stream ordering, exit codes, stop, kill, freeze/thaw, timeout) against nspawn and a fake runner.
- Integration (fake machine): `sandbox_pacman_install_does_not_touch_live` (jq absent live after run); `sandbox_home_write_not_live`; `sandbox_masks_ssh_key` (`cat ~/.ssh/id_test` fails inside); `sandbox_hides_other_users_home`; `sandbox_sudo_only_inside_fork`; `sandbox_timeout_kills_scope`; `sandbox_memory_limit_enforced`.
- Security: escape attempts from inside the fork — write to `/run/priya/top`, mount, `nsenter`, access the helper socket (not bound in) — all fail.
- Regression: all integration and security scenarios above.

**Done when**
- [ ] The demo command runs in a fork and the live system is byte-identical afterwards (checked by `find-new` on `@` and `@home`)
- [ ] All escape attempts fail

---

### 2.8 — Booted runs (boot check)
**Outcome:** `priya sandbox boot-check <run>` boots a fork's system part in a container and reports whether it reaches a running state without new failed services.
**Features:** SBX-05.
**Depends on:** 2.7.

**Tasks**
- [ ] `SandboxRunner::boot(spec) -> BootCheckReport` via `systemd-nspawn -b --machine=priya-<run-id>` on a throw-away snapshot of the fork's root (the fork itself is not modified by booting).
- [ ] Wait for `systemctl is-system-running --wait` inside (via `systemd-run -M`), timeout 120 s (setting `sandbox.boot_check.timeout`).
- [ ] Compare failed units inside vs the live system; skip hardware-bound units from `sandbox.boot_check.skip_units` (defaults: `systemd-backlight@*`, `bluetooth.service`, `NetworkManager-wait-online.service`, `fwupd*`, `tlp*`, firmware/udev-settle units).
- [ ] `BootCheckReport {status: Ok|Degraded|Failed|Timeout, new_failed_units, journal_excerpt (last 200 lines of each new failure), kernel_changed}`; stored on the run (`run.finished` payload) and shown in review.
- [ ] Honest limitation: a container shares the host kernel, so a changed kernel or initramfs can't be boot-tested; report says "The new kernel can't be tested before restart; the previous system stays available with `priya rollback`."

**Tests**
- Unit: report builder from fixture `systemctl --failed` outputs; skip-list glob matching.
- Integration (fake machine): `boot_check_ok_on_clean_fork`; `boot_check_detects_broken_unit` (fork enables a unit with a bad `ExecStart`); `boot_check_times_out` (unit that never finishes); `boot_check_flags_kernel_change` (fork installs another kernel package).
- Regression: all four.

**Done when**
- [ ] Boot check of a clean fork completes < 30 s on the Zenbook
- [ ] A deliberately broken unit is reported in plain language

---

### 2.9 — ChangeSet engine I (paths, classification, blobs)
**Outcome:** every sandbox run produces an exact, machine-wide list of changed paths (added, modified, deleted, metadata-only) with before/after contents stored in the blob store.
**Features:** SBX-02.
**Depends on:** 2.6, 2.7.

**Tasks**
- [ ] Domain types in `priya-core`: `ChangeSet {id, origin, base, entries, summary, external_actions, tainted}`, `ChangeEntry {path, area: System|Home, kind: Added|Modified|Deleted|Meta, before_blob, after_blob, mode, owner, symlink_target, xattrs_changed}`, `Origin::{Run, Manual, Settings, Revert}`.
- [ ] Dump parser in `priya-sandbox::dump` for `btrfs receive --dump` (all commands incl. `rename`, `link`, `unlink`, `rmdir`, `write`, `clone`, `truncate`, `chmod`, `chown`, `utimes`, `set_xattr`, `remove_xattr`, `symlink`, `mkfifo`, `mksock`, `mknod`, `update_extent`, `encoded_write`, `fallocate`, `fileattr`); octal-escaped names; temp names `o<ino>-<gen>-0` filtered; both sides of renames collected.
- [ ] Freeze step: read-only snapshots `final-root`, `final-home` of the fork after the run.
- [ ] `helper.changeset.scan {run_id}`: runs the dump for both areas, then `lstat` + size + BLAKE3 (parallel) for each touched path in base vs final; returns `RawEntries`.
- [ ] Classifier (pure): added / deleted / modified (content hash differs) / meta (mode, owner, xattr, symlink target only); file↔dir↔symlink type changes; hardlinks; sparse files.
- [ ] Ignore list (settings `sandbox.ignore`, globs; defaults `/tmp`, `/var/tmp`, `/var/cache`, `/var/log`, `/run`, `~/.cache`, shell histories, agent session caches, `/etc/sudoers.d/90-priya-fork`, `/run/priya/fork`).
- [ ] Blob capture: before/after contents of regular files ≤ 50 MiB into the blob store (dedup by hash, refcounted); larger files keep a snapshot reference with `retained_until`.
- [ ] `changeset.recorded` event + `changesets` projection; `ChangeSetId` linked to the run.
- [ ] Docs: [03 §5.1](../architecture/03-execution-and-sandbox.md) updated with the exact ignore defaults and edge-case rules.

**Tests**
- Unit: dump parser on every 2.2 fixture (golden output); classifier decision table; ignore glob matching; rename both-sides collection.
- Integration (fake machine): `changeset_matches_full_tree_walk` — property test: random file operations (create, write, rename, chmod, symlink, delete, hardlink, Devanagari names) in a fork; ChangeSet equals a naive full-tree-walk diff oracle; `changeset_ignores_cache_and_tmp`; `changeset_records_package_install` (`/usr/bin/jq` added, pacman db modified); `large_file_kept_by_snapshot_reference`.
- Perf: `changeset_10k_files_under_10s`.
- Regression: all integration tests above.

**Done when**
- [ ] Property test runs 500 cases per CI run with zero mismatches against the oracle
- [ ] ChangeSet for 10k touched files < 10 s on the fake machine

---

### 2.10 — ChangeSet engine II (summaries)
**Outcome:** `priya change show <id>` begins with a plain-language summary ("Installed jq. Turned on 1 service. Changed 2 files in your project api.") followed by groups and counts.
**Features:** SBX-02, SBX-09.
**Depends on:** 2.9.

**Tasks**
- [ ] Package diff by parsing `/var/lib/pacman/local/*/desc` in base vs final (pure, no `pacman` call needed): added, removed, upgraded (old → new version), explicit vs dependency.
- [ ] Service diff from `*.wants/` and `*.requires/` symlinks in `/etc/systemd/system` and `~/.config/systemd/user`: enabled, disabled, masked.
- [ ] Users/groups diff from `/etc/passwd`, `/etc/group`; `/etc/shadow` and `/etc/gshadow` are reported as "changed" only, never diffed or blob-displayed.
- [ ] Flags: `kernel_changed` (kernel packages or `/boot/vmlinuz-*`), `bootloader_changed` (`grub` package or `/boot/grub`), `initramfs_changed`.
- [ ] Per-project grouping: nearest ancestor with `.git` or `.priya.toml`; other home files grouped by top-level folder.
- [ ] `Summary` → deterministic plain-language sentences (template-based, translatable strings), "Details" lines with exact counts.
- [ ] API `v1.change.get {changeset_id}`, `v1.change.file_diff {changeset_id, path, context}` (text diffs via `similar`; binary → size and type only); CLI `priya change show <id> [--json]`, `priya change diff <id> <path>`.

**Tests**
- Unit: pacman `desc` parser and package diff (fixtures with upgrades, removals, epochs); service-link parser; summary sentences golden-tested (English, and Hindi file names render correctly); shadow files never produce content diffs.
- Integration (fake machine): `summary_lists_installed_package`, `summary_lists_enabled_service`, `summary_groups_by_project`, `summary_flags_kernel_change`, `shadow_file_content_never_exposed`.
- Regression: all integration tests above.

**Done when**
- [ ] The demo run's summary reads correctly to the owner 👤 ("Installed jq. Created 1 file in your home folder.")
- [ ] `--json` output validates against the exported schema

---

### 2.11 — Home merge and conflicts
**Outcome:** home changes from a fork are applied to the live home immediately and atomically; if the user changed the same file meanwhile, the owner chooses mine / agent's / both / ask Priya to merge.
**Features:** SBX-03, SBX-04.
**Depends on:** 2.10.

**Tasks**
- [ ] Three-way decision function in `priya-sandbox::merge` (pure): per path compare base, live, fork → `Apply | Skip (fork == live) | Conflict`; covers add/add, modify/delete, delete/modify, type changes, mode-only changes.
- [ ] `helper.merge.home {changeset_id, parts}`: for each `Apply`, reflink-copy the fork version to a temp file in the target directory, fsync, re-check the live hash immediately before `renameat2` (changed since check → becomes `Conflict`), atomic rename; deletions and directory creations; owner and mode restored; each file op journaled.
- [ ] Partial merges: `v1.change.merge {changeset_id, parts}` where parts are groups, projects or individual paths.
- [ ] Conflicts recorded as `merge.conflict` events; live file left untouched until resolved; `v1.change.resolve {changeset_id, path, choice: mine|agent|both|ai}`.
- [ ] Resolutions: **mine** (keep live), **agent's** (apply fork version), **both** (agent's version written next to the file as `<name>.priya-agent` and opened in `$EDITOR`), **ask Priya** (Priya AI role `coding` for code, `smart` otherwise, proposes a merged text shown as a further diff that must be accepted; offline → option disabled with an explanation).
- [ ] Merge result states on the ChangeSet: `merged`, `merged_with_conflicts`, `partially_merged`; `merge.applied` event with counts.
- [ ] CLI `priya change merge <id> [--only <part>]`, `priya change resolve <id> <path> --mine|--agent|--both|--ai`, `priya change discard <id>` (deletes the fork, keeps ChangeSet and blobs).
- [ ] After full merge or discard the fork is deleted (2.6).

**Human in the loop**
- Conflicts never resolve themselves. The owner sees the three versions, picks one choice per file, and an AI-proposed merge is only a suggestion shown as a diff. Merged home changes can be reverted later from the ChangeSet (Rewind generalises this in Phase 7).

**Tests**
- Unit: three-way decision table for every combination of (base, live, fork) ∈ {absent, A, B, C, dir, symlink}; resolution application logic.
- Integration (fake machine): `home_merge_add_modify_delete`; `home_merge_conflict_keeps_live_file`; `home_merge_detects_change_during_merge` (live file modified between check and rename → conflict, not overwrite); `home_partial_merge_only_selected_project`; `resolve_both_writes_agent_copy`; `discard_deletes_fork_keeps_blobs`; `merge_preserves_mode_and_owner`.
- Chaos: kill the helper after each journaled file op → no file is ever half-written; journal report lists the rest.
- Regression: all integration and chaos scenarios above.

**Done when**
- [ ] Merging 1,000 changed home files completes < 5 s on the Zenbook (baseline recorded for 2.15)
- [ ] No test ever observes a partially written live file

---

### 2.12 — System merge, rollback and previous-system boot
**Outcome:** system changes from a fork become the live system on the next restart; `priya rollback` returns to the previous system root on the next restart; the dev USB can roll back offline if the new root doesn't boot.
**Features:** SBX-05, SBX-03, SBX-13.
**Depends on:** 2.8, 2.11.

**Tasks**
- [ ] Drift check: `find-new @ <base_root_gen>` filtered by the ignore list; drift → refuse with the list of live paths changed since the fork; `--force-drift` requires typed confirmation (approval tier from Phase 3).
- [ ] Stage and swap (`helper.system.swap {changeset_id}`, journaled step by step): snapshot the fork's `final-root` as `@priya-next`; `mv @ @priya-prev-<ts>`; `mv @priya-next @`; write `/var/lib/priya/pending-swap.json` (on `@priya`, survives the swap). Home part merges first (2.11).
- [ ] GRUB: if `kernel_changed` or `initramfs_changed`, regenerate `grub.cfg` inside the new `@` (helper mounts it with `/dev`, `/proc`, `/sys` binds and runs `grub-mkconfig -o /boot/grub/grub.cfg`). If the `grub` package itself changed, the swap is refused with a plain explanation (re-running `grub-install` is never automated; owner-only via the guide).
- [ ] After reboot: helper verifies the booted root's subvolume id equals the staged one → `system.swapped` event; mismatch → `system.swap_failed` notice in `priya doctor`.
- [ ] Late-change report: files changed on the old live root after staging and before reboot are found with `find-new @priya-prev-<ts> <stage_gen>` and reported ("3 files changed after the update was staged; they're kept in the previous system"), offered as a manual ChangeSet in Phase 7.
- [ ] Rollback: `helper.system.rollback` swaps `@` with the newest `@priya-prev-*` (the current root becomes a new `@priya-prev-<ts>`, so rollback is itself reversible) on the next restart; `system.rolled_back` event.
- [ ] Retention: keep N previous roots (setting `sandbox.prev_roots_keep`, default 3); never delete the newest; pruning journaled.
- [ ] Offline rollback in the dev ISO (2.5) shares the same pure swap planner.
- [ ] CLI `priya change merge <id>` (shows "System part applies on restart"), `priya system status` (pending swap, previous roots, booted root), `priya rollback`. No automatic reboot: the CLI asks the user to restart.
- [ ] `/etc/grub.d/41_priya_previous` (installed by the `priya` package): after every swap, a boot-menu entry "PriyaOS (previous system)" boots the newest `@priya-prev-*` with its own kernel and initramfs via `rootflags=subvol=@priya-prev-<ts>`; older retained previous systems in a submenu
- [ ] The helper regenerates `grub.cfg` inside the new `@` on **every** swap (not only when the kernel changed) — update [03 §5.2](../architecture/03-execution-and-sandbox.md)
- [ ] fstab: the `/` entry carries no `subvol=` (the initramfs mounts root from `rootflags`), so booting a previous system doesn't fail at remount; `priya doctor` checks this
- [ ] Booted-from-previous detection (`findmnt -no FSROOT /`): `priya status` and a bar notice say "You started the previous system"; `priya rollback` from there swaps it into `@` and keeps the failed root as a previous system (journaled)

**Human in the loop**
- The owner sees which part applies now (home) and which on restart (system), is told before restarting, and can roll back with one command. Nothing reboots automatically.

**Tests**
- E2E (VM): seeded update that breaks the kernel boot after the container boot check (corrupted initramfs) → restart fails → pick "PriyaOS (previous system)" → boots → `priya rollback` → restart → healthy (`prev_entry_boots_vm`, `rollback_from_prev_vm`)
- Unit: swap planner (state → ordered journal steps), retention selection never picks the newest prev, drift filter, GRUB-regeneration decision.
- Integration (fake machine): `system_merge_installs_jq_after_boot` (merge, `FakeMachine::boot()`, `jq --version` works); `system_merge_refused_on_drift`; `system_merge_refused_when_grub_package_changed`; `rollback_restores_previous_root`; `rollback_is_reversible`; `prev_roots_retention_keeps_newest`; `late_changes_reported_after_swap`.
- E2E (VM): `sandbox_install_merge_reboot_rollback` across real reboots with GRUB; `kernel_update_in_sandbox_boots_new_kernel_and_rolls_back`; `offline_rollback_from_rescue_iso`.
- Chaos: kill the helper and power-cut the VM (QMP) at every swap journal step → after restart there is always exactly one bootable `@`, the newest prev exists, and the journal reports the unfinished step.
- Regression: all integration, E2E and chaos scenarios above.

**Done when**
- [ ] In the VM, install → merge → restart → rollback → restart works ten times in a row
- [ ] Every power-cut point leaves a bootable machine

---

### 2.13 — Read-only and direct modes
**Outcome:** `--mode read_only` sees the live machine but can't change it; `--mode direct` changes the live machine after a safety snapshot and the owner can keep or revert; `--mode direct_raw` runs without a snapshot only when explicitly requested.
**Features:** SBX-06, SBX-08.
**Depends on:** 2.7, 2.9, 2.11.

**Tasks**
- [ ] `read_only`: helper takes read-only snapshots of `@` and `@home`, runs nspawn with `--read-only` on them (writable tmpfs only for `/tmp` and `~/.cache`), deletes the snapshots after; same mask list; no ChangeSet.
- [ ] `direct` safety snapshots: `helper.snapshot.safety {run_id}` → read-only `base-root`, `base-home` under `@priya-forks/<run-id>/` (same layout as forks, no writable part).
- [ ] Launcher `priya-run-exec` (in `priya-sys`): applies a Landlock ruleset (writes only to allowed paths, default = the cwd project; reads allowed everywhere except the mask list — implemented by allow-listing sibling hierarchies, since Landlock cannot deny a subdirectory of an allowed one), sets `NoNewPrivileges` (so `sudo` fails in direct mode), then `exec`s the command in a transient user scope `priya-run-<id>.scope`.
- [ ] After a direct run: freeze the live state with read-only snapshots, compute the ChangeSet snapshot → live with the 2.9 engine; ChangeSet `origin = Run`, state `awaiting_keep`.
- [ ] Keep = record and finish (safety snapshots retained per settings, default 24 h). Revert = apply the inverse ChangeSet with the three-way rules against current live (restore before-blobs, delete added, re-add deleted); conflicts if something changed since; `revert.applied` event.
- [ ] `direct_raw`: no snapshot, ChangeSet marked "unknown changes" (commands have no tool log; agents add one in Phase 4); CLI requires `--mode direct_raw` plus typed `direct raw`; never a default.
- [ ] CLI `priya sandbox run --mode read_only|direct|direct_raw`, `priya change keep|revert <id>`.

**Human in the loop**
- In direct mode the change is already live; the owner reviews and chooses Keep or Revert. `direct_raw` warns in plain language that undo is limited.

**Tests**
- Unit: Landlock ruleset builder (mask siblings computed correctly for nested masks like `~/.local/share/keyrings`); inverse ChangeSet builder.
- Integration (fake machine): `read_only_run_cannot_write` (writes to `/etc`, `~`, `/var` all fail; tmpfs writes vanish); `direct_run_revert_restores_exactly` (hash of every touched path equals the safety snapshot); `direct_revert_conflict_when_file_changed_since`; `direct_run_cannot_write_outside_allowed_paths`; `direct_run_cannot_read_masked_ssh`; `direct_run_sudo_fails`; `direct_raw_requires_typed_confirmation`.
- Security: Landlock bypass attempts (symlink into a masked dir, hardlink, `/proc/self/root`, `openat` with `..`) fail.
- Regression: all integration and security scenarios above.

**Done when**
- [ ] Revert after a direct run restores byte-identical content and metadata for every touched path
- [ ] Read-only runs leave `find-new` empty on `@` and `@home`

---

### 2.14 — Review TUI and diff components
**Outcome:** `priya review` opens a terminal review screen (plain summary, groups, file diffs, conflicts, merge/discard/keep/revert), and the QML diff and ChangeSet components exist in the component gallery for the Phase 4 GUI.
**Features:** UX-01, SBX-03, SBX-09.
**Depends on:** 2.10, 2.11, 2.12, 2.13.

**Tasks**
- [ ] `priya review [<changeset>]` in `priya-cli` with ratatui: header (plain summary sentence, mode badge with icon + word, origin, boot-check result, "System part applies on restart"), left pane groups **Apps · Services · System files · Your files (by project)** with counts, right pane file diff.
- [ ] Diff view: unified / side-by-side toggle (`s`), syntax highlighting (`syntect`), word-level highlights, binary files ("binary, 2.1 MB → 2.3 MB"), diffs > 2,000 lines truncated with "show all", virtualised file list for 10k entries.
- [ ] Actions: select parts (`space`), Merge (`M`), Discard (`D`), Keep / Revert for direct runs (`K` / `R`), each with a confirmation line; conflict view with `m` mine / `a` agent's / `b` both / `p` ask Priya; drift warning view; `?` help overlay with every key.
- [ ] Placeholders wired to real data later: untrusted banner (filled in 3.11), external-actions section (3.11).
- [ ] Themes from `ui/design/tokens.json` via a generated Rust palette: light, dark, OLED-black, high-contrast (truecolor), 256-colour and 16-colour fallbacks, `NO_COLOR` support; mode and risk always shown with icon + word, never colour alone.
- [ ] Screen-reader fallback: `priya review --plain` prints a linear, numbered transcript with prompts (TUIs are not readable by Orca); non-TTY output falls back to `priya change show`.
- [ ] QML components in `ui/PriyaUI/` (gallery only in this phase): `DiffView` (unified/side-by-side), `FileChangeRow`, `ChangeGroupList`, `ChangeSetSummaryCard`, `ConflictChooser`, `RestartPendingBanner`, `ModeBadge` (if not present), each with gallery entries for every state.
- [ ] QML type helpers regenerated from the API schemas for `ChangeSet` and `Summary`.

**UI**
- Screens/components: review TUI (summary, groups, diff, conflicts, drift, help); gallery components listed above.
- States: empty ("No changes"), loading (computing ChangeSet, with progress count), success/clean, conflicts, tainted (placeholder), partial (stopped by budget, placeholder), restart pending, drift refused, error (ChangeSet failed, with "Try again"), long content (10k files; 5k-line diff; very long paths), Hindi text (Devanagari file names and content), binary files. Offline: "Ask Priya to merge" disabled with reason.
- Light, dark, OLED-black, high-contrast for QML components and TUI palettes; QML at scale 1.0 and 2.0; TUI checked at 80×24 and 200×60.
- Keyboard: every TUI action has a key shown in the footer; QML components fully keyboard-operable (Tab/Shift+Tab, arrows in lists, Enter, Esc) with visible 2 px focus ring.
- Screen-reader names: `Accessible.name`/`role` on every interactive QML item (e.g. "Merge 12 changes", "Choose my version for notes.md"); `--plain` mode for the TUI.
- 👤 Mockup approval (HTML/PNG in `ui/mockups/2.14/`, light + dark) before building; gallery sign-off and a recorded TUI session after.

**Human in the loop**
- The review screen is where the owner decides: merge all, merge some, discard, keep or revert. Every destructive choice needs a second key press; the plain summary always comes first, details on demand.

**Tests**
- Unit: TUI layout and key handling with ratatui `TestBackend` + insta snapshots for every state; palette generation from tokens; `--plain` transcript golden files.
- UI: Qt Quick tests for each component (keyboard, signals, states); screenshot tests for every state × 4 themes × 2 scales; accessibility lint; contrast check for TUI palettes.
- Integration (fake machine): `review_tui_merge_journey` (scripted keys: open, select project, merge → live files changed); `review_tui_conflict_journey`; `review_tui_direct_revert_journey`.
- Regression: the three journeys plus all screenshot baselines.

**Done when**
- [ ] The owner reviews and merges the demo ChangeSet using only the TUI 👤
- [ ] Gallery sign-off by the owner 👤
- [ ] All screenshot and accessibility checks green

---

### 2.15 — Chaos and performance suites
**Outcome:** nightly CI proves on every commit to `main` that the sandbox survives crashes at every journaled step, stays within performance budgets, and installs and works end to end in a VM.
**Features:** QA-16, QA-17, QA-18, QA-19.
**Depends on:** 2.5, 2.12, 2.13.

**Tasks**
- [ ] Failpoints: `fail` crate failpoints at every `JournalStep` in the helper and every priyad-side step, compiled only with `--features chaos`; `PRIYA_FAILPOINTS=merge.home.after_rename=panic` style configuration.
- [ ] Chaos harness (`tests/chaos/`): for each journal step × {kill helper, kill priyad, power-cut VM via QMP}: run the scenario, crash, restart, assert invariants — `@`, `@home`, `@priya` exist; newest prev exists; no partial live file; blob refcounts consistent (`priya-store` fsck); unfinished entries reported, nothing auto-retried; machine boots.
- [ ] Perf suite (`tests/perf/`, tag `perf`): fork create < 2 s; ChangeSet for 10k touched files < 10 s; home merge 1k files (baseline from 2.11); boot check (baseline from 2.8); helper round-trip p99; results stored as JSON; a regression > 20 % vs the rolling baseline fails the nightly; PR comment shows perf deltas when the perf label is set.
- [ ] Nightly workflow: VM e2e (install journey from 2.5 + sandbox journey: install → sandbox `pacman -S jq` → review → merge → reboot → rollback → reboot), perf, chaos; GitHub-hosted KVM runner where available, otherwise the Zenbook runner (`priya-ci`, VMs only); any failure opens or updates a dashboard issue automatically.
- [ ] Perf runs on a fixed runner (the Zenbook runner, plugged in, performance power profile) so numbers are comparable.
- [ ] Docs: perf budgets and chaos invariants added to [09 §2](../architecture/09-testing.md).

**Tests**
- Chaos: the full matrix above (named `chaos_<step>_<crash_kind>`).
- Perf: `fork_create_under_2s`, `changeset_10k_files_under_10s`, `home_merge_1k_files_baseline`, `boot_check_baseline`, `helper_rpc_p99`.
- E2E (VM): `install_next_to_fake_windows`, `sandbox_install_merge_reboot_rollback`.
- Regression: all chaos, perf and e2e scenarios join the nightly regression set.

**Done when**
- [ ] Nightly green five nights in a row
- [ ] Every `JournalStep` variant has at least one chaos test (enforced by a test that enumerates the enum)
- [ ] Perf budgets met on the Zenbook runner

---

### 2.16 — Phase gate
**Outcome:** Phase 2 is complete; the owner has used Priya Sandbox on the Zenbook.
**Features:** all Phase 2 features.
**Depends on:** 2.1–2.15.

**Tasks**
- [ ] Run the checklist in "Phase gate" below; record results in this file.

**Done when**
- [ ] Every item in "Phase gate" ticked

---

## Phase gate

- [ ] Full regression suite green on `main` (unit, contract, integration, UI, security fast subset) and nightly (e2e VM, perf, chaos) green three nights in a row
- [ ] `cargo xtask trace`: every Phase 2 feature has passing tests tagged with its ID; all acceptance tests tagged `regression`
- [ ] Owner demo on the Zenbook 👤: `priya sandbox run --cmd "sudo pacman -S --noconfirm jq && echo hi > ~/x"` → `priya review` → merge → restart → `jq` present and `~/x` exists → `priya rollback` → restart → `jq` gone; a direct-mode edit kept and another reverted; a read-only run that tries to write and fails; a home conflict resolved
- [ ] L4 checklist on the Zenbook 👤:
  - [ ] GRUB menu shows PriyaOS and Windows Boot Manager; both boot; firmware boot menu (Esc) still lists Windows Boot Manager
  - [ ] BitLocker: Windows boots without a recovery prompt (or the saved key works); protection resumed
  - [ ] Wi-Fi, audio (sof-firmware), suspend/resume, brightness, battery reporting, external display work on PriyaOS
  - [ ] `priya doctor` all green; helper hardened; no `subvolid=` in fstab or cmdline
  - [ ] Fork create < 2 s and boot check < 30 s measured on the Zenbook
  - [ ] Swap + rollback across two real restarts on the Zenbook
  - [ ] Dev USB rescue `priya rollback --offline` tried once on the Zenbook (on a deliberately staged swap)
  - [ ] Phase 0–1 features still work on hardware (Priya bar, `priya ask`, model switching)
- [ ] Docs updated: [03](../architecture/03-execution-and-sandbox.md) (ignore list, swap details, Landlock approach), [01 §2](../architecture/01-system-overview.md) (helper ops, groups), [02 §1.1](../architecture/02-core.md) (new events), [09](../architecture/09-testing.md) (perf budgets, chaos invariants, runner), [10](../architecture/10-base-os-and-distro.md) (dev installer); ADRs added for any design change
- [ ] Feature statuses for all 17 Phase 2 features updated in [features.md](../features.md)
- [ ] `docs/human/07-install-on-your-zenbook.md` reflects what the owner actually did; `docs/human/05-the-phases.md` status updated

## Out of scope

- Tasks, plans and the durable task engine; Cedar policies, approvals, secrets injection, budgets, taint (Phase 3). Mask lists are static settings until 3.5.
- Agents of any kind (Phase 4); review GUI and Task center (Phase 4).
- Privileged package transactions on the live system and the update pipeline (Phase 6).
- Rewind timeline, selective undo, restore by time (Phase 7).
- Previous-system boot entry in GRUB (SBX-13, Phase 9) — until then, the dev USB rescue covers a root that won't boot.
- Graphical installer, encryption option, signed repo, branding (Phase 9).
- Per-run network isolation and egress proxy (Phase 14); forks share the host network in this phase.
- Running `grub-install` or changing EFI variables automatically, ever.
