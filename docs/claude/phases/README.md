# Phases

PriyaOS is built in 17 phases. Each phase is a complete, tested slice. No dates: a phase is finished when its gate passes, not when a calendar says so.

## Workflow for every phase

1. **Plan**: re-read the phase file and the architecture docs it links. If anything is unclear or wrong, fix the docs first (or add an ADR), then commit "Plan phase N".
2. **Build**: work through the build checklist in order. Small commits. Tick boxes as items land.
3. **Test**: run every test listed for the phase at every applicable level ([../architecture/09-testing.md](../architecture/09-testing.md)). Fix until green.
4. **Gate**: all "Done when" boxes ticked, demo recorded or written up, `features.md` statuses updated, simple status updated in `docs/human/05-the-phases.md`. Commit "Complete phase N".
5. Only then start the next phase.

Phases may be split into sub-phases (e.g. `3a`, `3b`) if they turn out large; record the split in the phase file.

## Phase list

| # | Phase | Delivers | First usable by owner? |
|---|---|---|---|
| 0 | [Foundations](phase-00-foundations.md) | Repo, crates, event log, module host, settings, API, CLI, test harness, CI | — |
| 1 | [Priya Sandbox](phase-01-sandbox.md) | Forks, diffs, merge, conflicts, rollback, execution modes, root helper | On the fake machine |
| 2 | [Tasks, policy, security base](phase-02-tasks-policy-security.md) | Task engine, Cedar policies, secrets masking, approvals, audit, budgets, kill switch, taint v1 | — |
| 3 | [Priya AI](phase-03-priya-ai.md) | Providers, catalog, roles, strategies, routing, learning, token optimizer, local proxy | `priya ask` answers questions |
| 4 | [Agents, first end-to-end](phase-04-agents.md) | Claude Code / Codex / Gemini CLI / OpenCode / native agent in the sandbox, review TUI, package for existing Arch | **Yes** — on existing btrfs Arch or a VM |
| 5 | [Natural language and desktop](phase-05-desktop.md) | Priya bar, answers with cards, notifications, top bar, review GUI, NL settings, NL shell | Yes |
| 6 | [System admin and safe updates](phase-06-sysadmin-updates.md) | `priya update`, diagnostics, fixes, health checks | Yes |
| 7 | [Priya Rewind](phase-07-rewind.md) | Timeline, selective revert, restore, undo by prompt | Yes |
| 8 | [Memory, files, tools](phase-08-memory-tools.md) | Project/personal memory, file search by description, tool index, MCP | Yes |
| 9 | [Distro v0.1](phase-09-distro.md) | Package repo, base OS defaults, ISO, installer, first-boot setup, Zenbook install | **Installed on the Zenbook** |
| 10 | [Voice](phase-10-voice.md) | Push-to-talk, wake word, local STT/TTS, voice approvals | Yes |
| 11 | [Priya Vision](phase-11-vision.md) | Screen and image understanding, acting on visible UI | Yes |
| 12 | [Orchestration v2 and dev lifecycle](phase-12-orchestration-v2.md) | Multi-agent plans, parallel runs, verifier, conflict manager, PRs, CI, deploy | Yes |
| 13 | [Automation](phase-13-automation.md) | Schedules, triggers, morning brief, dreaming, teach by showing | Yes |
| 14 | [Advanced security](phase-14-advanced-security.md) | Egress firewall, value-level taint, NL policy builder, signing, Secure Boot, eBPF tracing | Yes |
| 15 | [Devices and teams](phase-15-devices-teams.md) | Phone approvals, remote machines, sharing, team policies | Yes |
| 16 | [Ecosystem](phase-16-ecosystem.md) | Module SDK, skill directory, mini-apps, self-restyling desktop, Omarchy layer | Yes |

## Order rationale

- 0–2 build the foundation every feature stands on (events, modules, sandbox, policy). Nothing user-facing is safe without them.
- 3 before 4: agents and every later feature ask Priya AI for models.
- 4 is the first complete loop (ask → sandbox → review → merge). From here the owner dogfoods.
- 5–8 make it pleasant and powerful on the existing install.
- 9 packages everything as a real OS once the core is proven.
- 10–16 add new input modes, scale, automation, hardening and the ecosystem; each is mostly modules on the existing core.
