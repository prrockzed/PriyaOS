# 02 — Core: events, tasks, plans, API, modules, settings

## 1. Event store

SQLite (WAL) table, append-only. Never `UPDATE` or `DELETE` an event (retention compaction is the only exception, §1.4).

```sql
CREATE TABLE events (
  seq          INTEGER PRIMARY KEY AUTOINCREMENT,  -- global order; never reused
  id           TEXT NOT NULL UNIQUE,               -- ULID
  ts           TEXT NOT NULL,                      -- RFC 3339 UTC
  type         TEXT NOT NULL,                      -- e.g. 'task.created'
  version      INTEGER NOT NULL CHECK (version >= 1),  -- schema version of this event type
  stream       TEXT NOT NULL,                      -- aggregate id, e.g. 'task:01J…'
  correlation  TEXT,                               -- the task id the event belongs to
  causation    TEXT,                               -- event id that caused this one
  actor        TEXT NOT NULL,                      -- 'user' | 'priya-ai' | 'agent:<name>' | 'module:<name>' | 'policy' | 'system'
  payload      TEXT NOT NULL CHECK (json_valid(payload)),  -- JSON, validated against the type's schema
  prev_hash    TEXT NOT NULL CHECK (length(prev_hash) = 64),  -- hash chain → tamper-evident audit
  hash         TEXT NOT NULL CHECK (length(hash) = 64)        -- BLAKE3(prev_hash || JCS(row without hash))
) STRICT;
CREATE INDEX events_stream ON events(stream, seq);
CREATE INDEX events_type ON events(type, seq);
-- Triggers raise on UPDATE and DELETE; only compaction (§1.4) lifts them.
```

### 1.1 Event types (initial catalogue, extended by modules)

| Area | Types |
|---|---|
| Intent | `intent.submitted`, `intent.classified`, `intent.answered`, `intent.cancelled` |
| Task | `task.created`, `task.state_changed`, `task.budget_exceeded`, `task.cancelled` |
| Plan | `plan.proposed`, `plan.approved`, `plan.revised`, `step.ready`, `step.completed`, `step.failed` |
| Run | `run.started`, `run.output`, `run.tool_called`, `run.usage`, `run.heartbeat`, `run.finished` |
| Model | `model.called` (provider, model, tokens in/out/cached, cost, latency, role, outcome ref) |
| Change | `changeset.recorded`, `merge.applied`, `merge.conflict`, `revert.applied`, `system.swapped`, `system.rolled_back` |
| Policy | `policy.decision`, `approval.requested`, `approval.granted`, `approval.denied`, `taint.added` |
| Settings | `setting.changed` (key, old, new, source: user / natural-language / module / file / rewind), `settings.load_failed` (file, error) |
| Module | `module.started`, `module.stopped`, `module.crashed`, `module.event` (namespaced custom events) |
| Daemon | `daemon.started`, `daemon.stopped` (actor `system`: priyad itself) |

Event payload schemas live in `priya-api::events` as Rust types with `schemars`; the JSON Schema is exported to `docs/claude/api/` at build time (0.9). `priya_api::events::REGISTRY` lists every `(type, version)` that may be written, and payloads reject unknown fields. When a type gains a version, an upcaster from the previous one goes into `UPCASTERS` in the same PR. Phase 0 registers: `setting.changed`, `settings.load_failed`, `intent.submitted`, `intent.answered`, `intent.cancelled`, `model.called` (costs in millionths of a dollar, so sums are exact), `module.started`, `module.stopped`, `module.crashed`, `module.event`, `daemon.started`, `daemon.stopped`.

The catalogue above is a starting point: each phase file adds the event types it needs (e.g. `fork.*`, `run.tool_result`, `run.stalled`, `check.*`, `approval.expired`, `killswitch.engaged`, `ai.*`), additively and with schema snapshots, and updates this table in the same PR.

### 1.2 Projections

Read models built from events, rebuilt on schema change: `tasks`, `plans`, `steps`, `runs`, `changesets`, `approvals`, `costs_daily`, `model_stats` (for routing), `history_fts` (FTS5 search over titles, prompts, final answers, ChangeSet summaries). Projections are updated in the same transaction as the event append. Each projection gets the full payload (loaded from its blob if needed); one that missed events catches up from its `last_seq` when the store opens, and an event it can't decode is skipped with a warning rather than failing the append.

### 1.3 Subscriptions

Surfaces and modules subscribe with a filter (`types`, `stream` prefix) and a starting `seq`; delivery is at-least-once, ordered per stream; consumers store their last `seq`. This is the only way modules learn about state changes.

### 1.4 Retention and forgetting

