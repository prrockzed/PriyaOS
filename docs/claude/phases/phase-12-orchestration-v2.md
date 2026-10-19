# Phase 12 — Orchestration v2 and software lifecycle

**Goal.** Many agents across model families work together on one goal: multi-step plans the user can read and edit, parallel runs in isolated forks and git worktrees, a shared blackboard and handoffs, sub-agents that can never exceed their parent's permissions, cross-family verification, a conflict manager, a resource-aware scheduler and supervision that survives crashes and reboots — and, on top, the software lifecycle: understanding a repository, pull requests, CI diagnosis and approved deploys.

**Usable at the end:** "Add user accounts to my app: backend, frontend, tests, docs, and open a PR" → plan card with parallel lanes → edit "do the frontend with Gemini" → approve → watch several agents in the agent workspace → a different model family reviews each step → conflicts resolved → review → PR opened after approval → CI failure diagnosed and fixed → deploy to staging after approval.

**Depends on:** Phase 4 (agents, single-step supervision, checks), Phase 5 (bar cards, Task center, agent workspace, review comments), Phase 7 (Rewind for undoing merges), Phase 8 (project memory, tool index, `priya mcp` complete), Phase 3 (policy, approvals, budgets).

**Features:** ORC-12, ORC-13, ORC-14, ORC-15, ORC-16, ORC-17, ORC-18, ORC-19, ORC-20, ORC-21, ORC-22, DEV-02, DEV-03, DEV-04, DEV-05, DEV-06, DEV-07, DEV-08.

**Read first:** [06-orchestration](../architecture/06-orchestration.md) (all), [02 §2–3 task and plan](../architecture/02-core.md), [03 §3 forks, §5 ChangeSets, §6 limits](../architecture/03-execution-and-sandbox.md), [04 §5–7 routing, learning, agent levers](../architecture/04-priya-ai.md), [05 §2 capabilities and permission inheritance, §4 approvals, §5 taint](../architecture/05-security.md), [08 §2 memory](../architecture/08-rewind-memory-tools.md), [11 §4 agent workspace and Task center, §5 autonomy](../architecture/11-design-system-and-ux.md).

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 12.1 | Planner and plan DAG | Phase 4 | `priya plan "…"` produces a validated multi-step plan with estimates; plain-language edits revise it |
| 12.2 | Plan UI | 12.1 | Plan card shows lanes, chips and totals; approve or edit in place |
| 12.3 | Scheduler and resources | 12.1 | Independent steps run in parallel within CPU/RAM/battery/rate/budget limits |
| 12.4 | Workspaces (worktrees, shared forks, ports) | 12.3 | Parallel code steps each get a branch + worktree + port range; system steps get their own forks |
| 12.5 | Blackboard, messages, handoffs | 12.4 | Agents in one task share notes and a claimable task list; dependants receive handoffs |
| 12.6 | Sub-agents and permission inheritance | 12.3 | Sub-agents run with a provable subset of their parent's permissions and budget |
| 12.7 | Cross-family verifier | 12.3 | Every step is reviewed by a model (or agent) from another family; failures go back as revisions |
| 12.8 | Conflict manager | 12.4, 12.7 | Parallel results are merged in the best order; text conflicts auto-merged or shown; semantic ones asked |
| 12.9 | Supervision v2 | 12.3 | Stuck agents are nudged, escalated or replaced; plans resume after crash or reboot |
| 12.10 | More agent adapters | Phase 4 | Grok Build, Aider, Goose, Cursor CLI, Copilot CLI usable as installed |
| 12.11 | Multi-agent views and scoreboards | 12.3, 12.9 | Agent workspace shows many runs by task; per-agent/model success and cost scoreboards |
| 12.12 | Repository understanding and pull requests | 12.4, 12.7 | "Explain this repo"; PRs opened with summaries after approval; protected merges need confirmation |
| 12.13 | CI diagnosis | 12.12 | Failed CI on a PR is diagnosed in plain language and a fix task is offered |
| 12.14 | Deploys | 12.12 | Project-defined deploys run after approval, are health-checked and can be rolled back |
| 12.15 | Phase gate | all | Full multi-agent demo on the Zenbook |

---

### 12.1 — Planner and plan DAG
**Outcome:** complex intents produce a validated DAG of steps (goal, dependencies, agent/model role, mode, workspace sharing, budget, verifiers, capabilities) with cost and time estimates; plain-language edits and re-planning produce `plan.revised`.
**Features:** ORC-12.
**Depends on:** Phase 4.

