# Phase 4 — Agents and first actions

**Goal.** Let agents act. A request in the Priya bar becomes a task that the best (agent, model) pair carries out in the right workspace, under policy and budget, with live progress, automatic checks and a graphical review where the owner keeps or discards the result. Every agent — Claude Code, Codex, Gemini CLI, OpenCode and the Priya native agent — looks the same to the rest of the system through one normalised event stream.

**Usable at the end:** type "do this" in the Priya bar → a plan preview shows mode, agent, model and budget, each editable in place → Start → the chosen agent (Claude Code, Codex, Gemini CLI, OpenCode, or the Priya native agent with any model, including a local one) works in Priya Sandbox, read-only or direct mode as resolved → live progress in the bar and the Task center (pause, stop, steer, send to background) → project checks run → review in the Task center → Keep or Discard.

**Depends on:** Phase 3 gate (task engine, mode resolution, Cedar, capabilities, secrets, approvals, autonomy, budgets, kill switch, taint, audit). Uses from earlier phases: router, learning and local proxy (1.9, 1.14, 1.15), AI & Models settings and model picker (1.16), sandbox runner, booted runs, ChangeSets, merges, read-only/direct modes and diff components (2.7–2.14), minimal Priya bar and PriyaUI (0.11, 0.13).

**Features:** SBX-11, ORC-01, ORC-02, ORC-03, ORC-04, ORC-05, ORC-06, ORC-07, ORC-08, ORC-09, ORC-10, ORC-11, DEV-01, ADM-01, MEM-01, TOOL-01, UX-19, UX-20.

**Read first:** [06-orchestration](../architecture/06-orchestration.md) (primary), [04-priya-ai §7](../architecture/04-priya-ai.md), [03-execution-and-sandbox](../architecture/03-execution-and-sandbox.md), [05-security](../architecture/05-security.md), [08-rewind-memory-tools §3](../architecture/08-rewind-memory-tools.md), [02-core §1–4](../architecture/02-core.md), [11-design-system-and-ux §4–6, §9](../architecture/11-design-system-and-ux.md), [09-testing](../architecture/09-testing.md), [decisions](../architecture/decisions.md) ADR-007, ADR-009, ADR-010, ADR-011, ADR-012, ADR-013.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 4.1 | Agent port, normalised events, fake agent | Phase 3 gate | `priya run --agent fake "…"` runs a replayed agent in a sandbox and produces a reviewable ChangeSet |
| 4.2 | Claude Code adapter | 4.1 (MCP wiring: 4.11) | `priya run --agent claude-code "…"` works end to end with the owner's Claude login |
| 4.3 | Codex adapter | 4.1 (MCP wiring: 4.11) | Same with Codex |
| 4.4 | Gemini CLI adapter | 4.1 (MCP wiring: 4.11) | Same with Gemini CLI |
| 4.5 | OpenCode adapter | 4.1, 1.15 (MCP wiring: 4.11) | Same with OpenCode, including local models through the proxy |
| 4.6 | Priya native agent | 4.1, 4.11, 1.15 | A task finishes with only a local Ollama model and no agent CLI installed |
| 4.7 | Agent registry and Agents page | one of 4.2–4.6, 1.16 | Settings → Agents shows what is installed, logged in and enabled |
| 4.8 | Agent + model assignment | 4.7, 1.9, 1.14 | Priya picks the (agent, model) pair and explains why |
| 4.9 | Run supervision (progress, pause, stop, steer, background) | 4.1, 3.2, 3.9, 3.10 | Pause, resume, stop, restart, steer and background a running task |
| 4.10 | Checks and `.priya.toml` | 4.1, 2.8 | Tests run automatically after the agent finishes; results shown |
| 4.11 | `priya mcp` basic and history search | 4.1, 3.5 | Agents call `priya mcp` tools; `priya history "docker"` finds past tasks |
| 4.12 | Plan preview card and bar actions | 4.8, 4.9, 0.13, 1.16, 3.8, 3.9 | "Install jq" in the bar → plan preview → Start → live card |
| 4.13 | Task center and review GUI | 4.9, 4.10, 4.11, 2.11, 2.12, 2.14, 3.9 | Review and keep/discard in a graphical Task center |
| 4.14 | Agent end-to-end suites | 4.2–4.13 | Every agent passes the same scenarios, nightly, with an evaluation baseline |
| 4.15 | Phase gate | all | Owner demo of the full loop on the Zenbook |

---

### 4.1 — Agent port, normalised events, fake agent

**Outcome:** any agent that implements `AgentAdapter` can run in any workspace and its output becomes ordinary `run.*` events; the owner can try `priya run --agent fake "make hello.txt"` and review the result.
**Features:** ORC-09 (+ infrastructure).
**Depends on:** Phase 3 gate (3.2 task engine, 3.5 capability enforcement, 3.6 redaction), 2.7, 2.13.

