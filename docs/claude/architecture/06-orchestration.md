# 06 — Priya Orchestration: agents and multi-agent work

Priya Orchestration turns a task into a plan, picks the right agent and model for each step, runs steps in the right workspaces, supervises them, verifies results, resolves conflicts, and brings everything back as reviewable ChangeSets. It is durable: every step and every model/tool call is a logged, idempotent activity, so work survives crashes and reboots.

## 1. Agent registry and adapters

Port `AgentAdapter` (crate `priya-agents`):

```rust
trait AgentAdapter {
    fn id(&self) -> AgentId;                                  // "claude-code", "codex", "gemini-cli", "grok-build", "opencode", "priya-native", …
    fn detect(&self) -> Detection;                            // installed? version? logged in? supported models?
    fn launch(&self, step: &StepSpec, env: &RunEnv) -> Command;   // headless argv + env, model flag, MCP config, output format
    fn parse(&self, line: &str) -> Vec<AgentEvent>;           // normalise the agent's JSONL stream
    fn resume(&self, run: &RunRecord) -> Option<Command>;     // continue a session after interruption, if supported
    fn steer(&self, run: &RunRecord, msg: &str) -> SteerMethod;   // how to send mid-run guidance
    fn traits(&self) -> AgentTraits;                          // strengths, model families, needs network, subagent support
}
```

| Agent | Headless mode (verify flags against the installed version) | Notes |
|---|---|---|
| Claude Code | `claude -p --output-format stream-json` (+ `--verbose`), permission mode for unattended runs, `--model` | Stream has `parent_tool_use_id` for subagents; agent teams exist (experimental) |
| Codex CLI | `codex exec --json`, `--sandbox` tiers, `-m`, `exec resume --last` | Inside Priya Sandbox use its full-access tier; outside, its own read-only default |
| Gemini CLI | `gemini -p --output-format stream-json`, `-m` | Refuses headless in untrusted folders → trust the fork's project path |
| Grok Build | xAI CLI (beta) | Adapter when stable |
| OpenCode | `opencode run` with OpenAI-compatible providers | Easiest to point at local models / Priya AI proxy |
| Aider, Goose, Cursor CLI, Copilot CLI | each one's headless mode | Added as adapters on demand |
| **Priya native agent** | built in | A tool-using loop over Priya AI + `priya mcp` tools + shell inside the workspace. Works with **any** model that has an API key, including local ones. Also the only agent where value-level taint tracking is possible (Phase 14). |

Normalised `AgentEvent`s (text, tool call, tool result, subagent start/end with parent id, usage/cost, final answer, error) are written as `run.*` events, so every agent looks the same in logs, review, cost reports and Rewind.

Agent identity: each run carries `Agent::"<id>"` for policy and audit, plus its own systemd scope (cgroup) for resource limits and attribution. Separate Unix users per agent (as Windows does with agent accounts) were considered; deferred because merged files must be owned by the user — see ADR-010.

## 2. Planning

Priya AI (role `smart`) produces a plan DAG ([02-core.md](02-core.md) §3) from the intent, the project memory, the agent registry and the tool index:

- Simple intents → one step, no visible planning.
- Complex intents → steps with dependencies, each with a goal, suggested agent/model role, mode, budget, verifiers.
- Plans are shown when policy requires approval; the user can edit in plain language ("do the frontend with Gemini", "skip the deploy").
- Re-planning: when a step fails after retries, the planner revises the remaining plan (`plan.revised`).

## 3. Assignment

For each ready step Priya Orchestration asks Priya AI for the best **(agent, model)** pair:
- filters: agent installed and authenticated, supports the step's needs (repo editing, shell, GUI, long context), model allowed by strategy/privacy;
- scores: agent's measured success on this category (same outcome tracking as models), model score, cost;
- preference for diversity in verification: the `review` step uses a different family than the step it checks.

## 4. Workspaces and isolation

