# Phase 4 — Agents and the first end-to-end loop

**Goal.** The first complete PriyaOS experience: ask for something → Priya AI picks the agent and model → the agent works with full autonomy in Priya Sandbox (or the mode chosen) → checks run → review in a TUI → merge. Installable as a package on an existing btrfs Arch for daily dogfooding.

**Depends on:** Phases 1–3.
**Features:** ORC-01 … ORC-11, SBX-11, DEV-01, ADM-01, MEM-01, TOOL-01, UX-01, OS-01.
**Read first:** [06-orchestration §1–6, §8](../architecture/06-orchestration.md), [04 §7](../architecture/04-priya-ai.md), [08 §3](../architecture/08-rewind-memory-tools.md).

## Build

### 4.1 Agent adapters (`priya-agents`)
- [ ] Record real stream fixtures per agent first (Claude Code stream-json, Codex `exec --json`, Gemini CLI stream-json, OpenCode) — verify flags against installed versions and note them in the fixture README
- [ ] Adapters: Claude Code, Codex CLI, Gemini CLI, OpenCode (`detect`, `launch`, `parse`, `resume`, `steer`, `traits`)
- [ ] Normalised `AgentEvent` → `run.*` events (with parent ids for subagents)
- [ ] Prompt preamble ("you are in a disposable copy…"), unattended permission settings only inside sandbox/read-only workspaces
- [ ] MCP config injection so every agent gets `priya mcp`

### 4.2 Priya native agent
- [ ] Tool-using loop over Priya AI (role `coding`/`smart`), tools: shell in workspace, file read/write/edit, `priya mcp` tools; every model and tool call is a logged idempotent activity
- [ ] Works with any provider incl. local models

### 4.3 Assignment and supervision (single-step)
- [ ] Agent registry (`priya agents list`), (agent, model) choice via Priya AI, agent outcome stats
- [ ] Statuses and live progress; pause/resume (SIGSTOP/SIGCONT), stop, restart; live steering via adapter
- [ ] Background tasks (`--detach`) with completion notification hook (desktop in Phase 5)

### 4.4 Checks
- [ ] Project `.priya.toml` `setup`/`run`/`check`; auto-detect (`Cargo.toml` → `cargo test`, `package.json` test script, `pyproject` → pytest, `Makefile` test target)
- [ ] Verifier results on the task and the review card

### 4.5 `priya mcp` (basic) and history
- [ ] MCP stdio server: `task_history`, `system_info`, `rewind_log` (stub until Phase 7); read-only socket for forks
- [ ] `history_fts` + `priya history "<query>"`

### 4.6 Review TUI
- [ ] Summary (packages, services, system files, home grouped by project), file diffs, conflicts, check output, logs, cost, model/agent used, untrusted banner; keys: merge, discard, keep/revert (direct), resolve conflicts

### 4.7 Package for existing Arch
- [ ] PKGBUILD (`priya`), systemd units (helper system, priyad user), sysusers (`priya`, `priya-admin`), tmpfiles, default configs
- [ ] Install guide in `docs/human/07-install-on-your-zenbook.md` "try it on your current Arch" section

## Tests

- L0: parsers on fixtures for every agent, preamble building, check auto-detection.
- L1: `AgentAdapter` contract suite (fake agent binary that replays fixtures).
- L2: per agent: "create a Rust hello world in ~/hello with a passing test" → review → merge → exists live; "install jq and write ~/bin/pp that pretty-prints JSON" → package + file merged; direct mode on a small file edit → keep and revert; native agent with a local Ollama model completes the hello-world task.
- L3 (VM, package install): same scenarios on a VM with Arch + btrfs + the package.

## Done when

- [ ] All four external agents and the native agent pass the L2 scenarios
- [ ] Priya AI picks different (agent, model) pairs for a code task vs a quick admin task, with explanations
- [ ] Owner installs the package on the Zenbook (if Phase 0 found a usable btrfs Arch) or uses the VM, and completes one real task end to end

**Demo.** `priya run "add a /health endpoint to my api project and make the tests pass"` → live progress → review TUI → merge.