When the user asks Priya to forget something (memory, a task), a `redaction.performed` event replaces the affected payloads with a tombstone and records the gap in the hash chain (ADR-019), the same mechanism compaction uses.


Run output (`run.output`) can be large: payloads over 8 KiB are stored as blobs and referenced. Compaction (configurable, default: keep 180 days of `run.output`, everything else forever) writes a `compaction.performed` event; the hash chain records the gap.

## 2. Task state machine

```
created ─▶ planning ─▶ awaiting_plan_approval? ─▶ running ─▶ verifying ─▶ awaiting_review ─┬▶ merging ─▶ done
   │                                                │                                     │           └▶ done_pending_reboot
   │                                                │                                     │           └▶ done_with_conflicts
   │                                                │                                     └▶ discarded
   └──────────── any active state ─▶ failed | cancelled | paused (resume ─▶ previous state)
```

- `answer`-only tasks skip to `done` after `intent.answered`.
- Direct-mode tasks go `verifying ─▶ awaiting_review` where review means **keep** or **revert** (the change is already live).
- Transitions are a pure function in `priya-core`: `fn apply(state, event) -> Result<state>`; table-driven tests cover every edge.

## 3. Plans

A plan is a DAG of steps. Each step:

```rust
struct Step {
    id: StepId,
    goal: String,                    // natural-language goal for the agent
    depends_on: Vec<StepId>,
    agent: AgentChoice,              // Auto | Specific(agent) — resolved by Priya Orchestration
    model_role: Role,                // Auto | Fast | Smart | Coding | Vision | Private | Specific(model)
    mode: ModeChoice,                // Auto | ReadOnly | Sandbox | Direct | DirectRaw
    workspace: WorkspaceShare,       // Own | SharedWith(StepId) — share one fork across sequential steps
    budget: Budget,                  // secs, usd, tokens
    verify: Vec<Verifier>,           // tests, boot check, review-by-other-family, custom command
    capabilities: CapabilityRequest, // paths, network domains, privileged ops, secrets
}
```

Simple tasks get a one-step plan without user-visible planning. Plan approval is required when policy says so (e.g. >3 steps, any `Direct` step, any external action, est. cost > threshold).

## 4. Public API (surfaces ↔ priyad)

JSON-RPC 2.0 over `$XDG_RUNTIME_DIR/priya/priyad.sock` (mode 0600). Versioned namespace `v1.`. Every method has a CLI equivalent.

**Framing**: newline-delimited JSON. Each line is one compact JSON value (a request, a notification, a response, or a batch array), at most 16 MiB; a longer line is refused with −32600 and the connection carries on with the next line. A trailing `\r` is ignored and doesn't count toward the limit, blank lines are skipped, and a last line without a newline still counts when the stream ends. A batch holds at most 1024 messages. A message with `"id": null` is a call (answered with a null id); only one without an `id` member is a notification, which is never answered. Ids may be any JSON number or string. `params` must be named (an object); absent or null means none. Requests refuse members JSON-RPC doesn't define, so adding one is a deliberate API change. A subscription's first notification always comes after the response that names it. This is what Quickshell's `Socket` with a `SplitParser` reads. Module stdio uses `Content-Length` framing instead (§5.2). The types are in `priya_api::rpc` and `priya_api::framing`, and every method's params and result types, with JSON Schemas, are in `priya_api::methods` (`methods::ALL`).

| Group | Methods |
|---|---|
| Intent | `v1.intent.submit {text?, audio_ref?, images?, screen?, context}` → `{task_id \| answer}`; `v1.intent.stream`; `v1.intent.classify` (dry run for UI hints) |
| Tasks | `v1.task.create` (typed task without natural language, used by CLI commands), `v1.task.list`, `get`, `cancel`, `pause`, `resume`, `steer {task_id, message}` |
| Plans | `v1.plan.get`, `approve`, `edit` |
| Changes | `v1.change.get {changeset_id}`, `file_diff`, `merge {parts}`, `resolve {path, choice}`, `discard`, `keep`, `revert` |
| System | `v1.system.status`, `v1.system.methods` (introspection: every method with its schemas), `v1.system.doctor {fix?: dry_run \| apply}`, `v1.system.rollback` |
| Audit | `v1.audit.verify {from?, to?}` (checks the hash chain) |
| AI | `v1.ai.models`, `v1.ai.use {strategy \| model \| family}`, `v1.ai.stats`, `v1.ai.ask {prompt, role}` (for modules/surfaces) |
| Agents | `v1.agents.list`, `v1.agents.enable/disable` |
| Settings | `v1.settings.get {key?, explain?}`, `set {key, value}`, `unset {key}`, `schema`, `nl_change {text}` (natural-language settings → validated patch) |
| Modules | `v1.module.call {module, method, params}`, `v1.modules.list` |
| History | `v1.history.search`, `v1.events.subscribe {types?, stream_prefix?, from_seq?}` → `{subscription}` then `v1.events.event {subscription, event}` notifications, `v1.events.unsubscribe {subscription}`. A stream that stops on its own (daemon stopping, unreadable history) sends `v1.events.ended {subscription, reason: cancelled\|error, message}` last. At most 64 subscriptions per connection and 64 type patterns of 256 bytes each; a subscribe sent as a notification is ignored |
| Approvals | `v1.approval.list`, `decide {id, allow, remember?}` |

