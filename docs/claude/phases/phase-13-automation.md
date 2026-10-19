# Phase 13 — Automation

**Goal.** PriyaOS works without being asked each time: schedules, triggers, service monitors, safe maintenance, a morning brief, overnight memory consolidation on batch APIs, suggestions to automate repeated work, shareable recipes and teaching by showing — always inside explicit limits, always visible, never doing `confirm`-tier things while the user is away.

**Usable at the end:** "Every weekday at 9, check my projects' CI and tell me what's broken" → preview card → confirm → next morning the brief is waiting when the owner unlocks the laptop; a recorded five-step routine becomes a recipe the owner runs with one sentence.

**Depends on:** Phase 5 (bar, notifications, Settings), Phase 6 (health checks and maintenance playbooks, update check), Phase 7 (Rewind retention), Phase 8 (memory, file index), Phase 11 (AT-SPI reader and UI actions for teach by showing), Phase 12 (plan DAGs, scheduler priorities, CI watcher). Phase 10 is optional (spoken brief).

**Features:** AUTO-01, AUTO-02, AUTO-03, AUTO-04, AUTO-05, AUTO-06, AUTO-07, AUTO-08, AUTO-09, AUTO-10, MEM-12, AI-20.

**Read first:** [02 §1.3 subscriptions, §3 plans, §5 modules](../architecture/02-core.md), [04 §8 token optimizer (batch)](../architecture/04-priya-ai.md), [05 §4 approvals, §5 taint](../architecture/05-security.md), [06 §5 scheduling priorities](../architecture/06-orchestration.md), [08 §2 memory and dreaming](../architecture/08-rewind-memory-tools.md), [11 §4 top bar (morning brief), Settings → Automations, §5 autonomy](../architecture/11-design-system-and-ux.md).

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 13.1 | Automation engine and limits | Phase 12 | `priya automation add` creates a limited automation that runs as a normal, labelled task |
| 13.2 | Triggers | 13.1 | Time, file-change, event and condition triggers fire automations reliably, without loops |
| 13.3 | Creating automations by plain language and the Automations page | 13.2 | "Every Friday at 5 clean my Downloads" → preview → confirm; manage all automations in Settings |
| 13.4 | Monitors | 13.2 | "Watch nginx and my API at localhost:8080" → plain-language failure reports with diagnosis |
| 13.5 | Maintenance automations | 13.2 | Opt-in safe maintenance (package cache, journal, scrub, orphans) suggested from the Health page |
| 13.6 | Morning brief | 13.3 | Click the clock (or unlock in the morning) → yesterday, today's blockers and suggested next steps |
| 13.7 | Overnight "dreaming" and batch jobs | 13.6 | Overnight, on AC: memory consolidated, indexes refreshed, brief prepared — at batch prices |
| 13.8 | Spotting repetitive work | 13.3 | "You've done this 4 times this week — automate it?" card, at most once a day |
| 13.9 | Recipes | 13.1 | `priya recipe run deploy-notes --param version=1.2`; recipes shared and imported safely |
| 13.10 | Teach by showing | 13.9 | Record a routine once → edit the drafted recipe → run it by name |
| 13.11 | Phase gate | all | A week of real use with automations and the brief |

---

### 13.1 — Automation engine and limits
**Outcome:** an `automation` module stores automations (trigger, action, mode, autonomy, limits) and turns each firing into a normal task labelled with its origin, with hard limits on time, money, resources, frequency and external actions.
**Features:** AUTO-10, AUTO-01.
**Depends on:** Phase 12.