**Tasks**
- [ ] `PlanDag` in `priya-core`: validation (acyclic, dependencies exist, `SharedWith` only along a dependency chain, step budgets ≤ task budget, capabilities ⊆ task capabilities), topological order, critical path, parallel width; property tests.
- [ ] Planner in `priya-orchestra` (role `smart`): input = intent, project memory, agent registry summary, `.priya.toml`, top tool-index hits; output constrained to the plan JSON Schema; up to 2 repair rounds on invalid output, then a single-step fallback plan.
- [ ] Estimator: per-step cost/time ranges from `model_stats` and agent stats (p50/p90); plan totals use the critical path for time and the sum for cost; event `plan.estimated`.
- [ ] Plan approval rules: existing policy triggers (> 3 steps, any `Direct` step, external action, cost above threshold) combined with the autonomy level (11 §5); task enters `awaiting_plan_approval`.
- [ ] Plain-language edits: `v1.plan.edit {plan_id, text}` → planner returns a JSON Patch over the DAG → validated → before/after diff → user confirms → `plan.revised {source: "user"}`. CLI `priya plan edit <id> "skip the deploy"`.
- [ ] Re-planning: when a step exhausts retries (12.9), the planner receives completed outputs and the failure, revises only not-yet-started steps; `plan.revised {source: "replan", reason}`; requires re-approval if the revision adds a trigger from the approval rules.
- [ ] CLI `priya plan show|approve|edit|estimate` with ASCII lanes; `v1.plan.get` returns layout hints (lanes) so every surface draws the same picture.

**Tests**
- Unit: DAG validation table (cycles, dangling deps, illegal sharing, budget overflow, capability escalation), topological order, critical path; estimator maths on fixture stats; JSON Patch application and rejection.
- Contract: plan JSON Schema snapshot (schema compatibility); planner against the fake provider with recorded plan outputs (valid, invalid → repaired, garbage → fallback).
- Integration: "add a /health endpoint and docs" → 2–4 step plan; edit "do the docs with Gemini" → only the docs step's agent changes; failing step → re-plan keeps completed steps.
- Eval: 40 labelled intents → plan quality rubric (correct decomposition, sensible parallelism, no missing verifier) ≥ 80 % pass; simple intents stay single-step ≥ 95 %.
- Regression: `plan_dag_validation`, `plan_edit_by_language`, `replan_keeps_completed_steps`, `simple_intent_single_step`.

**Done when**
- [ ] Eval thresholds met; every plan has an estimate shown with a range.
- [ ] Invalid planner output never reaches the user (fallback path covered).

---

### 12.2 — Plan UI
**Outcome:** the plan card (bar and Task center) shows steps in parallel lanes with agent, model, mode, budget and verifier chips, totals and risks; the user approves, edits by chip or by plain language, and watches step states live.
**Features:** ORC-12.
**Depends on:** 12.1.

**Tasks**
- [ ] `PriyaUI` components `PlanGraph` (lanes + dependency connectors, virtualised), `StepRow` (goal, agent glyph + label, model chip, mode chip with colour + word, budget, verifier icons), `PlanTotals` (cost range, time range, approvals expected), added to the gallery first.
- [ ] Plan card in the bar (extends the Phase 4 single-step preview): collapsed summary ("5 steps · 2 in parallel · about 12 min · $0.40–0.90"), expand to the graph.
- [ ] In-place edits: change a step's agent/model/mode/budget via chips; "Ask to change the plan" input → `v1.plan.edit`; revision diff highlights changed steps.
- [ ] Live states in the Task center Overview tab: queued, waiting on dependency, running, verifying, needs you, done, failed, skipped, re-planned.
- [ ] Approve / Approve and don't ask for this kind / Cancel; keyboard Y/N; plan approval also available from notifications.

**UI**
- States: proposing (streaming steps in), proposed, editing, revised (diff highlight), approved, running, re-planned, step failed, cancelled, long plan (> 12 steps: collapsed groups), long goals, offline (cannot plan → message), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced motion (no connector animation).
- Keyboard: arrows move between steps, Enter opens a step, E edits a chip, Y approves, N cancels. Screen-reader: graph exposed as a list ("Step 3 of 5, runs after step 1, Gemini CLI, Sandbox, about $0.10"); never colour-only states.
- 👤 Mockup approval (`ui/mockups/12.2/`) before building; journey sign-off ("propose → edit by language → approve → watch").

**Human in the loop:** the user sees every step, who will do it, in which mode, at what cost, before anything runs (when the autonomy level asks); any chip can be changed; cancel at any time stops queued steps.

**Tests**
- UI: component tests (focus order, chip edits emit correct API calls); screenshots of every state × theme × scale; a11y lint; journey against mock priyad with scripted `plan.*`/`step.*` events.
- Perf: 30-step plan renders < 100 ms; live updates don't drop frames.
- Regression: `plan_card_states`, `plan_chip_edit_calls_api`, `plan_journey_edit_approve`.

**Done when**
- [ ] 👤 Gallery and journey signed off; owner edits a real plan by language and by chip.

---

### 12.3 — Scheduler and resources
**Outcome:** ready steps start automatically when dependencies are done, in priority order, within limits on concurrency, CPU, RAM, memory pressure, battery, provider rate limits and budgets; every wait has a plain reason.
**Features:** ORC-18.
**Depends on:** 12.1.

