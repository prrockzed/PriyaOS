# Phase 2 — Tasks, policy and security base

**Goal.** Turn raw sandbox runs into managed tasks with a durable state machine, and put deterministic security around everything: Cedar policies, capability sets, secrets, approvals, budgets, audit, kill switch, untrusted labels, mode rules.

**Depends on:** Phase 1.
**Features:** CORE-07, SBX-07, SBX-10, SEC-02, SEC-03, SEC-04, SEC-05, SEC-06, SEC-07, SEC-08, SEC-09, SEC-10.
**Read first:** [02-core §2–3](../architecture/02-core.md), [03 §1](../architecture/03-execution-and-sandbox.md), [05-security](../architecture/05-security.md), ADR-006, ADR-010.

## Build

### 2.1 Task engine
- [ ] Task and step state machines in `priya-core` (pure, table-driven tests)
- [ ] Single-step plans (multi-step DAG comes in Phase 12 but the data model is complete now)
- [ ] Queue with max concurrency; durable resume/fail on priyad restart
- [ ] API: `v1.task.*`, `v1.change.*` (replacing the Phase 1 temporary CLI with `priya run --cmd`, `priya ps`, `priya logs`, `priya diff`, `priya merge`, `priya discard`, `priya kill`)

### 2.2 Mode resolution
- [ ] `modes.toml` rules (module, category, paths, privileged, lock), resolution order from [03 §1.1](../architecture/03-execution-and-sandbox.md)
- [ ] Explicit mode in the request overrides (parsed from flags now; from language in Phase 5)

### 2.3 Policy (`priya-policy`)
- [ ] Cedar schema (principals, actions, resources from [05 §2](../architecture/05-security.md)), default policies, user policies, validation on load
- [ ] Capability sets per run; enforcement points: mask list and binds at launch, Landlock in direct mode, helper operations, model use (hook for Phase 3), merge
- [ ] Plain-language explanation for every deny

### 2.4 Secrets
- [ ] Secret Service integration (libsecret) for `priya secrets set/list/remove`
- [ ] Injection into runs only when the capability allows; never into events
- [ ] Log redaction filter for `run.output`

### 2.5 Approvals
- [ ] Risk tiers and approval requests (`approval.*` events), CLI prompts now (`priya approvals`), notifications in Phase 5
- [ ] Remembered approvals become visible, removable policy entries

### 2.6 Budgets and kill switch
- [ ] Time and resource budgets per run/task (money budgets wired in Phase 3)
- [ ] `priya stop-all` (+ keybind in Phase 5): terminate all run scopes, tasks → `paused`

### 2.7 Taint v1 and external actions
- [ ] Taint rules (tool names, command patterns, URLs in prompt, `~/Downloads` inputs), `taint.added` events
- [ ] External-action detection from tool logs (git push, curl POST, mail tools…) → flagged on the ChangeSet and gated by approval tier

### 2.8 Audit
- [ ] `priya audit report --since 7d` (plain language: tasks, runs, merges, denies, approvals)

## Tests

- L0: state machines, mode resolution, Cedar policy fixtures (allow/deny matrix), redaction patterns, taint rules.
- L2: masked secrets invisible in sandbox and denied in direct mode; denied merge for tainted system change without confirmation; budget kill; stop-all; priyad restart mid-run → task `failed` with reason, fork kept; basic prompt-injection corpus (repo README asking to read `~/.ssh`) → secrets not readable, taint set.

## Done when

- [ ] Every sensitive operation passes through Cedar and is logged as `policy.decision`
- [ ] All L2 security scenarios pass
- [ ] Audit report readable by a non-technical person (owner reviews one)

**Demo.** A command that tries to read `~/.ssh` and `curl` a URL in the sandbox: show the masked folder, the untrusted label, the approval prompt on merge, and the audit report.

**Out of scope.** Network firewall (Phase 14), value-level taint (Phase 14), NL policy builder (Phase 14).