**Tasks**
- [ ] `modules/automation` process module: manifest (methods `automation.create|update|list|pause|resume|delete|fire|history`; subscribes to event filters; settings `[automation]`).
- [ ] `Automation` type in `priya-api`: `{id, name, trigger, action: Intent(text) | Recipe(ref, params) | Method(module.method, args), mode, autonomy, limits, notify, enabled, created_by}`; events `automation.created|updated|fired|skipped|limited|paused`.
- [ ] Firing → `v1.intent.submit` with `origin = automation:<id>` → task at automation priority (lowest, 06 §5) with the automation's mode as a `lock`ed rule unless the action is answer-only.
- [ ] Limits (AUTO-10): per-run budget (default $0.20), daily spend cap per automation and for all automations (default $1.00), max runs per day (default 24), max duration per run (default 15 min), cgroup limits, concurrency 1 per automation (overlapping firings skipped with `automation.skipped`).
- [ ] External and `confirm`-tier actions are never auto-allowed: the task pauses on approval and notifies; approvals older than the automation's `approval_timeout` (default 12 h) expire and the run is cancelled.
- [ ] Runaway protection: > N firings per hour (default 6) or 3 consecutive failures → automation auto-paused with a plain notification; global "Pause all automations" (CLI, Settings, kill-switch overlay).
- [ ] CLI `priya automation add|list|show|pause|resume|rm|run-now|history`.

**Tests**
- Unit: limit enforcement table (budget, daily cap, runs/day, duration, overlap), runaway detector, approval-expiry logic, fake clock.
- Contract: `Automation` schema snapshot; module manifest validation.
- Integration (fake machine): automation firing every minute with a fake agent → task labelled `automation:<id>`, lowest priority; 7th firing in an hour → auto-paused; external action in an automation → paused awaiting approval, never executed without it; daily cap reached → further firings `limited`.
- Chaos: priyad restart during an automation run → resumes or fails cleanly per 12.9, no double firing.
- Regression: `automation_task_labelled`, `automation_limits_enforced`, `automation_runaway_paused`, `automation_external_action_waits`.

**Done when**
- [ ] Every limit has a test; an automation can never spend more than its cap in any test scenario.

---

### 13.2 — Triggers
**Outcome:** automations fire on time schedules, file changes, PriyaOS events and conditions (on AC, idle, network up, time window), survive reboots and suspends, and never trigger themselves in a loop.
**Features:** AUTO-01, AUTO-02.
**Depends on:** 13.1.

**Tasks**
- [ ] Time triggers as systemd **user** timers generated per automation (`priya-auto-<id>.timer` + `.service` running `priya automation fire <id>`), `OnCalendar=` from the automation spec, `Persistent=true` for catch-up after suspend/shutdown (catch-up policy: run once, not N times).
- [ ] Calendar spec parser/validator (subset of systemd calendar syntax) and human rendering ("every weekday at 09:00") in `priya-core`.
- [ ] File triggers: inotify (`notify` crate) on chosen paths with globs, debounce (default 5 s), size/rate caps; paths must be within the automation's read capabilities.
- [ ] Loop prevention: changes made by a task with `origin = automation:<id>` (from its ChangeSet or direct-mode writes) are ignored for that automation's file triggers; chains between automations limited to depth 2.
- [ ] Event triggers: subscriptions (02 §1.3) with filters, e.g. `step.failed` with check failure, `health.unit_failed`, `updates.available`, `system.low_disk`, `dev.ci_failed`; new event types documented.
- [ ] Conditions evaluated at fire time: on AC power, idle ≥ N min (ext-idle-notify / hypridle), network online, within a time window, not in DND; unmet → `automation.skipped {reason}` or deferred until met (per spec).
- [ ] Webhook triggers are not provided (see Out of scope).

**Tests**
- Unit: calendar parser and renderer (round-trip on 50 specs), debounce, loop-prevention rule, condition evaluation.
- Integration (fake machine): timer fires within 1 s of schedule (fake systemd clock via `systemd-run --on-calendar` in a user manager); suspend simulated → one catch-up run; file trigger on `~/inbox/*.pdf` fires once per burst of 10 files; automation that writes into its own watched folder does not re-fire.
- E2E (VM): reboot with an automation due during downtime → runs once after login.
- Regression: `time_trigger_fires`, `persistent_catchup_once`, `file_trigger_debounced`, `automation_no_self_loop`, `condition_on_ac_defers`.