**Tasks**
- [ ] Ready queue in `priya-orchestra::scheduler`: priorities interactive > user-started background > automations; fairness between tasks (round-robin within a priority); pure decision function `next(state, resources) -> Vec<Start | Wait(reason)>`.
- [ ] Concurrency: `orchestration.max_concurrent_runs` (default 3) and the existing fork limit (default 4, 03 §6); worktrees count against runs, not forks.
- [ ] Per-run cgroup limits through the transient scope: `CPUQuota`, `MemoryMax`, `MemoryHigh`, `TasksMax`, `IOWeight` from step budget or defaults by agent type; OOM in a run → step failure with reason, never priyad.
- [ ] Battery and power: UPower over D-Bus; on battery below 20 % background runs are frozen (cgroup v2 freezer via the scope) and resume on AC; power-saver profile halves background concurrency.
- [ ] Pressure: PSI (`/proc/pressure/memory`, `cpu`) above thresholds delays new background starts.
- [ ] Provider and agent rate limits: token buckets per provider fed by response headers (`anthropic-ratelimit-*`, `x-ratelimit-*`, 429 `retry-after`); agent subscription limit errors parsed by adapters → `Wait(rate_limited, until)` or reassignment.
- [ ] Budget admission: a step starts only if remaining task/day budget ≥ its estimate lower bound; otherwise `needs you` with "raise budget / skip / cheaper model".
- [ ] `step.queued {reason}` events and `priya sched status` ("Waiting: on battery", "Waiting: Anthropic rate limit, about 40 s").

**Tests**
- Unit: scheduler decision table (priorities, fairness, limits, battery, PSI, budget), property test: never exceeds limits, never starves an interactive step.
- Integration (fake machine): 6 independent steps with fake agents → max 3 concurrent; simulated battery 15 % → background frozen, interactive continues; memory-hungry fake agent → killed by `MemoryMax`, priyad unaffected; fake 429 → wait then resume.
- Perf: scheduling decision < 5 ms for 50 ready steps; freeze/thaw of a run < 200 ms.
- Chaos: priyad killed while steps queued → queue rebuilt from events on restart.
- Regression: `sched_respects_concurrency`, `sched_battery_freezes_background`, `sched_rate_limit_waits`, `sched_budget_admission`.

**Done when**
- [ ] Owner sees parallel steps run on the Zenbook with plain wait reasons; unplugging below 20 % pauses background work and plugging in resumes it.

---

### 12.4 — Workspaces (worktrees, shared forks, ports)
**Outcome:** each step gets the right workspace automatically: a shared fork for sequential steps, one fork with a branch + git worktree + port range per parallel code step, separate forks for parallel system steps.
**Features:** ORC-13, DEV-03.
**Depends on:** 12.3.

**Tasks**
- [ ] Workspace allocator in `priya-orchestra::workspace` implementing the 06 §4 table as a pure function of (step kind, mode, sharing, parallel siblings); decision recorded in `run.started {workspace}`.
- [ ] Worktrees: inside the task's fork, `git worktree add -b priya/<task-short>/<step-short> <path> <base>`; base commit recorded; worktree paths under the fork's home; removed after merge/discard (branch kept until the task ends).
- [ ] Port ranges: allocator hands each workspace a block (default 20 ports from 41000) exported as `PRIYA_PORT_BASE`/`PRIYA_PORT_COUNT`; `.priya.toml` `run` commands can use them; collisions detected by bind test.
- [ ] Per-workspace `setup` from `.priya.toml` run once per worktree with shared caches bind-mounted (cargo registry, npm/pnpm store, pip cache) to keep setup fast.
- [ ] Parallel system steps: separate forks; merged one after another by the conflict manager (12.8) with drift checks.
- [ ] Direct mode in parallel: at most one direct step at a time per overlapping path set (Landlock path sets compared); others queue with reason.
- [ ] Cleanup and GC: worktrees and forks of finished/discarded tasks removed per 03 §6; `priya workspaces list|gc`.

**Tests**
- Unit: allocator decision table; port block allocation and release; path-set overlap for direct steps.
- Integration (fake machine): 3 parallel code steps on a test repo → 3 worktrees on distinct branches in one fork, distinct port blocks, each `check` passes; 2 parallel system steps → 2 forks; direct steps on overlapping paths serialised.
- Perf: worktree creation + setup with warm caches < 10 s on the test repo; fork creation stays < 2 s.
- Regression: `parallel_code_steps_get_worktrees`, `parallel_system_steps_get_forks`, `port_blocks_distinct`, `direct_steps_serialised_on_overlap`.

**Done when**
- [ ] A 3-lane code plan on a real project runs in parallel worktrees on the Zenbook and cleans up after itself (no stray worktrees or forks after `gc`).

---

### 12.5 — Blackboard, messages, handoffs
**Outcome:** all runs of a task share a notes board and a claimable task list through `priya mcp`; dependants receive concise handoffs; agents can message each other, labelled by sender, never carrying approval.
**Features:** ORC-15.
**Depends on:** 12.4.

