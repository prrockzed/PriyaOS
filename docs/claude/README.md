# docs/claude — specs Claude builds from

Precise technical specs. Read in this order before working on anything.

## 1. Architecture (how the system is designed)

| File | Covers |
|---|---|
| [architecture/00-principles.md](architecture/00-principles.md) | The rules every design follows, including naming |
| [architecture/01-system-overview.md](architecture/01-system-overview.md) | Layers, processes, privilege split, domain model, request lifecycle, storage, crates |
| [architecture/02-core.md](architecture/02-core.md) | Event store, task/plan state machines, public API, module system, layered and natural-language settings |
| [architecture/03-execution-and-sandbox.md](architecture/03-execution-and-sandbox.md) | Execution modes (read-only / sandbox / direct / direct raw), Priya Sandbox, ChangeSets, merge, revert, rollback |
| [architecture/04-priya-ai.md](architecture/04-priya-ai.md) | Priya AI: providers, model catalog, roles, strategies, routing, learning, agent proxy, token optimizer |
| [architecture/05-security.md](architecture/05-security.md) | Threat model, Cedar policies, secrets, approvals, taint, egress, kill switch, audit |
| [architecture/06-orchestration.md](architecture/06-orchestration.md) | Priya Orchestration: agent adapters, planning, assignment, workspaces, supervision, verification, conflicts |
| [architecture/07-interfaces.md](architecture/07-interfaces.md) | Natural language pipeline, surfaces, voice, Priya Vision, desktop |
| [architecture/08-rewind-memory-tools.md](architecture/08-rewind-memory-tools.md) | Priya Rewind, memory layers, file index, tool index, `priya mcp` |
| [architecture/09-testing.md](architecture/09-testing.md) | Testing, CI pipeline, regression protection, traceability, schema checks, UI testing |
| [architecture/10-base-os-and-distro.md](architecture/10-base-os-and-distro.md) | Building on Arch, base OS features, ISO, installer, first boot, hardware |
| [architecture/11-design-system-and-ux.md](architecture/11-design-system-and-ux.md) | UX principles, design tokens, themes, screens and states, human in the loop and autonomy, accessibility, UI architecture, design process |
| [architecture/12-automation-devices-ecosystem.md](architecture/12-automation-devices-ecosystem.md) | Automation model, devices and teams without a central server, directory, mini-apps, themes |
| [architecture/decisions.md](architecture/decisions.md) | Architecture decision records (ADRs) and open questions |

## 2. Features

[features.md](features.md) — the feature catalog: every feature with an ID, source and phase. **Single source of truth.** New ideas are added here first.

## 3. Phases (how we build it)

[phases/README.md](phases/README.md) — the workflow, the sub-phase template and the full sub-phase map. One file per phase; each sub-phase has tasks, UI, human-in-the-loop notes, tests and a done checklist. Progress is tracked by checkboxes in those files.

## Keeping docs true

- When code changes a design, update the matching architecture file in the same commit (or add an ADR).
- When a phase finishes, tick its checklist, update feature statuses in `features.md`, and update the simple status in `docs/human/05-the-phases.md`.
