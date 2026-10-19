# Phase 7 — Priya Rewind

**Goal.** One timeline of every change on the machine — by agents, modules, settings and the user by hand — with precise undo of a single change, restore of any file to any time, full-system return, and undo by plain language.

**Depends on:** Phases 1–5 (ChangeSets, blob store, Priya AI, surfaces).
**Features:** RWD-01 … RWD-10.
**Read first:** [08 §1](../architecture/08-rewind-memory-tools.md), [03 §5](../architecture/03-execution-and-sandbox.md).

## Build

### 7.1 Timeline capture (`rewind` module + helper ops)
- [ ] `@priya-timeline` subvolume; hourly snapshots of `@` and `@home` (systemd timer), pacman pre/post hooks (skipped in forks)
- [ ] Retention policy (hourly 48 h, daily 30 d, weekly 6 mo; bracketing snapshots kept while referenced), pruning with notification
- [ ] Lazy manual ChangeSets between timeline snapshots (same diff engine)

### 7.2 Browse
- [ ] `priya rewind log [--path] [--since] [--agent] [--kind]`, unified with Priya runs and settings changes; attribution (agent/model/user, task, reason)
- [ ] Rewind panel in the Quickshell UI: timeline, filter, per-change details

### 7.3 Undo operations
- [ ] Selective revert of a ChangeSet or chosen entries (task in sandbox: fork current state, apply inverse with three-way checks against later changes, review, merge)
- [ ] `priya rewind restore <path> --at <time>`
- [ ] Full-system return to a timeline point (system part on reboot)
- [ ] Settings revert from `setting.changed` history
- [ ] Post-undo verification: re-run the original task's verifiers or a symptom check

### 7.4 Undo by prompt
- [ ] Query builder (Priya AI `smart`): time window, areas, paths, packages, services, symptoms
- [ ] Candidate retrieval (FTS + structured filters + manual windows), ranking with plain-language explanations, user choice or "most likely"
- [ ] Bar integration: "undo …" intents route to Rewind

### 7.5 Space
- [ ] `priya rewind usage`, low-space pruning, settings for retention

## Tests

- L0: inverse ChangeSet construction, three-way checks for reverts, retention selection, query building fixtures.
- L2: agent installs and configures something, user edits a related file later, selective revert removes only the agent's change; manual edit captured by timeline and restorable; settings revert; full-system return; undo-by-prompt on a seeded history picks the right change in ≥ 9/10 scripted cases.
- Chaos: interrupt a revert merge at every journaled step.

## Done when

- [ ] "Undo whatever broke X on <day>" works on seeded scenarios with explanations the owner understands
- [ ] Rewind never removes the user's later work silently (conflicts always surfaced)
- [ ] Timeline space stays within the configured budget on a week of simulated activity

**Demo.** Break Wi-Fi config on purpose (manually), do other work, then Super+Space → "undo whatever broke my Wi-Fi this afternoon" → candidates → revert → Wi-Fi back.