**Done when**
- [ ] One week of a 5-minute test timer on the VM shows zero missed and zero duplicate firings (outside documented catch-up).

---

### 13.3 — Creating automations by plain language and the Automations page
**Outcome:** the user says what they want ("every Friday at 5 clean my Downloads of installers older than a month"), sees a preview card with trigger, action, mode, autonomy and limits, confirms, and manages everything on Settings → Automations.
**Features:** AUTO-01, AUTO-02, AUTO-10.
**Depends on:** 13.2.

**Tasks**
- [ ] Intent category `automation` in the classifier; NL → `Automation` spec via Priya AI (role `smart`) with schema-constrained output; validated (capabilities, limits within global caps).
- [ ] Preview card: plain sentence ("Every Friday at 17:00 I'll move installers older than 30 days from Downloads to Trash"), chips for trigger, mode, autonomy, budget per run, and "What it can touch"; first run option "Show me the first run before doing it" (default on for actions that change files).
- [ ] Edit by language ("make it Saturdays", "cap it at 10 cents") and by chips; `automation.updated` with diff.
- [ ] Settings → Automations page: list (name, trigger in words, next run, last result, spend this month), toggle, Run now, History (runs as tasks), Edit, Delete (with undo toast); "Pause all".
- [ ] Natural-language management in the bar: "pause my CI check", "what automations do I have?", "delete the Downloads cleanup".
- [ ] Automation tasks labelled in the Task center and notifications with an automation glyph and name.

**UI**
- Automation preview card, Automations page, history view; states: empty (with 3 example suggestions), list, paused, running, failed last run, limited (cap reached), auto-paused (runaway), editing, invalid spec, locked by admin, long names, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: full list navigation, Space toggles, Enter opens, R runs now, Delete deletes; screen-reader names ("CI check, every weekday at 9, next run tomorrow 09:00, enabled").
- 👤 Mockup approval (`ui/mockups/13.3/`) before building; journey sign-off (create by language → first-run preview → enable).

**Human in the loop:** nothing is scheduled without the preview and confirmation; the first run can be previewed; every automation is visible, pausable and deletable in one place; deleting is undoable.

**Tests**
- Unit: spec validation (capabilities, caps), NL edit patches.
- Eval: 50 labelled automation requests → correct trigger ≥ 95 %, correct action/mode ≥ 90 %, never a broader capability than the request implies (100 %).
- Integration: create by language → timer exists → fires → task; delete → timer removed; undo → restored.
- UI: card and page screenshots for every state × theme × scale; a11y lint; journey.
- Regression: `nl_automation_create_preview`, `automation_page_states`, `automation_delete_undo`.

**Done when**
- [ ] Owner creates three useful automations by plain language without editing any file.

---

### 13.4 — Monitors
**Outcome:** the user picks services to watch (systemd units, local URLs, ports, containers, disk space); failures produce one plain-language report with a read-only diagnosis and a suggested fix, not a flood of alerts.
**Features:** AUTO-03.
**Depends on:** 13.2.

**Tasks**
- [ ] Monitor kinds in the automation module: `systemd_unit` (system and user, via D-Bus `PropertiesChanged` instead of polling), `http` (status, latency, keyword), `tcp`, `container` (podman/docker status), `disk` (free % per mount).
- [ ] Monitor = trigger + check + on-failure action: default "notify + diagnose" — a read-only run using the Phase 6 diagnosis flow (logs, recent ChangeSets, Rewind attribution).
- [ ] Flap suppression: failure must persist for N checks (default 2); recovery notification; one open incident per monitor.
- [ ] Incident card: what failed, since when, likely cause ("started failing after yesterday's update of openssl"), actions Fix it (sandbox task) / Restart service (approval per tier) / Mute for 1 h / Details.
- [ ] Plain-language creation: "watch nginx and tell me if my API at localhost:8080 goes down".
- [ ] Monitors section on the Automations page and status in the Health page.

