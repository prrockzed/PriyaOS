# Phase 3 — Tasks, policy and safety

**Goal.** Turn raw sandbox runs into managed, durable **tasks** with live status, and put deterministic safety around everything before any agent can act: execution-mode rules, Cedar policies checked at every enforcement point, capability sets, secrets that runs never see unless granted, approvals with risk tiers and remembered decisions, per-feature autonomy levels, budgets, a kill switch, untrusted labels with external-action detection, and a plain-language audit report.

**Usable at the end.**
- `priya run --cmd "…"` creates a task; `priya ps -w` and a status strip in the Priya bar show live status (queued, running, needs you, ready for review, done, failed).
- Every sensitive operation is a Cedar decision logged as `policy.decision`; denies are explained in plain language.
- Secrets are masked from runs, injected only when granted, and redacted from output, logs and events.
- Approvals arrive as an approval card in the bar, a desktop notification with Allow/Deny, an approvals inbox and `priya approvals`; "Always allow here" becomes a visible, removable rule.
- Autonomy per feature (Ask me first / Ask when it matters / Just do it) in Settings → Autonomy & Safety.
- Budgets for time, money and resources; Super+Shift+Escape stops everything and shows the kill-switch overlay.
- Runs that read untrusted input are labelled Untrusted; external actions (push, send, deploy) are detected and gated.
- `priya audit report --since 7d` reads like a diary; a basic security corpus runs on every PR.

**Depends on:** Phase 2 gate (helper, forks, runner, ChangeSets, merge, modes, review TUI). Uses Phase 1's classifier risk output, key storage (Secret Service) and cost budgets, and Phase 0's event store, settings, module host, Priya bar and component gallery.

**Features:** CORE-07, SBX-07, SBX-10, SEC-02, SEC-03, SEC-04, SEC-05, SEC-06, SEC-07, SEC-08, SEC-09, SEC-10, UX-18, DS-09, DS-10, DS-11, QA-20.

**Read first:**
- [05 — Security and governance](../architecture/05-security.md) (primary)
- [02 — Core §1 events, §2 task state machine, §3 plans, §4 API, §5 modules, §6 settings](../architecture/02-core.md)
- [03 — Execution §1 modes, §1.1 mode resolution, §1.2 external actions](../architecture/03-execution-and-sandbox.md)
- [11 — Design system §4 Approval, Approvals inbox, Notifications, Kill switch overlay, Settings; §5 autonomy levels; §9 process](../architecture/11-design-system-and-ux.md)
- [04 — Priya AI §router filters, budget guard](../architecture/04-priya-ai.md)
- [09 — Testing §2 security suite, §5 traceability](../architecture/09-testing.md)
- [decisions.md](../architecture/decisions.md): ADR-005, ADR-006, ADR-010, ADR-011, ADR-013

> **Safety (CLAUDE.md).** All enforcement tests run on the fake machine or in the VM. Kill-switch, merge and revert scenarios never target the host root during development.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 3.1 | Task and step state machines | Phase 2 | Every task/step transition is a tested pure function; state diagram generated from code |
| 3.2 | Task engine and API | 3.1 | `priya run --cmd`, `priya ps -w`, `priya logs -f`; tasks survive a priyad restart |
| 3.3 | Mode resolution | 3.2 | Tasks pick read-only / sandbox / direct from request, rules or AI risk; `priya mode explain` |
| 3.4 | Cedar policy engine | 3.1 | Policies load, validate and decide with plain-language reasons; `priya policy check` |
| 3.5 | Capability enforcement | 3.2, 3.4 | Each run gets a capability set; every enforcement point asks Cedar |
| 3.6 | Secrets masking, injection, redaction | 3.5 | Secrets hidden from runs, granted ones injected safely, values redacted everywhere |
| 3.7 | Approvals engine | 3.2, 3.4 | Risk tiers, `priya approvals`, remembered approvals as removable rules |
| 3.8 | Autonomy levels | 3.3, 3.7 | Settings → Autonomy & Safety: three levels globally and per feature |
| 3.9 | Approval card, inbox and minimal notifications | 3.7, 3.8 | Approvals in the bar, notifications and an inbox; bar status strip |
| 3.10 | Budgets and kill switch | 3.2, 3.7, 3.9 | Time/money/resource limits; Super+Shift+Escape stops all with an overlay |
| 3.11 | Untrusted labels and external actions | 3.5, 3.7 | Untrusted banner; push/send/deploy detected and gated |
| 3.12 | Audit report | 3.4, 3.7, 3.10, 3.11 | `priya audit report --since 7d` in plain language |
| 3.13 | Security test corpus (basic) | 3.6, 3.11 | Injection/leak/bypass corpus on PRs (fast) and nightly (full) |
| 3.14 | Phase gate | all | Owner demo of the full safety story |

---

### 3.1 — Task and step state machines
**Outcome:** task, step and run lifecycles from [02 §2](../architecture/02-core.md) exist as pure, table-driven state machines in `priya-core`, with a generated state diagram that keeps the docs true.
**Features:** CORE-07.
**Depends on:** Phase 2 gate.

**Tasks**
- [ ] `TaskState` enum: `created, planning, awaiting_plan_approval, running, verifying, awaiting_review, merging, done, done_pending_reboot, done_with_conflicts, discarded, failed, cancelled, paused{resume_to}`; answer-only shortcut `created → done`; direct-mode review means keep/revert.
- [ ] `StepState` (`pending, ready, running, verifying, completed, failed, skipped, cancelled, paused`) and `RunState` (`queued, starting, running, frozen, finished, killed, lost`).
- [ ] Pure transition functions `fn apply(state, &Event) -> Result<State, TransitionError>` driven by a transition table; every illegal transition is a typed error with `user_message`.
- [ ] Task aggregate: task ↔ plan (one step for now, full DAG data model per [02 §3](../architecture/02-core.md)) ↔ runs ↔ ChangeSets; `task.state_changed {from, to, reason}` events.
- [ ] `cargo xtask state-diagrams` renders Mermaid diagrams from the tables into `docs/claude/api/state-machines.md`; CI fails if the committed diagram differs.
- [ ] Docs: [02 §2](../architecture/02-core.md) links to the generated diagram; any added edge documented.

