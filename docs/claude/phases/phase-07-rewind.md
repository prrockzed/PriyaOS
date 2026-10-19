# Phase 7 — Priya Rewind

**Goal.** One timeline of every change on the machine — by agents, modules, settings, live actions and the user by hand — with precise undo: revert one change and keep everything after it, restore any file or folder to any time, return the whole system to a point, undo settings, check that an undo worked, and do all of it by plain language ("undo whatever broke my Wi-Fi on Tuesday").

**Usable at the end:** Super+Space → "undo whatever broke my Wi-Fi this afternoon" → ranked candidates with plain explanations → pick one → review → Wi-Fi back, verified. The Rewind app shows who changed what, when and why; "restore ~/notes/plan.md to yesterday 6 pm" works; "undo what I changed in settings yesterday" works.

**Depends on:** Phase 2 (snapshot backend, ChangeSet engine 2.9–2.10, home merge 2.11, system merge and rollback 2.12, direct mode 2.13), Phase 3 (tasks, approvals, Cedar), Phase 4 (`priya mcp` with the `rewind_log` stub 4.11, `history_fts`, Task center 4.13), Phase 5 (bar, Undo card 5.4, Settings app, NL settings 5.9), Phase 6 (symptom probes and playbook `verify` blocks 6.2–6.6, live actions).

**Features:** RWD-01, RWD-02, RWD-03, RWD-04, RWD-05, RWD-06, RWD-07, RWD-08, RWD-09, RWD-10.

**Read first:** [08 §1 Priya Rewind](../architecture/08-rewind-memory-tools.md), [03 §2 layout, §5 ChangeSets, merge, revert, rollback, §6 limits](../architecture/03-execution-and-sandbox.md), [02 §1 events, §6.2 settings as ChangeSets](../architecture/02-core.md), [01 §5 storage](../architecture/01-system-overview.md), [04 §4 roles, §6 learning from reverts](../architecture/04-priya-ai.md), [05 §4 approvals](../architecture/05-security.md), [09 §2 eval suite](../architecture/09-testing.md), [11 §4 Rewind, Cards (Undo)](../architecture/11-design-system-and-ux.md), ADR-003, ADR-004, ADR-008.

## Design notes for this phase