**Tasks**
- [ ] Board store owned by priyad at `~/.local/share/priya/tasks/<id>/board/` (`board.md`, `tasks.json`, `messages.jsonl`), bind-mounted **read-only** into every workspace at `/priya/task/<id>/`; all writes go through `priya mcp` so priyad can lock, validate and log them.
- [ ] `priya mcp` tools: `board_read`, `board_note {text}`, `task_list`, `task_claim {id}`, `task_complete {id, summary}`, `send_message {to_step, text}`, `read_messages`; claims are leases (expire if the run dies).
- [ ] Handoff builder: on `step.completed`, compose a handoff (goal, ChangeSet summary, notes, artifacts, verifier results; ≤ 1.5k tokens, `fast` role compaction if longer) injected into dependants' prompts and shown in the Task center.
- [ ] Sender labels on every note and message (`agent:<id> step:<id>`); content from agents is treated as data, never instructions with authority; any "approval" text in a message is ignored by the approval engine.
- [ ] Taint propagation: notes/messages/handoffs from a tainted run taint every reader.
- [ ] Task center "Board" tab: notes, task list with claim owners, messages; the user can add notes (labelled "you").
- [ ] Events `board.noted`, `board.task_claimed`, `board.task_completed`, `message.sent`.

**UI**
- Board tab; states: empty, notes, claimed tasks, stale claim, many messages (virtualised), tainted source banner, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard navigation of lists; screen-reader names include sender.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: lease expiry, claim conflicts (two runs claim the same item → one wins), handoff size cap, taint propagation rule.
- Contract: MCP tool schemas snapshot.
- Integration: two fake agents in parallel claim from a 4-item list → no double work; tainted step's note → dependant run tainted; workspace write to the mounted board file fails (read-only).
- Security: message containing "the user approved deploy" → no approval recorded; agent cannot read another task's board.
- Regression: `board_claims_exclusive`, `handoff_reaches_dependant`, `message_cannot_approve`, `board_taint_propagates`.

**Done when**
- [ ] In a 3-step plan with two agent families, the dependant step demonstrably uses the handoff (asserted in its prompt fixture) and the board shows who did what.

---

### 12.6 — Sub-agents and permission inheritance
**Outcome:** any child run — planner-created, native-agent `spawn_subagent`, or a third-party agent's own sub-agent — has capabilities, budget and taint provably bounded by its parent.
**Features:** ORC-14.
**Depends on:** 12.3.

**Tasks**
- [ ] `CapabilitySet::is_subset_of` in `priya-policy` (paths by glob containment, domains, secrets, privileged ops, external actions) with property tests; enforced in `priya-orchestra` before any child run starts; violation → `policy.decision deny` + plain message.
- [ ] Budget split: children's budgets reserved from the parent's remaining budget; overspend stops the child, not the parent.
- [ ] Native agent tool `spawn_subagent {goal, capabilities?, model_role?}`: default = parent's set; narrower sets allowed; wider refused; depth limit 3, fan-out limit 5 (settings).
- [ ] Third-party internal sub-agents (Claude Code `parent_tool_use_id`, Codex/Gemini equivalents) run inside the parent's scope and workspace, so they inherit by construction; adapters normalise them as child nodes for UI and cost.
- [ ] Taint inheritance: child of a tainted parent is tainted; a tainted child taints the parent's ChangeSet.
- [ ] Audit: `run.started {parent_run, capabilities_digest}`; `priya task explain` shows the tree with capabilities.

**Tests**
- Unit: subset checks (glob edge cases `~/p/**` vs `~/p/a/**`, symlinks resolved, domain wildcards), budget reservation maths, depth/fan-out limits.
- Integration: native agent spawns a child with a wider path → refused; child overspends → child stopped, parent continues; Claude Code fixture with sub-agents → nested nodes with costs.
- Security: 25 escalation attempts (wider path, new secret, new domain, privileged op, external action, deeper nesting) → all denied and logged.
- Mutation: `cargo-mutants` on the subset checker — zero surviving mutants.
- Regression: `child_caps_subset_enforced`, `child_budget_reserved`, `child_taint_inherited`, `escalation_corpus_denied`.

**Done when**
- [ ] Escalation corpus 100 % denied; mutation run clean.

---

### 12.7 — Cross-family verifier
**Outcome:** after each step's checks, a `review`-role model from a different family (or, for code, a different agent in read-only mode) judges the diff against the goal; failures go back to the worker as a revision.
**Features:** ORC-17, DEV-04.
**Depends on:** 12.3.

