# 01 — Fork, diff, merge

The core of PriyaOS. A **fork** is a writable copy-on-write copy of the user's machine (root and home) where an agent can do anything. A **diff** lists what the fork changed. A **merge** applies those changes to the real machine.

## 1. Required disk layout

PriyaOS requires btrfs as the root filesystem with this flat subvolume layout (the archinstall default plus `@priya`):

| Subvolume | Mounted at | In forks? | Notes |
|---|---|---|---|
| `@` | `/` | yes (system part) | Includes `/boot` (kernels) — see §6 |
| `@home` | `/home` | yes (home part) | |
| `@log` | `/var/log` | no | |
| `@pkg` | `/var/cache/pacman/pkg` | no, bind-mounted shared | |
| `@priya` | `/var/lib/priya` | no | priyad state. Must live outside `@` or a system merge would roll it back. |
| `@priya-forks` | not mounted | — | Container for all forks |
| `@priya-prev-<ts>` | not mounted | — | Previous roots kept after a system merge (keep last 3) |

priyad mounts the btrfs top level (`subvolid=5`) at `/run/priya/top` on startup.

`fstab` and the kernel command line **must refer to `/` by `subvol=@` only, never `subvolid=`**. genfstab writes `subvolid=256,subvol=/@`; after a system merge `@` has a new id and the mount would fail. `priya doctor` detects this and `priya doctor --fix` removes `subvolid=` from the `/` line (after backing up fstab to `/var/lib/priya/backups/`).

All paths and subvolume names come from `/etc/priya/config.toml` so tests can point priyad at a fake machine:

```toml
top_device = "auto"          # auto = device backing /
top_mount = "/run/priya/top"
root_subvol = "@"
home_subvol = "@home"
forks_subvol = "@priya-forks"
keep_prev_roots = 3
```

## 2. Creating a fork

For task `<id>`, under `top/@priya-forks/<id>/`:

```
base-root    read-only snapshot of @        (the "before" picture)
base-home    read-only snapshot of @home
root         writable snapshot of base-root  (agent works here)
home         writable snapshot of base-home
meta.json    { base_root_generation, base_home_generation, created_at, scope }
```

Commands:

```
btrfs subvolume snapshot -r top/@      forks/<id>/base-root
btrfs subvolume snapshot -r top/@home  forks/<id>/base-home
btrfs subvolume snapshot    forks/<id>/base-root forks/<id>/root
btrfs subvolume snapshot    forks/<id>/base-home forks/<id>/home
```

Record the generation of `@` and `@home` at fork time (`btrfs subvolume show` → `Generation`). Used for drift detection in §5.

Snapshots are instant and cost no space until files change.

## 3. Running inside a fork

Use `systemd-nspawn` without booting (no `-b`) for agent work:

```
systemd-nspawn \
  --quiet --register=no --keep-unit \
  --machine=priya-<short-id> \
  --directory=forks/<id>/root \
  --bind=forks/<id>/home:/home \
  --bind=/var/cache/pacman/pkg \
  --bind-ro=/run/priya/fork-marker:/run/priya/fork \
  --resolv-conf=bind-host --timezone=off --link-journal=no \
  --user=<username> --chdir=<cwd mapped into fork> \
  --setenv=PRIYA_TASK=<id> --setenv=HOME=/home/<username> \
  [--tmpfs=<masked secret path> ...] \
  -- <agent command>
```

- Network is shared with the host in week 1 (agents need their APIs). Egress filtering is after week 1.
- Secrets are hidden by mounting an empty tmpfs over each path in `policy.toml` `mask` (see [03-agents-and-policy.md](03-agents-and-policy.md)).
- `/run/priya/fork` exists only inside forks. Anything that must not run in a fork (the pacman tools hook) checks for it.
- The cwd: if the user ran `priya run` from `/home/u/projects/api`, the agent starts in the same path inside the fork.
- Stream stdout/stderr to the task log.

For **boot checks** (`priya update`), boot the fork: `systemd-nspawn -b -D forks/<id>/root --machine=priya-<short-id> ...`, wait for `systemctl is-system-running --wait` via `systemd-run --machine=priya-<short-id> --wait --pipe`, collect `systemctl --failed --plain --no-legend`, then `machinectl poweroff priya-<short-id>`. Timeout 120 s.

## 4. Diff

Two layers: a **file diff** (exact) and a **summary** (human-friendly).

### 4.1 File diff

1. Freeze the fork: `btrfs subvolume snapshot -r forks/<id>/root forks/<id>/final-root` (same for home).
2. List touched paths: `btrfs send --no-data -p base-root final-root | btrfs receive --dump`. Parse every line and collect **all paths mentioned** (both sides of `rename`/`link`). Ignore btrfs's temporary `o<ino>-<gen>-<n>` names. Do not try to replay the operations.
3. For each touched path, `lstat` it in base and in final and classify:
   - `added` (missing in base), `deleted` (missing in final), `modified` (content or type differs), `meta` (only mode/owner/xattrs differ), or drop it (identical, e.g. a temp file created and removed).
   - Content comparison: size first, then BLAKE3 hash. Symlinks: compare targets.
