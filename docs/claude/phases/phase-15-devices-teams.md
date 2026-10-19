# Phase 15 — Devices and teams

**Goal.** PriyaOS beyond one laptop: pair devices over an encrypted private network, watch and approve from the phone with the same design language, run steps on a more powerful machine, move a task to another computer, talk to external agents over A2A, share selected workspaces with teammates with shared policies and audit, and look after several machines from one place — without a central PriyaOS server and without ever sharing personal files.

**Usable at the end:** start a long task on the laptop, walk away, get a phone notification, review the summary and approve the merge on the phone with a fingerprint; later continue the same task on the desktop; hand a heavy build step to the home server; a teammate works in a shared project under the team's policies while personal files stay private.

**Depends on:** Phase 9 (installed distro, multi-user, firewall), Phase 12 (plans, scheduler, workspaces, blackboard, adapters), Phase 14 (egress proxy, external-action holds, signing, MCP registry, hardened firewall). Phase 13 automations and Phase 10 voice are reused where present.

**Features:** DEVC-01, DEVC-02, DEVC-03, DEVC-04, DEVC-05, DEVC-06, DEVC-07, DEVC-08, DEVC-09, ORC-25, ORC-26, ORC-27.

**Read first:** [07 §2 surfaces (phone companion)](../architecture/07-interfaces.md), [05-security](../architecture/05-security.md) (§2 principals, §3 secrets, §4 approvals, §6 egress, §8 audit), [06 §3 assignment, §4 workspaces, §7 communication and A2A, §11 durability](../architecture/06-orchestration.md), [03 §3 forks, §5 ChangeSets](../architecture/03-execution-and-sandbox.md), [01 §2 processes](../architecture/01-system-overview.md), [11 (all) — the phone must use the same tokens, components, states and copy rules](../architecture/11-design-system-and-ux.md), [08 §2 profiles](../architecture/08-rewind-memory-tools.md).

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 15.1 | Secure connectivity and pairing | Phase 14 | Pair a phone or second computer by QR code over Tailscale or WireGuard; per-device permissions; instant revoke |
| 15.2 | Phone companion app | 15.1 | Phone PWA shows status, tasks, review summaries and the kill switch in PriyaOS's design |
| 15.3 | Phone approvals | 15.2 | Push notification → approve on the phone (biometric for confirm) → task continues at home |
| 15.4 | Remote machines | 15.1 | "Run the build on my desktop" — a step runs on another PriyaOS machine and comes back as a reviewable result |
| 15.5 | Portable tasks | 15.4 | "Continue this on my desktop" moves a task with its workspace; or switch it to another model |
| 15.6 | A2A | 15.1 | Selected PriyaOS agents callable by external A2A agents; external A2A agents usable in plans |
| 15.7 | Shared workspaces | 15.1, 15.5 | Share one project with a teammate; assign tasks to people or agents; personal files never shared |
| 15.8 | Team policies and audit | 15.7 | Signed team policy bundles enforced on every member; team usage and audit view |
| 15.9 | Fleet view | 15.4, 15.8 | One page listing every machine's health, updates and policies, with staged updates |
| 15.10 | Phase gate | all | Phone approval, cross-machine continuation and a teammate session on real devices |

---

### 15.1 — Secure connectivity and pairing
**Outcome:** PriyaOS machines and phones talk only over an encrypted private network (Tailscale or plain WireGuard); devices are paired by QR code with key exchange, carry roles and Cedar permissions, and can be revoked instantly.
**Features:** DEVC-01, DEVC-09 (foundation).
**Depends on:** Phase 14.