| Situation | Workspace |
|---|---|
| Single step, needs system changes | Own Priya Sandbox fork |
| Sequential steps on the same goal | Shared fork (`SharedWith`) |
| Parallel code steps in one repo | One fork, separate **git worktrees + branches** per step (cheap), each with a setup script and its own port range |
| Parallel steps touching system state | Separate forks, merged one after another with conflict checks |
| Direct mode | Live machine, safety snapshot, Landlock-limited paths |

Per-project `.priya.toml` can define `setup` (install deps), `run` (dev server) and `check` (tests) commands, used for every worktree/fork — the same idea as Conductor's per-workspace scripts.

## 5. Scheduling and resources

- Ready steps run when dependencies are done, subject to: max concurrent runs (default 3; separate from the fork limit of 4 in [03 §6](03-execution-and-sandbox.md) because a finished run may still hold a fork while it waits for review), CPU/RAM cgroup limits per run (systemd scope `CPUQuota`, `MemoryMax`), battery state (pause background work below 20 % unless plugged in), provider rate limits, task and daily budgets.
- Priorities: interactive > user-started background > automations.

## 6. Supervision

- Heartbeats from the event stream; **stuck detection** (no events for N minutes, or repeated identical tool calls) → nudge via `steer`, then restart with a different model, then fail.
- Retries: same agent with escalated model, then a different agent; each attempt is a new run under the same step.
- Pause/resume: SIGSTOP/SIGCONT the run scope for short pauses; for long pauses or reboots, `resume` via the adapter's session resume, else restart the step from its workspace state.
- Live steering: the user (or Priya AI) can send guidance to a running step ("use postgres, not sqlite").

## 7. Communication between agents

- **Blackboard**: a per-task shared notes file and task list (stored in the user's data dir, mounted **read-only** at `/priya/task/<id>/` (`board.md`, `tasks.json`) in every workspace of the task; all writes go through `priya mcp`), with claim/complete operations through the `priya mcp` tools (locked by priyad). Inspired by Claude Code agent teams' shared task list and mailboxes.
- **Handoffs**: a step's outputs (ChangeSet summary, notes, artifacts) are given to dependent steps' prompts.
- **Messages**: `priya mcp` `send_message(step, text)` for agent-to-agent notes, labelled with the sender (an agent's message can never carry user approval).
- External agents over **A2A** and remote MCP: Phase 15.

## 8. Verification

Each step's verifiers run after it finishes, in its workspace:
- project checks (`check` command, tests, lints, build);
- boot check for system changes;
- **cross-family review**: a `review`-role model from a different family than the worker reads the diff and the goal and returns pass/fail with reasons;
- custom commands from the plan.

Failed verification → retry/escalate per §6. Results are shown on the review card.

## 9. Conflict manager

When parallel steps finish: compute overlap between their ChangeSets (same paths, same packages, same config keys). Text conflicts → `git merge-file`; if that fails, Priya AI proposes a merged version (shown as its own diff); semantic conflicts (two different package choices, incompatible configs) → surfaced to the user as a plain-language question. Merge order is chosen to minimise conflicts.

## 9a. Development lifecycle

The `dev` module (Phase 12) handles repository understanding, pull requests (via `gh`/`glab`; external action → approval), CI watching and diagnosis, and deploys. Deploys are defined per project in `.priya.toml` (`[deploy] command`, `health_check`, `rollback`), always `ask` or `confirm` tier, followed by the health check and an offered rollback.

## 10. Observability

- Unified run timeline (all agents, normalised events, parent/child tree, costs) in the review UI and `priya logs`.
- Per-agent and per-model cost and success dashboards (`priya ai stats`, `priya agents stats`).
- Phase 14: eBPF tracing of agent processes (process exec, file and network activity, LLM endpoints) attached to runs without modifying agents — the approach Alibaba's AgentSight uses.

## 11. Durability

Steps and runs are event-sourced. Every model call and tool execution that Priya itself performs (native agent, planner, verifier) is an **activity**: logged with an idempotency key before execution and its result after, so replay after a crash never repeats a side effect (same idea as running agent loops on Temporal). Third-party agent runs are resumed through their own session resume when available, otherwise restarted from the workspace state with a summary of prior progress.