**Tests**
- Unit: table-driven test of every (state, event) pair — legal ones reach the expected state, illegal ones error; `proptest` random event sequences never produce an undefined state and `paused` always resumes to its saved state; projection rebuild from events equals incremental state.
- Regression: `task_transition_table_complete`, `task_random_sequences_stay_valid`, `paused_resumes_to_previous_state`.

**Done when**
- [ ] 100 % of transition-table rows covered by tests; mutation testing (`cargo-mutants` on the module) leaves no surviving mutants in transition logic
- [ ] Generated diagram committed and checked in CI

---

### 3.2 — Task engine and API
**Outcome:** the owner runs `priya run --cmd "sudo pacman -S --noconfirm jq" --mode sandbox`, watches it live with `priya ps -w` / `priya logs -f`, reviews it, and the task survives a priyad restart.
**Features:** CORE-07.
**Depends on:** 3.1.

**Tasks**
- [ ] Task engine in `priya-core` (logic) + `priyad` (wiring): create task → one-step plan → run in the resolved workspace (Phase 2 runner) → ChangeSet → `awaiting_review` → merge/discard/keep/revert → terminal state.
- [ ] Durable queue: tasks and runs are projections of the event log; global max concurrent runs (setting `tasks.max_concurrent_runs`, default 3) on top of the fork limit; FIFO with priority for user-initiated over background.
- [ ] Restart recovery: on priyad start, for each non-terminal run ask the helper (`helper.run.list`) and systemd whether its scope is alive → reattach output streaming; otherwise mark the run `lost` and the task `failed` with reason "Priya restarted while this was running"; the fork is kept for inspection until GC.
- [ ] Pause/resume via cgroup freezer (`helper.run.freeze/thaw` for sandbox runs, `systemctl --user freeze/thaw` for direct runs); cancel = stop the run, discard the fork unless `--keep-fork`.
- [ ] API (additive to [02 §4](../architecture/02-core.md)): `v1.task.create {command, cwd, mode?, budget?}`, `v1.task.list {filter}`, `get`, `cancel`, `pause`, `resume`; `v1.change.*` becomes task-aware (merge/discard/keep/revert move the task state); live updates through `v1.events.subscribe {stream: "task:<id>"}`.
- [ ] CLI replacing the Phase 2 temporary commands (kept as hidden aliases for one phase): `priya run --cmd`, `priya ps [-w]`, `priya logs [-f] <task>`, `priya task show <task>`, `priya review <task>`, `priya merge|discard|keep|revert <task>`, `priya pause|resume|cancel <task>`.
- [ ] Task titles: commands get a short plain title ("Install jq in the sandbox"); Priya AI (role `fast`) may suggest a better title, offline → deterministic fallback.
- [ ] Finished-task hooks for notifications (used in 3.9): `task.state_changed` to `done*`, `failed`, `awaiting_review`.

**Tests**
- Unit: engine orchestration with fake helper, fake runner and fake clock (create → review → merge, cancel at every state, pause/resume); queue fairness and limits.
- Contract: API schema snapshot for the new methods (additive check passes).
- Integration (fake machine): `task_sandbox_install_review_merge`; `task_direct_edit_keep_and_revert`; `task_survives_priyad_restart_reattaches` (restart priyad mid-run, run continues, output complete); `task_fails_cleanly_when_run_lost` (kill the scope while priyad is down); `task_pause_freezes_cgroup`; `task_cancel_discards_fork`; `queue_respects_max_concurrent_runs`.
- Chaos: kill priyad at each task-engine step → after restart every task is in a valid state and no fork is orphaned without a report.
- Regression: all integration and chaos scenarios above.

**Done when**
- [ ] `priya ps -w` reflects state changes within 200 ms
- [ ] Restart recovery scenarios pass on the fake machine and in the VM

---

### 3.3 — Mode resolution
**Outcome:** each task gets an execution mode by the rule order in [03 §1.1](../architecture/03-execution-and-sandbox.md), and `priya mode explain <task>` says why ("Sandbox: anything needing root always runs in the sandbox (locked rule)").
**Features:** SBX-07.
**Depends on:** 3.2.

**Tasks**
- [ ] `modes.toml` settings section with JSON Schema: `default`, `[[rule]] when {module, category, paths, privileged}`, `mode`, `lock`; hot reload, invalid file keeps last valid set.
- [ ] Pure resolver `priya-core::mode::resolve(explicit, plan_step, rules, module_default, ai_suggestion, global_default) -> ModeDecision {mode, source, rule_id, locked}` in the documented order; locked rules beat AI suggestions but not an explicit request.
- [ ] Per-feature defaults from module manifests (`[requires] default_mode`) inserted as rules after user rules.
- [ ] AI risk suggestion from the Phase 1 classifier (`risk: none|low|high` → `read_only|direct|sandbox`); offline or classifier error → skip this step.
- [ ] Privileged detection for commands: `sudo`, `pacman -S/-R/-U/-Syu`, `systemctl enable/start` (system scope), writes under `/etc` → `privileged = true`.
- [ ] `ModeDecision` stored in the `task.created` payload; `priya mode explain <task>`; `priya modes list` shows rules in plain language.
- [ ] Escalation rule: a run in `read_only` that needs to write fails with "This needs Sandbox or Direct — run again with `--mode sandbox`?" (never silently escalates).

**Tests**
- Unit: resolver decision table (every source wins/loses correctly; locked vs explicit vs AI); rule matching on paths (globs, `~` expansion), categories, modules; privileged detector corpus.
- Integration (fake machine): `mode_rule_sends_notes_edit_direct`; `mode_locked_privileged_rule_beats_ai`; `explicit_mode_beats_locked_rule`; `read_only_needing_write_fails_with_hint`.
- Regression: all above.

