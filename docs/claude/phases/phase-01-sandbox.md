# Phase 1 — Priya Sandbox and execution modes

**Goal.** Fork the whole machine, run commands inside the fork, compute a machine-wide ChangeSet, merge it (home live, system on reboot), handle conflicts, roll back — plus read-only and direct modes with safety snapshots. All privileged work in `priya-helper`.

**Depends on:** Phase 0.
**Features:** CORE-06, SBX-01, SBX-02, SBX-03, SBX-04, SBX-05, SBX-06, SBX-08, SBX-09, SEC-01.
**Read first:** [03-execution-and-sandbox](../architecture/03-execution-and-sandbox.md), [01 §2](../architecture/01-system-overview.md), ADR-004, ADR-005, ADR-008.

## Build

### 1.1 Root helper
- [ ] `priya-helper` system service: socket `/run/priya/helper.sock` (0660, group `priya`), peer-credential auth, fixed operation set, no network (systemd hardening), journal in `/var/lib/priya/system.db`
- [ ] Invariants enforced in the helper: protected subvolumes, only the caller's home, `priya-admin` for system operations, path canonicalisation
- [ ] Startup: mount top level at `/run/priya/top`, recover/report unfinished journal entries

### 1.2 Snapshot backend (`priya-sys` btrfs adapter behind `SnapshotBackend`)
- [ ] Create read-only / writable snapshots, delete, generation lookup, `find-new`, `send --no-data` dump
- [ ] `priya doctor --fix`: create `@priya`, `@priya-forks`, fix fstab `subvolid=`, mount `@priya` (L2 only during development)

### 1.3 Forks and runner
- [ ] Fork create/discard for a run id; meta.json
- [ ] `SandboxRunner` nspawn adapter: not-booted runs as the user, cwd mapping, cache bind, fork marker, fork-only sudoers, mask list from settings (policy integration comes in Phase 2), cgroup scope limits
- [ ] Booted runs for boot checks (used by Phase 6) — implement and test now
- [ ] `read_only` runs (read-only snapshots + `--read-only`)
- [ ] `direct` runs: safety snapshots, run live in a transient scope with Landlock write limits; `direct_raw` without snapshot

### 1.4 ChangeSets (`priya-sandbox`, pure)
- [ ] Dump parser (fixtures from Phase 0), path collection incl. renames, temp-name filtering
- [ ] Classifier (lstat, size, BLAKE3), ignore list from settings
- [ ] Summary builder: packages (`pacman -Q --root`), services (`*.wants`), users/groups, per-project grouping
- [ ] Blob capture of before/after for files ≤ 50 MiB
- [ ] `changeset.recorded` event + projection

### 1.5 Merge, revert, rollback
- [ ] Home merge (three-way, reflink + atomic rename, conflicts recorded), resolve choices (mine / agent's / both)
- [ ] System merge: drift check, swap, GRUB regen when needed, `done_pending_reboot`, prev-roots retention
- [ ] Direct-mode keep/revert (inverse ChangeSet with three-way checks)
- [ ] `priya rollback`
- [ ] All live-changing steps journaled before/after

### 1.6 CLI (temporary, before the task engine)
- [ ] `priya sandbox run --cmd "<cmd>" [--mode read_only|sandbox|direct|direct_raw]`, `priya change show <id>`, `priya change merge|discard|keep|revert <id>`, `priya rollback`

## Tests

- L0: dump parser fixtures, classifier, 3-way decision table, ignore matching, summary building, journal recovery logic.
- L1: `SnapshotBackend` and `SandboxRunner` contract suites.
- L2 (fake machine): home add/modify/delete merge; conflict leaves live file untouched; `pacman -S jq` in sandbox shows package and merges via swap; drift refusal; rollback; read-only run cannot write; direct run + revert restores exactly; secrets mask hides `~/.ssh/id_test`; discard deletes all subvolumes; kill the helper at every journaled step → recovery report, no corruption.

## Done when

- [ ] All L2 scenarios pass on the Zenbook
- [ ] Fork creation < 2 s on the fake machine; ChangeSet for 10k touched files < 10 s
- [ ] No privileged operation exists outside `priya-helper`
- [ ] Docs updated for anything that changed

**Demo.** On the fake machine: run `pacman -S --noconfirm jq && echo hi > ~/x` in a sandbox, show the diff, merge, reboot the fake system (nspawn boot), show `jq` present; roll back.

**Out of scope.** Agents, models, policies (masking uses a static list until Phase 2).