- **Rewind never rewinds itself.** `~/.local/share/priya/**` (event store), `@priya` (helper state and blobs), and the timeline subvolume are excluded from every revert, restore and system return.
- **Machine-level vs per-user.** Timeline snapshots are taken by the helper for the whole machine (a system timer, not a user's priyad). Each user's priyad sees system entries plus only its own home; contents of root-only files (for example `/etc/shadow`) are recorded as hashes only and never copied into a user-readable blob store.
- **Package-aware inverse.** Reverting a ChangeSet that installed, upgraded or removed packages uses pacman operations in the fork (remove, reinstall the cached old version, re-install), then file-level inverse for the rest, so the package database stays consistent. Reverting a whole system update is steered to "Return the system to a point" because partial downgrades are unsupported on Arch.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 7.1 | Timeline snapshots | Phase 6 | Hourly and around-pacman snapshots of system and home exist; `priya rewind snapshots` lists them |
| 7.2 | Retention and space | 7.1 | Snapshots and blobs stay within a space budget; `priya rewind usage` explains what uses space |
| 7.3 | Manual ChangeSets | 7.1 | Changes made by hand between snapshots appear as attributed entries ("you, or a program you ran") |
| 7.4 | Timeline query and attribution | 7.3 | `priya rewind log --path /etc/NetworkManager --since tuesday`, `priya rewind blame <file>`; agents see real `rewind_log` |
| 7.5 | Selective revert | 7.4 | `priya rewind revert <id>` undoes one change and keeps later work, via sandbox review |
| 7.6 | Restore a path at a time | 7.4 | `priya rewind restore ~/notes/plan.md --at "yesterday 6pm"` |
| 7.7 | Return the system to a point | 7.4 | `priya rewind return --to <point>` boots the system as it was, home untouched by default |
| 7.8 | Settings revert | 7.4 | "Undo what I changed in settings yesterday" |
| 7.9 | Post-undo verification | 7.5–7.8 | Every undo ends with "Restored and checked" or a clear "didn't help" |
| 7.10 | Undo by prompt | 7.5–7.9 | "Undo whatever broke my Wi-Fi on Tuesday" → ranked candidates → revert → verified |
| 7.11 | Rewind UI | 7.4–7.10 | Rewind app: scrubbable timeline, filters, details, all undo actions |
| 7.12 | Phase gate | all | A week of real timeline on the Zenbook; undo by prompt meets its eval targets |

---

### 7.1 — Timeline snapshots
**Outcome:** read-only snapshots of `@` and `@home` are taken hourly and before/after every pacman transaction on the live system, into `@priya-timeline/<ts>-{root,home}`, with an index the user can list.
**Features:** RWD-02.
**Depends on:** Phase 6.

**Tasks**
- [ ] `modules/rewind` process module skeleton: `methods = ["rewind.snapshots", "rewind.log", "rewind.show", "rewind.blame", "rewind.versions", "rewind.revert", "rewind.restore", "rewind.return", "rewind.usage", "rewind.prune", "rewind.ask"]`, `tools = ["rewind_log"]`, `subscribes = ["changeset.recorded", "merge.applied", "revert.applied", "system.swapped", "setting.changed", "module.event"]`, settings schema `[rewind]`.
- [ ] Helper operations: `timeline.snapshot {reason}` (creates both read-only snapshots, records them in `system.db` with generation numbers and reason `hourly | pacman_pre | pacman_post | manual | before_return`), `timeline.list`, `timeline.delete {id}` (refuses snapshots pinned by 7.2); `doctor` creates `@priya-timeline` if missing.
- [ ] `priya-timeline.timer` + `priya-timeline.service` (system units, oneshot) calling the helper hourly (`rewind.interval`, default 1 h); coalesce: skip when the last snapshot is < 10 min old and the root generation hasn't changed.
- [ ] pacman hooks `/usr/share/libalpm/hooks/90-priya-timeline-pre.hook` (PreTransaction) and `zz-priya-timeline-post.hook` (PostTransaction): skipped inside forks (`/run/priya/fork`), never block pacman (log a warning on failure, exit 0), 15 s timeout.
- [ ] Space guard: below 10 % free, take the snapshot only if retention (7.2) can free space first; below 5 %, skip and notify.
- [ ] Nested-subvolume check: `priya doctor` lists nested subvolumes under `/` or `/home` (e.g. `/var/lib/docker/btrfs`, `/var/lib/machines`) which snapshots do not include, and explains the consequence.
- [ ] Events: helper journal records snapshots; each user's priyad receives `timeline.snapshot_taken` through the rewind module (projection `timeline_points`).
- [ ] CLI: `priya rewind snapshots [--since]`, `priya rewind snapshot now` (reason `manual`).

**Tests**
- Unit: coalescing rules, reason tagging, hook skip logic (fork marker present), space-guard decisions.
- Contract: `SnapshotBackend` suite extended with timeline create/list/delete (btrfs adapter on a loop device).
- Integration (fake machine): timer fires (fake clock) → two snapshots; `pacman -S jq` live on the fake machine → pre and post snapshots with matching reasons; same command inside a fork → no snapshot; failing helper → pacman still completes.
- Chaos: kill the helper between the root and home snapshot → on restart the half-pair is completed or removed and reported, never left as an unindexed subvolume.
- Perf: snapshot pair < 1 s on the fake machine; hourly timer adds < 1 % CPU over a day (perf runner).
- Regression: `timeline_hourly_pair`, `timeline_pacman_hooks`, `timeline_skipped_in_fork`, `timeline_hook_never_blocks_pacman`.

**Done when**
- [ ] A simulated day on the fake machine yields the expected snapshot list (hourly + pacman pairs, coalesced).
- [ ] pacman never fails or slows by more than 2 s because of Rewind (perf assertion).

---

### 7.2 — Retention and space
**Outcome:** timeline snapshots and Rewind blobs are pruned by rule and by space budget, never removing anything still needed by a ChangeSet in retention; the user can see and change the rules.
**Features:** RWD-10.
**Depends on:** 7.1.

**Tasks**
- [ ] Retention selector (pure, in `priya-core`): hourly for 48 h, daily for 30 days, weekly for 6 months; pins: snapshots bracketing a ChangeSet whose blobs are retained, snapshots referenced by an open revert/restore/return task, `before_return` snapshots for 14 days.
- [ ] Blob retention per ChangeSet origin ([08 §1.4](../architecture/08-rewind-memory-tools.md)): runs 90 days, manual follows timeline, settings forever (small); refcount GC in the blob store.
- [ ] Space measurement: ADR choosing btrfs simple quotas (`squota`, kernel ≥ 6.7) for per-snapshot exclusive size versus estimation from free-space deltas; implement the chosen method behind `SnapshotBackend::space_used`.
- [ ] Budget: `rewind.max_space_percent` (default 15 % of the filesystem); over budget → prune oldest unpinned first, notify once with what was removed ("Removed hourly points older than 3 days to free 6 GB").
- [ ] `priya rewind usage` (snapshots, blobs, previous systems, forks — each with size and oldest point), `priya rewind prune [--dry-run]`.
- [ ] Rewind section in Settings → Autonomy & Safety: keep-for rules, space budget, snapshot interval, "pause timeline" (with a warning).
- [ ] Never touches `@`, `@home`, `@priya` or the newest `@priya-prev-*` (helper invariant re-checked).

**UI**
- Space view in the Rewind app (bar chart by kind, oldest point) and the Settings section.
- States: within budget, near budget, over budget (pruning), pruning done, paused timeline, error (quota unavailable → estimate shown), long content, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard path through rule fields; screen-reader names ("Rewind uses 18 GB, 12 % of disk, budget 15 %").
- 👤 Mockup approval (`ui/mockups/7.2/`) before building; gallery sign-off.

**Tests**
- Unit: retention selection on a year of synthetic timestamps (table-driven), pin handling, budget pruning order, GC refcounts.
- Integration (fake machine): a simulated week of activity (fake clock, scripted edits and pacman transactions) stays under budget; pinned bracketing snapshots survive; GC removes unreferenced blobs only.
- Security: helper refuses to delete protected subvolumes and the newest previous system even when the selector (fuzzed) asks.
- Regression: `retention_selector_table`, `retention_keeps_pinned`, `rewind_budget_week`, `rewind_never_deletes_protected`.

**Done when**
- [ ] A simulated week stays within the configured budget with correct pins.
- [ ] `priya rewind usage` numbers match `btrfs` reality within 5 % (squota) or are labelled as estimates.

---

### 7.3 — Manual ChangeSets
**Outcome:** changes made outside Priya (editing a file by hand, `sudo pacman -S` in a terminal, an app rewriting its config) appear in the timeline as manual ChangeSets between two snapshots, computed lazily and attributed as well as possible.
**Features:** RWD-01, RWD-07.
**Depends on:** 7.1.

**Tasks**
- [ ] Lazy computation `rewind.manual_window {from, to, area}` reusing the 2.9 engine (`send --no-data` dump between consecutive timeline snapshots, classification, summary); result cached as a ChangeSet with `Origin::Manual(window)`.
- [ ] Subtraction: entries already explained by recorded ChangeSets merged inside the window (same path and same after-hash) are removed, so agent work isn't shown twice.
- [ ] Attribution hints for the remainder: `pacman.log` transactions in the window → "pacman -S foo (you)"; `setting.changed` events → settings; live actions (Phase 6 `sysadmin.action_performed`) → listed as non-file entries; otherwise "you, or a program you ran".
- [ ] System entries computed by the helper (`timeline.diff {from, to}`) with root-only files recorded as metadata and hash only; home entries computed by the owning user's priyad; other users' homes never diffed for a user.
- [ ] Blob capture for manual windows: before/after contents of changed regular files ≤ 50 MiB readable by the user; larger files reference the snapshot while it is retained.
- [ ] Background pre-computation for the last 48 h when idle and on AC power, so the timeline opens fast.
- [ ] Ignore list shared with forks ([03 §5.1](../architecture/03-execution-and-sandbox.md)) plus Rewind's own state.

**Tests**
- Unit: subtraction logic (same path different hash stays; same hash removed), `pacman.log` window parser, attribution precedence.
- Integration (fake machine): hand-edit `~/notes/a.md` and `/etc/hosts`, run `pacman -S jq` live, merge an agent ChangeSet — all in one window → manual ChangeSet contains the two edits and the jq install attributed to pacman, not the agent's files; user B cannot see user A's home entries; `/etc/shadow` change shows "content hidden".
- Perf: manual ChangeSet for a 1-hour window with 5k touched files < 10 s.
- Regression: `manual_changeset_window`, `manual_subtracts_agent_entries`, `manual_root_only_hash_only`, `manual_other_user_hidden`.

**Done when**
- [ ] Every change in the integration scenario is accounted for exactly once across agent, settings, live-action and manual entries.

---

### 7.4 — Timeline query and attribution
**Outcome:** one query over every ChangeSet origin (runs, settings, manual, reverts) and live actions, filterable by time, path, actor, kind, package and service; `priya rewind blame <path>` shows who changed a file, when, and why; agents get the real `rewind_log` MCP tool.
**Features:** RWD-01, RWD-07.
**Depends on:** 7.3.

**Tasks**
- [ ] Projection `rewind_timeline` (id, ts, origin, actor `user | agent:<id> | module:<name> | settings | pacman`, model, task id, reason = task intent/goal/summary, areas, counts, external actions, reversible flag) built from `changeset.recorded`, `merge.applied`, `revert.applied`, `setting.changed`, `sysadmin.action_performed`, manual windows.
- [ ] Projection `changeset_paths` (path, changeset, kind, before_hash, after_hash, ts) for fast path queries; package and service indexes from ChangeSet summaries.
- [ ] `rewind.log {since?, until?, path?, actor?, kind?, area?, package?, service?, text?}` with FTS over reasons and summaries (`history_fts`); CLI `priya rewind log`, `priya rewind show <id>`, `priya rewind blame <path>`, `priya rewind versions <path>` (distinct versions, deduplicated by hash, from blobs and snapshots).
- [ ] Natural-language time parsing (`--since tuesday`, "yesterday 6pm") with a deterministic parser and Priya AI (`fast`) fallback; ambiguous dates are confirmed.
- [ ] Replace the 4.11 `rewind_log` stub in `priya mcp` (read-only; inside forks too).
- [ ] "Why did this change?" answers: for a path, link to the task, the agent and model used, the intent text and the review decision.
- [ ] Plain-language summaries per entry ("Codex added 3 files to api and installed postgresql") reused from 2.10 summaries.

**Tests**
- Unit: filter combination semantics, time parser table (English phrases, Hindi-English mixes from 10.7 later), blame ordering.
- Integration (fake machine): scripted history (two agent tasks, a settings change, a live action, a manual edit) → `log --path`, `--actor agent:codex`, `--package jq`, `blame` and `versions` return the expected entries; agent in a fork calls `rewind_log` and gets read-only results.
- Perf: `rewind.log` over 10k ChangeSets < 200 ms (p95).
- Regression: `rewind_log_filters`, `rewind_blame_attribution`, `rewind_versions_dedupe`, `mcp_rewind_log_real`.

**Done when**
- [ ] For every file touched in the scripted history, `blame` names the right actor and reason.

---

### 7.5 — Selective revert
**Outcome:** any single ChangeSet (or chosen entries of it) can be undone on top of the current state — later work is kept — through a sandbox task with review; conflicts with later changes are always shown, never resolved silently.
**Features:** RWD-03.
**Depends on:** 7.4.

**Tasks**
- [ ] Inverse builder (pure, `priya-core`): for each entry, three-way decision with base = the ChangeSet's after-state, target = its before-state, current = live; current == after → restore before; current == before → nothing; otherwise conflict.
- [ ] Package-aware inverse: added packages → `pacman -Rns` when nothing installed later depends on them (else conflict); upgraded → `pacman -U` of the cached old version from `@pkg` (else conflict "old version no longer cached"); removed → reinstall; package-owned files are left to pacman, other files use the file inverse.
- [ ] Update-ChangeSet guard: reverting a ChangeSet from the `updates` module offers "Return the system to before this update" (7.7) instead of a partial downgrade.
- [ ] Revert task `rewind.revert {changeset_id, entries?}`: fork current state, apply the inverse in the fork, record a ChangeSet with `Origin::Revert(of)`, review (Task center), merge (home live, system on restart).
- [ ] Live actions: entries with an inverse (rfkill, audio profile) are offered as a live action; entries without one are shown as "can't be undone" (e.g. a service restart).
- [ ] External actions in the original ChangeSet (push, send) are listed as "left your computer — can't be undone here".
- [ ] Learning signal: reverting a run's ChangeSet within 7 days records a negative outcome for its (agent, model) ([04 §6](../architecture/04-priya-ai.md)).
- [ ] CLI: `priya rewind revert <id> [--only <path>…]`; Undo card (5.4) wired to this method.

**Human in the loop:** the review shows what will be undone, what was changed later and is kept, and every conflict with a choice (keep current / restore old / both); system parts apply on restart only after the user chooses.

**Tests**
- Unit: three-way inverse table (every combination incl. deleted-then-recreated, mode/owner changes, symlinks), package inverse planner with dependency checks.
- Integration (fake machine): agent installs Docker and edits `~/projects/api/compose.yaml`; user later edits the same file and an unrelated file → revert removes Docker and the agent's compose change, keeps the user's unrelated edit, raises a conflict on the shared file; revert of a revert restores the original; revert of an update ChangeSet is steered to 7.7.
- Chaos: kill priyad/helper at each journaled step of a revert merge → recovery report, live state either before or after, never half.
- Regression: `revert_keeps_later_work`, `revert_conflict_surfaced`, `revert_package_aware`, `revert_of_revert`, `revert_update_steered_to_return`, `revert_chaos`.

**Done when**
- [ ] Rewind never removes the user's later work silently (conflict assertion across the whole inverse table).
- [ ] Package database is consistent after every package revert (`pacman -Dk` clean).

---

### 7.6 — Restore a path at a time
**Outcome:** any file or folder can be put back to how it was at a given time, from blobs or timeline snapshots, as an undoable change; or saved as a copy next to the current one.
**Features:** RWD-04.
**Depends on:** 7.4.

**Tasks**
- [ ] Version resolver: newest version at or before T from (a) ChangeSet blobs, (b) the nearest timeline snapshot ≤ T; reports the exact source and time used ("as it was at 17:00, the closest point before 18:00").
- [ ] Home restore: direct-mode task with safety snapshot (so the restore itself is undoable), three-way against current; default overwrites the current version, `--as-copy` writes `<name>.restored-<date>`; folders: deleted files return, files added after T are kept unless `--exact`.
- [ ] System path restore (e.g. `/etc/…`): sandbox task, review, system merge on restart; root-only files restored by the helper without exposing content.
- [ ] `priya rewind restore <path> --at <time> [--as-copy] [--exact]`; module method `rewind.restore`.
- [ ] File-manager integration: "Restore older version…" action in Nautilus via a `.desktop` service menu / Nautilus Python extension shipped with `priyaos-shell` (opens the Rewind app filtered to that path).
- [ ] Large files (> 50 MiB) restored by reflink copy from the snapshot when still retained; otherwise explained as no longer available.

**Human in the loop:** the user sees a preview (diff or file list) before anything is written; restoring over a file that changed since is shown as a conflict choice.

**Tests**
- Unit: version resolution across blobs and snapshots, folder restore planner (`--exact` vs default), time selection explanation text.
- Integration (fake machine): edit `~/notes/plan.md` three times over three simulated hours → restore to the middle version; delete a folder → restore it; `--as-copy`; restore of `/etc/hosts` via system merge; restore undone with Rewind (revert of the restore).
- Perf: restore a single file < 1 s; restore a 1 GiB folder by reflink < 5 s.
- Regression: `restore_file_at_time`, `restore_folder_default_vs_exact`, `restore_as_copy`, `restore_system_path`, `restore_is_undoable`.

**Done when**
- [ ] Every restore is itself undoable and recorded in the timeline.

---

### 7.7 — Return the system to a point
**Outcome:** the whole system part can be returned to any timeline point (kernel included), applied on restart like a rollback; the user's files stay as they are unless they also choose to restore their home.
**Features:** RWD-06.
**Depends on:** 7.4.

**Tasks**
- [ ] Helper op `timeline.return {point}`: take a `before_return` snapshot, create a writable snapshot of `<point>-root`, then the 2.12 swap path (current `@` → `@priya-prev-<ts>`, new `@` from the point) on restart; GRUB config regenerated in the new `@`; journaled before/after.
- [ ] Preflight: boot check (2.8) of a writable copy of the point; show the package difference (versions now vs then) and services enabled then vs now; warn that `/var/log`, the package cache and Priya's own state are not returned.
- [ ] Home option: "also return my files" → per-user restore of the home part as a 7.6 folder restore with conflicts listed (off by default); other users' homes are never touched.
- [ ] Permissions: `priya-admin` only; approval tier `confirm`; on multi-user machines other logged-in users are notified before restart.
- [ ] Post-restart: verification (7.9) and a notification "Your system is back to Tuesday 14:00. Your files were not changed." with "Undo this return" (returns to the `before_return` snapshot).
- [ ] CLI: `priya rewind return --to <point|time>`; `priya rollback` stays the shortcut for "previous system".

**Human in the loop:** typed confirmation; a plain list of what will be different after restart; the restart itself is always the user's choice.

**Tests**
- Unit: preflight diff builder, plan for home option, permission checks.
- Integration (fake machine): install packages and edit `/etc` over a simulated day → return to the morning point → fake restart → packages and `/etc` match the point, home unchanged, `before_return` exists → undo the return → back to evening.
- E2E (VM): return across a kernel update (ALA dates from 6.8) → VM restarts on the old kernel → undo return → new kernel.
- Chaos: VM killed at each journaled step of a return → boots either old or new system, never neither.
- Regression: `return_system_point`, `return_keeps_home`, `return_undo`, `return_vm_kernel`, `return_chaos`.

**Done when**
- [ ] VM return across a kernel update passes nightly; a failed return never leaves an unbootable machine in the chaos suite.

---

### 7.8 — Settings revert
**Outcome:** any settings change (by the user, by plain language, by a module, or by Rewind) can be undone individually or as a batch ("undo what I changed in settings yesterday"), including user policies, mode rules and remembered approvals.
**Features:** RWD-09.
**Depends on:** 7.4.

**Tasks**
- [ ] Settings ChangeSets of kind `settings` from `setting.changed` (key, old, new, source) — group consecutive changes from one request into one entry.
- [ ] Revert = new `setting.changed` with `source = "rewind"`; three-way: if the current value ≠ the change's new value, show a conflict ("you changed it again on Wednesday").
- [ ] Hand edits of `~/.config/priya/*.toml` arrive through hot reload as `setting.changed` with `source = "file"` (extend the source enum and its schema; additive change).
- [ ] User Cedar policies (`~/.config/priya/policy/*.cedar`), `modes.toml` rules and remembered approvals are revertible the same way (as file ChangeSets, with policy validation before apply).
- [ ] Undo toast from Settings (11 §4) and NL settings confirmations wired to `rewind.revert`.
- [ ] Batch revert by time window and by source ("undo what Priya changed in my settings this week").

**Tests**
- Unit: grouping, three-way on values (scalars, arrays, tables), policy validation before re-apply.
- Integration: change theme, strategy and a mode rule via NL settings → "undo what I changed in settings yesterday" → all three back; a later manual change to the theme → conflict surfaced; reverted policy file validated, invalid result refused with explanation.
- Regression: `settings_revert_single`, `settings_revert_batch_window`, `settings_revert_conflict`, `policy_revert_validated`.

**Done when**
- [ ] Every `setting.changed` source can be reverted, and the revert itself shows in the timeline.

---

### 7.9 — Post-undo verification
**Outcome:** after any revert, restore, return or settings undo, Rewind checks that the expected state is really back and, where the undo was for a symptom, that the symptom is gone.
**Features:** RWD-08.
**Depends on:** 7.5, 7.6, 7.7, 7.8.

**Tasks**
- [ ] State check (exact): re-hash restored paths against the expected before-blobs, compare package versions and enabled units with the target; report mismatches by path.
- [ ] Behaviour check: re-run the original task's verifiers where meaningful (boot check for system parts, project `check` for code changes).
- [ ] Symptom check: when the undo came from a request with a symptom ("Wi-Fi broke"), run the matching Phase 6 playbook `verify` probes (`wifi.*`, `audio.*`, `display.*`, `svc.*`).
- [ ] Result card: "Restored and checked" / "Restored, but Wi-Fi is still down — try the next candidate, return the system to before Tuesday, or ask Priya to look deeper".
- [ ] Event `rewind.verified {ok, checks}`; failures feed Priya AI learning for undo-by-prompt ranking.

**Tests**
- Unit: state-check comparison, choice of checks per undo kind and symptom.
- Integration (fake machine): revert the `wifi.backend_conflict` change → state and NetworkManager unit checks green; deliberately incomplete restore (file changed by a seeded hook after apply) → state check reports the path.
- Regression: `undo_state_check_exact`, `undo_symptom_check_wifi`, `undo_verify_reports_mismatch`.

**Done when**
- [ ] Every undo path in 7.5–7.8 ends with a verification result event (asserted in integration).

---

### 7.10 — Undo by prompt
**Outcome:** a plain request like "undo whatever broke my Wi-Fi on Tuesday" becomes a structured query, finds candidate changes, ranks and explains them, and runs the chosen revert with verification; it asks when unsure and says so when nothing matches.
**Features:** RWD-05.
**Depends on:** 7.5, 7.6, 7.7, 7.8, 7.9.

**Tasks**
- [ ] Query builder `rewind.ask {text}` (Priya AI role `smart`) → `RewindQuery {window, areas, paths, packages, services, symptoms, actors, kind}` validated against a JSON Schema; deterministic date resolution from 7.4.
- [ ] Symptom map `data/rewind/symptoms.toml`: per symptom (wifi, bluetooth, audio, display, brightness, boot, login, printing, docker, shell, hyprland, keyboard, network/DNS, performance) the paths, packages, services and Phase 6 playbooks involved (e.g. wifi → `/etc/NetworkManager/**`, `/etc/iwd/**`, `/etc/modprobe.d/**`, `networkmanager`, `iwd`, `wpa_supplicant`, `linux`, `linux-firmware*`, `NetworkManager.service`).
- [ ] Candidate retrieval: structured filters on `changeset_paths` and summaries + `history_fts` + manual windows (computed on demand) + live actions; cap 30 candidates.
- [ ] Ranking: deterministic pre-score (overlap with symptom map, time proximity, risk of the change kind) then Priya AI ranking with a one-sentence explanation per candidate and a confidence; privacy: only metadata (paths, package names, summaries) goes to cloud models; diffs of home files only to `private`-role models.
- [ ] Decision policy: high confidence and a clear gap → suggest one with "Undo this"; close scores → ask the user to pick; nothing relevant → say so and offer "Return the system to before <time>" or a Phase 6 diagnosis instead.
- [ ] "Do the most likely one" → still a sandbox revert with review (system merges stay `ask`); then 7.9 verification; if verification fails, offer the next candidate.
- [ ] Bar intents ("undo …", "what changed in /etc last week?", "remove the Docker setup but keep my later edits") routed to the rewind module.
- [ ] Evaluation set `eval/rewind/undo-by-prompt/`: ≥ 30 seeded histories built by scripts on the fake machine (and stored as ChangeSet fixtures for fast replay), ≥ 120 prompts in English (plus 20 Hinglish), each labelled with the correct ChangeSet id(s), or "ambiguous — must ask", or "nothing matches — must say so". Scenarios include Wi-Fi backend change, firmware update, audio config, Hyprland monitor edit, shell rc edit, Docker install with later user edits, settings strategy change, and decoys (unrelated changes in the same window).

**UI**
- Undo-by-prompt card in the bar: understood query in plain words ("Looking for changes to Wi-Fi between Tue 00:00 and 23:59"), ranked candidates (time, who, what, why it might be the cause), "Undo this" per candidate, "None of these".
- States: searching, candidates (1, several), ambiguous (asks), nothing found, undo in progress, verified, verify failed (next candidate offered), offline (local model; keyword-only ranking with a notice), long content (30 candidates, virtualised), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: arrow keys through candidates, Enter = "Undo this", Esc cancels. Screen-reader names: "Candidate 1 of 4: Tuesday 14:02, update installed linux-firmware, likely cause".
- 👤 Mockup approval (`ui/mockups/7.10/`) before building; journey sign-off ("undo whatever broke my Wi-Fi" → candidate → review → verified).

**Human in the loop:** nothing is undone without the user picking or confirming "the most likely one"; the review shows exactly what will be reverted; low confidence always asks.

**Tests**
- Unit: query schema validation, symptom-map lookup, pre-score, decision policy thresholds.
- Integration (fake machine): demo scenario — the owner-style manual Wi-Fi backend edit, then unrelated work → "undo whatever broke my Wi-Fi this afternoon" → correct manual candidate first → revert → 7.9 Wi-Fi check green.
- Eval (nightly, fixed model set and a local-only run): top-1 ≥ 85 %, top-3 ≥ 95 %, correct "ask" on ambiguous prompts ≥ 90 %, correct "nothing matches" ≥ 90 %, confidently wrong (top-1 wrong with high confidence) ≤ 3 %. Local-only baseline recorded, not gated.
- Security: candidate explanations never include file contents when routed to a cloud model (asserted on request logs).
- Regression: `undo_prompt_wifi_demo`, `undo_prompt_eval_gate`, `undo_prompt_asks_when_ambiguous`, `undo_prompt_nothing_matches`.

**Done when**
- [ ] Eval thresholds met on the full set; results stored as the baseline in `eval/rewind/baseline.json`.
- [ ] 👤 Owner reads five candidate explanations and finds each understandable.

---

### 7.11 — Rewind UI
**Outcome:** a Rewind app with a scrubbable timeline and list, filters, change details and every undo action, plus Rewind entry points from the bar, Task center, Settings and the file manager.
**Features:** RWD-01, RWD-03, RWD-04, RWD-06, RWD-07.
**Depends on:** 7.4, 7.5, 7.6, 7.7, 7.8, 7.9, 7.10.

**Tasks**
- [ ] `ui/apps/Rewind` (Quickshell floating window) using `PriyaUI`; new components (`TimelineScrubber`, `ChangeRow`, `ActorBadge`, `FilterChips`) added to the library and gallery first.
- [ ] Timeline: horizontal scrubber with day/hour density marks and a list view (virtualised); jump to time; live updates from events.
- [ ] Filters (11 §4): Who (you, each agent, settings, updates, live actions), What (files, packages, services, settings), Where (path picker), text search.
- [ ] Detail pane: plain summary, who/why/model/task link, grouped entries, diffs (reusing 2.14/4.13 diff components), external actions, reversibility line; actions **Undo this change**, **Restore file**, **Return system here**.
- [ ] Undo-in-progress view linked to the revert task; verification result inline.
- [ ] Entry points: Undo card and "Ask about this" in the bar, "Open in Rewind" from the Task center, Settings → Autonomy & Safety → Rewind, Nautilus "Restore older version…".
- [ ] Space view from 7.2.

**UI**
- States (11 §4): empty (new install: "Rewind starts keeping history now"), loading, list, detail, undo in progress, verified, conflict, verify failed, low space, paused timeline, error, offline (all local — no change), long content (10k entries), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced-motion variant of the scrubber.
- Keyboard path: ←/→ moves along the timeline, ↑/↓ through the list, Enter opens detail, U undo, R restore, F focuses filters. Screen-reader names on every row ("Tuesday 18:40, you edited wifi.conf in /etc/NetworkManager") and on scrubber position.
- 👤 Mockup approval (`ui/mockups/7.11/`) before building; gallery sign-off; journey sign-off ("open Rewind → filter by agent → undo one change → verified").

**Tests**
- UI: component tests (scrubber keyboard, filter chips, detail actions), screenshots for every state × theme × scale, a11y lint, journey tests against a mock priyad with a 10k-entry history.
- Integration: Rewind app actions call the same module methods as the CLI (contract on request payloads).
- Perf: app opens < 300 ms; scrolling 10k entries at 60 fps (frame-time trace).
- Regression: `rewind_app_states`, `rewind_app_journey_undo`, `rewind_app_perf_10k`.

**Done when**
- [ ] 👤 Gallery and journey signed off.
- [ ] Every CLI Rewind capability is reachable from the app (parity checklist).

---

## Phase gate

### 7.12 — Phase gate
**Outcome:** Rewind runs for a week on the owner's Zenbook within its space budget, and undo by prompt works on real and seeded histories.
**Features:** all Phase 7 features.
**Depends on:** 7.1–7.11.

**Tasks**
- [ ] Run the full regression suite and nightly suites; fix failures.
- [ ] L4 checklist on the Zenbook (below); record results in this file.
- [ ] Update `features.md` statuses for RWD-01…RWD-10; update [08 §1](../architecture/08-rewind-memory-tools.md) (machine-level timeline, package-aware inverse, live actions, root-only content rule, space measurement ADR) and [02 §1.1](../architecture/02-core.md) (new event types).
- [ ] Update `docs/human/05-the-phases.md` and the Rewind section of `docs/human/04-everything-priyaos-will-do.md`.

**Done when**
- [ ] Whole regression suite green on `main`, plus nightly e2e (return across a kernel update), chaos (revert, restore and return at every journaled step), perf and the undo-by-prompt eval at its thresholds.
- [ ] Owner demo: break Wi-Fi config by hand (fake machine or VM), do other work, Super+Space → "undo whatever broke my Wi-Fi this afternoon" → candidates → revert → verified; restore a file to yesterday; undo yesterday's settings changes; return the VM system to a point and undo the return.
- [ ] L4 on the Zenbook: timeline runs for 7 days of normal use within the space budget; the owner restores a real file in their home to an earlier version and undoes a settings change; system returns are exercised only in the VM (safety rule) unless the owner explicitly chooses to try one on the Zenbook.
- [ ] Docs and feature statuses updated; `docs/human/05-the-phases.md` status updated.

## Out of scope

- Off-device backups (Phase 9 9.5); Rewind is local history, not a backup.
- Undo of external actions (pushes, messages, deploys) — shown as "can't be undone".
- Automatic overnight pruning schedules beyond retention rules (Phase 13 "dreaming").
- Snapshot coverage for nested subvolumes (reported by doctor; layout changes need an ADR).
- Rewind across machines (Phase 15).