**UI**
- Incident card and notification, monitors list; states: healthy, failing, diagnosing, diagnosed, recovered, muted, unknown (check error), Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names (status always with word + icon).
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: flap suppression, incident lifecycle, check parsers.
- Integration (fake machine): stop a test unit → one incident after 2 checks → diagnosis cites the unit's journal → restart approved → recovered notification; flapping HTTP endpoint → no alert storm (≤ 1 incident per 30 min).
- Perf: 20 monitors cost < 1 % CPU average; D-Bus subscriptions instead of polling for units.
- Regression: `monitor_unit_incident`, `monitor_flap_suppressed`, `monitor_diagnosis_attached`.

**Done when**
- [ ] Owner monitors at least two real services; a real failure is explained correctly once.

---

### 13.5 — Maintenance automations
**Outcome:** a set of predefined, safe maintenance automations — each off by default, explained in one sentence, suggested from the Health page and onboarding — keeps the machine tidy.
**Features:** AUTO-04.
**Depends on:** 13.2.

**Tasks**
- [ ] Shipped automation templates in `/usr/share/priya/automations/maintenance/*.toml`: package cache trim (`paccache -rk2`, helper privileged op), journal vacuum (size cap), monthly btrfs scrub of `/` (helper op, on AC, idle), orphan package report (read-only; removal is a separate approved task), Flatpak unused runtimes report, weekly update check summary (Phase 6.7), stale fork/worktree GC report.
- [ ] Each template declares mode, capabilities, expected effect and reversibility; privileged ones go through the helper and produce ChangeSets where applicable.
- [ ] Health page "Keep things tidy" section and onboarding hint offering them; enabling shows exactly what will run and when.
- [ ] Results summarised in the morning brief, not as notifications (unless something fails).

**UI**
- Health page section and enable sheet; states: none enabled, some enabled, ran ok, failed, needs approval, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: template validation (no template requests more than its declared capabilities).
- Integration (fake machine): each template runs on the fake machine with expected effects (cache trimmed to 2 versions, journal under cap, scrub started via helper); orphan report makes no changes.
- Security: templates cannot be edited to escalate without the change being shown as a new capability prompt.
- Regression: `maint_paccache_trims`, `maint_scrub_via_helper`, `maint_orphans_read_only`.

**Done when**
- [ ] Owner enables at least two maintenance automations; they run for a week without a notification unless something failed.

---

### 13.6 — Morning brief
**Outcome:** the first unlock after the configured morning time (or a click on the top-bar clock) shows a brief: what got done, what failed, what's waiting for the user with a suggested next step for each blocked task, costs, updates and project summaries.
**Features:** AUTO-05, AUTO-06, MEM-12.
**Depends on:** 13.3.

**Tasks**
- [ ] Brief builder (role `smart`, local-preferred): sections Done, Needs you (blocked tasks), Failed, Automations, Updates, Costs (yesterday and month vs budget), Projects (per active project: commits, PRs, CI state, open tasks); each item links to its task.
- [ ] Blocked-task suggestions (AUTO-06): for tasks waiting on approval > 1 h, failed after retries, budget-stopped, or with unresolved conflicts → one concrete next step ("Approve the nginx restart", "Retry with Opus — about $0.30", "Pick PostgreSQL or SQLite") with an action button; also shown live in the Task center "Needs you" group.
- [ ] Daily project summaries (AUTO-05): per project, plain summary of yesterday's changes from ChangeSets, git log and CI.
- [ ] Delivery: calendar popover from the top-bar clock (11 §4), brief card in the bar on first morning unlock (setting `brief.time`, default 07:00, weekdays configurable), optional spoken summary via voice (Phase 10) ≤ 45 s.
- [ ] Uses data prepared overnight (13.7) when available, else built on demand (< 10 s with a fast model).
- [ ] "Make my brief shorter", "skip costs", "only weekdays" adjust brief settings by language (natural-language settings).

**UI**
- Brief card and clock popover; states: loading, ready, nothing to report, many items (sections collapse), offline (local-only brief), stale (prepared > 24 h ago), Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: Tab through items, Enter opens task, action buttons reachable; screen-reader reads section headings and counts first.
- 👤 Mockup approval before building; journey sign-off (unlock → brief → act on a blocked task).

