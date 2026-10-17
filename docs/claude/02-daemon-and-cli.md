# 02 — priyad and the priya CLI

## priyad

- Runs as root: `priyad.service` (`Type=notify`, `Restart=on-failure`).
- On start: mount the btrfs top level at `/run/priya/top`, open `/var/lib/priya/priya.db`, recover unfinished journal entries (report, don't retry), mark tasks that were `running` as `failed` with reason `daemon restarted`, create `/run/priya/fork-marker`.
- Listens on `/run/priya/priyad.sock`, mode `0660`, group `priya`. Authorize with `SO_PEERCRED`: uid 0 or a member of group `priya`. The requesting uid is recorded on the task; agents run as that user inside the fork.
- Runs at most `max_parallel_tasks` (default 2) tasks at once; others wait in `queued`.

### Task states

```
queued → forking → running → checking → review ─┬─ merge → merging → merged
                                                │                 └→ merged_pending_reboot
                                                │                 └→ merged_with_conflicts
                                                └─ discard → discarded
any active state → failed | cancelled
```

The state machine lives in `priya-core` (pure, unit tested): `fn transition(state, event) -> Result<State>`.

### SQLite schema (initial)

```sql
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,            -- ULID
  created_at TEXT NOT NULL,
  uid INTEGER NOT NULL,
  kind TEXT NOT NULL,             -- 'agent' | 'command' | 'update'
  title TEXT NOT NULL,            -- first 80 chars of prompt
  prompt TEXT,
  agent TEXT,                     -- 'claude' | 'codex' | NULL
  cwd TEXT,
  state TEXT NOT NULL,
  budget_secs INTEGER, budget_usd REAL,
  usd_used REAL DEFAULT 0, tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0,
  tainted INTEGER DEFAULT 0, taint_reasons TEXT,      -- JSON array
  check_cmd TEXT, check_status TEXT, check_output TEXT,
  fork_dir TEXT, base_root_gen INTEGER, base_home_gen INTEGER,
  error TEXT, finished_at TEXT
);
CREATE TABLE events (            -- agent output, tool calls, state changes
  task_id TEXT, ts TEXT, kind TEXT, payload TEXT
);
CREATE TABLE changes (
  task_id TEXT, area TEXT, path TEXT, kind TEXT,   -- area: system|home; kind: added|modified|deleted|meta
  conflict INTEGER DEFAULT 0, resolution TEXT
);
CREATE TABLE summary (task_id TEXT PRIMARY KEY, json TEXT);
CREATE TABLE journal (            -- live-machine changes, see 01 §8
  id INTEGER PRIMARY KEY, task_id TEXT, ts TEXT, action TEXT, detail TEXT, done INTEGER DEFAULT 0
);
CREATE VIRTUAL TABLE history USING fts5(task_id, title, prompt, summary_text, agent_final_message);
```

Use simple numbered migrations in code (`PRAGMA user_version`).

### JSON-RPC methods

Newline-delimited JSON-RPC 2.0. Each method maps 1:1 to a CLI command.

| Method | Params | Result |
|---|---|---|
| `task.create` | `{kind, prompt?, agent?, cmd?, cwd, budget_secs?, budget_usd?, check?, scope?}` | `{id}` |
| `task.list` | `{all?: bool}` | `[TaskRow]` |
| `task.get` | `{id}` | `TaskRow + summary` |
| `task.logs` | `{id, follow?: bool}` | stream of `events` notifications |
| `task.cancel` | `{id}` | `{}` |
| `task.diff` | `{id}` | `{summary, changes}` |
| `task.file_diff` | `{id, path}` | `{base, fork, live}` text (≤1 MiB each, else `binary: true`) |
| `task.merge` | `{id, parts: ["home","system"], force?: bool, confirm_tainted?: bool}` | `{state, conflicts}` |
| `task.resolve` | `{id, path, choice: "mine"|"agent"}` | `{}` |
| `task.discard` | `{id}` | `{}` |
| `system.rollback` | `{}` | `{state}` |
| `system.doctor` | `{fix?: bool}` | `[Check{name, ok, detail, fixable}]` |
| `events.subscribe` | `{}` | stream of `{task_id, state, title}` notifications |
| `history.search` | `{query}` | `[{task_id, title, snippet}]` |

IDs: accept any unique prefix of the ULID (8 chars shown in UI).

## priya CLI

```
priya run "<prompt>" [--agent claude|codex] [--budget 30m] [--max-usd 2] [--check "<cmd>"] [--detach]
priya run --cmd "<shell command>"            # no agent, just run a command in a fork
priya update [--fix/--no-fix]                # see 03 §5
priya ps [-a]                                # task table
priya logs <id> [-f]
priya diff <id>                              # summary (see 01 §4.2)
priya review [<id>]                          # TUI; without id → newest task in review
priya merge <id> [--home-only|--system-only] [--force]
priya discard <id>
priya kill <id>
priya rollback
priya history "<query>"
priya doctor [--fix]
priya status --waybar                        # JSON for Waybar (04)
priya ask                                    # fuzzel prompt → priya run (04)
priya tools index --stdin | search <q>       # 05
priya mcp                                    # MCP stdio server (05)
priya gc
```

`priya run` without `--detach` follows the logs and, when the task reaches `review`, prints the diff summary and asks `Merge? [y/N/r=review]`.

Output: plain text by default, `--json` on every read command.

### Review TUI (`priya review`)

```
┌ Task 01J9Z3K2 · set up docker for the api project · claude · $0.42 · 6m12s · ✔ check ┐
│ SYSTEM (needs reboot)            │ /home/u/projects/api/docker-compose.yml           │
│  + docker 28.1.1                 │ @@ -3,6 +3,12 @@                                   │
│  + docker.service                │ + services:                                       │
│  ~ /etc/docker/daemon.json       │ +   db:                                           │
│ HOME                             │ +     image: postgres:16                          │
│ ▸ projects/api  (5)              │ ...                                               │
│ CONFLICTS (0)                    │                                                   │
└ m merge  d discard  l logs  c check output  ⏎ open  q quit ──────────────────────────┘
```

- Tainted tasks show a red `UNTRUSTED INPUT` banner with the reasons, and `m` asks the user to type `merge` to confirm.
- Conflicts section lists conflicting paths; `1` keep mine, `2` take agent's, `e` open both in `$EDITOR`.