4. Drop paths matching the **ignore list** (configurable, defaults): `/tmp/**`, `/var/tmp/**`, `/var/cache/**`, `/var/log/**`, `/run/**`, `/home/*/.cache/**`, `/var/lib/systemd/random-seed`, `/etc/.pwd.lock`, `/var/lib/pacman/db.lck`, `/home/*/.bash_history`, `/home/*/.zsh_history`, `/home/*/.claude/projects/**`, `/home/*/.claude/statsig/**`.

Put the dump parser and the classifier in `priya-core` with unit tests built from recorded dump output (commit sample dumps under `crates/priya-core/tests/fixtures/`).

### 4.2 Summary

Computed from the file diff plus:

- **Packages**: `pacman -Q --root base-root` vs `pacman -Q --root final-root` → added / removed / upgraded (old → new version).
- **Services**: symlinks under `/etc/systemd/system/*.wants/` and `/home/*/.config/systemd/user/*.wants/` → enabled / disabled units.
- **Groups**: `system` = changes under `final-root`, `home` = changes under `final-home`; within home, group by top-level project directory.

Example output of `priya diff <id>`:

```
Task 01J9Z3K2  "set up docker for the api project"   ✔ check passed
System (needs reboot)
  + packages   docker 28.1.1, docker-compose 2.36.0
  + services   docker.service
  ~ files      /etc/docker/daemon.json (new)
Home
  ~ projects/api   4 modified, 1 added
      M docker-compose.yml   M README.md   M .env.example   M Makefile   A Dockerfile
```

Persist the diff in SQLite (`changes` table) so review and merge use the same list.

## 5. Merge

Merge always re-checks the live machine first. Home and system merge separately; a task can have either or both.

### 5.1 Home merge (live, no reboot)

Three-way per path, `base = base-home`, `live = /home` (via `top/@home`), `fork = final-home`:

| live vs base | Action |
|---|---|
| live identical to base | apply fork's version |
| live differs from base and fork's version equals live | nothing to do |
| live differs from base otherwise | **conflict** — skip, record |

Apply:
- added/modified file → `cp --reflink=always --preserve=mode,ownership,timestamps,xattr` from fork to a temp name in the same directory, then `rename` over the target (atomic per file).
- deleted → `unlink` / `rmdir` (only if empty).
- meta → `chmod`/`chown`.
- Create parent directories as needed with the fork's mode and owner.

Conflicts are shown in `priya review` with choices: keep mine / take agent's / open both in `$EDITOR`. Merge of non-conflicting paths proceeds; the task state becomes `merged_with_conflicts` until each conflict is resolved or dismissed.

### 5.2 System merge (takes effect on reboot)

1. **Drift check**: `btrfs subvolume find-new top/@ <base_root_generation>` lists files changed on the live root since the fork. Filter with the ignore list. If anything remains, refuse with `system changed since fork: <paths>` unless `--force`. The user should re-run the task.
2. **Swap**:
   ```
   mv top/@  top/@priya-prev-<UTC timestamp>
   btrfs subvolume snapshot forks/<id>/final-root  top/@
   ```
   The running system keeps working: its mount refers to the old subvolume by id, which still exists under its new name.
3. Regenerate the GRUB config inside the new `@` only if the kernel or GRUB packages changed: run `grub-mkconfig -o /boot/grub/grub.cfg` inside an nspawn of `top/@` (not booted).
4. Mark task `merged_pending_reboot`. Notify: "Restart to finish applying system changes."
5. Delete old `@priya-prev-*` beyond `keep_prev_roots`.

**Rollback**: `priya rollback` = the same swap in reverse with the most recent `@priya-prev-*`, also on reboot. Always offer it in the GRUB menu too (after week 1).

## 6. /boot and the kernel

PriyaOS installs with **GRUB and `/boot` inside `@`** (ESP mounted at `/efi`, holding only GRUB's EFI binary). GRUB reads btrfs and loads the kernel from `/@/boot`, so kernels are part of every snapshot and system merges and rollbacks include the kernel automatically.

If the machine has `/boot` on a separate partition (typical existing Arch with systemd-boot), `priya doctor` warns, and `priya update` passes `--ignore linux,linux-lts,linux-zen,linux-hardened,linux-firmware*,intel-ucode,amd-ucode,systemd-boot*` and says so in the summary. Never write to the ESP from a merge in week 1.

## 7. Cleanup

- `priya discard <id>` deletes all subvolumes under `forks/<id>/` (`btrfs subvolume delete`) and the directory.
- After a successful merge, delete the fork (keep the diff and logs in SQLite).
- `priya gc` deletes forks of tasks in `failed`, `discarded`, `cancelled` older than 24 h, and warns when free space on the filesystem is below 10 %.

## 8. Failure rules

- Every step that changes the live machine (home merge, swap, fstab fix) writes a journal entry to SQLite **before** acting, and a completion entry after. On startup priyad finds unfinished entries and reports them; it never auto-retries a live change.
- Never delete `@`, `@home`, `@priya` or the newest `@priya-prev-*`.
- Never run any command against block devices other than via `btrfs` subvolume operations on the configured filesystem.