**Human in the loop:** the brief only reports and suggests; every suggested action is one click and goes through the usual approval tiers.

**Tests**
- Unit: section builders on event fixtures (a recorded day), blocked-task rules, cost maths.
- Eval: 10 recorded days → owner rates brief usefulness ≥ 4/5 for 8; blocked-task suggestions judged actionable ≥ 80 %.
- Integration: fake day of events → brief contains the failed task with a suggestion; first unlock after 07:00 shows it once.
- UI: screenshots for every state × theme × scale; journey.
- Regression: `brief_sections_from_day`, `blocked_task_suggestion`, `brief_shown_once_morning`.

**Done when**
- [ ] Owner reads the brief daily for a week and acts on at least one suggestion from it.

---

### 13.7 — Overnight "dreaming" and batch jobs
**Outcome:** while the laptop is on AC and idle overnight, PriyaOS consolidates the day into project and personal memory (as reviewable changes), deduplicates and flags stale memories, refreshes indexes, prunes per retention and prepares the brief — using provider batch APIs or local models to cut cost.
**Features:** MEM-12, AI-20.
**Depends on:** 13.6.

**Tasks**
- [ ] Dreaming window: `dreaming.window = "01:00-06:00"`, requires AC power and idle; skipped (and reported) otherwise; wake-from-suspend off by default (`WakeSystem=` opt-in).
- [ ] Jobs as an automation chain: summarise each task of the day into project memory candidates; deduplicate personal/project memories; flag stale memories (contradicted by later tasks or files); refresh file index embeddings for changed files; coordinate with Rewind retention pruning (7.2); prepare the morning brief.
- [ ] Memory changes produced as a memory ChangeSet: additions applied and marked "new"; edits and deletions held for review in the brief ("3 memory updates — review"); all undoable from the Memory page.
- [ ] Batch API support (AI-20) in `priya-ai`: `ModelProvider::batch_submit/poll` for providers that offer batches (Anthropic Message Batches, OpenAI Batch, Gemini batch); router picks batch-capable models for `latency = background` jobs; results collected before the brief; fallback to local or normal calls if not finished by 06:00.
- [ ] Cost ceiling per night (default $0.25) and savings recorded in `priya ai savings` under "batch".
- [ ] `priya dream run-now` for testing; dreaming report in the brief ("Learned 4 things about project api; 1 memory looked outdated").

**UI**
- Brief "Memory updates" section and Memory page "new/needs review" badges; states: nothing learned, updates applied, needs review, skipped (on battery), batch pending, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** memory deletions and edits are never silent; additions are visible and undoable; dreaming never runs on battery.

**Tests**
- Unit: window/condition rules, memory diff builder, staleness rules, batch request assembly and result mapping.
- Contract: batch adapters against recorded cassettes (submit, poll pending, completed, partially failed, expired).
- Integration: recorded day → dream run → memory ChangeSet with expected additions; on-battery → skipped with reason; batch not complete by deadline → fallback path used.
- Eval: 10 recorded days → memory additions judged correct ≥ 90 %, no secrets in memory (scan).
- Perf: dreaming on the Zenbook completes in < 20 min for a typical day; batch path cost ≤ 50 % of the same jobs via normal calls.
- Regression: `dream_requires_ac_idle`, `dream_memory_changes_reviewable`, `batch_fallback_on_deadline`, `no_secrets_in_memory`.

**Done when**
- [ ] A week of dreaming on the Zenbook: owner reviews memory updates in the brief; batch savings reported.

---

### 13.8 — Spotting repetitive work
**Outcome:** PriyaOS notices repeated intents, command sequences and window routines, and at most once a day offers to turn one into an automation or recipe, pre-filled.
**Features:** AUTO-07.
**Depends on:** 13.3.