**Done when**
- [ ] Every task records its mode decision and source; `priya mode explain` output reviewed by the owner as understandable 👤

---

### 3.4 — Cedar policy engine
**Outcome:** `priya-policy` loads shipped and user Cedar policies, validates them against the schema, decides every request with a tier and a plain-language reason, and logs `policy.decision`.
**Features:** SEC-03, SEC-02, QA-20.
**Depends on:** 3.1.

**Tasks**
- [ ] Cedar schema `priya.cedarschema` in `priya-policy`: entity types `User`, `Agent` (attrs `family`, `version`), `Module`, `Run` (attrs `task`, `mode`, `tainted`, `agent`, `module`, `project`), `Path` (attrs `path`, `classes`, `area`), `Domain`, `Secret`, `ModelProvider` (attrs `local`, `privacy_class`), `SettingKey`, `ChangeSetPart` (attrs `area`, `file_count`); actions `ReadPath`, `WritePath`, `Exec`, `NetConnect`, `UseSecret`, `PrivilegedOp`, `ExternalAction`, `UseModel`, `Merge`, `ChangeSetting` ([05 §2](../architecture/05-security.md)).
- [ ] Agent identity (SEC-02): principal naming `Agent::"<id>"` ↔ event actor `agent:<id>`; `Run::"<run-id>"` carries its agent/module; run scopes named `priya-run-<run-id>.scope`; test-only identity `Agent::"test-agent"` (available in test builds) until Phase 4 adapters exist.
- [ ] `PathClassifier`: pre-expands globs into `Path.classes` (`ssh`, `gnupg`, `keyrings`, `documents`, `downloads`, `project:<name>`, `system`, `home`) from settings, because Cedar's `like` has no `**`.
- [ ] Tiers in Cedar: policies carry annotations `@id`, `@tier("auto"|"notify"|"ask"|"confirm")`, `@explain("…")`; the strictest tier among determining `permit` policies wins; any matching `forbid` → `deny` with its `@explain`.
- [ ] Policy sources: `/usr/share/priya/policy/*.cedar` (defaults: the three examples in [05 §2](../architecture/05-security.md) plus mask-list forbids and the tier table from [05 §4](../architecture/05-security.md)), `/etc/priya/policy/`, `~/.config/priya/policy/*.cedar`; strict validation on load; invalid file rejected with a plain-language error and the last valid set kept; hot reload.
- [ ] `PolicyEngine::authorize(Request) -> Decision {effect, tier, determining: Vec<PolicyId>, explanation}`; `policy.decision` event per decision (high-volume path checks are evaluated once per launch, not per file).
- [ ] CLI `priya policy list|show <id>|check <request.json>|explain <decision-id>|validate`.
- [ ] Security suite skeleton (QA-20): `tests/security/` with tag `security`, fast subset selected by `security-fast` on PRs, full set nightly; each later sub-phase adds cases.

**Tests**
- Unit: allow/deny/tier matrix fixtures (`tests/fixtures/policy/*.toml`: request → expected effect, tier, explanation) covering every action; annotation parsing; strictest-tier selection; path classifier globs; validation errors produce readable messages.
- Contract: policy fixture matrix runs identically on macOS and Linux.
- Perf: `authorize` p99 < 1 ms with 200 policies loaded.
- Security: policy files with syntax/schema errors never weaken the active set; a user `permit` cannot override a shipped `forbid`.
- Regression: the full fixture matrix, `user_permit_cannot_override_forbid`, `invalid_policy_keeps_last_valid`.

**Done when**
- [ ] Every action in the schema has at least one allow and one deny fixture
- [ ] Every deny in the fixture matrix has a non-empty plain-language explanation

---

### 3.5 — Capability enforcement
**Outcome:** every run gets an explicit capability set, and every enforcement point (launch mounts, Landlock, exec, helper operations, merge, secrets, model use, settings, module calls) asks Cedar before acting.
**Features:** SEC-03, SEC-02.
**Depends on:** 3.2, 3.4.

**Tasks**
- [ ] `CapabilitySet {read_paths, write_paths, exec, net_domains, secrets, privileged_ops, external_actions, models}` in `priya-core`; derived per run as the step's `CapabilityRequest` ∩ what policy allows; recorded on `run.started`.
- [ ] Enforcement-point registry (`priya-policy::points`), each point a typed call site:

  | Point | Where | Actions checked |
  |---|---|---|
  | Launch mounts (sandbox, read-only) | priyad builds `LaunchSpec` → helper | `ReadPath`, `WritePath` → masks, read-only binds |
  | Landlock (direct) | `priya-run-exec` ruleset from the capability set | `ReadPath`, `WritePath` |
  | Exec pre-launch | priyad before launching a command | `Exec`, `PrivilegedOp`, `ExternalAction` |
  | Helper operations | priyad before every `helper.*` call (helper still re-checks invariants) | `PrivilegedOp`, `Merge` |
  | Merge / keep / revert | `v1.change.*` | `Merge` (per part: home, system) |
  | Secrets | injection (3.6) | `UseSecret` |
  | Model calls | Priya AI router filter and local proxy | `UseModel` (Phase 1 privacy rules migrated to Cedar policies) |
  | Settings writes | `v1.settings.set` | `ChangeSetting` |
  | Module host calls | module host capability check | module principal × action |
  | Network | recorded only (`NetConnect` decisions logged as advisory) | enforced by the egress proxy in Phase 14 |

- [ ] Cedar-driven mask list replaces the static `sandbox.mask` setting (2.7): `forbid ReadPath` on a path class → tmpfs/`/dev/null` bind in forks and read-only runs, excluded from Landlock read hierarchies in direct mode; agent-auth allow-list kept.
- [ ] Migration of Phase 1 privacy routing rules to equivalent `UseModel` policies, with a one-time converter and a test that old and new decisions match on the router evaluation set.
- [ ] Every deny surfaces as a plain sentence in CLI, events and (3.9) approval/notice UI: "Blocked: agents never read your SSH keys."
- [ ] Meta-test: every registered enforcement point has at least one deny fixture exercised in integration tests (fails CI otherwise).

