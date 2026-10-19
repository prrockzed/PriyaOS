# Phase 14 — Advanced security

**Goal.** Harden PriyaOS for a hostile world: every run gets its own network namespace behind an egress proxy with domain allow-lists, external actions are held until approved, the native agent tracks untrusted values end to end (CaMeL-style information-flow control), users write policies in plain language, everything third-party is scanned and signed, agent processes are traced with eBPF, MCP servers are registered with per-agent access, untrusted work can run in a microVM, per-agent Unix users are evaluated, and the platform gets Secure Boot, TPM-backed disk unlock and a hardened firewall. A full prompt-injection corpus and red-team suite prove it.

**Usable at the end:** a repo with a poisoned README tells the agent to upload `~/.ssh` to a paste site → the file is masked, the connection is blocked at the proxy, the attempt is shown in review in plain language and recorded in the audit report. The owner types "agents can read my projects but never deploy to production" and sees it enforced. The Zenbook boots with Secure Boot on and Windows still boots.

**Depends on:** Phase 2 (helper, nspawn runner), Phase 3 (Cedar, approvals, taint v1, secrets, audit), Phase 4 (native agent, MCP injection), Phase 9 (installed distro, nftables defaults, LUKS option, signed repo), Phase 12 (sub-agents, verifier, deploys), Phase 13 (recipes and automations to scan and constrain).

**Features:** SEC-11, SEC-12, SEC-13, SEC-14, SEC-15, SEC-16, SEC-17, SEC-18, SEC-19, SEC-20, SBX-12.

**Read first:** [05-security](../architecture/05-security.md) (all — primary), [03 §1.2 external actions, §4 running](../architecture/03-execution-and-sandbox.md), [06 §1 native agent, §7 messages, §10 observability](../architecture/06-orchestration.md), [01 §2 processes](../architecture/01-system-overview.md), [10 §2 security, §4 Zenbook notes](../architecture/10-base-os-and-distro.md), [09 §2 security suite](../architecture/09-testing.md), [decisions ADR-005, ADR-006, ADR-008, ADR-010](../architecture/decisions.md), [11 §4 Approval, Settings → Autonomy & Safety](../architecture/11-design-system-and-ux.md).

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 14.1 | Per-run network isolation and egress proxy | Phase 9 | Every run reaches only allow-listed domains; review shows blocked attempts |
| 14.2 | External actions held at the proxy | 14.1 | Pushes, posts and unknown hosts wait for approval instead of happening |
| 14.3 | Value-level information-flow control | 14.2 | The native agent can read untrusted content without letting it steer tool calls unapproved |
| 14.4 | Plain-language policy builder and Policy page | Phase 3 | "Never deploy to production" → readable policy → confirm → enforced |
| 14.5 | Skill and script scanning | 14.1 | Skills, recipes, modules, MCP servers and new scripts are scanned before use or merge |
| 14.6 | Signing and install permissions | 14.5 | Modules/skills/recipes install only signed (or explicitly allowed), with a permissions prompt |
| 14.7 | eBPF agent tracing and alerts | 14.1 | Every run has a system-activity lane; out-of-scope access raises plain alerts |
| 14.8 | System MCP registry | 14.6 | Each agent sees only the MCP servers it is allowed; tool changes need re-approval |
| 14.9 | MicroVM tier | 14.1 | Untrusted work runs in a cloud-hypervisor microVM with the same review and merge |
| 14.10 | Per-agent users evaluation | 14.7 | ADR decides whether direct-mode runs use per-agent Unix users; prototype measured |
| 14.11 | Secure Boot, TPM unlock, firewall hardening | Phase 9 | Secure Boot on (Windows still boots); TPM unlock per the boot ADR; hardened firewall and sysctls |
| 14.12 | Full injection corpus and red team | 14.1–14.9 | Nightly corpus across every agent: zero exfiltration, zero unapproved external actions |
| 14.13 | Phase gate | all | Poisoned-repo demo on the Zenbook with Secure Boot on |

---

### 14.1 — Per-run network isolation and egress proxy
**Outcome:** every run (sandbox, read-only, direct) starts in its own network namespace whose only route is an HTTP(S) proxy that allows CONNECT/HTTP to domains on the run's allow-list; everything else is blocked and recorded.
**Features:** SEC-11.
**Depends on:** Phase 9.

