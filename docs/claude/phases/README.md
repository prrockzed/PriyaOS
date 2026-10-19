# Phases

PriyaOS is built in **17 phases**, each split into **small sub-phases**. No dates: a sub-phase is finished when its gate passes.

**Usable from day 1** (ADR-012): Phase 0 ends with a working Priya bar that answers questions on any Arch install, in light and dark. Every later sub-phase ends with something the owner can try.

## Workflow

For every sub-phase:

1. **Plan**: re-read the sub-phase and the architecture it links. Fix unclear docs first. For UI work, make a mockup (light + dark) and get the owner's approval ([11 §9](../architecture/11-design-system-and-ux.md)).
2. **Build** on a branch, in small pull requests (one task or a few per PR).
3. **Test**: write the listed tests **with the code**, tagged `covers: <feature IDs>`. CI must be green ([09](../architecture/09-testing.md)).
4. **Gate**: "Done when" all ticked; acceptance tests tagged `regression`; feature statuses updated in [features.md](../features.md); owner sign-off where marked 👤.
5. Next sub-phase. A phase ends with its **phase gate**: whole regression suite green, owner demo, docs updated, status updated in `docs/human/05-the-phases.md`.

Sub-phases inside a phase may run in parallel when their "Depends on" allows it.

## Sub-phase template

Every sub-phase in the phase files uses this structure:

```markdown
### N.k — Title
**Outcome:** one sentence: what works after this (say what the owner can try, if anything).
**Features:** feature IDs, or "infrastructure".
**Depends on:** sub-phase IDs.

**Tasks**
- [ ] small task (≈ one pull request)

**UI** (only if the sub-phase has UI)
- Screens/components, every state (empty, loading, streaming, success, error, offline, long content, Hindi text)
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0
- Keyboard path; screen-reader names
- 👤 Mockup approval before building; gallery sign-off after

**Human in the loop** (only if relevant): what the user sees, decides, can undo.

**Tests**
- Unit: …
- Contract: …
- Integration (fake machine): …
- UI: components, screenshots, accessibility, journeys
- E2E (VM) / Perf / Chaos / Security / Eval: … (where relevant)
- Regression: which acceptance tests join the permanent suite

**Done when**
- [ ] measurable criteria
```

## Phase list and sub-phase map

### Phase 0 — [Foundations and walking skeleton](phase-00-foundations.md)
Usable at the end: **Super+Space on the current Arch → ask anything → streamed answer from one model, light/dark themed.**
0.1 Zenbook survey and dev environments · 0.2 Repository and workspace scaffold · 0.3 CI pipeline v1 and PR workflow · 0.4 Test infrastructure (testkit, tags, traceability, diff coverage, regression runner) · 0.5 Event store · 0.6 Settings system · 0.7 priyad, public API and CLI skeleton · 0.8 Module host and SDK · 0.9 Schema compatibility checks · 0.10 Design tokens and theme generator · 0.11 PriyaUI base components, gallery and UI test harness · 0.12 Walking skeleton: minimal model access and `priya ask` · 0.13 Walking skeleton: minimal Priya bar · 0.14 Package for any Arch and first install · 0.15 Phase gate

### Phase 1 — [Priya AI](phase-01-priya-ai.md)
Usable: **many models, automatic best-model choice, switching by chip or command, cost view, local models.**
1.1 Unified model types and provider port · 1.2 Anthropic and OpenAI adapters · 1.3 Google and xAI adapters · 1.4 OpenAI/Anthropic-compatible adapter and local discovery · 1.5 Key storage and "Connect a provider" flow · 1.6 Model catalog, priors, discovery · 1.7 Roles, strategies, control · 1.8 Classifier · 1.9 Router · 1.10 Reliability and offline mode · 1.11 Costs and budgets · 1.12 Token optimizer I (cache layout, provider caching, trimming, caps) · 1.13 Token optimizer II (compaction, response cache, savings) · 1.14 Learning from outcomes · 1.15 Local proxy for agents · 1.16 AI & Models settings, model picker, usage view · 1.17 Router evaluation suite · 1.18 Phase gate