**Tests**
- Unit: capability derivation (request ∩ policy); Landlock ruleset from capabilities; mask rendering from path classes.
- Integration (fake machine): `ssh_masked_in_sandbox_by_policy`; `ssh_denied_in_direct_by_landlock`; `exec_denied_by_policy_never_launches`; `system_merge_denied_by_policy`; `settings_key_change_denied_by_policy`; `module_undeclared_call_denied`; `documents_only_to_local_models` (fake cloud + fake local providers).
- Security: symlink/hardlink/bind tricks to reach a masked path from inside a fork or a direct run all fail.
- Regression: all above plus the enforcement-point meta-test.

**Done when**
- [ ] No code path performs a registered action without a `policy.decision` event (checked by an integration test that replays a full task and compares actions to decisions)
- [ ] Old and new `UseModel` decisions agree on 100 % of the router evaluation set

---

### 3.6 — Secrets masking, injection, redaction
**Outcome:** runs never see secret files or values unless granted; a granted secret reaches the run without appearing in argv, files in the fork, events or logs; known secret formats and every granted value are redacted from output, events, logs, prompts and notifications.
**Features:** SEC-04, SEC-21, SEC-22.
**Depends on:** 3.5.

**Tasks**
- [ ] `SecretRef {name, kind: api_key|token|git_credential|env}` stored in the Secret Service collection `priya` (Phase 1 key storage reused); CLI `priya secrets set|list|remove|grant <name> --to <project|module|agent>` (grants become `permit UseSecret` policies).
- [ ] Injection without argv exposure: the helper writes granted values to a root-only tmpfs file, passes it with `systemd-nspawn --load-credential=`, and `priya-run-exec` inside the run exports them as environment variables, then the file is removed; direct runs receive them over a pipe from priyad. Never as files inside the fork.
- [ ] Git credential helper `priya-git-credential` configured for runs through `GIT_CONFIG_*` env: asks priyad over the run socket; priyad checks `UseSecret` and `ExternalAction` (push) before answering.
- [ ] Redaction filter `priya-policy::redact`: patterns for known formats (Anthropic, OpenAI, Google, xAI, GitHub, GitLab, AWS, Slack keys, JWTs, PEM private-key headers) plus exact-value matching of every secret granted to the run (also base64 and URL-encoded forms); applied to `run.output` before storage, `tracing` logs, event payloads (append is rejected and redacted if a known secret value appears), prompts built by Priya AI, notification text, audit reports and diff display (merged file content is unchanged; display masks the value).
- [ ] Secrets in ChangeSets: entries whose after-content contains a known secret value are flagged "contains a secret" in review.
- [ ] Mask list defaults and allow-list from [05 §3](../architecture/05-security.md) shipped as default `forbid ReadPath` policies; `priya secrets doctor` lists secret-looking files outside masked paths (e.g. `.env` in projects) as advice.
- [ ] `priya secrets doctor`: finds secret-looking files outside masked folders (keys, `.env`, tokens by pattern) and offers to add them to the mask list
- [ ] ChangeSets are scanned for secret patterns; a ChangeSet that adds or exposes a secret is flagged in review and needs `confirm` to merge

**Tests**
- Unit: redaction corpus (positive and negative samples for every pattern, encoded variants, split across output chunks); credential helper protocol.
- Integration (fake machine): `granted_secret_visible_as_env_only`; `secret_not_in_nspawn_argv` (`/proc/*/cmdline` scan during launch); `secret_never_in_fork_filesystem` (ChangeSet + full scan of fork); `run_output_secret_redacted`; `event_with_secret_value_rejected`; `git_push_credential_requires_approval`; `ungranted_secret_denied`.
- Security: a script that prints, base64-encodes, splits and writes the secret to a file → stored output and event log contain no secret value.
- Regression: all above.

**Done when**
- [ ] A grep for every test secret value across `priya.db`, journald output, blobs and fork snapshots after the full suite finds nothing
- [ ] Redaction adds < 5 % to output throughput (perf test)

---

### 3.7 — Approvals engine
**Outcome:** actions that need a human produce approval requests with risk tiers; the owner answers with `priya approvals` (CLI now, UI in 3.9); "always allow here" creates a visible, removable rule.
**Features:** SEC-05.
**Depends on:** 3.2, 3.4.

**Tasks**
- [ ] `Approval {id, task, kind, tier, title, why, what, reversibility, risk, scope_options, created_at, state: pending|approved|denied|expired}`; kinds: `merge_home`, `merge_system`, `privileged_op`, `external_action`, `secret_use`, `plan`, `budget_raise`, `force_drift`, `direct_raw`.
- [ ] Tier behaviour from [05 §4](../architecture/05-security.md): `auto` proceed; `notify` proceed + notification + easy revert; `ask` task waits (`approval.requested`); `confirm` task waits and requires typing (or later saying) the confirmation word shown; `deny` blocked with explanation.
- [ ] Waiting never auto-allows: unanswered approvals keep the task paused; reminder after 10 minutes; an approval becomes `expired` when its subject changes (ChangeSet recomputed, task cancelled or killed) and a new one is requested if still needed.
- [ ] Batching: related pending approvals in one task grouped into one request ("Allow these 3 package installs?") with per-item override.
- [ ] Remembered approvals: scopes `this task`, `this project`, `this feature/module`, `everywhere`, optional expiry; written as annotated `permit` policies in `~/.config/priya/policy/remembered.cedar` (`@id`, `@created`, `@scope`, `@explain`); can never lower `confirm` tier, external actions on tainted runs, or override a `forbid`; `approval.remembered` event.
- [ ] API `v1.approval.list`, `v1.approval.decide {id, allow, remember?, confirm_text?}`, `v1.approval.rules.list|remove`; CLI `priya approvals [list|allow <id> [--remember <scope>]|deny <id>|rules|forget <rule>]`.
- [ ] Docs: new events `approval.expired`, `approval.remembered` in [02 §1.1](../architecture/02-core.md); tier annotations documented in [05 §4](../architecture/05-security.md).

