# 08 — Priya Rewind, memory, tool index

## 1. Priya Rewind

Goal: see and undo any change on the machine — made by an agent, by a module, by a settings change, or by the user by hand — by browsing or by plain language ("undo whatever broke my Wi-Fi on Tuesday", "remove the Docker setup but keep my later edits", "what changed in /etc last week?").

### 1.1 Sources of history

| Source | How it's captured | Granularity |
|---|---|---|
| Priya runs (sandbox, direct) | ChangeSets with before/after blobs ([03](03-execution-and-sandbox.md) §5) | exact, attributed |
| Settings changes | `setting.changed` events | exact |
| Manual changes | **Timeline snapshots**: read-only snapshots of `@` and `@home` into `@priya-timeline/` every hour (configurable) and before/after every pacman transaction (pacman hook) | between snapshots |

Timeline retention (default): hourly for 48 h, daily for 30 days, weekly for 6 months, plus always the snapshots that bracket a merged ChangeSet until its blobs expire. A manual ChangeSet between two timeline snapshots is computed lazily on demand (same diff code as forks).

Scope rules: Rewind never reverts its own state (`~/.local/share/priya/**`, `@priya`, `@priya-timeline`); timeline snapshots are machine-level (taken by the helper, indexed in `system.db`), and each user's view shows system entries plus only their own home; root-only file contents are kept as hashes only; manual ChangeSets subtract Priya ChangeSets merged in the same window; snapshot space is measured with btrfs simple quotas (squota). Live actions (ADR-016) appear in the timeline too.

### 1.2 Operations

- **Browse**: `priya rewind log [--path P] [--since T]` → unified timeline of ChangeSets (Priya runs, settings, manual windows).
- **Selective revert**: revert one ChangeSet (or a subset of its entries) on top of the current state — like `git revert`, not a time-travel reset. Implemented as a new task in sandbox mode: fork current state, apply inverse entries with three-way checks against later changes, show review, merge.
- **Restore a path** to its state at time T (`priya rewind restore ~/notes/plan.md --at "yesterday 6pm"`).
- **Full-system return** to a timeline point (system part on reboot, like rollback; home is untouched unless the user includes it).
- Package-aware inverses: reverting a ChangeSet that changed packages uses pacman operations in the fork (remove / reinstall cached versions) so the package database stays consistent; reverting a whole update is steered to full-system return, since partial downgrades are unsupported on Arch.

### 1.3 Undo by prompt

1. Priya AI turns the request into a query: time window, paths/areas/services/packages involved, symptoms ("Wi-Fi broke").
2. Candidate ChangeSets are retrieved from `history_fts` + structured filters (paths under `/etc/NetworkManager`, packages `networkmanager`/`iwd`/`linux-firmware`, services) + timeline manual windows.
3. Priya AI ranks candidates and explains each in plain language ("Tuesday 14:02 the update task upgraded linux-firmware; Tuesday 18:40 you edited /etc/NetworkManager/conf.d/wifi.conf").
4. User picks (or "do the most likely one") → selective-revert task in sandbox → optional verifier (e.g. "is Wi-Fi up?" via a boot check or a live check) → review → merge.

### 1.4 Storage

Blob store on `@priya`: content-addressed (BLAKE3), zstd-compressed, reference-counted by ChangeSets; retention per ChangeSet origin (default 90 days for runs, follows timeline retention for manual). `priya rewind usage` shows space used; low disk space triggers pruning oldest-first with a notification.

## 2. Memory

Layers, all local, all inspectable, editable and deletable (`priya memory …` and the settings GUI):

| Layer | Content | Store |
|---|---|---|
| Task history | Every task: intent, plan, answer, ChangeSet summary, outcome | events + `history_fts` |
| Project memory | Per project: conventions, decisions, commands that work, past tasks (auto-summarised after tasks, user-editable markdown at `<project>/.priya/memory.md` or `~/.local/share/priya/projects/<id>.md`) | files + index |
| Personal memory | Preferences ("I use pnpm", "answer briefly"), facts the user asked to remember | `~/.local/share/priya/memory/*.md` |
| File index | Semantic search over permitted folders (default: none until the user picks; suggested: `~/Documents`, `~/projects`) | local embeddings (embedding model via Priya AI role `embed`, local by default) in `sqlite-vec` |

- Separate **profiles** (personal / work) with separate memory and file indexes; the active profile follows the project path or a manual switch.
- Memory retrieval for agents goes through the `priya mcp` tools; nothing is injected silently into prompts except a short, visible "context used" list in the run log.
- Excluded paths and file types (secrets, mask list, `.env`, keys) are never indexed.
- Overnight consolidation ("dreaming", Phase 13): summarise the day's tasks into project memory, deduplicate, flag stale memories, prepare the morning brief.

## 3. Tool index

Every installed program becomes a discoverable tool for agents.

- The system-wide index lives in `/var/lib/priya/tools/index.db`, built by `priya-tools-index.service` (`DynamicUser=yes`, no network); per-user embeddings of it live in the user's data dir. System merges re-index changed packages (pacman hooks don't run when a fork is swapped in).
- pacman hook on `usr/bin/*` install/upgrade (PostTransaction, NeedsTargets) → `priya tools index --stdin`; skipped inside forks (`/run/priya/fork`); always exits 0.
- Per binary: owning package, package description, first 60 lines of `--help` (or man page synopsis) captured under bubblewrap with no network and a 2 s timeout.
- Desktop apps: `.desktop` files; D-Bus services (introspection) for scriptable apps; AT-SPI for GUI-only apps (used by Priya Vision).
- Exposed through **`priya mcp`** (stdio MCP server) to every agent: `search_tools`, `describe_tool`, `task_history`, `project_memory`, `search_files` (permitted index only), `system_info`, `rewind_log` (read-only). Inside forks, a read-only socket serves only these methods.
- Later (Phase 16): app developers ship structured action manifests (MCP servers) that the tool index registers automatically.