**Tasks**
- [ ] Detector over the event log: clusters of similar intents (embeddings, role `embed`, local) recurring ≥ 3 times in 14 days; recurring time-of-day patterns; repeated navigation/layout intents.
- [ ] Command sequences: from `priya sh` history by default; regular shell history only with explicit opt-in (`automation.learn_from_shell = false` default; secrets redacted before analysis).
- [ ] Suggestion card ("You've asked for the weekly sales summary 4 Mondays in a row. Do it automatically every Monday at 9?") with the pre-filled automation preview (13.3); options Set it up / Not now / Never for this.
- [ ] Rate limit: ≤ 1 suggestion per day, none during DND or focus layouts; dismissed patterns never re-suggested.
- [ ] All analysis local; suggestions and dismissals stored as events.

**UI**
- Suggestion card in the bar and the brief; states: suggestion, accepted (opens preview), not now, never, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: clustering thresholds, periodicity detection, rate limiter, dismissal memory.
- Eval: 4 recorded weeks with planted routines → recall ≥ 70 %, false suggestions ≤ 1 per week.
- Privacy: shell history not read when opt-in is off (file access audit); no model calls leave the device for detection with default settings.
- Regression: `repetition_detects_planted_routine`, `suggestion_rate_limited`, `shell_history_opt_in_only`.

**Done when**
- [ ] Owner accepts at least one suggestion during the phase-gate week and finds no more than one useless suggestion per week.

---

### 13.9 — Recipes
**Outcome:** shareable workflow files (`*.recipe.toml`) with typed parameters, steps and sub-recipes compile to plans; the user runs them by name or sentence, shares them as files, and imports others' recipes safely.
**Features:** AUTO-08.
**Depends on:** 13.1.

**Tasks**
- [ ] Recipe format (TOML, schema in `priya-api`): `name, description, version, params [{name, type, default, prompt}], requires {capabilities}, mode, budget, steps [{intent | method | recipe, depends_on, mode?, verify?}]`; ADR noting the TOML choice.
- [ ] Compiler: recipe + params → plan DAG (12.1 validation); requested capabilities must cover every step; sub-recipes inlined with depth limit 3.
- [ ] Storage: `~/.local/share/priya/recipes/` (user), `/usr/share/priya/recipes/` (shipped examples), project `.priya/recipes/`.
- [ ] CLI and intents: `priya recipe list|show|run|new|edit|share|import`; "run my release-notes recipe for 1.2" fills params from the sentence and asks for missing ones.
- [ ] Share: export a single file (no secrets; parameters for anything personal); import: treated as **untrusted** — capability review card (like app permissions), task taint, mode locked to `sandbox`/`read_only` for its first run, external actions always `ask`; scanning and signatures arrive in Phase 14 (14.5/14.6).
- [ ] Recipes usable as automation actions (13.1) and from the bar's local results ("Run recipe: weekly cleanup").

**UI**
- Recipe run sheet (params form), import review card, recipes list in Settings → Automations; states: list, empty, run with params, missing params, invalid recipe, imported (untrusted badge), Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard-complete params form; screen-reader labels per param.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** imported recipes show every capability before first use; the first run of an imported recipe is sandboxed and reviewed.

**Tests**
- Unit: parser and schema errors (plain messages), compiler (params substitution, capability coverage, sub-recipe depth), golden plans.
- Contract: recipe schema snapshot.
- Integration: shipped example recipe runs end to end; imported recipe with an undeclared capability → refused at compile time; imported recipe → tainted, sandboxed first run.
- Security: recipe trying to read `~/.ssh` via a step → denied by policy; recipe file containing a secret-looking value → export blocked with explanation.
- Regression: `recipe_compiles_to_plan`, `recipe_import_untrusted`, `recipe_capability_coverage`.

**Done when**
- [ ] Owner writes one recipe by plain language, runs it with parameters, exports it and re-imports it on the VM.

---

### 13.10 — Teach by showing
**Outcome:** the user presses Record, does a routine once (terminal commands, app clicks, file moves), presses Stop; Priya AI drafts a recipe with parameters; the user edits and test-runs it; it's saved as a recipe limited to the capabilities it observed.
**Features:** AUTO-09.
**Depends on:** 13.9.