**Human in the loop**
- The owner sees a verb-first title, why, exactly what, risk and whether it can be undone, then decides Allow once / Always allow here / Deny. Remembered decisions are listed and removable at any time.

**Tests**
- Unit: tier mapping fixtures; batching rules; remember-scope → generated policy text (golden); remembered rule never lowers `confirm` or overrides `forbid`; expiry on subject change.
- Integration (fake machine): `system_merge_waits_for_approval`; `denied_merge_leaves_live_untouched`; `confirm_tier_rejects_wrong_text`; `remembered_project_rule_skips_next_prompt`; `forget_rule_restores_prompt`; `unanswered_approval_never_auto_allows` (fake clock + reminder event).
- Regression: all above.

**Done when**
- [ ] Every approval kind has an integration test from request to decision
- [ ] Remembered rules appear in `priya approvals rules` and in `priya policy list`

---

### 3.8 — Autonomy levels
**Outcome:** the owner sets how often Priya asks — **Ask me first**, **Ask when it matters** (default), **Just do it** — globally and per feature in Settings → Autonomy & Safety, and the approval behaviour changes accordingly without ever skipping external actions or `confirm`-tier steps.
**Features:** DS-09.
**Depends on:** 3.3, 3.7.

**Tasks**
- [ ] Settings schema `autonomy.default` and `autonomy.features.{updates, file_edits, installs, code, automations}` with values `ask_first | ask_when_it_matters | just_do_it`; module manifests may declare which feature they belong to.
- [ ] Pure mapping `priya-policy::autonomy::adjust(tier, level, context) -> tier` from [11 §5](../architecture/11-design-system-and-ux.md): **Ask me first** → plan approval for every task, every merge/keep at least `ask`; **Ask when it matters** → tiers as in [05 §4](../architecture/05-security.md), plan approval only for multi-step, direct, privileged, external or expensive work; **Just do it** → `ask` home merges of untainted tasks become `notify`; never changes `confirm`, `deny`, external actions, tainted system merges, or mode rules.
- [ ] Autonomy applied after Cedar; `policy.decision` records both the policy tier and the autonomy-adjusted tier.
- [ ] Settings → **Autonomy & Safety** page (Quickshell, extends the Phase 1 Settings window): three-step control (global), per-feature overrides, read-only list of mode rules with plain explanations, remembered approvals list with Remove, mask list ("Folders Priya never shows to agents"), link to `priya audit report`.
- [ ] CLI `priya autonomy get|set [--feature <f>] <level>`.

**UI**
- Screens/components: `AutonomyControl` (three-step segmented control with one-line explanation of each level), `FeatureAutonomyRow`, `RuleList` (mode rules and remembered approvals), Autonomy & Safety page.
- States: default, edited (Undo toast), invalid (settings file rejected — shows the plain error), locked by admin (`/etc/priya` sets it; control disabled with explanation), empty (no remembered approvals), long content (50 rules), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: Tab between controls, arrow keys move between levels, Enter/Space selects, Delete removes a focused rule (with confirm).
- Screen-reader names: "Autonomy level for installs: Ask when it matters, 2 of 3"; "Remove rule: always allow pacman installs in project api".
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop**
- The owner decides per feature how often to be asked; the page always states what can never be skipped ("Sending, publishing and deploying always ask").

**Tests**
- Unit: mapping table for every (tier, level, tainted, external) combination; invariants: `confirm`/`deny`/external never lowered.
- UI: component tests (keyboard, states), screenshots per state × theme × scale, accessibility lint; journey "set installs to Just do it → run sandbox install → home merge notifies instead of asking".
- Integration (fake machine): `just_do_it_never_skips_external_action`; `ask_first_requires_approval_for_home_merge`; `admin_locked_autonomy_cannot_be_changed_by_user`.
- Regression: all above.

**Done when**
- [ ] Mapping invariants hold for 100 % of combinations (exhaustive test)
- [ ] Owner sets autonomy per feature and sees the behaviour change in a demo task 👤
- [ ] Gallery sign-off 👤

---

### 3.9 — Approval card, inbox and minimal notifications
**Outcome:** approvals reach the owner where they are — an approval card in the Priya bar, a desktop notification with Allow/Deny, and an approvals inbox; finished tasks notify; the bar shows a live status strip ("1 running · 1 needs you").
**Features:** DS-10, UX-18, SEC-05.
**Depends on:** 3.7, 3.8.

**Tasks**
- [ ] `Notifier` port in `priya-core` + desktop adapter (freedesktop Notifications over D-Bus, works with mako or any notification server) with actions; fake notifier in testkit; contract suite.
- [ ] Notification types: approval needed (`ask` → Allow / Deny buttons; `confirm` → "Review…" only, because typing is required), task ready for review, task done / done pending restart / failed, budget warning (3.10), kill switch engaged (3.10). Grouped by task; never steal focus; respect the server's Do Not Disturb (approvals still go to the inbox and the bar badge).
- [ ] `ApprovalCard` (PriyaUI): verb-first title ("Restart nginx?"), why, exactly what (command or change summary, expandable), risk badge (icon + word), reversibility line ("Undo with Rewind" / "Can't be undone: leaves your computer"), mode badge, untrusted badge (3.11), buttons **Allow once · Always allow here · Deny** (keys Y / A / N), scope picker for "Always allow here", confirm-tier text field.
- [ ] Approvals inbox: Quickshell floating window `ui/apps/approvals/` opened from the bar, a notification or `priya approvals --gui`; list of pending approvals across tasks, grouped by task, batch Allow/Deny for groups (never for `confirm` tier).
- [ ] Priya bar integration: "Needs you" section with pending approval cards at the top of results; status strip with running / needs-you / ready-for-review counts, live from `v1.events.subscribe`; clicking opens `priya review` (TUI in a kitty window) until the Task center lands in Phase 4.
- [ ] Reminder after 10 minutes re-notifies once; answered elsewhere → notification withdrawn within 1 s.
- [ ] QML types regenerated from the API schemas for `Approval`.