Errors: JSON-RPC error codes + `data: {kind, retryable, user_message, detail?}`; `user_message` is always plain language. `message` is the standard text for standard codes ("Parse error", …) and the kind otherwise. An error received without `data` (from a module) gets one made up from its code and message. Standard codes: −32700 parse error, −32600 invalid request, −32601 method not found, −32602 invalid params, −32603 internal. Application codes, one per kind: −32001 `not_found`, −32002 `invalid_input`, −32003 `conflict`, −32004 `permission_denied`, −32005 `unavailable`, −32006 `timeout`, −32007 `cancelled`, −32008 `provider_error` (`internal` uses −32603).

## 5. Module system

### 5.1 Manifest — `module.toml`

```toml
[module]
name = "updates"
version = "0.1.0"
api = "v1"
kind = "process"                 # "builtin" (Rust, in priyad) | "process" (any language, stdio JSON-RPC)
exec = "/usr/lib/priya/modules/updates"
description = "Safe system updates tested in Priya Sandbox"

[provides]
methods = ["updates.check", "updates.run"]                 # callable via v1.module.call
intents = ["update the system", "upgrade packages"]        # examples Priya AI uses to route intents here
tools = ["updates_status"]                                 # exposed to agents via MCP
settings_schema = "settings.schema.json"
surfaces = [{ kind = "bar_item", id = "updates" }, { kind = "command", id = "update-now", title = "Update system safely" }]

[subscribes]
events = ["system.swapped", "timer.daily"]

[requires]
capabilities = ["helper.package_transaction", "helper.sandbox", "ai.role:smart", "network:mirrors"]
default_mode = "sandbox"                                   # per-feature default execution mode
```

### 5.2 Host

- priyad starts enabled modules, supervises them (restart with exponential backoff, max 5/min, then `module.crashed` + notification), and enforces declared capabilities: a module's calls to priyad are checked against its manifest; process modules run in their own systemd scope with seccomp/Landlock profiles derived from the manifest.
- Module ↔ priyad protocol: JSON-RPC over stdio, LSP-style framing (`Content-Length`). Calls: `host.emit_event`, `host.call`, `host.ai.ask`, `host.task.create`, `host.settings.get`, `host.secret.get` (only declared secrets).
- Intent routing: Priya AI's classifier sees each module's `intents` examples; a matching intent becomes a task owned by that module.

### 5.3 Built-in vs process modules

Core-adjacent modules that need tight integration (desktop events, tool index) may be `builtin`; everything else defaults to `process`. Both use the identical API, so a module can move between kinds without changing behaviour.

## 6. Settings

### 6.1 Layers (later overrides earlier)

1. Shipped defaults: `/usr/share/priya/defaults/*.toml`
2. System: `/etc/priya/*.toml` (admin)
3. User: `~/.config/priya/*.toml`
4. Project: `.priya.toml` in a project directory (applies to tasks with that cwd)
5. Task: overrides in the intent ("use Opus for this", "do it directly")

An admin can **lock** a key in `/etc/priya/*.toml` (`locked = ["ai.strategy", …]`); locked keys can't be overridden by user, project or task layers, and the UI shows them as locked with the reason.

Every key is typed and has a JSON Schema (generated from Rust types with `schemars`). Modules contribute their own schema sections. Settings hot-reload via inotify (the system and user folders; project files are read fresh for each request); invalid files are rejected with a plain-language error, last valid config stays active.

### 6.2 Natural-language settings

`v1.settings.nl_change {text: "only use local models for anything in ~/Documents"}`:
1. Priya AI (role `smart`) receives the schema subset and current values, returns a JSON Patch.
2. The patch is validated against the schema and policy (some keys require admin or confirmation).
3. The user sees a plain-language summary + diff → confirm → applied as a `setting.changed` event.
4. Settings changes are ChangeSets of kind `settings`, so Priya Rewind can undo them ("undo what I changed in settings yesterday").