**Tasks**
- [ ] Recording session (`priya teach start|stop`, bar command, Super+Shift+R): consent sheet listing exactly what will be recorded; persistent red recording indicator in the top bar; auto-stop after 15 min.
- [ ] Recorders: shell commands via a preexec hook for bash/zsh/fish installed by `priya desktop install` (active only while recording); Hyprland IPC events (window open/focus/workspace); AT-SPI events from 11.5 (focus, activated actions, text changes) excluding private windows (11.6) and password fields; inotify on folders the user selects.
- [ ] Redaction before storage: secrets patterns, password fields, private-window events dropped.
- [ ] Drafting (role `smart`): recording → recipe draft with inferred parameters (file names, versions, dates), steps as intents or app actions (11.8), and the minimal capability set observed.
- [ ] Editor: draft shown as steps in plain language with the recipe file behind "Details"; edit by language ("make the folder a parameter"); test run in sandbox with review.
- [ ] Save as recipe; recordings deleted after saving unless the user keeps them.

**UI**
- Consent sheet, recording indicator, draft editor; states: consent, recording, paused, stopped/drafting, draft ready, test running, test failed, saved, discarded, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: Super+Shift+R toggles; editor fully keyboard-operable; screen-reader announces "Recording started/stopped".
- 👤 Mockup approval before building; journey sign-off (record → draft → edit → test → save).

**Human in the loop:** recording only starts after consent and is always visibly on; the user reviews and edits the draft; the first run is sandboxed and reviewed.

**Tests**
- Unit: event merging into steps, parameter inference on fixture recordings, redaction (password field events, private window events, secrets in commands).
- Integration (headless Hyprland + test apps): scripted 5-step routine (create folder, move files, run a command, click Export in a GTK app, rename output) → draft recipe → test run in sandbox reproduces the result.
- Security: recording never includes events from a deny-listed window or password field (asserted on raw recorder output); recorder hooks inactive when not recording.
- Eval: 10 recorded routines → drafts that run correctly after ≤ 2 user edits for ≥ 7.
- Regression: `teach_draft_runs`, `teach_redacts_private`, `teach_hooks_inactive_when_idle`.

**Done when**
- [ ] Owner turns a real recorded 5-step workflow into a working recipe.

---

### 13.11 — Phase gate
**Outcome:** automation is useful, bounded and quiet over a week of real use.
**Features:** all Phase 13 features.
**Depends on:** 13.1–13.10.

**Tasks**
- [ ] Full regression and nightly suites green.
- [ ] One-week L4 trial on the Zenbook; record results below.
- [ ] Update `features.md`, `docs/human/05-the-phases.md`, `docs/human/04-everything-priyaos-will-do.md`.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green, including nightly e2e (timers across reboot, dreaming on a recorded day), chaos (restart during automation runs), security (recipe import, recording redaction) and eval (automation NL, brief, repetition, teach-by-showing) suites.
- [ ] Owner demo: "every morning at 9, check my projects' CI and tell me what's broken" → next morning's brief shows it; a monitor reports a stopped service with a correct cause; a recorded routine runs as a recipe by sentence.
- [ ] L4 (Zenbook, one week): ≥ 3 owner-created automations running; brief rated ≥ 4/5 on ≥ 5 days; dreaming ran on AC only; total automation spend within caps; no notification storms (≤ 5 automation notifications per day); suspend/resume never caused duplicate runs.
- [ ] Docs: automation section added to [02-core](../architecture/02-core.md) or a new architecture file (engine, triggers, limits, recipes format); batch support in [04 §8](../architecture/04-priya-ai.md); feature statuses and human status updated.

## Out of scope

- Inbound webhooks and triggers from internet services (needs the Phase 14 network model and Phase 15 connectivity).
- Calendar, email or chat integrations as triggers or brief sections (no feature IDs yet).
- Signed/scanned recipes and a public recipe directory (Phases 14 and 16).
- Automations that run on other machines (Phase 15).