**UI**
- Screens/components: `ApprovalCard`, `ApprovalGroup`, approvals inbox window, bar "Needs you" section, bar status strip, notification templates.
- States: approval card pending / approved / denied / expired / confirm-tier (with text field, wrong text error) / batched group / long command (scroll, copy) / untrusted; inbox empty ("Nothing needs you") / list / many (100 approvals, virtualised); status strip idle / running / needs you / review / offline (priyad unreachable: "Priya isn't running"); Hindi text in titles and reasons.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: Y / A / N on a focused card, Tab order title → details → buttons, Esc closes the inbox, Super+Space then Down reaches "Needs you".
- Screen-reader names: card announced as "Approval needed: Restart nginx? Risk medium. Can be undone."; buttons "Allow once", "Always allow in project api", "Deny".
- 👤 Mockup approval (`ui/mockups/3.9/`, light + dark) before building; gallery sign-off and a recorded journey after.

**Human in the loop**
- Approvals pause the task until answered; the owner can answer from any surface, and the same decision is reflected everywhere. Nothing is allowed by timeout.

**Tests**
- Unit: notification content builder (redacted, plain language); grouping; withdraw-on-answer logic.
- Contract: `Notifier` suite on the desktop adapter (against a test D-Bus notification server) and the fake.
- UI: Qt Quick tests for card and inbox (keys, signals, confirm text), screenshots for every state × theme × scale, accessibility lint; journey against mock priyad: "sandbox install → approval notification → Allow → merge → done notification".
- Integration (fake machine + headless compositor): `approval_answered_in_notification_unblocks_task`; `approval_answered_in_cli_withdraws_notification`; `confirm_tier_not_answerable_from_notification`.
- Regression: all journeys and screenshot baselines.

**Done when**
- [ ] Approval appears in bar, notification and inbox within 500 ms of `approval.requested`
- [ ] Owner completes the journey on the Zenbook 👤; gallery sign-off 👤

---

### 3.10 — Budgets and kill switch
**Outcome:** every run and task has explicit time, money and resource limits with warnings and hard stops; Super+Shift+Escape (or `priya stop-all`) stops every run within seconds and shows the kill-switch overlay with Resume buttons.
**Features:** SEC-07, SEC-08, DS-11.
**Depends on:** 3.2, 3.7, 3.9.

**Tasks**
- [ ] `Budget {wall_secs, usd, tokens, cpu_quota, memory_max, tasks_max}` on steps and tasks; defaults in settings `budgets.*`; daily and monthly money budgets come from Phase 1 (AI-16) and are enforced at task start.
- [ ] Enforcement: time by the engine's timer; CPU/RAM/process count by cgroup properties on the run scope; money and tokens by summing `model.called` events correlated to the task (via the local proxy's per-run tokens); soft limit 80 % → `budget.warning` + notification; hard limit → run frozen, task `paused`, `task.budget_exceeded`, approval of kind `budget_raise` ("Allow $0.50 more?").
- [ ] Partial results on a budget stop: the ChangeSet is still computed and shown as "partial (stopped by budget)" in review.
- [ ] Disk guard: new forks refused below 5 % free (2.6); running forks paused with a notice below 3 %.
- [ ] Kill switch: `priya stop-all` → priyad cancels every run (SIGTERM, 5 s, SIGKILL on all `priya-run-*.scope`), tasks → `paused`, no merge starts, an in-flight journaled helper step finishes its current step and stops; `killswitch.engaged {stopped_runs}` event.
- [ ] Works when priyad is frozen or dead: `priya stop-all` falls back to `helper.run.kill_all {uid}` and `systemctl --user kill 'priya-run-*.scope'` directly; the Hyprland binding `Super+Shift+Escape` in `/usr/share/priya/desktop/hyprland.conf` calls it; cannot be disabled in settings.
- [ ] Kill-switch overlay (Quickshell layer-shell, top layer): dimmed screen, "Stopped everything", list of stopped tasks (title, elapsed, cost), **Resume** per task, **Resume all**, **Close** (Esc); shown on `killswitch.engaged`; if priyad is dead the overlay shows the fallback result from the CLI.
- [ ] `priya budgets show|set` CLI; budget shown in `priya task show`.

**UI**
- Screens/components: `KillSwitchOverlay`, `StoppedTaskRow`, budget warning notification and card.
- States: overlay with 0 runs ("Nothing was running"), 1 run, many runs (20, scrolling), stopping in progress (SIGTERM grace), stop failed for a run (plain error + "Force stop"), priyad unreachable (fallback list); budget card warning / exceeded / raised; Hindi task titles.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced-motion variant (no fade).
- Keyboard: overlay grabs focus; Tab through Resume buttons; Esc closes; Enter on a row resumes it.
- Screen-reader names: overlay announces "Stopped everything. 3 tasks stopped."; buttons "Resume Install jq", "Resume all".
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop**
- One shortcut stops everything; the owner decides what to resume. Budget stops ask before spending more; nothing continues on its own.