### Phase 2 — [Priya Sandbox](phase-02-sandbox.md)
Usable: **the Zenbook runs the PriyaOS disk layout; any command can run in a sandbox, be reviewed and kept or thrown away; rollback works.**
2.1 Root helper · 2.2 Snapshot backend · 2.3 Fake machine in CI · 2.4 Disk-layout doctor and fixes · 2.5 Dev installer and moving the Zenbook onto the layout · 2.6 Fork lifecycle · 2.7 Sandbox runner · 2.8 Booted runs (boot check) · 2.9 ChangeSet engine I (paths, classification, blobs) · 2.10 ChangeSet engine II (summaries) · 2.11 Home merge and conflicts · 2.12 System merge, rollback and previous-system boot · 2.13 Read-only and direct modes · 2.14 Review TUI and diff components · 2.15 Chaos and performance suites · 2.16 Phase gate

### Phase 3 — [Tasks, policy and safety](phase-03-tasks-policy-security.md)
Usable: **tasks with live status, approvals in the bar and notifications, autonomy levels, secrets hidden, kill switch.**
3.1 Task and step state machines · 3.2 Task engine and API · 3.3 Mode resolution · 3.4 Cedar policy engine · 3.5 Capability enforcement · 3.6 Secrets masking, injection, redaction · 3.7 Approvals engine · 3.8 Autonomy levels · 3.9 Approval card, inbox and minimal notifications · 3.10 Budgets and kill switch · 3.11 Untrusted labels and external actions · 3.12 Audit report · 3.13 Security test corpus (basic) · 3.14 Phase gate

### Phase 4 — [Agents and first actions](phase-04-agents.md)
Usable: **"do this" in the bar → the best agent does it in the sandbox → review in the Task center → keep.**
4.1 Agent port, normalised events, fake agent · 4.2 Claude Code adapter · 4.3 Codex adapter · 4.4 Gemini CLI adapter · 4.5 OpenCode adapter · 4.6 Priya native agent · 4.7 Agent registry and Agents page · 4.8 Agent + model assignment · 4.9 Run supervision (progress, pause, stop, steer, background) · 4.10 Checks and `.priya.toml` · 4.11 `priya mcp` basic and history search · 4.12 Plan preview card and bar actions · 4.13 Task center and review GUI · 4.14 Agent end-to-end suites · 4.15 Phase gate

### Phase 5 — [Desktop experience](phase-05-desktop.md)
Usable: **PriyaOS feels like a complete, beautiful desktop driven by plain language.**
5.1 Intent pipeline and context capture · 5.2 Navigation intents · 5.3 Priya bar complete · 5.4 Full card set · 5.5 Top bar and OSDs · 5.6 Notification centre · 5.7 Agent workspace and review comments · 5.8 Settings app complete · 5.9 Natural-language settings · 5.10 Natural-language shell · 5.11 Whole-desktop theming, OLED black, high contrast, accents, auto schedule · 5.12 Workspace layouts · 5.13 Accessibility, motion and performance pass · 5.14 `priya desktop install` · 5.15 Phase gate (owner usability session)

### Phase 6 — [Looking after the computer](phase-06-sysadmin-updates.md)
6.1 System tools for agents · 6.2 Diagnosis flow · 6.3 Performance playbooks · 6.4 Services and logs playbooks · 6.5 Packages and dependencies playbooks · 6.6 Hardware playbooks · 6.7 Update check and Arch news · 6.8 `priya update` pipeline · 6.9 Update fix loop · 6.10 Health page and maintenance · 6.11 Dev environment setup · 6.12 Phase gate

### Phase 7 — [Priya Rewind](phase-07-rewind.md)
7.1 Timeline snapshots · 7.2 Retention and space · 7.3 Manual ChangeSets · 7.4 Timeline query and attribution · 7.5 Selective revert · 7.6 Restore a path at a time · 7.7 Return the system to a point · 7.8 Settings revert · 7.9 Post-undo verification · 7.10 Undo by prompt · 7.11 Rewind UI · 7.12 Phase gate

### Phase 8 — [Memory, files and tools](phase-08-memory-tools.md)
8.1 Tool index · 8.2 Desktop and D-Bus registration · 8.3 `priya mcp` complete · 8.4 Project memory · 8.5 Personal memory and profiles · 8.6 Memory UI · 8.7 File index · 8.8 Search by description and summaries · 8.9 Duplicates, obsolete files, relationships · 8.10 Continue where I left off · 8.11 Phase gate

