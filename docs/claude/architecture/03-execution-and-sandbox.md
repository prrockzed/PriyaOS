# 03 — Execution modes and Priya Sandbox

## 1. Execution modes

| Mode | What happens | Undo | Typical use |
|---|---|---|---|
| `read_only` | Run sees the live machine read-only (nspawn with read-only binds, or direct with Landlock read-only). Cannot write anything. | nothing to undo | Questions, diagnostics, analysis |
| `sandbox` | **Priya Sandbox**: run works on a copy-on-write fork of the machine. Changes reach the real machine only on merge. | discard before merge; Rewind after | Installs, updates, refactors, anything risky |
| `direct` | Run works on the live machine. A **safety snapshot** is taken first; after the run the ChangeSet is computed from snapshot → live. | Revert from the ChangeSet; Rewind | Small, low-risk edits where speed matters |
| `direct_raw` | Live machine, no snapshot. Changes are only known from the agent's own tool log (best effort). | Limited | Explicitly requested by the user; never a default |

### 1.1 Mode resolution (first match wins)

1. Explicit in the request: "do it directly", "in the sandbox", "just look, don't change".
2. Step-level choice in an approved plan.
3. Per-feature rule from settings (`[modes]` below), matched on module, intent category, or path.
4. Priya AI risk suggestion: classifier risk `high` → `sandbox`; `none` → `read_only`; `low` → `direct`.
5. Global default (`sandbox`).

```toml
# ~/.config/priya/modes.toml
default = "sandbox"

[[rule]]
when = { module = "updates" }
mode = "sandbox"

[[rule]]
when = { category = "file_edit", paths = ["~/notes/**"] }
mode = "direct"

[[rule]]
when = { category = "question" }
mode = "read_only"

[[rule]]
when = { privileged = true }      # anything needing root
mode = "sandbox"
lock = true                       # cannot be overridden by Priya AI suggestions, only by explicit user request
```

Users change these by plain language too ("always do note edits directly") through natural-language settings ([02-core.md](02-core.md) §6.2).

### 1.2 External actions

Actions whose effects leave the machine are classified **external**: network writes to non-allowlisted hosts, `git push`, sending mail/messages, deploys, package publishing, payments. No mode can undo them. They follow the approval policy ([05-security.md](05-security.md) §4) in every mode. In `sandbox` mode, external actions are blocked at the egress proxy until approved (Phase 14); before that phase, the policy warns and the review highlights detected external actions from tool logs.

## 2. Required disk layout

btrfs root with flat subvolumes:

| Subvolume | Mount | In forks? | Notes |
|---|---|---|---|
| `@` | `/` | yes (system part) | includes `/boot` (GRUB reads btrfs) so kernels are part of every snapshot |
| `@home` | `/home` | yes (home part) | |
| `@log` | `/var/log` | no | |
| `@pkg` | `/var/cache/pacman/pkg` | no; bind-shared into forks | |
| `@priya` | `/var/lib/priya` | no | helper state, blob store; must survive system swaps |
| `@priya-forks` | not mounted | — | forks |
| `@priya-timeline` | not mounted | — | Rewind timeline snapshots |
| `@priya-prev-<ts>` | not mounted | — | previous roots after a system swap (keep N) |

fstab and kernel cmdline must mount `/` by `subvol=@` only (no `subvolid=`). `priya doctor` checks and fixes this. Machines with `/boot` on the ESP are supported with kernel packages excluded from sandboxed system changes (doctor warns).

## 3. Forks

Under `@priya-forks/<run-id>/`: `base-root`, `base-home` (read-only snapshots), `root`, `home` (writable snapshots), `meta.json` (base generations, uid, created_at). Only the requesting user's home subtree is exposed inside the fork; other users' homes are masked.

Sharing: sequential steps of one plan may share a fork (`WorkspaceShare::SharedWith`). Parallel steps get their own forks (or, for code-only steps, git worktrees inside one fork — cheaper; Priya Orchestration decides).

## 4. Running

`priya-helper` launches runs with `systemd-nspawn` (not booted) inside a transient scope with cgroup limits:

- `--directory=<fork>/root`, `--bind=<fork>/home:/home`, `--bind=/var/cache/pacman/pkg`
- `--user=<uid>`, `--chdir=<mapped cwd>`, `--setenv=PRIYA_RUN=<id>`
- masked secrets: `--tmpfs=<dir>` / `--bind-ro=/dev/null:<file>` from policy
- `--resolv-conf=bind-host --timezone=off --link-journal=no --register=no`
- marker `/run/priya/fork` (pacman hooks and modules check it)
- inside forks the user has passwordless sudo via `/etc/sudoers.d/90-priya-fork` written only into the fork and on the diff ignore list
- network: shared with host until the per-run egress proxy lands (Phase 14), then `--private-network` + veth + proxy with domain allowlist

Boot checks: `systemd-nspawn -b` on the fork, wait for `systemctl is-system-running --wait`, compare failed units against the live system, skip hardware-bound units (configurable list), timeout 120 s.

`read_only` runs: take read-only snapshots of `@` and `@home`, run nspawn with `--read-only` on them (writable tmpfs only for `/tmp` and `~/.cache`), delete the snapshots afterwards. Same code path as sandbox, nothing can be written.

`direct` runs: helper takes read-only safety snapshots of `@` and `@home`, the agent runs as the user on the live system (in a transient scope with cgroup limits, Landlock restricting writes to the paths the capabilities allow), then the ChangeSet is computed snapshot → live.

## 5. ChangeSets

```rust
struct ChangeSet {
    id: ChangeSetId,
    origin: Origin,                 // Run(run_id) | Manual(timeline window) | Settings | Revert(of)
    base: Snapshots,                // base generations / snapshot ids
    entries: Vec<ChangeEntry>,      // path, area (system|home), kind (added|modified|deleted|meta), before_blob, after_blob, mode/owner
    summary: Summary,               // packages ±, services ±, groups/users ±, per-project file counts
    external_actions: Vec<ExternalAction>,
    tainted: Vec<TaintReason>,
}
```

### 5.1 Computing

1. Freeze: read-only snapshot of the end state.
2. Touched paths: `btrfs send --no-data -p <base> <final> | btrfs receive --dump`; collect every path mentioned (both sides of renames); ignore btrfs temp names.
3. Classify each path by `lstat` + size + BLAKE3 in base vs final.
4. Apply the ignore list (`/tmp`, `/var/tmp`, `/var/cache`, `/var/log`, `/run`, `~/.cache`, shell histories, agent session caches, fork-only sudoers file…).
5. Store before/after contents of changed regular files ≤ 50 MiB in the blob store (dedup by hash; larger files keep a reference to the snapshot only while it is retained).
6. Summary: `pacman -Q --root` base vs final, service symlinks in `*.wants/`, `/etc/passwd`/`group` changes, per-project grouping.

Pure parts (dump parsing, classification, summary building) live in `priya-sandbox` with fixture tests.

### 5.2 Merge (sandbox → live)

- **Home part, live, no reboot**: three-way per path (base, live, fork). Live == base → apply fork version (reflink copy to temp + atomic rename). Fork == live → nothing. Otherwise → conflict (user chooses: mine / agent's / both in editor / ask Priya AI to merge the text, shown as a further diff).
- **System part, on reboot**: drift check with `btrfs subvolume find-new @ <base_gen>` (filtered by ignore list) → refuse if drifted unless forced; then `mv @ @priya-prev-<ts>`; snapshot fork's final root as `@`; regenerate GRUB config inside the new `@` if kernel/GRUB changed; state `done_pending_reboot`.
- Every live-changing step is journaled before and after in `system.db`; on startup unfinished entries are reported, never auto-retried.

### 5.3 Keep / revert (direct mode)

Keep = record and finish. Revert = apply the inverse ChangeSet with the same three-way rules (current live vs the ChangeSet's after-state): restore before-blobs, delete added files, re-add deleted ones; conflicts if something changed since.

### 5.4 Rollback

`system.rollback` swaps `@` with the newest `@priya-prev-*` (on reboot). Rewind (Phase 7) generalises this to any point and any ChangeSet.

## 6. Cleanup and limits

- Delete forks after merge/discard (keep ChangeSet + blobs).
- GC failed/discarded forks older than 24 h; warn below 10 % free space; refuse new forks below 5 %.
- Max concurrent forks (default 4); queue beyond.