**Tests**
- Unit: budget accounting from events (fake clock, fake model calls); soft/hard thresholds; stop-all sequencing.
- UI: overlay and budget card screenshots per state × theme × scale; accessibility lint; journey "two runs → Super+Shift+Escape → overlay → Resume one".
- Integration (fake machine): `time_budget_pauses_task`; `memory_budget_kills_runaway`; `money_budget_pauses_on_fake_model_spend`; `budget_raise_approval_resumes`; `stop_all_kills_every_run_scope`; `stop_all_works_with_priyad_frozen` (SIGSTOP priyad); `stop_all_during_merge_finishes_step_then_stops`.
- Perf: all runs dead ≤ 6 s after the shortcut (SIGKILL after 5 s); overlay visible < 300 ms after `killswitch.engaged`.
- Chaos: stop-all at every task-engine and helper journal step → consistent states, no partial merge.
- Regression: all above.

**Done when**
- [ ] Kill switch stops 10 concurrent sandbox runs within 6 s on the Zenbook, including with priyad frozen
- [ ] Budget stops leave a reviewable partial ChangeSet
- [ ] Gallery sign-off 👤

---

### 3.11 — Untrusted labels and external actions
**Outcome:** a task that reads untrusted input (downloads, cloned repos, URLs, `~/Downloads`) is labelled **Untrusted** and needs typed confirmation to merge; actions that leave the machine (push, send, deploy, publish) are detected before they run where possible and gated by approval in every mode.
**Features:** SEC-09, SBX-10, SEC-23.
**Depends on:** 3.5, 3.7.

**Tasks**
- [ ] `TaintReason {source: UrlInPrompt|Download{url}|ClonedRepo{url}|DownloadsFile{path}|WebTool|Message, evidence, at_event}`; `taint.added` events; task-level propagation (every run and ChangeSet of a tainted task carries the label; `Run.tainted` in Cedar).
- [ ] Taint rule engine over events and commands (`priya-policy::taint`): URLs in the prompt or command; argv patterns (`curl`, `wget`, `aria2c`, `git clone|fetch|pull` from non-allowlisted remotes, `pip install <url>`, AUR builds); cwd or path arguments under `~/Downloads`; agent tool events (`run.tool_called` with web/fetch/search tools) — the hook is ready for Phase 4 agents. Rules shipped as data (`/usr/share/priya/taint-rules.toml`), user-extendable.
- [ ] Effects ([05 §5](../architecture/05-security.md)): untrusted banner in review (TUI 2.14 placeholder filled, `UntrustedBanner` QML component in the gallery), `confirm` tier for merges of tainted tasks, tainted system merges follow policy (default policy requires confirmation). No automatic clearing; the owner can see why a task is tainted.
- [ ] `ExternalAction {kind: GitPush|HttpWrite|Mail|Message|Deploy|Publish|Payment|Unknown, target, evidence, detected_at: PreLaunch|Credential|ToolLog}` on the ChangeSet ([03 §5](../architecture/03-execution-and-sandbox.md)).
- [ ] Detection and gating in every mode: (1) pre-launch argv classifier (`git push`, `curl -X POST|PUT|DELETE`/`-d`/`-F`/`-T`, `wget --post-*`, `scp`/`rsync` to remote, `ssh` remote commands, `npm|cargo|twine publish`, `gh pr create|release`, `docker push`, `kubectl apply`, `terraform apply`, `sendmail`/`msmtp`/`mail`) → `ExternalAction` approval before launch; (2) `priya-git-credential` (3.6) gates HTTPS pushes at credential time; (3) post-run tool-log scan (agents, Phase 4) records and highlights detected actions in review.
- [ ] Honest limitation shown in review until Phase 14: "Programs inside the sandbox can still reach the internet; Priya checks known commands but can't block every connection yet."
- [ ] `priya task why-untrusted <task>` and an "External actions" section in `priya review` / `priya change show`.
- [ ] "Mark as trusted" on an untrusted task: the user reviews the taint reasons and confirms; recorded as an approval event; never available for automations

**UI**
- Screens/components: `UntrustedBanner`, `UntrustedBadge` (danger outline + icon + word "Untrusted"), `ExternalActionRow` in review and approval card.
- States: banner with one reason / many reasons (expandable) / long URL; external-action row pending approval / approved / denied / detected after the fact ("may have contacted github.com"); Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; never colour alone.
- Keyboard: banner "Why?" expander focusable; Enter toggles.
- Screen-reader names: "Untrusted: this task read a file downloaded from example.com"; "External action: push to github.com/owner/repo, waiting for approval".
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop**
- Untrusted work still runs in its mode, but merging it needs typed confirmation; external actions always wait for the owner, whatever the autonomy level.

**Tests**
- Unit: taint rule corpus (positives and negatives for every rule); argv external-action classifier corpus (quoting, `sh -c`, aliases via `env`, `git -C dir push`, `--data-binary @file`).
- Integration (fake machine): `curl_download_taints_task`; `downloads_file_taints_task`; `tainted_home_merge_requires_confirm`; `git_push_blocked_until_approved` (local bare repo over HTTP on the fake machine); `curl_post_requires_approval_in_direct_mode`; `just_do_it_still_asks_for_external_action`.
- UI: screenshots and accessibility for banner, badge and rows.
- Security: obfuscated external actions (`$(echo Z2l0IHB1c2g= | base64 -d)`) are at least recorded as `Unknown` when detectable and listed in the corpus' known-gap file when not.
- Regression: all above.

**Done when**
- [ ] Every rule in `taint-rules.toml` and every external-action pattern has a positive and a negative test
- [ ] The known-gap file is reviewed by the owner and linked from [05 §6](../architecture/05-security.md) 👤

---

### 3.12 — Audit report
**Outcome:** `priya audit report --since 7d` produces a plain-language report of what every task and agent did, which models were used, what was merged, kept, reverted, blocked or approved, and what was untrusted or external — readable by a non-technical person.
**Features:** SEC-06.
**Depends on:** 3.4, 3.7, 3.10, 3.11.