### Phase 9 — [PriyaOS 1.0 distribution](phase-09-distro.md)
9.1 Packages, signed repo, release pipeline · 9.2 Default apps and shell · 9.3 System panels complete · 9.4 App installs through the sandbox · 9.5 Firewall, printing, backup · 9.6 Multi-user · 9.7 Graphical installer · 9.8 Encryption option · 9.9 Boot counting, automatic fallback and recovery UI · 9.10 First-boot onboarding · 9.11 Branding (boot, login, lock, wallpapers) · 9.12 Zenbook hardware validation · 9.13 Phase gate (fresh install by the owner)

### Phase 10 — [Voice](phase-10-voice.md)
10.1 Audio capture and push-to-talk · 10.2 Speech-to-text · 10.3 Listening UI and live transcripts · 10.4 Spoken answers · 10.5 Wake word · 10.6 Voice approvals and stop phrase · 10.7 Hindi · 10.8 Privacy controls · 10.9 Optional cloud voice · 10.10 Phase gate

### Phase 11 — [Priya Vision](phase-11-vision.md)
11.1 Screen capture and window metadata · 11.2 Region capture and preview · 11.3 Image inputs · 11.4 Vision routing and OCR · 11.5 Accessibility-tree reader · 11.6 Private-window protection · 11.7 Acting on the screen · 11.8 App actions in the tool index · 11.9 Phase gate

### Phase 12 — [Orchestration v2 and software lifecycle](phase-12-orchestration-v2.md)
12.1 Planner and plan DAG · 12.2 Plan UI · 12.3 Scheduler and resources · 12.4 Workspaces (worktrees, shared forks, ports) · 12.5 Blackboard, messages, handoffs · 12.6 Sub-agents and permission inheritance · 12.7 Cross-family verifier · 12.8 Conflict manager · 12.9 Supervision v2 · 12.10 More agent adapters · 12.11 Multi-agent views and scoreboards · 12.12 Repository understanding and pull requests · 12.13 CI diagnosis · 12.14 Deploys · 12.15 Phase gate

### Phase 13 — [Automation](phase-13-automation.md)
13.1 Automation engine and limits · 13.2 Triggers · 13.3 Creating automations by plain language and the Automations page · 13.4 Monitors · 13.5 Maintenance automations · 13.6 Morning brief · 13.7 Overnight "dreaming" and batch jobs · 13.8 Spotting repetitive work · 13.9 Recipes · 13.10 Teach by showing · 13.11 Phase gate

### Phase 14 — [Advanced security](phase-14-advanced-security.md)
14.1 Per-run network isolation and egress proxy · 14.2 External actions held at the proxy · 14.3 Value-level information-flow control · 14.4 Plain-language policy builder and Policy page · 14.5 Skill and script scanning · 14.6 Signing and install permissions · 14.7 eBPF agent tracing and alerts · 14.8 System MCP registry · 14.9 MicroVM tier · 14.10 Per-agent users evaluation · 14.11 Secure Boot, TPM unlock, firewall hardening · 14.12 Full injection corpus and red team · 14.13 Phase gate

### Phase 15 — [Devices and teams](phase-15-devices-teams.md)
15.1 Secure connectivity and pairing · 15.2 Phone companion app · 15.3 Phone approvals · 15.4 Remote machines · 15.5 Portable tasks · 15.6 A2A · 15.7 Shared workspaces · 15.8 Team policies and audit · 15.9 Fleet view · 15.10 Phase gate

### Phase 16 — [Ecosystem](phase-16-ecosystem.md)
16.1 Module SDKs and templates · 16.2 Developer docs site · 16.3 Directory backend · 16.4 Directory UI and install by asking · 16.5 App action SDK · 16.6 Generated mini-apps · 16.7 Desktop restyling with preview · 16.8 Wallpaper accents and theme sharing · 16.9 Omarchy layer · 16.10 Phase gate

## Order rationale

- **0** builds the foundations *and* a usable slice, so real use starts immediately.
- **1** makes that slice smart (all models, routing, costs) — still only answers, so no system changes yet.
- **2–3** add the safety machinery (sandbox, policy, approvals) before any agent can change anything. Phase 2 also moves the Zenbook onto the PriyaOS disk layout with a dev installer so dogfooding happens on real hardware.
- **4** lets agents act. **5** completes the desktop. **6–8** add system care, undo and memory.
- **9** polishes into a 1.0 distribution. **10–16** add voice, vision, multi-agent work, automation, hardening, devices and the ecosystem — mostly as modules on the proven core.