**Tasks**
- [ ] Helper operation `net.create_run_netns {run_id}`: netns + veth pair (host side in a private /30 per run), nftables inside the netns: drop everything except TCP to the proxy address; no DNS inside (the proxy resolves names); helper protocol schema and invariant tests.
- [ ] Launch paths: nspawn `--network-namespace-path=` for sandbox/read-only runs; direct runs launched by the helper inside the netns, then dropped to the user in the transient scope.
- [ ] `priya-egress` proxy (user-level, part of priyad's process tree, Rust, `hyper`): HTTP CONNECT and plain HTTP; no TLS interception; checks TLS ClientHello SNI matches the CONNECT host; IP-literal destinations denied unless listed; per-run allow-list keyed by veth address.
- [ ] Allow-list = Priya AI local proxy (exposed as `priya-ai.internal`) + model provider endpoints the run may use + package mirrors (from `/etc/pacman.d/mirrorlist`) + ecosystem registry sets (crates.io, npm, PyPI…) when the step is a code step + domains in the capability set; Cedar `NetConnect` evaluated per new host.
- [ ] Environment: `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY=` set for runs; agent CLIs' proxy settings verified per adapter; tools that ignore proxies fail with a plain explanation in the run log.
- [ ] Events `net.connect {run, host, port, decision, bytes_out, bytes_in}` aggregated per (run, host) to keep volume bounded; blocked attempts listed in the review ("The agent tried to reach pastebin.com — blocked").
- [ ] Mid-run domain request: unknown host → `ask` approval ("Allow this task to reach crates.io?"), rememberable per project; tainted runs → `confirm`.

**UI**
- Review "Network" section (allowed hosts with traffic, blocked attempts) and domain approval card; states: no network used, allowed only, blocked attempts, pending domain approval, many hosts (grouped), Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names (blocked rows read "blocked" in words, not colour).
- 👤 Mockup approval (`ui/mockups/14.1/`) before building; gallery sign-off after.

**Human in the loop:** the user sees every host a run talked to and every blocked attempt; new domains need approval; remembered domains are visible and removable on the Policy page (14.4).

**Tests**
- Unit: allow-list matching (wildcards, IDNs, trailing dots), SNI mismatch detection, aggregation.
- Contract: helper protocol additions snapshot; proxy behaviour suite (CONNECT allowed/denied, HTTP Host checks, IP literals, slowloris timeouts).
- Integration (fake machine): agent run with `curl https://pastebin.com` → blocked, event, review entry; `cargo build` with crates.io set → works; raw UDP/ICMP/DNS from the run → no route; direct-mode run gets the same isolation.
- Security: DNS exfiltration, IP-literal connect, SNI/Host mismatch (domain fronting), proxy-bypass via env unset → all blocked.
- Perf: added connection latency < 5 ms p95; throughput ≥ 200 MB/s for package downloads; netns setup < 100 ms per run.
- Regression: `egress_blocks_unlisted`, `egress_allows_registry_set`, `egress_no_dns_in_netns`, `egress_sni_mismatch_blocked`, `direct_run_isolated`.

**Done when**
- [ ] All Phase 4/12 agent e2e scenarios still pass with isolation on.
- [ ] Every run in a day of owner use has a network section in review; zero unlisted connections succeed.

---

### 14.2 — External actions held at the proxy
**Outcome:** in sandbox mode, external actions (pushes, posts to unknown hosts, deploys, publishing) are held until approved: the connection or the credential is paused, the user sees exactly what is being attempted, and only an approval releases it.
**Features:** SEC-11.
**Depends on:** 14.1.

**Tasks**
- [ ] Hold points (CONNECT-level proxies can't see `git push` vs `git fetch` on the same host, so holds happen where the action becomes distinguishable):
  - unknown or `write`-class hosts: CONNECT held (not answered) while approval is asked; timeout → 403 with reason;
  - credentials: git credential helper, `gh`/`glab` tokens and deploy secrets are issued by priyad only against an approval token for that action (05 §3);
  - plain HTTP: method + host visible, non-GET to non-allow-listed hosts held;
  - tool logs and exec events (14.7) attach the command (`git push origin main`) to the approval card.
- [ ] Host classes in the capability set: `read` (fetch only, no credentials issued), `write` (credentials on approval), `deny`.
- [ ] Approval card for held actions: verb-first title ("Push 3 commits to github.com/you/api?"), what is leaving, from which run/agent, reversibility line "Can't be undone: leaves your computer", Allow once / Allow for this task / Deny.
- [ ] Taint interaction: tainted runs → `confirm` for every external action (05 §4).
- [ ] Events `external.held`, `external.released`, `external.denied`; audit report section "What left this computer".

**UI**
- Held-action approval card (bar, notification, Task center, voice and phone where available); states: held, approved, denied, expired, multiple held (batched), Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; Y/A/N keys; screen-reader reads destination and reversibility.
- 👤 Mockup approval before building; journey sign-off (agent pushes → held → approve → PR flow continues).

**Human in the loop:** nothing leaves the machine through a hold point without an explicit decision; unanswered holds pause the task and never auto-allow (11 §5).

**Tests**
- Unit: host-class resolution, approval-token binding (token for push to repo A cannot release push to repo B), expiry.
- Integration: sandboxed run `git push` to a local Gitea → held → approval → pushed; deny → push fails, run continues with an explanation; `curl -X POST` to unknown host → held → denied.
- Security: replayed approval token rejected; agent-crafted message claiming approval ignored; credential request without approval refused.
- E2E (VM): Phase 12 PR and deploy journeys pass with holds active.
- Regression: `push_held_until_approved`, `approval_token_bound_to_action`, `unknown_host_post_held`.

**Done when**
- [ ] The 12.12–12.14 journeys work end to end with holds on, and every external action in the audit report has a matching approval.

---

### 14.3 — Value-level information-flow control
**Outcome:** the Priya native agent can run in IFC mode: a privileged planner never sees untrusted content, a quarantined model turns untrusted content into typed values, every value carries labels, and any tool call whose arguments derive from untrusted values needs approval (or is denied by policy).
**Features:** SEC-12.
**Depends on:** 14.2.

**Tasks**
- [ ] Design note + ADR (CaMeL-style): planner output is a small typed program (JSON AST of tool calls, variable bindings, conditionals, loops with bounds) rather than free text; interpreter in `priya-agents::native::ifc`.
- [ ] Labels in `priya-policy::ifc`: `Label {sources: {user, trusted_file, tool:<id>, web, screen, agent_message, …}, readers: Public | Only(set)}`; propagation rules for every interpreter operation; property tests.
- [ ] Quarantined model calls (`fast` or `smart` role, no tools): extract typed values from untrusted content into the schema the planner asked for; outputs labelled with the content's sources.
- [ ] Tool-call policy: Cedar context gets per-argument labels; shipped policies e.g. external actions with untrusted-derived recipients/URLs → `confirm`; writing outside the workspace with untrusted-derived paths → deny; reading secrets into values readable by untrusted sinks → deny.
- [ ] Mode selection: IFC mode on for tainted tasks and when `agents.native.ifc = "always"`; normal loop otherwise; mode shown on the run.
- [ ] Approval card provenance: "The email address in this request came from a web page you didn't write."
- [ ] Utility tracking: tasks where IFC mode blocked progress are recorded for tuning.

**UI**
- Provenance line on approval cards and a "Protected mode" chip on native-agent runs; states: protected, needs approval (untrusted argument), denied by policy, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; screen-reader reads provenance in words.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: label propagation for every AST node type; interpreter bounds (loop limits, no dynamic code); policy decisions on labelled arguments.
- Contract: planner program schema snapshot; quarantined-call schema.
- Eval: an AgentDojo-style suite (≥ 60 tasks over files, web pages, messages, repos, each with injected variants): attack success on protected tool calls = 0 %; benign utility in IFC mode ≥ 70 % of the normal loop's.
- Security: injected instruction to send `~/.config/…` contents to an address in a web page → approval with provenance or denial; never silent.
- Mutation: zero surviving mutants in the label-propagation module.
- Regression: `ifc_untrusted_arg_requires_approval`, `ifc_planner_never_sees_untrusted`, `ifc_agentdojo_suite`.

**Done when**
- [ ] Eval thresholds met; 05 §5 v2 marked implemented.

---

### 14.4 — Plain-language policy builder and Policy page
**Outcome:** the user states a rule in plain language; Priya AI drafts Cedar; the user sees a deterministic plain-language rendering, the code, and test scenarios; on confirmation the policy is active, visible and undoable.
**Features:** SEC-13.
**Depends on:** Phase 3.

**Tasks**
- [ ] `v1.policy.nl_draft {text}` → Cedar draft (role `smart`) → validated with the `cedar-policy` validator against the schema; up to 2 repair rounds.
- [ ] Deterministic explainer: renders any policy AST into plain language from templates (never relies on the model to describe what the code does); used for drafts and for every existing policy.
- [ ] Scenario check: generated example requests ("Codex deploys to production", "Claude Code reads ~/projects/api") evaluated against the full policy set before/after, shown as "Now allowed / Now blocked".
- [ ] Conflict and breadth lint: redundant/shadowed policies (scenario sampling plus Cedar's analysis tooling where available); warnings for overly broad permits ("any agent, any action").
- [ ] Activation at `confirm` tier (05 §4); saved under `~/.config/priya/policy/user-<slug>.cedar`; recorded as a settings-kind ChangeSet so Rewind can undo it.
- [ ] Settings → Autonomy & Safety → Policies page: all policies (shipped, admin, user, remembered approvals, remembered domains) in plain language with source, enable/disable (user ones), "Test a scenario", delete (undo toast), history; admin policies shown as locked.
- [ ] CLI `priya policy list|explain|test|add|rm`.

**UI**
- Policy draft sheet (plain language, scenarios, "Show code"), Policies page; states: drafting, draft ready, invalid (couldn't express), conflicts found, confirmed, disabled, locked by admin, empty user policies, long lists, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names (allow/forbid read as words).
- 👤 Mockup approval before building; journey sign-off (type rule → review scenarios → confirm → blocked action shown).

**Human in the loop:** a policy is never activated without confirmation; the user sees its effect on concrete scenarios first; every change is undoable.

**Tests**
- Unit: explainer golden outputs for every AST construct; scenario generator; breadth lint.
- Eval: 40 labelled plain-language rules → correct Cedar (scenario-equivalent to reference) ≥ 90 %; zero drafts broader than the request on the 40 (scenario check).
- Integration: "agents can read my projects but never deploy to production" → confirmed → Phase 12 deploy to `production` denied with plain explanation; undo via Rewind → allowed again.
- UI: screenshots, a11y, journey.
- Regression: `nl_policy_enforced`, `policy_explainer_golden`, `policy_change_undoable`.

**Done when**
- [ ] Owner writes two policies in plain language and sees each enforced.

---

### 14.5 — Skill and script scanning
**Outcome:** skills, recipes, modules, MCP servers, agent adapters and new executable scripts are scanned (static rules + a cross-family model review) before install, registration or merge; risky findings block until the user confirms.
**Features:** SEC-14.
**Depends on:** 14.1.

**Tasks**
- [ ] `priya-scan` crate: rule engine over files with rules for pipe-to-shell (`curl … | sh`), remote eval, base64/hex-decoded exec, credential paths (`~/.ssh`, `~/.aws`, keyrings), persistence (crontab, user units, shell rc, autostart), hidden Unicode/bidi characters, high-entropy blobs, network exfil patterns, agent-instruction text in skill/README files (prompt-injection markers); shellcheck for shell; tree-sitter queries for Python/JS/TS.
- [ ] AI review (role `review`, family ≠ the authoring agent's family when known): verdict `clean | warn | block` with reasons; budget-capped; result cached by content hash.
- [ ] Scan points: install/import of skills (agent skill dirs), recipes (13.9 imports), modules, MCP server registration (14.8), adapters; **merge gate**: new or changed executable files and scripts in a ChangeSet from a tainted run are scanned before merge.
- [ ] Verdict handling: `warn` → shown in review/install; `block` → needs `confirm` to override, recorded in audit.
- [ ] `priya scan <path>` CLI; scan results card.

**UI**
- Scan result card (findings with file:line, plain explanation, severity word + icon), review "Scripts" section; states: scanning, clean, warnings, blocked, override confirmed, scanner error, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: every rule against positive and negative fixtures (≥ 5 each); cache by hash.
- Eval: 60 malicious and 60 benign samples (skills, scripts, recipes) → block/warn recall ≥ 95 % on malicious, false-block ≤ 5 % on benign.
- Integration: tainted run adds `install.sh` with `curl | sh` → merge blocked pending confirm; clean skill installs without friction.
- Perf: static scan < 1 s for a 1 MB skill; AI review only for non-trivial content.
- Regression: `scan_blocks_curl_pipe_sh`, `scan_merge_gate_tainted_scripts`, `scan_eval_corpus`.

**Done when**
- [ ] Eval thresholds met; every install/import path calls the scanner (asserted by an integration test per path).

---

### 14.6 — Signing and install permissions
**Outcome:** modules, skills, recipes and adapters from outside the base packages install only with a valid signature from a trusted publisher (or explicit developer-mode override), after a plain-language permissions prompt; tampering after install is detected at load.
**Features:** SEC-15.
**Depends on:** 14.5.

**Tasks**
- [ ] ADR: signature format for non-pacman artifacts (proposed: Ed25519 detached signatures in a minisign-compatible format over a canonical manifest + content hash list; publisher key rotation; revocation list). Pacman packages keep the Phase 9 GPG model.
- [ ] Trust stores: `/usr/share/priya/trust/` (PriyaOS), `/etc/priya/trust/` (admin), `~/.config/priya/trust/` (user); revocation list fetched with updates.
- [ ] `priya sign` / `priya verify` CLI for publishers and users.
- [ ] Install flow: verify signature → scan (14.5) → permissions prompt (capabilities from the manifest in plain language, risky ones highlighted: network domains, secrets, privileged ops, external actions) → install; capability increase on update → prompt again.
- [ ] Load-time verification by the module host on every start (hash check); mismatch → module refused, notification, audit event.
- [ ] Developer mode: unsigned local artifacts allowed after `confirm`, shown with an "Unsigned" badge everywhere.

**UI**
- Install permissions prompt (like mobile app permissions), Unsigned badge, tamper notification; states: verified publisher, unknown publisher, unsigned (developer mode), revoked, signature invalid, capability increase on update, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; journey sign-off (install signed module → prompt → allow).

**Human in the loop:** the user sees every capability before install and on every increase; unsigned code needs a typed confirmation.

**Tests**
- Unit: canonical manifest serialisation, signature verify, revocation, capability diff on update.
- Integration: signed module installs; modified byte after install → refused at next start; revoked key → refused with explanation; update adding `network:*` → re-prompt.
- Security: signature-stripping, key-substitution and downgrade (older signed version with a known vulnerability, via revocation list) attempts fail.
- Regression: `unsigned_module_refused`, `tampered_module_refused_at_load`, `capability_increase_reprompts`.

**Done when**
- [ ] Every non-base module/skill/recipe/adapter on the owner's machine is signed or explicitly marked unsigned.

---

### 14.7 — eBPF agent tracing and alerts
**Outcome:** a small root tracing service attaches eBPF programs to run cgroups and reports exec, file and network activity per run; activity outside the run's scope raises a plain alert; the Task center gets a system-activity lane.
**Features:** SEC-17, SEC-16.
**Depends on:** 14.1.

**Tasks**
- [ ] `priya-trace` system service (root with `CAP_BPF`, `CAP_PERFMON`, `PrivateNetwork=yes`, no AI), Rust + Aya; streams aggregated events to priyad over a Unix socket; separate from `priya-helper` to keep the helper's operation set fixed (ADR).
- [ ] Probes filtered by run cgroup id: `sched_process_exec`, file opens (`security_file_open` via fentry/LSM hook), `connect`, DNS attempts; metadata only — no TLS plaintext capture.
- [ ] Scope checker in priyad: compares observed activity with the run's capability set → `trace.out_of_scope {run, kind, target}` (SEC-16) → alert card ("Codex tried to read ~/.aws/credentials. It was blocked by the mask.") and audit entry.
- [ ] Suspicious exec list (e.g. `nc`, `socat`, `ssh`, `scp`, `base64 -d | sh` chains) → alerts, configurable.
- [ ] Model-endpoint accounting: connections to known model hosts per run (agents' own model traffic made visible).
- [ ] Hang signal: no syscalls + no CPU for N minutes → fed to the stuck detector (12.9).
- [ ] Optional BPF-LSM enforcement for direct-mode runs where the kernel has `lsm=…,bpf` (evaluated; off by default; Landlock remains primary).
- [ ] Task center Activity tab "System activity" lane; `priya trace <run>` CLI.

**UI**
- Out-of-scope alert card/notification, system-activity lane; states: quiet, activity, alert, many events (aggregated), tracer unavailable (kernel lacks BTF/BPF LSM → plain notice), Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: scope checker (paths, domains, exec lists), aggregation windows.
- Integration (VM — eBPF needs a real kernel): fake agent reads a masked path, execs `nc`, connects to an unlisted host → three out-of-scope events attributed to the right run; activity of non-run processes never reported.
- Perf: tracing overhead < 3 % CPU on a `cargo build` run; event rate capped (≤ 1k/s per run after aggregation).
- Security: run cannot detach or blind the tracer (no `CAP_BPF` in runs; unprivileged BPF disabled by sysctl in 14.11).
- Regression: `trace_out_of_scope_alert`, `trace_attribution_by_cgroup`, `trace_overhead_budget`.

**Done when**
- [ ] Every run on the Zenbook has a system-activity lane; seeded out-of-scope attempts produce alerts within 2 s.

---

### 14.8 — System MCP registry
**Outcome:** every MCP server on the machine is registered with owner, transport, tools and trust; each agent's MCP configuration is generated from the registry with per-agent access lists; tool-description changes require re-approval.
**Features:** SEC-18.
**Depends on:** 14.6.

**Tasks**
- [ ] Registry in priyad (projection + `~/.config/priya/mcp/*.toml`): `{id, owner, transport (stdio|http), command|url, signature/trust, scan verdict, tools_hash, allowed_agents}`; shipped entries for `priya mcp` and app-action servers (11.8).
- [ ] Discovery: import MCP servers found in agents' own configs (`~/.claude.json`, Codex/Gemini/OpenCode configs) as "unregistered — review" items; they are not given to runs until reviewed.
- [ ] Cedar action `UseMcpServer` (principal `Agent`/`Run`, resource server) — documented in 05 §2.
- [ ] Per-run MCP config generation from the registry; adapters pass it so agents ignore other configured servers (e.g. Claude Code's strict MCP config option — verify per agent version); inside forks, agents' own MCP config files are masked if the agent can't be told to ignore them.
- [ ] Tool pinning: tool list and descriptions hashed at approval; change ("rug pull") → server disabled for runs until re-approved, with a diff of what changed.
- [ ] Remote (HTTP) MCP servers reached only through the egress proxy with their domain on the run's allow-list.
- [ ] `priya mcp-registry list|add|remove|allow|deny|review` and Settings → Agents → MCP servers section.

**UI**
- MCP servers section (list, per-agent access matrix, review queue, tool-change diff); states: empty, registered, unregistered found, tools changed, blocked, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard-navigable matrix with screen-reader cell labels ("Codex may use GitHub server: no").
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: config generation per adapter, tools hashing, access evaluation.
- Integration: Claude Code fixture run sees only allowed servers; server changes a tool description → disabled until re-approved; unregistered server from `~/.claude.json` not available to runs.
- Security: tool-poisoning corpus entries (malicious descriptions) are scanned and flagged; agent cannot add a server at runtime.
- Regression: `mcp_registry_per_agent_access`, `mcp_tool_change_reapproval`, `mcp_unregistered_not_injected`.

**Done when**
- [ ] All MCP servers on the owner's machine are registered and reviewed; each agent's effective server list is shown on the Agents page.

---

### 14.9 — MicroVM tier
**Outcome:** steps marked untrusted run in a cloud-hypervisor microVM whose root and home are the fork's subvolumes shared over virtiofs; ChangeSets, review and merge are identical; on machines without KVM the system falls back to nspawn with a notice.
**Features:** SBX-12.
**Depends on:** 14.1.

**Tasks**
- [ ] ADR: hypervisor choice (proposed cloud-hypervisor for virtiofs support; Firecracker lacks virtiofs and would need block-device images, breaking the btrfs fork model).
- [ ] `SandboxRunner` adapter `microvm`: helper starts `virtiofsd` for the fork's `root` and `home`, launches cloud-hypervisor with a minimal guest kernel (`priya-microvm-kernel` package) and a tiny init that mounts virtiofs, applies masks, starts the agent as the user; control and event stream over vsock.
- [ ] Networking: guest tap device placed in the run's netns from 14.1, so the egress proxy applies unchanged.
- [ ] Resource limits mapped from the step budget (vCPUs, memory, run timeout); VM inside the run's transient scope for accounting.
- [ ] Tier selection: `isolation = "nspawn" | "microvm"` per step; policy defaults microVM for tainted tasks, imported recipes, unsigned modules' tasks and steps running scanned-`warn` scripts; chip "VM" on the run.
- [ ] Boot checks of system changes stay on nspawn (`-b`) or use a full guest boot of the fork (evaluated; documented).
- [ ] Contract: `SandboxRunner` suite passes for `microvm`.

**Tests**
- Contract: `SandboxRunner` suite (run command, masks, limits, output streaming, kill) for `microvm`.
- Integration (VM with nested KVM / Zenbook runner): Phase 4 hello-world and package-install scenarios in microVM → ChangeSets identical to nspawn runs; kill switch stops a VM run within 5 s.
- Security: guest cannot see masked paths, other users' homes, or host network; virtiofs exposes only the fork.
- Perf: VM ready < 1.5 s; file-heavy build within 1.5× of nspawn time; memory overhead < 150 MiB per VM.
- Regression: `microvm_runner_contract`, `microvm_changeset_parity`, `tainted_task_uses_microvm`.

**Done when**
- [ ] Tainted tasks on the Zenbook run in microVMs by default with acceptable speed (owner check) and identical review.

---

### 14.10 — Per-agent users evaluation
**Outcome:** a measured prototype answers whether direct-mode runs should execute as per-agent Unix users with ID-mapped mounts; ADR-010 is reaffirmed or superseded, and if accepted the feature ships behind a setting.
**Features:** SEC-20.
**Depends on:** 14.7.

**Tasks**
- [ ] Prototype: sysusers `priya-agent-<id>`; direct-mode runs launched as that user; writable paths exposed through ID-mapped mounts (`mount_setattr` `MOUNT_ATTR_IDMAP`) so files land owned by the real user; agent auth dirs (`~/.claude`, `~/.codex`…) mapped per agent.
- [ ] Measure against the current model (Landlock + masks + eBPF): isolation gained (red-team cases from 14.12 that only per-agent users stop), compatibility (agents, git, editors, file ownership after merge), performance, complexity of the helper changes.
- [ ] ADR-0xx with the result; if accepted: implement behind `agents.direct_isolation = "landlock" | "agent_user"`, with contract and integration tests; if rejected: document why and what would change the answer.

**Tests**
- Integration (VM): prototype passes Phase 4 direct-mode scenarios; files created are user-owned; agent cannot read other masked paths even without Landlock.
- Security: comparison matrix run on both models with the 14.12 corpus subset for direct mode.
- Regression (if accepted): `agent_user_files_user_owned`, `agent_user_isolation`.

**Done when**
- [ ] ADR merged with measured evidence; decision reflected in [05](../architecture/05-security.md) and [06 §1](../architecture/06-orchestration.md).

---

### 14.11 — Secure Boot, TPM unlock, firewall hardening
**Outcome:** the boot chain is signed with the owner's keys (Windows still boots), disk unlock with TPM2 works within what the boot ADR allows, the firewall and kernel settings are hardened, and a Security page shows the status.
**Features:** SEC-19, SEC-25.
**Depends on:** Phase 9.

**Tasks**
- [ ] ADR: Secure Boot and encryption with `/boot` inside `@` (ADR-008). Facts to resolve: GRUB must load as a standalone signed image with embedded modules under Secure Boot; kernels inside `@` (and inside every fork and timeline snapshot) must be signed; GRUB's LUKS2 support is limited (PBKDF2 key slots) and it cannot unseal from TPM2. Options evaluated in the VM: (a) keep ADR-008, GRUB passphrase unlock + initramfs keyfile, no TPM unlock; (b) small unencrypted, signed `/boot` with kernels copied in per root generation, TPM2 unlock in initramfs; (c) UKIs. Decision recorded; ADR-008 amended or superseded.
- [ ] sbctl integration: `sbctl create-keys`; signing GRUB EFI and kernels via pacman hook; **signing happens in the helper at merge time**, never inside forks — signing keys (`/var/lib/sbctl`) are masked from all runs.
- [ ] Guided enrollment (owner, following a new section of `docs/human/07-install-on-your-zenbook.md`): firmware Setup Mode, `sbctl enroll-keys --microsoft` (keeps Windows and option ROMs working), BitLocker recovery key ready (PCR 7 changes).
- [ ] TPM2 unlock per the ADR: `systemd-cryptenroll --tpm2-device=auto` with PCR 7 (+ policy for kernel updates), recovery key printed/stored by the owner, passphrase fallback always kept.
- [ ] Previous-system boot entry (SBX-13) and rollbacks verified under Secure Boot (old roots must contain signed kernels).
- [ ] Firewall hardening on top of the Phase 9 nftables defaults: inbound deny except explicitly allowed services, rate-limited logging, outbound rules for `priya-egress` only where needed.
- [ ] Kernel/sysctl hardening: `kernel.unprivileged_bpf_disabled=1`, `kernel.kptr_restrict=2`, `kernel.dmesg_restrict=1`, `fs.protected_*`, Yama `ptrace_scope=1` (documented impact on debuggers).
- [ ] Automatic security-update tasks through Priya Sandbox (05 §10): daily check, security-only updates tested and offered (respecting autonomy).
- [ ] Settings → Autonomy & Safety → Security status section: Secure Boot (on/off/setup mode), disk encryption, TPM unlock, firewall, last security update; `priya doctor` checks for each.

**UI**
- Security status section; states: all good, Secure Boot off, setup mode, encryption off, TPM not enrolled, firewall modified, updates pending, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** key enrollment and firmware changes are done by the owner following the human guide — never automated against the real machine (CLAUDE.md safety rules); every step has a documented way back.

**Tests**
- Unit: doctor checks parsing `sbctl status`, `cryptsetup luksDump`, nft ruleset.
- E2E (VM, OVMF with Secure Boot and swtpm): install → enroll keys (VM only) → boot signed → kernel update via sandbox → merge → reboot still verified → rollback to previous root boots → TPM unlock (per ADR option) → fake Windows entry still chains.
- Security: a kernel installed inside a fork reaches `@` only signed by the helper at merge (unsigned kernels never land in `@`); runs cannot read `/var/lib/sbctl`.
- Regression: `secure_boot_vm_update_rollback`, `sbctl_keys_masked_in_runs`, `firewall_inbound_default_deny`, `sysctl_hardening_applied`.

**Done when**
- [ ] VM suite green; on the Zenbook Secure Boot is on, PriyaOS and Windows both boot, a kernel update and a rollback both boot.

---

### 14.12 — Full injection corpus and red team
**Outcome:** a versioned prompt-injection and red-team corpus runs nightly against every agent and the native agent (with and without IFC); exfiltration and unapproved external actions stay at zero.
**Features:** SEC-11, SEC-12, SEC-14, SEC-16, SEC-18 (verification; extends the Phase 3 basic corpus SEC-10).
**Depends on:** 14.1–14.9.

**Tasks**
- [ ] Corpus layout `tests/security/corpus/<vector>/<case>/` with `case.toml` (vector, goal, expected controls, covers) for vectors: web pages (hidden text, comments, CSS-hidden), repo READMEs/issues/code comments, documents (PDF/DOCX white text), images and screenshots with embedded text (Phase 11), MCP tool descriptions (tool poisoning), skills/recipes, agent-to-agent messages (12.5), CI logs (12.13), package install scripts.
- [ ] Attack goals per case: read secrets, exfiltrate, unapproved external action, capability escalation, persistence (cron/systemd/rc files), policy tampering, audit tampering.
- [ ] Runner `cargo xtask security corpus` across all installed agent adapters (fixture-replay for PRs, live agents nightly with budget caps), recording per-agent "attempted" vs "succeeded".
- [ ] Red-team scenarios: malicious module with undeclared capability; agent → paste site; skill with `curl | sh`; MCP rug pull; tampered signed module; forged approval in a message; helper protocol fuzzing (`cargo-fuzz` on the parser, 1 h nightly); priyad socket permission checks; audit chain tamper and `priya audit verify`.
- [ ] Nightly report as a CI dashboard issue; any success = release blocker.
- [ ] Owner-run "red-team hour" checklist for manual attempts, results added as new cases.

**Tests**
- Security (nightly, full): 100 % of cases with zero secret exfiltration and zero unapproved external actions; 100 % of attempts produce an event visible in review/audit.
- Security (PR, fast subset ≤ 3 min): 40 representative cases with fixture-replayed agents.
- Fuzz: no crashes in 1 h of helper-protocol fuzzing nightly.
- Regression: entire corpus tagged `regression` + `security`.

**Done when**
- [ ] Corpus ≥ 300 cases across every vector; green on two consecutive nightlies with all installed agents.

---

### 14.13 — Phase gate
**Outcome:** PriyaOS is hardened on the Zenbook and the evidence is reproducible.
**Features:** all Phase 14 features.
**Depends on:** 14.1–14.12.

**Tasks**
- [ ] Full regression, nightly security corpus, fuzz, e2e (Secure Boot VM) suites green.
- [ ] L4 checklist on the Zenbook (below); results recorded here.
- [ ] Update `features.md`, `docs/human/05-the-phases.md`, `docs/human/04-everything-priyaos-will-do.md`, and the install guide's Secure Boot section.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green, including the full injection corpus (nightly, two consecutive green runs), helper fuzzing, Secure Boot VM e2e, microVM contract and chaos suites.
- [ ] Owner demo: poisoned README → masked secret + blocked connection + plain review entry + audit report line; "agents can read my projects but never deploy to production" → enforced; held `git push` approved from a notification; an unsigned module refused; a tainted task shown running in a microVM.
- [ ] L4 checklist (Zenbook): Secure Boot enabled with the owner's keys + Microsoft keys; Windows boots (BitLocker recovery key used at most once, expected); kernel update and rollback boot under Secure Boot; TPM unlock (if the ADR enables it) and passphrase fallback; eBPF tracer works on the shipped kernel; microVM runs with KVM; no noticeable slowdown in daily use (bar < 100 ms, agent runs within 10 % of Phase 12 timings).
- [ ] ADRs merged: IFC design, signing format, `priya-trace` service, hypervisor choice, per-agent users decision, Secure Boot/encryption boot chain.
- [ ] Docs: [05-security](../architecture/05-security.md) updated to "implemented" with any design changes; [03 §4](../architecture/03-execution-and-sandbox.md) network section; [01 §2](../architecture/01-system-overview.md) process table gains `priya-trace` and `priya-egress`; feature statuses and human status updated.

## Out of scope

- TLS interception or content inspection of agent traffic.
- Information-flow control inside third-party agents (they stay on task-level taint + egress control, 05 §5).
- Hardware security keys for approvals (phone biometric approvals come in Phase 15).
- External security audit (recommended before a public release; not a phase gate item).
- Mandatory access control frameworks (AppArmor/SELinux profiles) beyond Landlock, BPF LSM evaluation and seccomp.
