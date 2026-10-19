# Phase 13 — Automation and proactive help

**Goal.** PriyaOS works for you without being asked each time — schedules, triggers, monitors, a morning brief, overnight memory consolidation, spotting repetitive work, recipes, and teaching by showing — always within explicit limits.

**Depends on:** Phases 5–8, 12.
**Features:** AUTO-01 … AUTO-10, MEM-12, AI-20.
**Read first:** [02 §5](../architecture/02-core.md) (modules, triggers), [08 §2](../architecture/08-rewind-memory-tools.md).

## Build

- [ ] `automation` module: automations = (trigger, intent or recipe, mode, budget, approval tier); created by plain language ("every weekday at 9 summarise my projects")
- [ ] Triggers: time (systemd timers), file changes (inotify), events (failed tests, failed units, new update, low disk), webhooks (later)
- [ ] Service monitors with plain-language reports
- [ ] Safe maintenance automations (defaults off, suggested during first-boot)
- [ ] Morning brief: yesterday's tasks and merges, failures, costs, updates waiting, project summaries — in the bar and optionally spoken
- [ ] Overnight "dreaming" (on AC power, idle): consolidate task history into project memory, dedupe memories, re-index, prune Rewind per retention, prepare brief; uses batch APIs where possible (AI-20)
- [ ] Repetitive-work detection from the event log (repeated command sequences, repeated intents) → suggestion card "automate this?"
- [ ] Recipes: shareable YAML (parameters, steps, sub-recipes), `priya recipe run/share/import`
- [ ] Teach by showing: record mode (shell history + window events + AT-SPI actions with consent) → Priya AI drafts a recipe → user edits → saved as a permission-limited skill
- [ ] Limits: per-automation budgets, max runs/day, never `confirm`-tier actions without the user present

## Tests

- L0: trigger scheduling, limit enforcement, recipe parsing.
- L2: time and file triggers fire tasks with correct modes; a runaway automation is stopped by limits; dreaming produces memory diffs.
- L4: one week of real use with morning brief; owner rates usefulness.

## Done when

- [ ] Owner has at least three useful automations created by plain language
- [ ] Teach by showing turns a recorded 5-step workflow into a working recipe

**Demo.** "Every morning at 9, check my projects' CI and tell me what's broken" → next morning the brief appears.