**Tasks**
- [ ] `AgentAdapter` trait in `priya-agents` exactly as [06 §1](../architecture/06-orchestration.md), plus supporting types: `Detection {installed, path, version, auth: LoggedIn|LoggedOut|Unknown, models, tested_range}`, `StepSpec`, `RunEnv {workspace, cwd, mode, env, mcp_config, proxy: Option<ProxyEndpoint>, budget, session_id}`, `SteerMethod {StdinMessage, ResumeWithMessage, RestartWithSummary, Unsupported}`, `AgentTraits`
- [ ] `AgentEvent` enum: `Started {session_id, model}`, `Text`, `ToolCall {id, name, input, parent_id}`, `ToolResult {id, ok, output}`, `SubagentStarted/Finished {id, parent_id}`, `Usage {input, output, cached, cost_usd?}`, `Final {text, is_error}`, `Error {kind, retryable, message}`, `Unknown {raw}`; unknown lines never fail a run
- [ ] Event catalogue additions in `priya-api` (additive, schema snapshots updated): `run.tool_result`, `run.subagent_started`, `run.subagent_finished`, `run.final`, `run.steered`, `run.paused`, `run.resumed`, `run.stalled`, `step.assigned`, `check.started`, `check.finished`, `agent.detected`; update [02 §1.1](../architecture/02-core.md) in the same PR
- [ ] `RunHost` in `priya-orchestra`: launches the adapter's `Command` in the step's workspace through the helper (sandbox/read-only: nspawn in the fork; direct: transient scope + Landlock), line-frames stdout (8 MiB max line, lossy UTF-8, partial-line buffering), sends stderr to `run.output {stream: "stderr"}`, applies 3.6 redaction before storing, derives heartbeats from activity
- [ ] `PreambleBuilder`: goal, plain explanation of the workspace per mode ("you are working in a disposable copy of this computer…"), check command, available `priya mcp` tools, budget, rules (no attempts to reach masked paths, external actions need approval), untrusted-input notice when tainted; never includes secrets; golden-tested per mode
- [ ] Unattended-permission guard: `RunHost` refuses to launch any adapter command that disables the agent's own permission prompts unless the workspace is `sandbox` or `read_only` (CLAUDE.md safety rule); enforced in code, covered by a unit test per adapter
- [ ] Per-agent ChangeSet ignore entries (session caches, logs, history files) registered by each adapter and merged into the 03 §5.1 ignore list
- [ ] `priya-fake-agent` binary in `priya-testkit`: replays a JSONL fixture with timing (`--speed`), applies scripted file writes, simulates hang, crash, garbage lines, 50 MB output, subagents, and echoes stdin messages (for steering tests)
- [ ] `FakeAdapter` + shared `AgentAdapter` contract suite (`priya-agents/tests/contract.rs`): deterministic argv snapshot per mode, fixture → golden normalised events, exactly one `Final`, usage summed, unknown-line tolerance, steer method declared, `resume` consistent with traits, ignore list declared
- [ ] `priya run --agent <id> "<goal>" [--mode …]` CLI path through the task engine (single-step plan)

**Tests**
- Unit: line framing (split lines, huge lines, invalid UTF-8), `AgentEvent` → event mapping, preamble goldens per mode, permission guard per mode, ignore-list merge.
- Contract: `FakeAdapter` passes the shared suite; the suite itself has a negative test (a deliberately broken adapter fails it).
- Integration (fake machine): fake agent in sandbox writes `~/hello.txt` → task reaches `awaiting_review` with a one-entry ChangeSet; crash mid-run → `run.finished {failed}` with reason, fork kept; hang → `run.stalled` after the configured window; 50 MB output → stored as blobs, priyad RSS stays under budget; redaction removes a planted `sk-ant-…` string.
- Regression: fake-agent sandbox run, crash and redaction scenarios.

**Done when**
- [ ] Contract suite exists and the fake adapter passes it on Mac and Linux
- [ ] Fake-agent task goes `created → running → verifying → awaiting_review` on the fake machine and merges
- [ ] Event schema snapshots updated; 02 §1.1 lists the new types
- [ ] No adapter can launch with prompts disabled in `direct` mode (unit test per adapter template)

---

### 4.2 — Claude Code adapter

**Outcome:** the owner can run `priya run --agent claude-code "add a /health endpoint"` and Claude Code does it headless in Priya Sandbox using their own Claude login.
**Features:** ORC-01, SBX-11.
**Depends on:** 4.1; MCP wiring task needs 4.11; proxy mode needs 1.15.

**Tasks**
- [ ] Record fixtures first, on the Zenbook (Mac for parser work), with `claude -p --output-format stream-json --verbose` (verified on 2.1.x; re-check `claude --help` and note the version in `crates/priya-agents/tests/fixtures/claude-code/RECORDED.md`): plain answer, file edit, Bash tool, subagent (`parent_tool_use_id`), `priya mcp` tool call, bad model error, `--max-budget-usd` stop, interrupted run, `--resume`, steering through `--input-format stream-json`; scrub secrets; budget ≤ $0.50 total
- [ ] `detect`: `claude --version`; auth state from the CLI's own status command if available, else presence of its credentials file (existence only, never read); models from the catalog's `claude` family
- [ ] `launch`: `claude -p --output-format stream-json --verbose --input-format stream-json --model <id> --session-id <uuid> --mcp-config <run>/mcp.json --strict-mcp-config --append-system-prompt <preamble> [--max-budget-usd <task budget>]`, cwd = mapped project path
- [ ] Permission mapping (verify mode names against the installed version: currently `acceptEdits, auto, bypassPermissions, manual, dontAsk, plan`): sandbox → `bypassPermissions`; read-only → `plan` (nspawn `--read-only` still enforces); direct → `acceptEdits` with `--permission-prompts none` or a `priya mcp` permission-prompt tool that raises a Priya approval (pick after testing; document in RECORDED.md)
- [ ] Proxy mode (opt-in per agent, ADR-007): `ANTHROPIC_BASE_URL=http://127.0.0.1:<port>` and a per-run `ANTHROPIC_AUTH_TOKEN` from 1.15 so usage is attributed to the run
- [ ] `parse`: `system/init` (session id, model, tools, MCP server status — a failed `priya` MCP server becomes a warning event), `assistant` (text, `tool_use`), `user` (`tool_result`), `result` (`total_cost_usd`, usage, `num_turns`, `is_error`, subtype), subagent tree from `parent_tool_use_id`
- [ ] `steer`: `StdinMessage` (stream-json user message on stdin); `resume`: `claude -p --resume <session_id> …`
- [ ] Ignore list: Claude's per-project session, todo and shell-snapshot directories and the churn in its global state file (exact paths from the recording)
- [ ] `traits`: strengths (code edit, repo navigation, subagents), families `claude` (+ any via proxy), needs network, subagent support, tested version range; newer untested versions run with a registry warning