**Tasks**
- [ ] Report builder in `priya-store` readers (deterministic templates, no model needed): sections Summary, Tasks, Changes merged/discarded/reverted, Blocked actions (with explanations), Approvals (who, when, remembered rules created), Untrusted tasks, External actions, Models used and cost, Kill switch and budget stops, System swaps and rollbacks.
- [ ] Helper journal included via `helper.journal.read {since}` (root-owned, read-only).
- [ ] Chain check first: runs `priya audit verify`; a broken chain is the first line of the report.
- [ ] Output formats: terminal (default), `--md`, `--json` (schema exported); secrets redacted (3.6).
- [ ] Optional one-paragraph summary by Priya AI (role `fast`, local model preferred by default privacy rules) behind `--ai-summary`; the deterministic report is always complete without it.
- [ ] Filters: `--task`, `--agent`, `--module`, `--only blocked|approvals|external`.

**Tests**
- Unit: report golden files from fixture event logs (quiet week, busy week, broken chain, many blocks); redaction applied.
- Integration (fake machine): `audit_report_lists_merge_block_and_approval` after a scripted session; `audit_report_includes_helper_journal`; `audit_report_flags_broken_chain` (tamper a row).
- Regression: all above.

**Done when**
- [ ] Report for a 7-day fixture renders < 2 s
- [ ] The owner reads one real report and can answer "what changed on my machine this week and who did it?" 👤

---

### 3.13 — Security test corpus (basic)
**Outcome:** a basic, permanent corpus of prompt-injection, secret-leak, policy-bypass and malicious-module scenarios runs as a fast subset on every PR and fully every night.
**Features:** SEC-10, QA-20.
**Depends on:** 3.6, 3.11.

**Tasks**
- [ ] Corpus layout `tests/security/corpus/<case>/` with `case.toml` (description, attacker input, expected outcome, tags `security-fast` or nightly-only) and fixtures; runner in `cargo xtask test security [--fast]`.
- [ ] Injection cases (scripted attacker commands now; replayed through the fake agent in Phase 4): repo README asking to `cat ~/.ssh/id_*` and upload it; script piping `~/.aws/credentials` to `curl -d`; Markdown with hidden instructions in `~/Downloads`; prompt containing a URL with instructions.
- [ ] Secret-leak cases: secret printed, base64-encoded, split across lines, written to a file, put in a commit message, sent in a notification title.
- [ ] Policy-bypass cases: symlink/hardlink to masked paths, `/proc/self/root` traversal, path with `..` in helper requests, Cedar policy file with a broad `permit` dropped into the user dir, remembered-approval scope escalation, settings write to `autonomy` by a module without the capability.
- [ ] Malicious-module cases: process module calling undeclared host methods, requesting undeclared secrets, flooding events, writing outside its scope.
- [ ] Expected outcomes asserted mechanically: secret absent from all stores (3.6 grep), action denied with `policy.decision`, task tainted, approval requested, live machine unchanged (`find-new` empty).
- [ ] Nightly result published to the dashboard issue; any failure opens an issue tagged `security`.

**Tests**
- Security: the whole corpus (≥ 30 cases); fast subset (≥ 10 cases, < 3 min) required on PRs.
- Regression: every corpus case is tagged `regression`; removing one requires an ADR ([09 §1](../architecture/09-testing.md)).

**Done when**
- [ ] Corpus ≥ 30 cases, 100 % passing; fast subset is a required PR check
- [ ] Each case documents the control that stops it (mask, policy, approval, taint, redaction)

---

### 3.14 — Phase gate
**Outcome:** Phase 3 is complete; the owner has seen every safety control work on the Zenbook.
**Features:** all Phase 3 features.
**Depends on:** 3.1–3.13.

**Tasks**
- [ ] Run the checklist in "Phase gate" below; record results in this file.

**Done when**
- [ ] Every item in "Phase gate" ticked

---

## Phase gate

- [ ] Full regression suite green on `main` (including all Phase 0–2 tests) and nightly (e2e VM, perf, chaos, security full) green three nights in a row
- [ ] `cargo xtask trace`: every Phase 3 feature has passing tests tagged with its ID; all acceptance tests tagged `regression`
- [ ] Owner demo on the Zenbook 👤: a command in the sandbox tries to read `~/.ssh` (masked), `curl`s a URL (task labelled Untrusted) and `git push`es (held for approval) → approval card in the bar and a notification → deny the push → review shows the Untrusted banner → merge needs typed confirmation → `priya audit report --since 1d` tells the story; autonomy for installs switched to "Just do it" and a sandbox install merges with a notification only; a budget stop with "Allow more?"; Super+Shift+Escape with two running tasks → overlay → Resume one
- [ ] L4 checklist on the Zenbook 👤: Super+Shift+Escape works from any workspace and with priyad frozen; notifications appear with working Allow/Deny buttons under the installed notification server; overlay and approval card readable in light, dark and OLED-black on the OLED screen; Orca reads the approval card
- [ ] Docs updated: [02](../architecture/02-core.md) (new API methods and events, generated state diagrams), [05](../architecture/05-security.md) (tier annotations, enforcement-point table, injection mechanism, taint rules, known gaps), [03 §1](../architecture/03-execution-and-sandbox.md) (mode resolution details), [11 §5](../architecture/11-design-system-and-ux.md) (exact autonomy mapping); ADRs for any design change
- [ ] Feature statuses for all 17 Phase 3 features updated in [features.md](../features.md)
- [ ] `docs/human/05-the-phases.md` status updated

## Out of scope

- Agents and their adapters, plan preview card, Task center and review GUI (Phase 4); commands and the test-only agent identity stand in until then.
- Multi-step plans, sub-agents and permission inheritance at runtime (Phase 12); the data model exists.
- Full notification centre, DND/quiet-hours UI, top-bar status (Phase 5).
- Natural-language settings and policy changes (Phase 5 / Phase 14 policy builder).
- Per-run network namespaces, egress proxy and blocking external actions at the network level (Phase 14).
- Value-level taint / information-flow control (Phase 14), skill and script scanning, signing (Phase 14).
- Voice approvals and the "Priya, stop everything" phrase (Phase 10); phone approvals (Phase 15).
