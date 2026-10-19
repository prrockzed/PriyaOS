# Phase 12 — Priya Orchestration v2 and the development lifecycle

**Goal.** Many agents across model families working together on one goal: multi-step plans, parallel runs in isolated forks and worktrees, handoffs and shared notes, cross-family verification, conflict management, resource scheduling, robust supervision — and the full software lifecycle through to PRs, CI and deploys.

**Depends on:** Phases 4–8.
**Features:** ORC-12 … ORC-22, DEV-02 … DEV-08.
**Read first:** [06-orchestration](../architecture/06-orchestration.md) (all), [04 §5–6](../architecture/04-priya-ai.md).

## Build

### 12.1 Plans
- [ ] Planner (role `smart`) producing DAGs with goals, agent/model roles, modes, budgets, verifiers; cost and time estimate per plan
- [ ] Plan approval rules; plan card in the bar; plain-language plan edits ("do the frontend with Gemini", "skip deploy")
- [ ] Re-planning after failures (`plan.revised`)

### 12.2 Parallel execution
- [ ] Scheduler: dependencies, concurrency limit, cgroup CPU/RAM per run, battery-aware, provider rate limits, budgets; priorities
- [ ] Workspaces: shared forks for sequential steps, per-step git worktrees + branches inside one fork for parallel code work, separate forks for parallel system work; per-workspace setup scripts and port ranges
- [ ] More adapters: Grok Build, Aider, Goose, Cursor CLI, Copilot CLI (as installed)

### 12.3 Collaboration
- [ ] Blackboard (`board.md`, `tasks.json`) mounted in every workspace of the task; claim/complete via `priya mcp` with locking
- [ ] Handoffs (outputs of a step into dependants' context); agent-to-agent messages labelled by sender
- [ ] Sub-agents with capability subsets (permission inheritance enforced)

### 12.4 Quality
- [ ] Cross-family verifier step (role `review`, different family than worker)
- [ ] Conflict manager: overlap detection, `git merge-file`, AI-proposed merges as their own diffs, semantic conflicts as user questions; merge ordering
- [ ] Supervision: stuck detection, nudge, escalate model, switch agent, fail; resume after reboot via adapter resume or workspace restart
- [ ] Agent and model dashboards (`priya agents stats`, panel)

### 12.5 Development lifecycle (`dev` module)
- [ ] Repository understanding: architecture summary into project memory
- [ ] PR creation (gh / GitLab CLIs) with summaries; external action → approval
- [ ] CI watch and failure diagnosis → fix tasks
- [ ] Deploy steps (project-defined commands) behind `ask`/`confirm`, post-deploy health check, rollback command

## Tests

- L0: DAG scheduling, permission-subset enforcement, conflict detection, merge ordering.
- L2: a 4-step plan (backend, frontend in parallel → integration test → docs) completes with two different agent families; forced conflict resolved; a stuck agent is detected and replaced; reboot mid-plan resumes.
- L3: PR creation against a local Gitea in the VM; CI failure diagnosis on a seeded failing workflow.

## Done when

- [ ] Parallel multi-agent plan finishes faster than sequential on the demo task with equal verifier results
- [ ] Cross-family review catches seeded bugs in ≥ 70 % of a 20-case suite
- [ ] Every plan is explainable: who did what, with which model, at what cost

**Demo.** "Add user accounts to my app: backend, frontend, tests, docs, and open a PR" → plan card → approve → watch agents in the Priya workspace → review → PR opened after approval.