**Tasks**
- [ ] Reviewer selection: hard constraint family ≠ worker family; if only one family is enabled, a different model tier is used and the review card says so.
- [ ] Verifier input builder: goal, step spec, trimmed diff (changed hunks + context), check results, project conventions from memory; output JSON `{verdict: pass|fail|unsure, issues: [{severity, path, line, reason}], confidence}` validated against a schema.
- [ ] Agent-review mode (DEV-04): for code steps, a different agent runs in a read-only workspace of the result with "review this change" and returns the same JSON via a `priya mcp` `submit_review` tool.
- [ ] Revision loop: `fail` → issues sent to the worker via `steer` or a new run on the same workspace (like ORC-23 review comments); max 2 rounds, then escalate (12.9) or ask the user.
- [ ] Verification budget: default ≤ 20 % of the step's spend; skipped (with a note) for trivial diffs below a size threshold unless policy requires it.
- [ ] Outcomes feed `model_stats`/agent stats (worker penalised on confirmed issues; reviewer credited when a later Rewind revert or test failure confirms its finding).
- [ ] Review card section "Checked by <model/agent>: 2 issues, 1 fixed" with expandable issues.

**UI**
- Checks section on the review card and Task center Checks tab; states: reviewing, pass, fail with issues, unsure, revised, skipped (trivial), same-family warning, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard expand/collapse; screen-reader reads verdict then issue count.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: family constraint, budget gate, JSON verdict validation, revision-round limits.
- Contract: verifier prompts against fake provider fixtures (pass, fail, malformed → retried once → `unsure`).
- Integration: seeded-bug step → `fail` → revision → `pass`; agent-review mode with Codex reviewing a Claude Code fixture diff.
- Eval: seeded-bug suite (≥ 30 cases across Rust/TS/Python/config) → catch rate ≥ 70 %; false alarms on 30 clean diffs ≤ 15 %; results tracked per (reviewer, worker) family pair.
- Regression: `verifier_cross_family`, `verifier_fail_triggers_revision`, `seeded_bug_eval`.

**Done when**
- [ ] Eval thresholds met with at least two family pairings; every step's review result is visible on the review card.

---

### 12.8 — Conflict manager
**Outcome:** when parallel steps finish, their ChangeSets are merged in the order that minimises conflicts; text conflicts are auto-merged or proposed as a separate diff; semantic conflicts become a plain question.
**Features:** ORC-16.
**Depends on:** 12.4, 12.7.

**Tasks**
- [ ] Overlap analysis in `priya-core`: same paths, same packages (different versions/choices), same services, same config keys (parsers for TOML, INI, JSON, YAML, `.env`), same port numbers; output `Overlap {kind, a, b}`.
- [ ] Merge ordering: dependency order first, then fewest overlaps (greedy), system changes last; recorded as `merge.plan`.
- [ ] Text merges: worktree branches merged with `git merge` (`merge-file` for non-repo files); on failure Priya AI (role `coding`) proposes a merged version as its **own** diff with its own verifier run; checks re-run on the merged result.
- [ ] Semantic conflicts: question card ("Step 2 chose PostgreSQL, step 3 chose SQLite. Which one?") with options and "ask Priya to decide"; the answer becomes a `steer` to the losing step or a small fix step.
- [ ] Conflict events `merge.conflict {kind, resolution: auto|ai_proposed|user}`; Task center Conflicts section.
- [ ] Integration with existing three-way home merge (03 §5.2) for the final merge into the live machine.

**UI**
- Conflicts section in the Task center and review: list, per-conflict view (both sides, AI proposal diff), question card; states: none, auto-resolved, AI proposal pending, needs your choice, resolved, failed, long diffs, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard choose-side keys (1/2/B/A); screen-reader labels sides by step and agent.
- 👤 Mockup approval before building; journey sign-off (forced conflict → choose → merged).

**Human in the loop:** the user sees every non-trivial resolution; AI merges are never applied without being shown as a diff; semantic choices are always the user's (or explicitly delegated).

**Tests**
- Unit: overlap detection per kind on fixtures; ordering algorithm (minimises conflicts on generated cases vs brute force for ≤ 6 ChangeSets).
- Integration: two worktrees editing the same function → `git merge` conflict → AI proposal diff → accepted → checks pass; package choice conflict → question card → answer → resolved.
- Eval: 20 conflict fixtures → AI proposal accepted by tests in ≥ 75 %.
- Regression: `conflict_text_ai_proposal`, `conflict_semantic_question`, `merge_order_minimises_conflicts`.