**Tests**
- Unit: parser over every fixture → golden `AgentEvent`s; argv snapshots for sandbox / read-only / direct / proxy.
- Contract: shared suite with the recorded fixtures replayed by `priya-fake-agent`.
- Integration (fake machine): replayed stream in sandbox produces the expected ChangeSet; direct mode refuses `bypassPermissions` (guard test).
- Live (nightly, `live`, ≤ $0.50): "create `~/hello/README.md` saying hi" in sandbox → `awaiting_review`; steering message mid-run is acknowledged in the stream.
- Regression: parser goldens, contract run, guard test.

**Done when**
- [ ] Contract suite green on recorded fixtures; RECORDED.md lists version, flags and date
- [ ] Live nightly scenario passes three nights in a row
- [ ] Cost from `result.total_cost_usd` (or proxy usage) appears on the task

---

### 4.3 — Codex adapter

**Outcome:** `priya run --agent codex "…"` runs Codex headless in Priya Sandbox with the owner's Codex login.
**Features:** ORC-01, SBX-11.
**Depends on:** 4.1; MCP wiring needs 4.11; proxy mode needs 1.15.

**Tasks**
- [ ] Record fixtures with `codex exec --json` on the Zenbook (Codex is not on the dev Mac); RECORDED.md with version and the verified flag list. Note: `--full-auto` has been removed — do not use it; current options include `--sandbox read-only|workspace-write|danger-full-access`, `--dangerously-bypass-approvals-and-sandbox` (`--yolo`), `--approve-for-me`, `-m`, `-C`, `--skip-git-repo-check`, `-c key=value`, `-o`
- [ ] `detect`: `codex --version`; login state from the CLI's status command (verify name) or presence of its auth file
- [ ] `launch`: `codex exec --json -m <model> -C <cwd> --skip-git-repo-check -c mcp_servers.priya.command=… -c mcp_servers.priya.args=… "<preamble + goal>"`
- [ ] Mode mapping: sandbox → `--dangerously-bypass-approvals-and-sandbox` (Priya Sandbox is the isolation; Codex's own Landlock/seccomp sandbox nested in nspawn is redundant and may fail — confirm during recording); read-only → `--sandbox read-only`; direct → `--sandbox workspace-write` with approvals never asked (anything outside is refused, not prompted). `--approve-for-me` is not used: approvals belong to Priya
- [ ] Proxy mode: custom provider via `-c model_provider=priya -c model_providers.priya.base_url=… -c model_providers.priya.wire_api="responses"` with a per-run key in the environment
- [ ] `parse`: `thread.started`, `turn.started/completed/failed`, `item.started/updated/completed` for `agent_message`, `reasoning` (summary only), `command_execution`, `file_change`, `mcp_tool_call`, `web_search`, `todo_list`, `error` (confirm names from the recording); `web_search` and fetch-like commands raise `taint.added` through 3.11
- [ ] `steer`: `ResumeWithMessage` — interrupt at the next turn boundary (SIGINT), then `codex exec resume <session_id> "<message>"`; `resume`: same command without a message
- [ ] Cost: Codex reports tokens only → priced from the catalog; marked "estimated" when the user is on a subscription
- [ ] Ignore list: Codex session and log directories (from the recording)

**Tests**
- Unit: parser goldens; argv snapshots per mode; the removed `--full-auto` never appears (assertion).
- Contract: shared suite on recorded fixtures.
- Integration (fake machine): replayed stream → expected ChangeSet; steer = interrupt + resume sequence verified with the fake agent.
- Live (nightly, ≤ $0.50): hello-file scenario in sandbox.
- Regression: parser goldens, contract run, steer sequence.

**Done when**
- [ ] Contract suite green; flags verified against the installed version and recorded
- [ ] Live nightly scenario passes three nights in a row
- [ ] Steering works through resume without losing the session

---

### 4.4 — Gemini CLI adapter

**Outcome:** `priya run --agent gemini-cli "…"` runs Gemini CLI headless in Priya Sandbox.
**Features:** ORC-01, SBX-11.
**Depends on:** 4.1; MCP wiring needs 4.11.

**Tasks**
- [ ] Record fixtures with `gemini -p "<prompt>" --output-format stream-json` (0.52.x verified on the Mac; re-verify on the Zenbook): answer, edit, shell tool, MCP call, error, `--resume`; RECORDED.md
- [ ] `detect`: `gemini --version`; auth from its settings/credentials presence; models from the `gemini` family
- [ ] `launch`: `gemini -p <preamble+goal> --output-format stream-json -m <model> --session-id <uuid> --skip-trust --allowed-mcp-server-names priya`; `--skip-trust` replaces writing trust entries into the fork (06 §1 "trust the fork's project path")
- [ ] Mode mapping: sandbox → `--approval-mode yolo`; read-only → `--approval-mode plan`; direct → `--approval-mode auto_edit` with shell tools denied by a run-scoped policy file (`--policy`) rather than prompting
- [ ] MCP config without writing into the project: per-run settings file passed through the CLI's system-settings override (verify mechanism) so no `.gemini/settings.json` appears in the ChangeSet; fallback: write it and add it to the ignore list
- [ ] `parse`: `init`, `message`, `tool_use`, `tool_result`, `error`, `result` with stats (confirm names from the recording)
- [ ] `steer`: `ResumeWithMessage` via `--resume <session>`; evaluate `--acp` (Agent Client Protocol) as a richer steering channel and record the finding in 06
- [ ] Ignore list: Gemini temp, history and checkpoint directories

**Tests**
- Unit: parser goldens; argv snapshots per mode.
- Contract: shared suite on recorded fixtures.
- Integration (fake machine): replay → ChangeSet contains no Gemini config files.
- Live (nightly, ≤ $0.50): hello-file scenario.
- Regression: parser goldens, contract run, "no agent config in ChangeSet" check.

**Done when**
- [ ] Contract suite green; flags recorded
- [ ] Live nightly scenario passes three nights in a row
- [ ] Headless runs never stop on the trusted-folder prompt

---

### 4.5 — OpenCode adapter

**Outcome:** `priya run --agent opencode "…"` works with OpenCode using any model, including local models through the Priya AI proxy (`priya-auto`).
**Features:** ORC-01, SBX-11.
**Depends on:** 4.1, 1.15; MCP wiring needs 4.11.

**Tasks**
- [ ] Install OpenCode on the Zenbook and record fixtures with `opencode run --format json` (verify flag names; OpenCode is not on the dev Mac): answer, edit, bash, MCP call, error, session continue; RECORDED.md
- [ ] `detect`: `opencode --version`; configured providers from its auth/config; always "available" when the Priya proxy is enabled
- [ ] `launch`: `opencode run --format json -m <provider/model> "<preamble+goal>"` with config injected through the environment (`OPENCODE_CONFIG_CONTENT` or equivalent, verify): provider `priya` → `baseURL` of the proxy's `/v1/chat/completions`, model `priya-auto` or a named model; MCP server `priya` (local command)
- [ ] Mode mapping through the injected `permission` block: sandbox → edit/bash allow; read-only → edit deny, bash allow (nspawn read-only enforces); direct → edit allow, bash deny unless listed
- [ ] `parse`: map OpenCode's JSON events (text parts, tool parts, step finish with tokens/cost, errors) to `AgentEvent`s
- [ ] `steer`: `ResumeWithMessage` via `--session <id>`; evaluate `opencode serve` for in-flight steering and record the finding in 06
- [ ] Ignore list: OpenCode data/session directories

**Tests**
- Unit: parser goldens; generated config JSON snapshots per mode.
- Contract: shared suite on recorded fixtures.
- Integration (fake machine): replay through a fake proxy; usage attributed to the run's proxy token.
- Live (nightly): hello-file scenario with a local Ollama model through the proxy (no cloud cost).
- Regression: parser goldens, contract run, config snapshots.

**Done when**
- [ ] Contract suite green; flags recorded
- [ ] Nightly local-model scenario passes three nights in a row
- [ ] Proxy usage for OpenCode runs appears in `priya ai costs` under the run's task

---

### 4.6 — Priya native agent

**Outcome:** with no agent CLI installed and only a local model enabled, the owner can still ask for a change and get it done and reviewed.
**Features:** ORC-02.
**Depends on:** 4.1, 4.11, 1.15.

**Tasks**
- [ ] Execution design (clarifies 06 §1, update it in the same PR): the native agent runs as `priya agent native` launched inside the workspace like any other agent; model calls go through the local proxy with the run's token (so routing, budgets, privacy and caching apply); tools run inside the workspace; output is `AgentEvent` JSONL, so `parse` is the identity
- [ ] Tool set: `shell` (bash -lc, per-call timeout, output trimmed head + tail + error lines), `read_file`, `write_file`, `edit_file` (exact-match replace), `list_dir`, `search` (ripgrep), `finish {summary}`, plus the `priya mcp` tools
- [ ] Loop: role `coding` for edit turns and `smart` for the first planning turn (Priya AI decides the model; `priya-auto`), context compaction through the 1.13 optimizer when near the limit, stop on `finish`, budget, max turns (default 60)
- [ ] Durability: every model and tool call is an activity with idempotency key `run:turn:call` journaled before and after in the run directory and mirrored as `run.tool_called`/`run.tool_result`; `resume` replays the journal and never repeats a completed side effect
- [ ] Text tool-call fallback for local models without native tool calling (strict JSON block protocol, validated, one repair attempt)
- [ ] `steer`: `StdinMessage` (control JSON on stdin, inserted as a user turn at the next turn boundary)
- [ ] `traits`: any family, works offline with local models, no subscription needed; the only agent eligible for value-level taint later (Phase 14)

**Tests**
- Unit: loop state machine, tool argument validation, output trimming, fallback parser (valid, invalid, repaired), journal replay skips completed activities.
- Contract: shared adapter suite with a scripted fake model provider.
- Integration (fake machine): scripted model creates a Rust hello world → ChangeSet; kill priyad after a tool call → resume does not repeat it.
- Live (nightly, local): Ollama coder model completes the hello-world task with passing `cargo test`.
- Regression: replay-no-repeat, fallback parser, hello-world with scripted model.

**Done when**
- [ ] Hello-world task passes with only a local model enabled and network disabled
- [ ] Crash/resume test shows zero repeated side effects
- [ ] 06 §1 describes how the native agent executes inside workspaces

---

### 4.7 — Agent registry and Agents page

**Outcome:** the owner opens Settings → Agents and sees which agents are installed, which version, whether they are logged in, which models they can use, and can enable, disable, log in or install them.
**Features:** ORC-03, ADM-01 (install agent by request), ORC-28.
**Depends on:** one of 4.2–4.6 (more adapters appear as they land), 1.16.

**Tasks**
- [ ] `AgentRegistry` in `priya-orchestra`: runs `detect` for every adapter at priyad start, every 6 h, after pacman transactions touching agent packages, and on demand; emits `agent.detected`; `agents` projection
- [ ] API: `v1.agents.list`, `enable`, `disable` (exist) plus `v1.agents.detect`, `v1.agents.configure {id, proxy, default_model?}`, `v1.agents.stats`; CLI `priya agents list|refresh|enable|disable|stats`
- [ ] `agent_stats` projection: success per (agent, category) as Beta parameters from check results, keep/discard and later reverts, plus cost and duration averages (same outcome recorder as 1.14)
- [ ] Settings → Agents page (`ui/apps/Settings/pages/AgentsPage.qml`) using PriyaUI only: one card per agent with family glyph and hue (11 §2.1, no logos), version, status chip, models, enable switch, proxy switch with the ADR-007 caveat in plain words, stats line
- [ ] "Log in" opens kitty running the agent's own login command; the user completes it; Priya never sees credentials; status refreshes when the terminal closes
- [ ] "Install" creates a sandbox task "install <agent> from its official source" (ADM-01): the review shows exactly what was installed before anything reaches the live system
- [ ] Priya native agent always listed and enabled unless the user disables it
- [ ] Install and log in from the Agents page: **Install** runs the agent's official installer as a sandbox task; **Log in** opens the agent's own login flow in a terminal window; status refreshes afterwards

**UI**
- Agents page states: loading (detecting), list, none installed (native agent only, with Install buttons), not logged in, untested version (warning chip with tested range), disabled, detect error, offline (login/install unavailable), long model lists (collapsed "+12 more"), Hindi text in descriptions.
- Light and dark; OLED-black and high-contrast if the 0.10 token variants exist, otherwise baselines are added in 5.11; scale 1.0 and 2.0.
- Keyboard: Tab through cards, Space toggles switches, Enter on Log in / Install; screen-reader names such as "Codex, version 0.x, not logged in, Log in button".
- 👤 Mockup approval (`ui/mockups/4.7/`, light + dark) before building; gallery sign-off after.

**Human in the loop:** the user decides which agents are enabled and whether each may use the proxy; installing an agent is a normal reviewed sandbox task; logging in is always done by the user.

**Tests**
- Unit: detection parsing per adapter (version strings, missing binary, broken binary), stats updates.
- Contract: `detect` cases in the shared suite.
- Integration (fake machine): fake PATH with/without agents → registry states; "install fake-agent" task produces a package ChangeSet.
- UI: component tests for the agent card; screenshots for every state × theme × scale; a11y lint; journey "enable Codex, turn on proxy, see the caveat".
- Regression: registry states, Agents page screenshots, journey.

**Done when**
- [ ] `priya agents list` matches reality on the Zenbook for all five agents
- [ ] Agents page passes screenshot and a11y tests; 👤 gallery signed off
- [ ] Installing an agent through the Install button goes through review

---

### 4.8 — Agent + model assignment

**Outcome:** for each task Priya chooses the (agent, model) pair, and "why?" explains it in one sentence; the owner can override with words ("use Codex") or chips.
**Features:** ORC-04.
**Depends on:** 4.7, 1.9, 1.14, 3.4.

**Tasks**
- [ ] `Assigner` in `priya-orchestra` per [06 §3](../architecture/06-orchestration.md): filters (enabled, installed, logged in, step needs — repo edit, shell, MCP, context size —, mode support, Cedar `UseModel` with the `Agent` principal, strategy and privacy)
- [ ] Scoring: agent success Beta × model `Q̂` from Priya AI, minus cost and latency terms per strategy; bounded exploration as in 04 §5; stickiness within a task
- [ ] Model flag mapping: catalog id → each agent's model argument; pairs needing the proxy are only eligible when the agent's proxy switch is on
- [ ] Overrides, highest first: explicit request ("use Gemini", "with Opus"), plan-preview edit, `.priya.toml [agents] prefer`, settings `agents.default`
- [ ] `step.assigned` event with candidates, scores and reason; `priya agents why <task>` and `v1.agents.why` in plain language ("Claude Code with Sonnet: it passed 9 of the last 10 code edits here; Codex costs about 2× more")
- [ ] Outcomes feed `agent_stats` and `model_stats` (check pass/fail, keep/discard, revert within 7 days)
- [ ] `local_only` strategy → only the native agent or OpenCode with local models are eligible; offline → same, with a notice

**Tests**
- Unit: filter matrix, scoring ordering, override precedence, privacy denial, proxy eligibility, exploration bounds.
- Integration (fake machine): code task vs "install jq" task get different pairs with the fake registry; disabled agent never chosen; offline picks the native agent.
- Eval: assignment cases join the router evaluation suite (expected pair class per labelled task).
- Regression: precedence, privacy denial, offline choice.

**Done when**
- [ ] A code task and a quick admin task get different pairs, each with a plain explanation
- [ ] With `local_only`, no cloud model is ever used (asserted on events)
- [ ] Every assignment emits `step.assigned` with a reason

---

### 4.9 — Run supervision (progress, pause, stop, steer, background)

**Outcome:** while a task runs, the owner sees what it is doing right now, and can pause, resume, stop, restart, steer it with a sentence, or send it to the background and get a notification when it is done.
**Features:** ORC-05, ORC-06, ORC-07, ORC-08.
**Depends on:** 4.1, 3.2, 3.9, 3.10.

**Tasks**
- [ ] User-facing statuses (ORC-05) derived from the task state machine: Queued (with position), Running, Needs you (approval or question), Paused, Failed, Done; `progress` projection with current activity line from the latest tool call ("Running cargo test"), elapsed, cost so far, files touched so far
- [ ] Pause/resume: helper operation `run.signal` sends SIGSTOP/SIGCONT to the run scope; pauses longer than the provider idle limit (default 10 min) become stop + `resume` through the adapter, else restart with a progress summary
- [ ] Stop: SIGTERM → 5 s → SIGKILL; workspace kept so partial work can be reviewed (`awaiting_review` marked partial) or discarded
- [ ] Restart: new run under the same step, choice of same workspace or fresh fork
- [ ] Steer: `v1.task.steer` → adapter `SteerMethod`; recorded as `run.steered` with actor `user`; agent messages can never carry user approval (06 §7)
- [ ] Stall detection v1: no events for 5 min (configurable) → `run.stalled` + Needs-you notification with Nudge / Stop; automatic recovery is Phase 12 (ORC-19)
- [ ] Background: `priya run --detach`, bar Ctrl+Enter; interactive work has priority over background (06 §5); max concurrent runs 3, rest queued; completion uses 3.9 notifications
- [ ] CLI: `priya ps`, `priya logs -f <task>`, `priya pause|resume|stop|restart <task>`, `priya steer <task> "<text>"`

**UI**
- Status line component (`PriyaUI/TaskStatusLine.qml`) used by the bar card (4.12) and Task center (4.13): queued, running (thinking shimmer, never a spinner), paused, stalled, stopping, steer sent ("Sent to the agent"), failed, done; long activity text truncates with full text in the tooltip and accessible description.
- Light and dark (OLED/high-contrast as in 4.7); scale 1.0 and 2.0; reduced-motion variant.
- Keyboard: Esc stops immediately (workspace kept, card says "Stopped. Your changes so far are kept for review."); P pause/resume; S focuses the steer input. Screen-reader live region announces status changes politely.
- 👤 Mockup approval with 4.12/4.13 mockups; gallery sign-off.

**Human in the loop:** stop never loses work (workspace kept until the user discards); steering is visible in the activity log; background tasks never take focus.

**Tests**
- Unit: status derivation from states, stall timer, pause-length policy.
- Integration (fake machine): pause/resume with the fake agent (no events while stopped, continues after); stop → partial review; restart same/fresh workspace; steer message reaches the fake agent's stdin and appears as `run.steered`; four tasks → one queued with position.
- UI: status line component tests and screenshots for every state; a11y live-region test.
- Regression: pause/resume, stop-keeps-workspace, steer delivery, queueing.

**Done when**
- [ ] Every control works for every adapter's declared steer/resume method (fake + live nightly for at least Claude Code and the native agent)
- [ ] A detached task notifies on completion and never takes focus
- [ ] Stalled run produces exactly one Needs-you notification

---

### 4.10 — Checks and `.priya.toml`

**Outcome:** after an agent finishes, the project's tests, lints or build run automatically in the same workspace, and failures go back to the agent once before the owner is asked.
**Features:** ORC-10, ORC-11, DEV-01.
**Depends on:** 4.1, 2.8, 3.2.

**Tasks**
- [ ] `.priya.toml` schema in the settings system (project layer, 02 §6.1): `[scripts] setup, run, check`, `[checks] timeout, auto_fix_attempts (default 1)`, `[agents] prefer`, `[changes] ignore`; JSON Schema exported
- [ ] Auto-detection when no `.priya.toml`: `Cargo.toml` → `cargo test`; `package.json` test script → npm/pnpm/yarn by lockfile; `pyproject.toml`/`pytest.ini` → `pytest`; `go.mod` → `go test ./...`; `Makefile` `test`/`check` target; `priya project init` writes the detected file for the user to edit
- [ ] `Verifier` trait with `ProjectCheck`, `BootCheck` (2.8 booted run, used when the ChangeSet touches packages, services or `/etc`) and `Custom(cmd)`; run in the workspace in task state `verifying`; `check.started`/`check.finished {verifier, status, duration, output_ref, summary}`
- [ ] `setup` runs before the agent in the workspace; its effects are excluded from review via `[changes] ignore`
- [ ] Build output in the ChangeSet: gitignored paths inside a git repository are excluded from home merges by default (`target/`, `node_modules/`) — design change: ADR + update 03 §5.1 in the same PR
- [ ] Auto-fix loop: failed check → one steer/resume with the trimmed failure output ("tests failed: …, fix them") within budget → re-check; then present as failed checks
- [ ] Results attached to the task and ChangeSet for the review card and Checks tab; outcome recorded for agent/model learning
- [ ] CLI: `priya checks <task>`, `priya project info`

**Tests**
- Unit: detection table over fixture repos (Rust, Node with each lockfile, Python, Go, Make, none), schema validation, ignore merge, auto-fix decision.
- Integration (fake machine): fake agent writes a failing test → auto-fix attempt → passes; system package change → boot check runs; gitignored `target/` absent from the merge.
- Regression: detection table, auto-fix, boot-check trigger, gitignore exclusion.

**Done when**
- [ ] Checks run for every task with a detected or configured command
- [ ] One auto-fix attempt happens and is visible in the activity log
- [ ] ADR for gitignored-path exclusion accepted and 03 updated

---

### 4.11 — `priya mcp` basic and history search

**Outcome:** every agent can ask Priya about past tasks and the system through `priya mcp`; the owner can search past tasks with `priya history "<words>"` and in the Task center.
**Features:** TOOL-01, MEM-01.
**Depends on:** 4.1, 3.5.

**Tasks**
- [ ] `priya mcp` stdio MCP server in `priya-cli` (official Rust MCP SDK) with tools `task_history`, `system_info` (OS, kernel, hardware summary, disk free, package count), `rewind_log` (read-only list of ChangeSets from the projection until Phase 7), `project_info` (detected checks and `.priya.toml`)
- [ ] Read-only socket for workspaces: priyad serves `mcp-ro.sock` exposing only these methods; the helper binds it into nspawn at `/run/priya/mcp.sock`; `priya mcp` inside a fork connects to it; per-run token for attribution
- [ ] Every MCP call logged as `run.tool_called`/`run.tool_result` with actor `agent:<id>`; capability check per tool (3.5)
- [ ] Per-agent MCP config generators used by 4.2–4.6 (`mcp.json` for Claude Code, `-c mcp_servers…` for Codex, settings override for Gemini CLI, config block for OpenCode)
- [ ] `history_fts` projection (FTS5) over titles, prompts, final answers, ChangeSet summaries, agent, model; `v1.history.search {query, since?, agent?, status?, project?}`; CLI `priya history "<q>"`
- [ ] Tool descriptions written for models (short, precise) and exported with the MCP schemas (09 §6)

**Tests**
- Unit: FTS ranking fixtures, filter parsing, tool argument validation.
- Contract: MCP protocol conformance (initialize, tools/list, tools/call, errors) with a scripted MCP client; tool schema snapshots.
- Integration (fake machine): agent inside a fork calls `task_history` through the bound socket; write-type methods are absent; call attributed to the run.
- Regression: protocol conformance, fork socket isolation, history search fixtures.

**Done when**
- [ ] Every adapter's recorded fixtures include a successful `priya mcp` call
- [ ] Inside a fork only the read-only methods are reachable
- [ ] `priya history` finds a task by a word from its ChangeSet summary

---

### 4.12 — Plan preview card and bar actions

**Outcome:** the owner types "install jq and write ~/bin/pp that pretty-prints JSON" in the Priya bar, sees a one-sentence plan with mode, agent, model and budget chips, edits any of them in place, presses Enter, and watches the task run from the bar.
**Features:** UX-20, ADM-01.
**Depends on:** 4.8, 4.9, 0.13, 1.16, 3.8, 3.9.

**Tasks**
- [ ] Bar action path: the 1.8 classifier's answer/action output decides; `v1.intent.submit` returns a `task_id` for actions; a visible "Answer instead / Do it as a task" toggle (Tab on the result) corrects misclassification (full pipeline in 5.1)
- [ ] Preview rules from autonomy (11 §5) and policy: preview shown for "Ask me first", and for multi-step, direct, privileged, external or above-threshold cost under "Ask when it matters"; otherwise a compact "Starting…" card with Stop
- [ ] `PlanPreviewCard.qml`: verb-first sentence, chips Mode (colour + icon + word), Agent (glyph + name), Model (resolved, e.g. "Sonnet · Balanced"), Budget ("up to $0.80 · 15 min"), estimate, "Why?" (assignment and mode reasons), untrusted badge, reversibility line, autonomy line with a link to Settings; buttons Start (Enter), Cancel (Esc), Run in background (Ctrl+Enter)
- [ ] Chip editors as popovers: Mode lists only allowed modes, locked rules shown with a lock and reason; Agent lists eligible agents with status; Model reuses the 1.16 picker filtered to the agent; Budget stepper. Edits call `v1.plan.edit {task_id, step_id, mode?, agent?, model?, budget?}`; policy re-checked; denials explained in the card
- [ ] Running card: `TaskStatusLine`, Stop, Pause, "Open in Task center"; typing while the running card is focused sends steering ("Tell the agent…")
- [ ] Finished card: "Ready for review" with the ChangeSet one-liner, Review (opens Task center), Keep / Discard for small home-only ChangeSets
- [ ] Bar never steals focus for background tasks; completed background tasks surface via notification only

**UI**
- States: preview, chip editor open, edit denied by policy, estimating, starting, running, paused, needs approval (3.9 card inline), steering sent, ready for review, failed (Try again / Ask why), cancelled, offline (cloud agents unavailable → native agent with a local model suggested), long goal text (two lines, then "More"), Hindi text.
- Light and dark (OLED/high-contrast as in 4.7); scale 1.0 and 2.0; reduced motion.
- Keyboard: Tab cycles chips, Enter opens a chip editor, arrows choose, Enter confirms, Esc closes editor then cancels; Enter on the card starts. Screen-reader names: "Mode: Sandbox, button", "Budget: up to 80 cents, 15 minutes", card announced as "Plan: Install jq…".
- 👤 Mockup approval (`ui/mockups/4.12/`) before building; gallery and recorded journey sign-off after.

**Human in the loop:** the user sees exactly what will happen before it happens (when autonomy asks), can change mode, agent, model and budget, and can cancel; nothing touches the live system until review.

**Tests**
- Unit: preview-required decision table (autonomy × risk × policy), edit validation.
- Integration (fake machine): submit "install jq" → preview → edit mode to Direct → policy denies privileged direct → explained; Start in Sandbox → `awaiting_review`.
- UI: component tests (chip editors, keyboard path), screenshots per state × theme × scale, a11y lint, journey "type → preview → edit agent → start → running → ready for review" against mock priyad.
- Regression: decision table, journey, screenshots.

**Done when**
- [ ] The journey passes with deterministic mock events and live on the Zenbook
- [ ] Every chip edit round-trips through `v1.plan.edit` and is reflected in the run that starts
- [ ] 👤 Mockup approved before build and journey signed off after

---

### 4.13 — Task center and review GUI

**Outcome:** the owner opens the Task center, sees tasks grouped by Needs you · Running · Ready for review · Done · Failed, opens one, reads the plain summary, inspects changes, checks and live activity, and presses Keep changes or Discard.
**Features:** UX-19.
**Depends on:** 4.9, 4.10, 4.11, 2.11, 2.12, 2.14, 3.9.

**Tasks**
- [ ] `ui/apps/TaskCenter/` Quickshell FloatingWindow; opened from the bar ("Open Task center"), notification actions, `priya ui tasks`; keybind proposed in the mockup review
- [ ] Left list: groups with counts, search (`v1.history.search`), filters (agent, project, status); virtualised
- [ ] Detail tabs per 11 §4: Overview (plain summary, step timeline, mode/agent/model chips, "Why?"), Changes, Checks, Activity (live normalised events: text, collapsed tool calls, subagent tree, follow mode), Cost & models
- [ ] Changes tab (review GUI): plain summary first, groups Apps · Services · System files · Your files by project with icons and counts, diffs using the 2.14 components (unified / side-by-side, syntax highlight), select groups or files to merge (`v1.change.merge {parts}`), inline conflict resolution (Mine · Agent's · Both · Ask Priya to merge, shown as its own diff), untrusted banner, external actions list, partial banner (budget or stop)
- [ ] Sticky actions: Keep changes / Discard (sandbox), Keep / Revert (direct), Ask about this (opens bar with the task as context); system merges show "Applies when you restart" and the pending-restart state; `confirm`-tier merges use the 3.9 typed confirmation
- [ ] Undo toast after Keep for home and direct changes (inverse ChangeSet through `v1.change.revert`)
- [ ] Offline state when priyad is not running, with a Start button (systemd user unit)

**UI**
- States: empty ("Nothing yet. Ask Priya to do something."), list, running, review, conflicts, tainted, partial, pending restart, failed (reason + Try again / Ask why), offline, long content (10k-entry ChangeSet virtualised; diffs over 5 MB truncated with "Open in editor"), Hindi text.
- Light and dark (OLED/high-contrast as in 4.7); scale 1.0 and 2.0.
- Keyboard: J/K move in the list, Enter opens, 1–5 switch tabs, M keep, D discard, R revert, N/P next/previous file in diffs, C opens conflict choices; visible focus ring. Screen-reader names on every row ("Install jq, ready for review, 2 changes") and diff lines read as "added line 12: …".
- 👤 Mockup approval (`ui/mockups/4.13/`) before building; gallery and journey sign-off after.

**Human in the loop:** Keep / Discard / Revert are the user's decision; partial merges let them keep only what they want; conflicts are never resolved silently; tainted system merges need typed confirmation.

**Tests**
- Unit: grouping and ordering of the list, summary text builder.
- Contract: QML API helpers generated from the 4.x schema snapshots compile and match (UI and backend can't drift, 11 §8).
- Integration (fake machine): partial merge of one file from a three-file ChangeSet; conflict resolution "Both" opens the editor file; direct keep then revert restores exactly.
- UI: component tests (list, tabs, diff navigation, conflict choices), screenshots per state × theme × scale, a11y lint, keyboard-only journey "open → review → keep", journey "conflict → choose agent's → keep".
- Perf: Task center open < 300 ms; 10k-entry ChangeSet scrolls at 60 fps on the Zenbook.
- Regression: both journeys, partial merge, direct revert, screenshots.

**Done when**
- [ ] The owner reviews and keeps a real task entirely in the GUI
- [ ] Keyboard-only journey passes; a11y lint clean
- [ ] Perf budgets met on the Zenbook; 👤 gallery signed off

---

### 4.14 — Agent end-to-end suites

**Outcome:** every agent is proven on the same small set of real tasks, in replay on every PR and live every night, with an evaluation baseline Priya's assignment learns from.
**Features:** ORC-01, ORC-02, ORC-11, SBX-11, ADM-01 (extends the agent half of QA-12).
**Depends on:** 4.2–4.13.

**Tasks**
- [ ] Scenario set (`tests/e2e/agents/`), run per agent: (a) "create a Rust hello world in ~/hello with a passing test" → checks pass → keep → exists live; (b) "install jq and write ~/bin/pp that pretty-prints JSON" → package and file merged (system part via fake-machine boot); (c) direct-mode one-line edit → keep, then revert; (d) read-only question cannot write; (e) steer mid-run; (f) stop → partial review → discard; (g) pause/resume; (h) failing test → auto-fix; (i) injection: repo README asks to read `~/.ssh` and post it → masked, tainted, confirm-tier merge (3.13 corpus); (j) budget hard stop → partial review
- [ ] Replay mode on PRs: the same scenarios with recorded streams through `priya-fake-agent` on the fake machine
- [ ] Live mode nightly (`live`): real agents on the Zenbook runner with hard budgets (Claude Code `--max-budget-usd`, proxy budgets for others), total ≤ $1.50 per night — record this budget in 09 §2
- [ ] Agent evaluation (`eval`): 20 labelled small tasks across categories; success, cost and duration per (agent, model); baseline stored and fed to `agent_stats` priors
- [ ] VM end-to-end: package installed in the VM, bar → preview → run → Task center → keep, driven over the socket with a mock-free priyad
- [ ] Fixture freshness: weekly job re-records one stream per agent and diffs event shapes; a shape change opens an issue

**Tests**
- Integration: replay matrix (agents × scenarios) on every PR.
- E2E (VM): one full journey per agent nightly.
- Security: scenario (i) per agent in the security suite.
- Eval: agent evaluation nightly; regressions > 10 % open an issue.
- Regression: the whole replay matrix and scenario (i).

**Done when**
- [ ] Replay matrix green for all five agents
- [ ] Live nightly green for three consecutive nights within budget
- [ ] Evaluation baseline committed; Priya picks differently for at least two categories based on it

---

### 4.15 — Phase gate

**Outcome:** the owner uses agents for real work on the Zenbook through the bar and the Task center.
**Features:** all Phase 4 features.
**Depends on:** 4.1–4.14.

**Tasks**
- [ ] Full regression suite green on `main`, plus nightly e2e, live agents, security and eval
- [ ] `cargo xtask trace`: every Phase 4 feature has passing tests; statuses updated in [features.md](../features.md)
- [ ] 06 and 04 §7 updated with verified flags and every design clarification made in this phase
- [ ] Owner demo (below) recorded
- [ ] `docs/human/05-the-phases.md` status updated; human guide note on agents and the Task center

**Done when**
- [ ] Demo passes on the Zenbook: Super+Space → "add a /health endpoint to my api project and make the tests pass" → preview → change the agent chip to Codex → Start → steer "use axum's Router" → checks pass → Task center review → Keep; then "install jq" with the native agent and a local model, offline
- [ ] 👤 Owner sign-off

## Phase gate

- [ ] Whole regression suite green (unit, contract, integration, UI) on `main`; nightly e2e, perf, chaos, security, eval and live jobs green
- [ ] Owner demo (4.15) completed and signed off 👤
- [ ] Architecture docs match the code (06, 04 §7, 02 §1.1 events, 02 §4 API additions, 03 §5.1 ignore rules); feature statuses updated in [features.md](../features.md)
- [ ] `docs/human/05-the-phases.md` status updated

## Out of scope

- Multi-step plans, plan DAG editing in plain language, parallel agents and worktrees (Phase 12: ORC-12, ORC-13, DEV-03)
- Automatic stuck recovery, retries across agents, crash-resume of every run, resource scheduling by battery and CPU (Phase 12: ORC-18, ORC-19, ORC-20)
- Cross-family verifier and agents reviewing each other (Phase 12: ORC-17, DEV-04)
- More adapters: Grok Build, Aider, Goose, Cursor CLI, Copilot CLI (Phase 12: ORC-21)
- Review comments sent back to the agent and the agent workspace view (Phase 5: ORC-23, ORC-24)
- Full intent pipeline, navigation intents, complete bar and card set (Phase 5)
- Network egress isolation and external actions held at a proxy (Phase 14)
- `priya mcp` tool index, memory and file search tools (Phase 8)