**Tasks**
- [ ] `devices` module and `Connectivity` port with adapters: Tailscale (LocalAPI over `tailscaled`'s socket: status, peer IPs, `tailscale cert` for HTTPS names) and WireGuard (NetworkManager WireGuard connections; keys in Secret Service); contract suite.
- [ ] `priya-remote` gateway (user process, separate from priyad): listens only on the tailnet/WireGuard interface address; mutual authentication with device keys; forwards an allow-listed subset of `v1.*` methods to priyad per device role; rate limits; every call is an event with `actor = device:<id>`.
- [ ] Pairing protocol: laptop shows a QR code (gateway address, one-time code valid 5 min, gateway key fingerprint); device generates an Ed25519 key (phones: WebCrypto non-extractable key) and requests pairing; laptop shows device name + fingerprint words → user confirms; device record `{id, name, kind, role, pubkey, paired_at, last_seen}`.
- [ ] Roles → Cedar principal `Device::"<id>"` with attributes: `phone` (monitor, approve, kill switch), `peer` (own machine: run steps, receive tasks), `teammate` (15.7), `fleet-admin` (15.9); documented in [05 §2](../architecture/05-security.md).
- [ ] Revocation: removes the key, closes sessions within 1 s, emits `device.revoked`; lost-phone flow ("Forget this phone") also invalidates its push topic.
- [ ] Firewall: nftables rule opening the gateway port on the tailnet/WireGuard interface only (14.11 hardening kept).
- [ ] Settings → Devices page: paired devices, role, last seen, network status, Pair / Revoke; `priya devices list|pair|revoke`.

**UI**
- Devices page, pairing sheet with QR code and fingerprint words; states: no network configured, Tailscale/WireGuard connected, no devices, pairing (waiting), confirm device, paired, expired code, revoked, device offline, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; QR shown with a typed-code alternative for accessibility.
- Keyboard and screen-reader names ("Pair a device", "Phone 'Pixel', can approve, last seen 2 minutes ago").
- 👤 Mockup approval (`ui/mockups/15.1/`) before building; journey sign-off (pair a phone).

**Human in the loop:** every pairing is confirmed on the laptop with matching fingerprint words; roles are chosen by the user; revoke is one click.

**Tests**
- Unit: pairing state machine (expiry, replay of a used code, wrong fingerprint), role → allowed-method mapping.
- Contract: `Connectivity` suite for Tailscale (against Headscale in CI, no external account) and WireGuard adapters; gateway method allow-list snapshot.
- Integration (two VMs on a Headscale tailnet and on a direct WireGuard link): pair → call allowed methods; disallowed method refused; revoke → next call refused within 1 s.
- Security: gateway not reachable on non-tailnet interfaces (port scan from LAN), unpaired key rejected, replayed pairing code rejected, MITM with wrong fingerprint detected.
- Regression: `pairing_requires_confirmation`, `revoke_is_immediate`, `gateway_tailnet_only`, `role_method_allowlist`.

**Done when**
- [ ] Owner pairs the phone and a second computer; revoking the phone cuts it off immediately.

---

### 15.2 — Phone companion app
**Outcome:** a PWA served by the gateway over HTTPS shows PriyaOS's status, tasks, plans, review summaries and the kill switch on the phone, built from the same design tokens, component rules, states and copy as the desktop.
**Features:** DEVC-01.
**Depends on:** 15.1.

**Tasks**
- [ ] Token generator target `ui/design/generate/css/` producing CSS custom properties for light, dark, OLED-black and high-contrast from `ui/design/tokens.json` (ADR-014 extended); contrast checker runs on it.
- [ ] `ui/web/` PWA (TypeScript + standard web components, no heavy framework): component set mirroring `PriyaUI` (status ring, task row, plan steps, ChangeSet summary, approval card, chips with mode colour + word, cost); API client generated from the JSON Schemas like the QML helpers (11 §8).
- [ ] Screens: Home (needs you · running · ready for review · failed, cost today), Tasks, Task detail (plain summary, step timeline, ChangeSet summary, checks; diffs read-only), Approvals, Kill switch (with confirmation), Device settings.
- [ ] HTTPS: Tailscale certificate for the machine's tailnet name; for WireGuard-only setups a per-install local CA created at pairing and installed on the phone (guided).
- [ ] PWA manifest, service worker caching only the app shell; task data kept in memory, not persisted on the phone (setting to keep a last-known snapshot, off by default).
- [ ] Translations: phone strings come from the same catalogue as the desktop (10.7 pipeline exports a JSON bundle); Hindi supported.
- [ ] Live updates over a WebSocket from the gateway (event subscription filtered by role).

**UI**
- Every phone screen in states: loading, empty, list, running (live), needs you, review ready, failed, offline / laptop unreachable, session expired (re-pair), long content, Hindi.
- Light, dark, OLED-black, high-contrast; text scale 1.0 and 2.0 (phone font scaling); follows the phone's light/dark preference.
- Touch targets ≥ 44 px; keyboard operable (external keyboards); ARIA names matching the desktop's screen-reader names; tested with TalkBack/VoiceOver on L4.
- 👤 Mockup approval (`ui/mockups/15.2/`, phone frames light + dark) before building; gallery sign-off (phone gallery page) and journey sign-off.

**Tests**
- Unit: TS component tests; generated client against schema fixtures.
- UI: Playwright (Chromium mobile emulation) screenshots of every screen × state × theme × text scale; axe accessibility checks; journey against a mock gateway.
- Integration: real gateway + priyad in a VM → phone app shows a running task's live progress; disallowed data (other users, personal memory) never in responses.
- Perf: first load < 2 s on a throttled 4G profile; live update latency < 1 s on the tailnet.
- Regression: `phone_screens_states`, `phone_tokens_contrast`, `phone_live_updates`, `phone_no_data_persisted_default`.

**Done when**
- [ ] Owner uses the phone app over cellular to watch a running task; 👤 phone gallery signed off as matching the desktop design.

---

### 15.3 — Phone approvals
**Outcome:** approvals reach the phone as a push notification without content; opening it shows the full approval card; `ask` approvals are a tap, `confirm` approvals need biometric user verification (WebAuthn) plus the confirmation word; the decision is signed by the device key.
**Features:** DEVC-01.
**Depends on:** 15.2.

**Tasks**
- [ ] `ntfy` adapter for the `Notifier` port (P2): self-hosted ntfy reachable over the tailnet by default; optional public ntfy server with **content-free** messages ("PriyaOS needs you") on a random per-device topic; payloads never include task details.
- [ ] Phone approval flow: notification → app opens the approval → card identical in content to the desktop's (verb-first title, why, exactly what, risk, reversibility) → decision posted with a device-key signature bound to the approval id (replay-proof, 14.2 approval tokens).
- [ ] WebAuthn registration at pairing (platform authenticator); `confirm` tier requires user verification + the confirmation word; `ask` tier a tap (biometric optional by setting).
- [ ] Rules: remembered approvals ("always allow here") cannot be created from the phone; approvals expire as on desktop; unanswered approvals keep the task paused; first answer wins across devices, others see "answered on <device>".
- [ ] Merge from the phone: "Keep changes" on a ready review allowed for `ask`-tier merges; `confirm`-tier merges (tainted system merges) need biometric + word.
- [ ] Kill switch from the phone: one confirmation, executes `priya stop-all`.
- [ ] Quiet hours and DND respected for non-urgent notifications; approvals respect the user's "wake me for" setting.

**UI**
- Phone approval card, push permission flow, answered-elsewhere state; states: pending, approving (biometric prompt), approved, denied, expired, answered on another device, offline, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; screen-reader reads destination and reversibility.
- 👤 Mockup approval before building; journey sign-off (notification → approve merge → laptop merges).

**Human in the loop:** the phone grants exactly what a click on the desktop would, never more; confirm-tier requires the person's biometric and the word; push messages reveal nothing.

**Tests**
- Unit: signature binding, first-answer-wins, expiry, tier rules (no remembered approvals from phone).
- Contract: `Notifier` suite for ntfy (local ntfy server in CI).
- Integration: Playwright with a virtual WebAuthn authenticator → approve `ask` → task continues; `confirm` without user verification → refused; replayed signed decision → rejected.
- Security: public-ntfy message capture shows no task content; revoked phone's pending decision rejected.
- Regression: `phone_approve_ask`, `phone_confirm_requires_uv`, `phone_decision_replay_rejected`, `push_payload_content_free`.

**Done when**
- [ ] Owner approves a merge from the phone away from home and the laptop completes it.

---

### 15.4 — Remote machines
**Outcome:** a paired peer machine running PriyaOS (or the `priya` package on another Arch) can execute plan steps in its own sandbox; events stream back; results return as reviewable ChangeSets or patches.
**Features:** DEVC-03, ORC-25.
**Depends on:** 15.1.

**Tasks**
- [ ] Remote execution protocol on the gateway: `v1.remote.run_step {step_spec, workspace_seed}` → remote priyad runs the step through its **own** helper and policy (helpers never gain network; remote work is priyad ↔ priyad); normalised `run.*` events streamed back and recorded locally with `host = <device>`.
- [ ] Workspace seeding: code steps ship a git bundle of the base commit (+ uncommitted changes as a patch); home-file steps ship only the declared paths; system steps run against the remote's own system only.
- [ ] Results: code → patch/branch applied to a local worktree for review; home files → ChangeSet with blobs transferred and merged with the usual three-way rules; remote system changes stay on the remote (review shown, merge happens there).
- [ ] Remote capacity in the scheduler (12.3): CPU/RAM/GPU, current load, battery; data-locality rules; Cedar `RunRemote` with data classes ("never send anything from ~/Documents to other machines").
- [ ] Secrets stay local: remote steps use the remote's own keys/subscriptions; local secrets never transferred.
- [ ] Remote models: Priya AI can add a peer's local model server (e.g. Ollama on the desktop) as an `openai_compatible` provider over the tailnet with a one-click "Use models from <device>".
- [ ] Plain-language: "run the tests on my desktop", "use the home server for heavy builds".

**UI**
- Device chip on steps and runs ("on desktop"), remote step progress, transfer status; states: queued for remote, transferring, running remotely, peer offline (retry/local fallback), result arriving, failed, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** plans show which steps run on which machine; data-locality rules are enforced before anything is sent; results are reviewed locally like any other.

**Tests**
- Unit: workspace seed builder (bundle + patch), locality policy evaluation, remote-capacity scoring.
- Integration (two VMs): code step runs on VM B → patch reviewed and merged on VM A; peer goes offline mid-run → step resumes or falls back per policy; `~/Documents` step refused for remote.
- Security: remote cannot call methods outside `peer` role; remote run cannot read local secrets (none transferred); results from a remote are treated as coming from that device (audit attribution).
- Perf: 200 MB repo bundle transfer + start < 30 s on a 100 Mbit link.
- Regression: `remote_step_patch_review`, `remote_locality_enforced`, `remote_offline_fallback`.

**Done when**
- [ ] Owner runs a heavy build step on a second machine from a plan and keeps the result locally.

---

### 15.5 — Portable tasks
**Outcome:** a task (state, plan, board, history and workspace) moves to another paired machine and continues there; or continues on the same machine with a different agent/model from a handoff summary.
**Features:** DEVC-02, ORC-27.
**Depends on:** 15.4.

**Tasks**
- [ ] Task export: event subset (task, plan, steps, run summaries, board, approvals pending) packaged as a signed archive; task ids preserved with origin host.
- [ ] Workspace transfer: helper writes `btrfs send` streams of the fork's `home` part (and worktrees) to a pipe owned by priyad; transferred encrypted over the gateway; receiver's helper `btrfs receive`s into `@priya-forks/imported-<id>`; code-only tasks may use git bundles instead.
- [ ] System-part changes are not moved as subvolumes (different base systems); they travel as a ChangeSet and are re-applied as a sandbox step on the target with three-way checks.
- [ ] Continue on target: steps resume via adapter session resume where supported (usually not across machines) else restart from workspace state with the handoff summary (12.5); in-flight runs on the source are paused and marked "moved".
- [ ] Switch model/agent mid-task (ORC-27 on one machine): "continue this with Gemini" → current step paused, handoff built, new run with the chosen pair on the same workspace.
- [ ] Source keeps a read-only record linking to the moved task; Rewind attribution keeps both hosts.

**UI**
- "Continue on…" device picker, transfer progress, "moved to desktop" state on the source, "arrived from laptop" notification on the target, "Continue with another model" chip; states: picking, packaging, transferring, arrived, resumed, failed (with retry), target offline, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; journey sign-off (laptop → desktop).

**Tests**
- Unit: export archive schema and signature, handoff on switch, state transitions (`moved`).
- Integration (two VMs): task with a fork + worktree moves; target completes it; review and merge happen on the target; system-part ChangeSet re-applied as a sandbox step; transfer interrupted → resumable, no partial fork left.
- Chaos: kill source or target priyad mid-transfer → no duplicate task, no orphan subvolumes after GC.
- Regression: `task_moves_with_workspace`, `system_changes_reapplied_on_target`, `switch_agent_midtask`.

**Done when**
- [ ] Owner continues a real task on a second machine and finishes it there.

---

### 15.6 — A2A
**Outcome:** selected PriyaOS capabilities are exposed as A2A agents to paired peers, and external A2A agents can be added to the agent registry and used in plans — all untrusted by default and under policy.
**Features:** ORC-26.
**Depends on:** 15.1.

**Tasks**
- [ ] Pin an A2A protocol version; contract tests against the reference SDK's test suite/fixtures.
- [ ] Server side: Agent Card served by the gateway (tailnet/paired peers only) listing chosen skills (e.g. "do a code task in a sandbox", "answer questions about this machine"); incoming tasks become PriyaOS tasks with principal `Agent::"a2a:<peer>"`, tainted, approval per policy; results returned as A2A artifacts (summaries by default, files only if allowed).
- [ ] Client side: `a2a` `AgentAdapter` wrapping external A2A agents (URL + credentials from Secret Service, egress via the proxy); registered in the agent registry with traits from their Agent Card; outputs untrusted; messages can never carry approval (06 §7).
- [ ] Settings → Agents: "Shared with other agents" (which skills are exposed, to whom) and "External agents" (add by URL, review card, enable per project).
- [ ] Streaming/status mapping between A2A task states and PriyaOS step states.

**UI**
- External agents list and add flow, exposed-skills toggles, A2A task label in the Task center; states: none, added (untrusted badge), connected, unreachable, exposed skills on/off, incoming request needs approval, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** nothing is exposed until the user picks skills and peers; incoming A2A work appears as a task needing approval per policy; external agents' outputs are reviewed like any other.

**Tests**
- Unit: state mapping, Agent Card generation, adapter parsing.
- Contract: A2A conformance fixtures (server and client); `AgentAdapter` suite for `a2a`.
- Integration: VM A's PriyaOS calls VM B's exposed code skill → task on B → approval on B → artifact back to A's plan; external reference A2A agent used as a plan step.
- Security: unpaired caller rejected; injected instructions in an external agent's output taint the task and cannot approve; exposed skills cannot exceed their declared capabilities.
- Regression: `a2a_expose_requires_pairing`, `a2a_output_untrusted`, `a2a_conformance`.

**Done when**
- [ ] Two PriyaOS machines delegate a task to each other over A2A with approvals on the receiving side.

---

### 15.7 — Shared workspaces
**Outcome:** the owner shares one project with a teammate's PriyaOS (or another local user): the project, its board and task list are shared; tasks can be assigned to people or agents; personal files, memories and secrets are never part of it.
**Features:** DEVC-04, DEVC-05, DEVC-08.
**Depends on:** 15.1, 15.5.

**Tasks**
- [ ] Team model without a central server: a team is a set of paired users (role `teammate`) with one **host** machine per shared workspace; ADR documenting the peer-to-peer model and its limits (host offline → read-only on others).
- [ ] Share flow: pick a project folder → shared via its git remote (or a host-served git endpoint over the tailnet) + shared board/task list synced from the host; the share manifest lists exactly what's included.
- [ ] Personal isolation (DEVC-08): runs in a shared workspace mount only the project; personal memory, personal profile file index, mask list and personal secrets excluded; the work profile (8.5) used for shared tasks; checks before every transfer that no path outside the manifest is included.
- [ ] Task assignment (DEVC-05): tasks assignable to a person ("Assigned to you" group in their Task center, notification) or an agent; status synced; reassign; comments on tasks.
- [ ] Same-machine teammates (Phase 9 multi-user): shared project via a group `priya-team-<id>` with ACLs; each user's priyad stays separate.
- [ ] Approvals in shared workspaces: the assignee approves their tasks; actions on the shared project's external resources follow team policy (15.8).

**UI**
- Share sheet (manifest of what's shared), team members list, assignment picker, "Assigned to you" group; states: not shared, shared (host online/offline), invitation pending, member joined, task assigned, reassigned, conflict, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; journey sign-off (share → assign → teammate completes → owner reviews).

**Human in the loop:** the owner sees exactly what is shared before sharing; teammates join only by invitation and pairing; unsharing is immediate.

**Tests**
- Unit: share-manifest checker (paths outside the project, symlinks out, `.env`/secret files), assignment state machine.
- Integration (two VMs, two users): shared project; teammate's agent run cannot read the owner's `~/Documents` or personal memory; assignment syncs both ways; host offline → teammate read-only with a plain message.
- Security: symlink escape from the shared project, attempts to request personal memory via `priya mcp` in a shared run, secrets in shared ChangeSets → all blocked.
- Regression: `shared_workspace_personal_isolation`, `task_assignment_sync`, `share_manifest_symlink_escape`.

**Done when**
- [ ] A teammate (second user on a second VM or machine) completes an assigned task in the shared project with no access to the owner's personal data (audited).

---

### 15.8 — Team policies and audit
**Outcome:** team admins publish signed Cedar policy bundles that every member's machine enforces (members can only tighten them); members' machines send signed audit summaries of team-workspace activity to the admin, who sees usage, costs, models used, blocked actions and access in one view.
**Features:** DEVC-06, DEVC-07, DEVC-10.
**Depends on:** 15.7.

**Tasks**
- [ ] Team policy bundle: Cedar files + manifest signed with the team admin key (14.6 format); distributed over the gateway; versioned; installed into a `team` policy layer that loads before user policies; Cedar's forbid-overrides-permit semantics guarantee members can only tighten (validated with scenario tests).
- [ ] Bundle UI on members' machines: plain-language rendering (14.4 explainer), "From team <name>", locked.
- [ ] Audit export: for team-workspace tasks only, members' priyad produce signed summaries (task, actor, models, cost, approvals, blocked actions, merges) plus hash-chain anchors (`priya audit verify` checkable) on a schedule; personal tasks never exported.
- [ ] Admin view (Settings → Devices → Team): usage and cost by member/agent/model, blocked-action list, policy versions per member, members and roles (add, change role, revoke).
- [ ] Optional two-person rule in team policy for named actions (e.g. production deploy needs a second member's approval), using phone/desktop approvals.

**UI**
- Team admin view, member policy view; states: no team, member, admin, policy update available, member out of date, audit pending, revoked member, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; tables with tabular figures; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: bundle signature and version checks, layer ordering, "can only tighten" scenario suite, audit summary builder (no personal tasks).
- Integration (three VMs: admin + two members): bundle forbidding production deploy → enforced on both members even with a local permit; audit summaries arrive and verify; revoked member's exports rejected.
- Security: tampered bundle refused; member cannot forge another member's audit summary; personal task never appears in exports (canary task).
- Regression: `team_policy_enforced_over_local_permit`, `team_audit_signed_and_verified`, `personal_tasks_not_exported`.

**Done when**
- [ ] Owner as team admin publishes a policy and sees it enforced on a teammate's machine, with the teammate's team activity in the admin view.

---

### 15.9 — Fleet view
**Outcome:** one page (and `priya fleet`) lists every machine the owner manages — health, pending updates, disk/Rewind space, policy version, running tasks, last seen — and runs actions across them, with updates staged one machine first.
**Features:** DEVC-09.
**Depends on:** 15.4, 15.8.

**Tasks**
- [ ] Fleet registry from paired `peer`/managed devices; periodic status pull over the gateway (health score from Phase 6, updates from 6.7, Rewind usage, policy bundle version, running tasks).
- [ ] Actions: run doctor, check updates, start a sandboxed update on each machine (each machine runs its own `priya update` pipeline and boot check), apply a policy bundle, view audit.
- [ ] Staged updates: first machine (canary) updates and passes its boot check and health; then the rest; any failure stops the rollout with an explanation.
- [ ] Remote system merges respect each machine's own approval tiers: the fleet admin's approval counts only where that machine's policy names the admin device; otherwise the local user is asked.
- [ ] `priya fleet list|status|update|doctor|policy` CLI.

**UI**
- Fleet page (machine cards/table with health ring, update badge, last seen), rollout progress; states: one machine, many machines, offline machine, update available, rollout in progress, rollout stopped, needs local approval, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** fleet-wide actions show the list of machines and the plan (canary first) before starting; each machine's system changes still go through its own approvals.

**Tests**
- Unit: rollout state machine (canary pass/fail, stop on failure), status aggregation.
- Integration (three VMs): staged update with the canary failing its boot check → rollout stops, others untouched; policy bundle applied to all; offline machine shown and skipped.
- Security: fleet actions refused from devices without `fleet-admin`; remote system merge without permitted approval waits for the local user.
- Regression: `fleet_canary_stops_rollout`, `fleet_role_required`, `fleet_status_aggregation`.

**Done when**
- [ ] Owner updates two real machines from the fleet page with a canary first.

---

### 15.10 — Phase gate
**Outcome:** devices and teams work on real hardware and real phones.
**Features:** all Phase 15 features.
**Depends on:** 15.1–15.9.

**Tasks**
- [ ] Full regression and nightly multi-VM suites green (Headscale tailnet, WireGuard link, ntfy, Playwright phone tests).
- [ ] L4 checklist with the owner's phone and a second machine; record results here.
- [ ] Update `features.md`, `docs/human/05-the-phases.md`, `docs/human/04-everything-priyaos-will-do.md`.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green, including nightly multi-VM e2e (pair, remote step, portable task, A2A, shared workspace, team policy, staged fleet update) and security suites (gateway exposure, revocation, personal-data isolation, replayed approvals).
- [ ] Owner demo: long task started on the laptop → phone push → review summary → biometric approve → merge at home; same task family continued on a second machine; teammate completes an assigned task in a shared project under a team policy.
- [ ] L4 checklist (real devices): owner's phone over cellular via Tailscale (or WireGuard) — pairing, push latency < 10 s, biometric confirm, kill switch from the phone, TalkBack/VoiceOver reads the approval card; second machine on a different network; revoke the phone and confirm loss of access; laptop sleeping → phone shows "unreachable" plainly.
- [ ] Docs: new architecture section for devices and teams (gateway, pairing, roles, remote execution, team model) in [07](../architecture/07-interfaces.md)/[05](../architecture/05-security.md) or a new file; ADRs for the peer-to-peer team model and the phone tech choice; feature statuses and human status updated.

## Out of scope

- A hosted PriyaOS cloud or central team server.
- Native phone apps (the PWA plus ntfy covers the need); voice on the phone.
- Remote desktop / screen sharing of the laptop on the phone.
- Running steps on non-PriyaOS machines without the `priya` package (e.g. plain SSH hosts, cloud VMs provisioned on demand).
- Billing, seat management or organisation directories (SSO/SCIM).