**Done when**
- [ ] Forced-conflict demo resolves with no silent data loss (both sides' intent preserved or explicitly chosen).

---

### 12.9 — Supervision v2
**Outcome:** stuck or failing runs are detected and handled by an escalation ladder; transient errors retry; plans survive priyad crashes and reboots and resume where they were.
**Features:** ORC-19, ORC-20.
**Depends on:** 12.3.

**Tasks**
- [ ] Stuck detector: no events for N minutes (per-agent default, 5 min), ≥ 3 identical consecutive tool calls, token spend without workspace change over a window, repeated identical errors; emits `run.stuck {signal}`.
- [ ] Escalation ladder (configurable): nudge via `steer` → same agent with a stronger model → different agent → re-plan (12.1) → fail with a plain explanation and options; each attempt a new run under the same step; reasons in events and notifications ("Codex got stuck on the migration; Claude Code took over").
- [ ] Transient errors (network, 5xx, 429) retried with jitter by the adapter layer before counting as failures.
- [ ] Orchestration activities: planner, verifier and merge calls journaled with idempotency keys before execution and results after (06 §11), so replay never repeats a side effect.
- [ ] Startup reconciliation: on priyad start, for each non-terminal step: if the scope is alive (priyad-only crash) reattach to its output; else adapter `resume` if supported; else restart from workspace state with a progress summary; forks and worktrees persist across reboot.
- [ ] Long pauses and reboots: paused tasks resume via the same path; `done_pending_reboot` merges unaffected.
- [ ] Notifications and Task center reasons for every escalation; user can pin "never switch agent for this task".

**Tests**
- Unit: stuck signals on recorded streams (loop, silence, error repetition), ladder state machine, idempotency-key replay.
- Integration: fake agent that loops → detected < 2 min with test thresholds → nudged → escalated → replaced; transient 503s → retried, not escalated.
- Chaos: kill priyad at every step transition of a 4-step plan (≥ 20 kill points) → plan completes with no duplicated side effects (merges, PR creation counted once); VM reboot mid-plan → resumes within 60 s of login.
- Regression: `stuck_loop_escalates`, `transient_error_retries`, `crash_replay_no_duplicate_effects`, `reboot_mid_plan_resumes`.

**Done when**
- [ ] Chaos suite green; owner sees a real stuck run replaced with a clear explanation.

---

### 12.10 — More agent adapters
**Outcome:** Grok Build, Aider, Goose, Cursor CLI and Copilot CLI work as agents when installed, with the same normalised events, checks and review as the Phase 4 agents.
**Features:** ORC-21.
**Depends on:** Phase 4.

**Tasks**
- [ ] For each agent, first record real stream fixtures (headless mode, JSON output where available) and note version and flags in the fixture `RECORDED.md`; verify flags against the installed version (they change often).
- [ ] Aider adapter (non-interactive message mode with auto-yes inside forks only; OpenAI-compatible endpoint → Priya AI proxy works).
- [ ] Goose adapter (headless run with an instruction; MCP extension config for `priya mcp`).
- [ ] Cursor CLI adapter (print/headless mode with stream JSON output; model flag).
- [ ] Copilot CLI adapter (programmatic prompt mode with tool permissions granted only inside forks).
- [ ] Grok Build adapter once xAI's CLI exposes a stable headless mode; until then `detect` reports "not yet supported" in the Agents page.
- [ ] Each adapter: `detect` (installed, version, login), `launch`, `parse`, `resume` (if supported), `steer` method, `traits` (strengths, families, sub-agent support), MCP config injection, unattended flags only in sandbox/read-only workspaces (CLAUDE.md safety rule).
- [ ] Agents page shows the new agents with family glyphs and labels (11 §2.1 hues; no company logos).

**Tests**
- Unit: `parse` on every recorded fixture (text, tool call, tool result, usage, error, final).
- Contract: each adapter passes the `AgentAdapter` contract suite with the fake agent binary replaying its fixtures.
- Integration (fake machine): per installed agent, "create a Rust hello world with a passing test" → review → merge.
- Security: unattended flags absent from argv when mode is `direct` (asserted per adapter).
- Regression: `adapter_contract_<agent>` for each, `unattended_flags_only_in_sandbox`.

**Done when**
- [ ] Every adapter whose CLI has a stable headless mode passes contract and integration tests; unsupported ones are reported plainly.

---

### 12.11 — Multi-agent views and scoreboards
**Outcome:** the agent workspace (Super+P) shows many runs grouped by task with a mini plan map and parent/child nesting; scoreboards show success, cost and time per agent, model and pair, per category.
**Features:** ORC-22.
**Depends on:** 12.3, 12.9.

**Tasks**
- [ ] Agent workspace multi-agent view: run tiles grouped by task (agent glyph + label, model, goal, last lines, progress, elapsed, cost, stuck/escalated badges), nested child runs, per-task mini plan map; Pause / Stop / Steer per run and per task; GUI-run windows stay in the special workspace.
- [ ] `agent_stats` projection (success rate by category and difficulty, cost per success, median time, escalation rate, verifier issue rate) per agent and per (agent, model) pair.
- [ ] `priya agents stats` (plain league table per category) and Agents page scoreboards; Usage & cost page gains "by agent" and "by task".
- [ ] `priya task explain <id>`: who did what, with which model, in which workspace, at what cost, which verifier, which escalations — also as a Task center "Explain" card.
- [ ] Assignment uses `agent_stats` (06 §3) — scoreboard numbers and routing explanations come from the same projection.

**UI**
- Agent workspace (multi-agent), Agents page scoreboards, explain card; states: empty, one task, many tasks (> 12 runs, virtualised grid), stuck, escalated, finished fade-out, no stats yet, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced motion.
- Keyboard: arrows across tiles, Space pause, S stop, Enter steer; screen-reader tile summary ("Claude Code, Sonnet, backend step, running 4 min, $0.12").
- 👤 Mockup approval before building; journey sign-off (watch a 4-step plan).

**Tests**
- Unit: projection maths from event fixtures; explain report on a recorded plan.
- UI: screenshots for every state × theme × scale; virtualised grid with 40 tiles; a11y lint; journey with scripted events.
- Perf: 40 live tiles at < 5 % CPU on the Zenbook; projection rebuild for 10k runs < 3 s.
- Regression: `agent_workspace_multi_states`, `agent_stats_projection`, `task_explain_report`.

**Done when**
- [ ] For any finished plan the owner can answer "who did what, with which model, at what cost" from one screen.

---

### 12.12 — Repository understanding and pull requests
**Outcome:** the `dev` module explains a repository and keeps an architecture summary in project memory; finished work becomes a pull request with a clear summary after the user approves the push; merges into protected branches need confirmation.
**Features:** DEV-02, DEV-05, DEV-06.
**Depends on:** 12.4, 12.7.

**Tasks**
- [ ] `modules/dev` process module (manifest: intents "explain this repo", "open a PR", "why did CI fail"; capabilities `git`, `network:<git hosts>`, `ExternalAction:push`).
- [ ] Repo map: languages, build systems, entry points, test commands, dependency manifests, tree-sitter symbol outline for supported languages; cached and refreshed on large ChangeSets.
- [ ] Architecture summary (role `smart`) written to the project memory's "Architecture" section (user-editable, shown as a diff before saving the first time); "explain this repo/module" answers use it plus `search_files`.
- [ ] `GitHost` port with adapters for GitHub (`gh`), GitLab (`glab`), Gitea (`tea`/API): create PR, update, status, comments, checks; contract suite with recorded fixtures.
- [ ] PR flow: task branch pushed (external action → `ask` approval showing remote, branch and commit list) → PR created as draft with title/body from plan, ChangeSet summary, checks, verifier results and agent/model attribution; PR link card.
- [ ] Protected merges (DEV-06): `.priya.toml [dev.protect] branches = ["main", "release/*"], paths = ["migrations/**"]` → merging (locally or via host) into protected targets or touching protected paths needs `confirm`; never auto-merged.
- [ ] PR review comments pulled into the Task center; "address the comments" → revision task (reuses ORC-23).

**UI**
- "Explain repo" answer layout, PR card (title, branch, checks, link), push approval card; states: drafting, awaiting push approval, pushed, PR open, checks running, merged, push denied, host unreachable, long bodies, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; journey sign-off (task → PR).

**Human in the loop:** pushing and PR creation are external actions — always approved; merges into protected branches need typed or spoken confirmation; the user edits the PR text before it's created if they want.

**Tests**
- Unit: repo-map detectors on fixture repos (Rust, Node, Python, mixed monorepo); PR body builder (golden); protection rule matching.
- Contract: `GitHost` suite against recorded fixtures for all three hosts.
- E2E (VM): local Gitea in the VM → plan finishes → push approval → PR created with expected body; protected-branch merge → confirm required.
- Security: push without approval impossible (credential helper refuses without an approval token); PR body never contains secrets (redaction test).
- Eval: 10 repositories → architecture summaries rated by owner ≥ 4/5 for 8 of 10.
- Regression: `pr_created_after_approval`, `protected_merge_requires_confirm`, `repo_map_fixtures`.

**Done when**
- [ ] Owner opens a real PR on GitHub from a task; summary needs no manual edits for 3 of 5 PRs.

---

### 12.13 — CI diagnosis
**Outcome:** when CI fails on a PR (or on a branch the user follows), PriyaOS fetches the logs, explains the failure in plain language, classifies it and offers a fix task; reruns happen after approval.
**Features:** DEV-07.
**Depends on:** 12.12.

**Tasks**
- [ ] CI watcher in `modules/dev`: polls `GitHost` checks for open Priya PRs and followed branches (backoff, rate-limit aware); `dev.ci_failed {pr, job}` events.
- [ ] Log fetch and trim: failed job/step logs, error lines with context, test names; size cap before any model call.
- [ ] Classifier: test failure, build/compile error, lint, flaky (passes on rerun or known-flaky list), infrastructure (runner, network, quota), secrets/permissions.
- [ ] Diagnosis (role `smart`, read-only run with the repo checked out): plain explanation + likely cause + suggested fix; local reproduction via the project's `check` command (or `act` for GitHub Actions when installed).
- [ ] "Fix it" → sandbox task on the PR branch → checks → review → push (approval) → CI rerun.
- [ ] Notification: "CI failed on PR #12 (2 tests). Likely cause: …" with Fix / Details / Ignore.

**UI**
- CI failure card (summary, class badge with icon + word, failing tests, Fix it); states: watching, failed, diagnosing, diagnosed, fixing, fixed, flaky, infra problem, logs unavailable, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: log trimming on 20 recorded CI logs (GitHub Actions, GitLab CI, Gitea Actions); classifier table.
- Integration: recorded failing run → diagnosis card → fix task created on the right branch.
- E2E (VM): Gitea Actions runner with a seeded failing workflow → diagnosis → fix → rerun green.
- Eval: 25 labelled CI failures → correct class ≥ 90 %, useful diagnosis (owner rubric) ≥ 70 %.
- Regression: `ci_failure_classified`, `ci_fix_task_on_branch`, `ci_log_trim_fixtures`.

**Done when**
- [ ] Eval thresholds met; owner fixes one real CI failure through the card.

---

### 12.14 — Deploys
**Outcome:** projects define deploy targets in `.priya.toml`; a deploy step runs only after approval (`confirm` for production), is health-checked for a monitoring window, and can be rolled back with one approved action.
**Features:** DEV-08.
**Depends on:** 12.12.

**Tasks**
- [ ] `.priya.toml` schema: `[deploy.<env>] command, health_check (url | command), rollback, approval = "ask" | "confirm", monitor_minutes = 10, domains = [...]`; production-like names default to `confirm`.
- [ ] Deploy step kind in the plan DAG; always an external action; runs with only the listed domains and secrets (`UseSecret`) in its capability set.
- [ ] Pre-deploy gate: all verifiers and checks green, PR merged (if configured), plan approved; deploy approval card shows env, commit, changes since last deploy, rollback command.
- [ ] Monitoring window: health checks every 30 s; failure → notification "staging is failing health checks since the deploy" with Roll back / Investigate; optional pre-approved automatic rollback chosen at deploy time.
- [ ] Rollback is itself an external action with its own approval (unless pre-approved); results recorded as `dev.deployed`, `dev.deploy_failed`, `dev.rolled_back` events and in the audit report.
- [ ] `priya deploy <env>` CLI and "deploy to staging" intent.

**UI**
- Deploy approval card, deploy progress/monitoring card, rollback card; states: ready, awaiting approval/confirm, deploying, monitoring, healthy, failing, rolling back, rolled back, denied, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard; screen-reader names; "Can't be undone: leaves your computer" reversibility line (11 §4).
- 👤 Mockup approval before building; journey sign-off (deploy → health fail → rollback).

**Human in the loop:** every deploy and rollback is approved by the user (confirm word for production); autonomy level "Just do it" never skips it.

**Tests**
- Unit: config validation, gate logic, approval tier selection by env name.
- Integration: fake deploy target (local HTTP service in the VM) → deploy after approval → health ok; seeded failing health → rollback offered → approved → rolled back.
- Security: deploy step cannot reach domains outside its list (until Phase 14's proxy, asserted via policy decision events); deploy without approval impossible; "Just do it" autonomy still asks.
- E2E (VM): full chain plan → PR → CI → merge → deploy to "staging" → monitor.
- Regression: `deploy_requires_approval`, `production_requires_confirm`, `deploy_health_fail_rollback`.

**Done when**
- [ ] Owner deploys a real project to a staging environment from a plan and rolls it back from the notification.

---

### 12.15 — Phase gate
**Outcome:** multi-agent work and the development lifecycle are dependable on the Zenbook.
**Features:** all Phase 12 features.
**Depends on:** 12.1–12.14.

**Tasks**
- [ ] Full regression, nightly e2e, chaos, security and eval suites green.
- [ ] Benchmark: demo task run sequentially vs in parallel, results recorded here.
- [ ] Update `features.md`, `docs/human/05-the-phases.md`, `docs/human/04-everything-priyaos-will-do.md`.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green on `main`, including nightly VM e2e (Gitea PR/CI/deploy chain), chaos (crash and reboot mid-plan), security (escalation corpus, message-cannot-approve) and eval (plans, seeded bugs, CI diagnosis).
- [ ] Owner demo: "add user accounts to my app: backend, frontend, tests, docs, and open a PR" → plan card → edit by language → approve → agent workspace shows ≥ 3 agents from ≥ 2 families → verifier catches and fixes an issue → conflict resolved → review → PR opened after approval → CI failure diagnosed → fix pushed → deploy to staging after confirm.
- [ ] Parallel plan finishes ≥ 30 % faster than the same plan run sequentially, with equal verifier results.
- [ ] Every plan explainable from one screen (who, which model, cost, verifier, escalations).
- [ ] L4 (Zenbook): 3 concurrent agent runs keep the desktop responsive (bar opens < 100 ms); battery rule observed on a real unplug.
- [ ] Docs: [06-orchestration](../architecture/06-orchestration.md) updated (board read-only mount, escalation ladder, dev lifecycle section); feature statuses and human status updated.

## Out of scope

- Agents on other machines, A2A and portable tasks (Phase 15).
- Egress-proxy enforcement of deploy domains and value-level IFC (Phase 14; this phase uses policy and approvals).
- Scheduled CI checks and recurring deploys (Phase 13 automations).
- Hosting a git server or CI service; PriyaOS only talks to the user's existing hosts.
