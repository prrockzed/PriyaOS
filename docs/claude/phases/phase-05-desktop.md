# Phase 5 — Desktop experience

**Goal.** Make PriyaOS feel like one complete, calm, beautiful desktop driven by plain language. Every request goes through one intent pipeline that knows what the user is looking at; the Priya bar becomes the launcher, the answer box and the control surface; every result is a card; the top bar, notifications and OSDs show state without stealing focus; agents can be watched and corrected; Settings is complete and changeable by words; the whole desktop — GTK, Qt, terminal, window borders, cursor, icons, fonts — follows one theme in light, dark, auto, OLED black or high contrast with the user's accent.

**Usable at the end:** PriyaOS feels like a complete, beautiful desktop driven by plain language: Super+Space opens the bar in under 100 ms, apps and files appear while Priya is still thinking, "open my api project" lays out the windows, "turn on bluetooth" just happens, "make answers shorter" changes a setting after one confirmation, `priya sh` mixes plain language and commands, the top bar shows what agents are doing, notifications carry actions, Super+P shows agents working and diff comments send them back to revise, and the whole desktop switches theme at sunset. Installs on any Hyprland Arch (Omarchy-aware) with `priya desktop install`. Gate: a usability session with the owner.

**Depends on:** Phase 4 gate. Uses: tokens and theme generator (0.10), PriyaUI and gallery (0.11), minimal bar (0.13), classifier (1.8), AI & Models settings and model picker (1.16), diff components (2.14), mode resolution (3.3), autonomy (3.8), approval card and minimal notifications (3.9), kill switch (3.10), plan preview and Task center (4.12, 4.13), supervision (4.9), Agents page (4.7).

**Features:** CORE-08, ORC-23, ORC-24, ADM-02, UX-03, UX-04, UX-05, UX-06, UX-07, UX-08, UX-09, UX-10, UX-11, UX-12, UX-13, UX-14, UX-17, DS-12, DS-13, DS-14, DS-15, DS-16, DS-17, DS-18.

**Read first:** [07-interfaces](../architecture/07-interfaces.md) (primary), [11-design-system-and-ux](../architecture/11-design-system-and-ux.md) (primary for every screen), [02-core §4–6](../architecture/02-core.md), [03 §1.1](../architecture/03-execution-and-sandbox.md), [05-security §4, §7](../architecture/05-security.md), [06-orchestration §6](../architecture/06-orchestration.md), [09-testing §7](../architecture/09-testing.md), [10 §2, §5](../architecture/10-base-os-and-distro.md), [decisions](../architecture/decisions.md) ADR-009, ADR-011, ADR-012, ADR-013, ADR-014.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 5.1 | Intent pipeline and context capture | Phase 4 gate | "Explain this error" in a terminal uses what's on screen; requests are routed to answer, task, settings or navigation |
| 5.2 | Navigation intents | 5.1 | "Open Firefox", "turn on bluetooth", "dimmer" happen instantly without an agent |
| 5.3 | Priya bar complete | 5.1, 5.2, 4.12 | The bar is the launcher: local results while thinking, chips, attachments, history |
| 5.4 | Full card set | 5.1, 0.11 | Every answer arrives as the right card, each with Undo where possible |
| 5.5 | Top bar and OSDs | 4.9, 0.11 | Quickshell top bar with Priya status, network, audio, battery; volume/brightness OSDs |
| 5.6 | Notification centre | 3.9, 4.13 | Grouped notifications with actions, history, Do Not Disturb, quiet hours |
| 5.7 | Agent workspace and review comments | 4.9, 4.13 | Super+P shows agents working; diff comments make the agent revise |
| 5.8 | Settings app complete | 1.16, 3.8, 4.7 | One Settings app with search, every page, undo, admin locks |
| 5.9 | Natural-language settings | 5.8, 5.1, 5.4 | "Make answers shorter" → confirm → done, with Undo |
| 5.10 | Natural-language shell | 5.1, 5.4, 3.3 | `priya sh`: plain language and commands in one terminal |
| 5.11 | Whole-desktop theming, OLED black, high contrast, accents, auto schedule | 0.10, 5.8 | One click (or sentence) re-themes every app; auto switches at sunset |
| 5.12 | Workspace layouts | 5.2 | "Set up my api project" opens editor, terminal and browser where they belong |
| 5.13 | Accessibility, motion and performance pass | 5.3–5.12 | Orca reads everything, reduced motion works, bar opens < 100 ms |
| 5.14 | `priya desktop install` | 5.3, 5.5, 5.6, 5.11 | One command integrates PriyaOS into any Hyprland setup, Omarchy included, reversibly |
| 5.15 | Phase gate (owner usability session) | 5.1–5.14 | Owner completes the session script without help |

---

### 5.1 — Intent pipeline and context capture

**Outcome:** whatever the owner types is routed to the right path — answer, task, settings change or navigation — using what they are looking at (focused app, terminal directory, project), and "explain this error" in a terminal works without copy-paste.
**Features:** UX-04, UX-06, UX-07.
**Depends on:** Phase 4 gate, 1.8.

**Tasks**
- [ ] `desktop` builtin module (02 §5.3) owning context capture and, later, navigation and layouts; manifest with declared capabilities
- [ ] `ContextCapture`: focused window from Hyprland IPC (`activewindow`: class, title, pid, workspace, monitor); terminal cwd by walking `/proc/<pid>` children of the focused kitty window (kitty remote control as fallback only when enabled); project = nearest `.git` or `.priya.toml` upward; selection (primary selection) and clipboard only on explicit request ("this", attach chip)
- [ ] Terminal output for UX-07: last command output from kitty (`get-text` via remote control when the user enabled it; otherwise the bar asks the user to select the text) — never read continuously
- [ ] Privacy: window classes on the private deny list (shared with Phase 11, VIS-07) are never captured; captured context is shown as removable chips before sending; context fields carry a privacy class so Priya AI routes private context to local models (AI-10)
- [ ] Final `IntentContext` schema in `priya-api` (`focused`, `cwd`, `project`, `selection?`, `clipboard?`, `attachments[]`); `v1.intent.submit` takes it; new `v1.intent.classify` (dry run for live UI hints such as "will run as a task")
- [ ] Classifier v2 over 1.8: `answer | action | settings | navigation | mixed`; deterministic phrase grammar first for overrides (ADR-011): mode ("do it directly", "just look", "in the sandbox"), agent ("use Codex"), model ("with Opus"), background ("in the background") → typed task overrides; model second
- [ ] `mixed` → answer plus an offered action card ("Want me to fix it?"); low confidence → clarification card with 2–3 choices instead of a guess
- [ ] Intent evaluation set (`eval`): 250 labelled requests including Hindi and Hinglish, with expected class and overrides; baseline stored

**UI**
- Context chips in the bar input row (rendered by 5.3): app, folder, project, selection; states: captured, removed by user, private (not captured, chip says "Private window — not shared"), unavailable.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; Backspace on an empty input removes the last chip; screen-reader names "Context: kitty in ~/projects/api, remove button".
- 👤 Mockup approval together with 5.3; gallery sign-off after.

**Human in the loop:** the user sees and can remove every piece of context before it is sent; nothing is captured from private windows; misclassification is corrected with one key (Tab "Answer instead / Do it as a task").

**Tests**
- Unit: phrase grammar fixtures (every override, negations, Hindi phrases), `/proc` cwd walk on fixture trees, project detection, privacy class assignment.
- Contract: `IntentContext` schema snapshot.
- Integration (headless Hyprland + kitty): focused kitty in `~/projects/api` → context has cwd and project; private-class window → nothing captured.
- Eval: class accuracy ≥ 95 %, override extraction 100 % on the grammar set, private context never sent to a cloud model (asserted on `model.called`).
- Perf: deterministic path < 10 ms; classification p50 < 400 ms with a local fast model.
- Regression: grammar fixtures, private-window test, eval thresholds.

**Done when**
- [ ] "Explain this error" with a failing command in kitty returns an explanation and a suggested fix card
- [ ] Eval thresholds met and recorded
- [ ] Context chips are visible and removable before every send

---

### 5.2 — Navigation intents

**Outcome:** "open Firefox", "open my Downloads", "turn on bluetooth", "make the screen dimmer", "volume 30 %", "open appearance settings" happen immediately, with no agent and an Undo where it makes sense.
**Features:** UX-05.
**Depends on:** 5.1.

**Tasks**
- [ ] `desktop` module methods: `open_app {query}` (desktop-entry index, fuzzy match, launch via `hyprctl dispatch exec`), `open_path` (xdg-open; folders in the file manager), `open_settings {page, key?}` (deep links `priya://settings/<page>#<key>`), `toggle {wifi | bluetooth | dnd | night_light}` (nmcli, bluetoothctl, 5.6 DND, hyprsunset), `set {brightness | volume}` (brightnessctl, wpctl), `focus_window`, `workspace {n}`
- [ ] All system commands behind a `CommandRunner` port so tests run against fakes
- [ ] Mapping: manifest `intents` examples + deterministic grammar ("open X", "turn on/off X", "brighter/dimmer", "louder/quieter", "volume to N %"); only when nothing matches does it become an agent task (07 §1)
- [ ] `navigation.performed` event (no task) with previous value for Undo (toggle back, restore brightness/volume)
- [ ] Ambiguity: several apps match → picker card; app not installed → "Not installed. Install it?" (becomes an ADM-01 task)
- [ ] CLI parity: `priya open <thing>`, `priya toggle <thing> on|off`, `priya set brightness 40%`

**UI**
- Compact "Done" card ("Bluetooth is on" + Undo), picker card, not-installed card, failure card ("Couldn't turn on Wi-Fi: the adapter is blocked. Ask why").
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; Enter accepts the first picker entry, arrows choose; screen-reader announces the result.
- 👤 Mockup approval with 5.4; gallery sign-off.

**Human in the loop:** navigation is immediate (it only does what was asked), always logged, and toggles are undoable from the card.

**Tests**
- Unit: grammar → method mapping table, fuzzy app matching on a fixture desktop-entry set, undo value capture.
- Contract: `CommandRunner` fake vs real argv snapshots.
- Integration (headless Hyprland): "open kitty" opens a kitty window; "workspace 3" switches; fake nmcli/wpctl receive the right calls.
- E2E (VM): Wi-Fi off/on and volume through the bar.
- Perf: resolution < 50 ms; visible effect < 300 ms for app launch dispatch.
- Regression: mapping table, open-kitty, undo of a toggle.

**Done when**
- [ ] 30 common navigation phrases resolve without a model call
- [ ] No navigation request ever starts an agent when a direct method exists (asserted over the grammar set)
- [ ] Undo works for every toggle and setter

---

### 5.3 — Priya bar complete

**Outcome:** Super+Space opens the full bar: local results (apps, files, recent tasks, settings pages, commands) appear instantly while Priya thinks, Enter launches the top match, chips set mode and model, files can be attached, and previous prompts are one arrow away.
**Features:** UX-17, UX-08, UX-13.
**Depends on:** 5.1, 5.2, 4.12.

**Tasks**
- [ ] Rebuild `ui/shell/PriyaBar/` to the 11 §4 anatomy: top-centre, 720 px (max 60 % width), radius 16, Priya mark with state animation, input, chips (mode, model, attach, mic — mic hidden until the voice module exists), results, footer key hints; pre-created and only shown (11 §8)
- [ ] `LocalResults` provider with a < 50 ms budget: apps (desktop-entry index), recent files (`recently-used.xbel`) and path completion, recent tasks (`history_fts`), Settings pages and keys, module `surfaces` commands; ranked by frecency
- [ ] Launcher rule: Enter launches the top local result when it is a strong app/file/settings match; AI results never reorder or steal the selection (stable list, reserved space, no layout jumps)
- [ ] Mode chip (Auto · Look · Sandbox · Direct) and model chip (1.16) set task overrides; context chips from 5.1
- [ ] Attachments: drag-and-drop onto the bar, attach chip opens the file portal, paste text; images accepted only when a vision-capable model is enabled (otherwise the chip explains "Images need a vision model — Phase 11 adds more"); size limits with plain errors
- [ ] Prompt history: Up/Down recalls, Ctrl+R searches
- [ ] Focus rule (UX-13): the bar takes keyboard focus only when opened by the user; background events never open it; opens on the focused monitor
- [ ] Integrate the full card set (5.4) and plan/progress/approval cards (UX-08) in the results area; max height 70 % of the screen with scrolling

**UI**
- States (11 §4): idle, typing, local results, thinking, streaming, needs approval, done, error, offline, many results, long answer, attachments (one, several, rejected), Hindi text. Listening is Phase 10.
- Light, dark, OLED-black (bar dims after 30 s idle), high-contrast; scale 1.0 and 2.0; reduced motion.
- Keyboard: Enter submit/launch, Ctrl+Enter background, Tab cycles chips, Alt+1…9 picks a result, Up/Down history, Esc stops streaming then closes; mouse-only path for every action. Screen-reader: input "Ask or tell Priya anything", results list with counts, streaming answer in a polite live region.
- 👤 Mockup approval (`ui/mockups/5.3/`, every state, light + dark) before building; gallery and recorded journey sign-off after.

**Human in the loop:** the user can always see what is local (instant) versus AI (thinking), change mode or model before sending, and stop with Esc.

**Tests**
- Unit: ranking and frecency, launcher rule decision table, attachment validation.
- Integration (headless Hyprland, mock priyad): type "fire" → Firefox result before the first AI token; Enter launches Firefox, not the AI answer.
- UI: component tests for chips, results list, history; screenshots every state × theme × scale; a11y lint; journeys "launch app", "ask → stream → card", "attach file → ask".
- Perf (`perf`, Zenbook runner): bar open p95 < 100 ms (09 §2), local results first paint < 50 ms after a keystroke, zero dropped frames while streaming.
- Regression: launcher rule, three journeys, perf budgets.

**Done when**
- [ ] Owner uses the bar as the only launcher for a week of dogfooding
- [ ] Perf budgets met on the Zenbook
- [ ] 👤 Mockup approved before build; journey signed off after

---

### 5.4 — Full card set

**Outcome:** every answer comes back as plain language plus the right card — command, plan, approval, change summary, diff, undo, file list, chart, cost, error — each with compact and expanded forms and an action that is a real API call.
**Features:** UX-03, UX-07, UX-08.
**Depends on:** 5.1, 0.11; shown in 5.3, Task center and notifications.

**Tasks**
- [ ] Typed `Card` enum in `priya-api` (answer payload `cards[]`): `Answer`, `Command` (exact command, mode it ran in, copy, run again), `PlanPreview` (4.12; supports a read-only step list for later multi-step plans), `Approval` (3.9 restyled into the set), `ChangeSetSummary`, `Diff` (inline mini-diff, "Open in Task center"), `Undo`, `FileList` (open, reveal, copy path), `Chart` (bar/line from typed series, never an image), `Cost`, `Error` (Try again / Ask why), `Clarification` (5.1), `Progress` (running task, 4.9 status line)
- [ ] `CardHost.qml` dispatches on type; unknown types render a safe fallback with "Details"
- [ ] Every card action is a typed API call (P7); every card that changed something carries Undo when an inverse exists
- [ ] Error card wording rules (11 §6): what happened, what to do next, never blame; technical detail behind "Details"
- [ ] `Clarification` and `Progress` added to 11 §4's card list in the same PR
- [ ] Gallery entries for every card × state × theme × scale

**UI**
- Each card: compact, expanded, loading, error; plus long content (200-line command output, 500-file list, long diff), Hindi text, offline (actions needing priyad disabled with reason).
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced motion.
- Keyboard: Tab into card actions, Enter activates, Space expands/collapses, C copies in Command cards. Screen-reader: card role and title ("Command card: sudo pacman -S jq, ran in Sandbox"), chart has a text summary.
- 👤 Mockup approval (`ui/mockups/5.4/`) before building; gallery sign-off after.

**Tests**
- Unit: card payload validation, undo availability rules.
- Contract: `Card` schema snapshot; generated QML helpers match.
- UI: component tests per card; screenshots every card × state × theme × scale; a11y lint (names, roles, chart text alternative); journey "error → Ask why → answer".
- Regression: all card screenshots and the journey.

**Done when**
- [ ] Every card listed in 11 §4 exists in the gallery in every state and theme
- [ ] Every action on every card maps to a documented API method
- [ ] 👤 Gallery signed off

---

### 5.5 — Top bar and OSDs

**Outcome:** a floating Quickshell top bar shows workspaces, clock, Priya's status ring (running count, review badge, needs-you dot), optional cost today, network, audio, battery and tray; volume, brightness, mic-mute and keyboard-layout changes show a small OSD.
**Features:** UX-10.
**Depends on:** 4.9, 0.11.

**Tasks**
- [ ] `ui/shell/TopBar/`: 32 px floating bar per 11 §4 on every monitor; workspaces from Hyprland IPC (click, scroll); centre clock with calendar popover (morning-brief slot hidden until Phase 13)
- [ ] Priya status ring from a `v1.events.subscribe` filter on task states: running count, review badge, needs-you dot; click → Task center; right-click → Open bar, Pause background work, Stop everything (kill switch)
- [ ] Cost today (setting `ui.topbar.show_cost`, off by default) from `costs_daily`; soft-budget warning colour + word (04 §8 budget guard)
- [ ] Network (NetworkManager D-Bus), audio (PipeWire), battery and power profile (UPower, power-profiles-daemon), tray (StatusNotifierItem); clicks open minimal popovers with toggles and "Open settings" — full panels are Phase 9 (OS-07)
- [ ] `priya status --waybar` JSON for users who keep Waybar (07 §2, Omarchy)
- [ ] OSDs (`ui/shell/Osd/`): volume, brightness, mic mute, keyboard layout; sources: PipeWire changes, backlight sysfs inotify, Hyprland `activelayout` event; bottom-centre, 1.5 s auto-hide, never focusable; media/brightness keys bound in our Hyprland snippet

**UI**
- Top bar states: normal, busy, needs you, offline (priyad down: ring shows "Priya isn't running"), battery saver, no battery (desktop), multi-monitor, many tray items (overflow), long SSID, Hindi clock/locale.
- OSD states: volume (0, mid, 100, muted, over-amplified), brightness, mic muted/unmuted, layout name.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced motion; never colour alone (ring has count text).
- Keyboard: no dedicated keybind; the bar command "focus top bar" moves focus there, then Tab/arrows move through items and Enter opens popovers; every popover action is also reachable from the bar or Settings. Screen-reader names "Priya: 2 running, 1 ready for review", "Battery 64 %, charging".
- 👤 Mockup approval (`ui/mockups/5.5/`) before building; gallery sign-off after.

**Tests**
- Unit: status-ring reduction from task events, budget colour thresholds, Waybar JSON.
- Integration (headless Hyprland, fake D-Bus services for NM/UPower, PipeWire null sink): every indicator reflects fake state changes; OSD appears on volume change and hides after 1.5 s.
- UI: component tests; screenshots per state × theme × scale; a11y lint; journey "task needs approval → dot appears → click → Task center".
- Perf: top bar idle CPU ≈ 0 (< 0.2 % over 60 s); OSD shows within 50 ms of the change event.
- Regression: ring reduction, OSD timing, journey, screenshots.

**Done when**
- [ ] Top bar replaces Waybar on the Zenbook for daily use (or runs beside it by choice)
- [ ] Needs-you dot appears within 1 s of an approval request
- [ ] 👤 Gallery signed off

---

### 5.6 — Notification centre

**Outcome:** notifications appear top-right (max three), grouped by task, with buttons (Review, Allow, Deny, Keep); a notification centre keeps history; Do Not Disturb and quiet hours keep the desktop calm.
**Features:** UX-09, UX-13.
**Depends on:** 3.9, 4.13.

**Tasks**
- [ ] Quickshell `NotificationServer` as the `org.freedesktop.Notifications` owner (replaces mako/dunst when `priya desktop install` enables it; 5.14); all apps' notifications, not only Priya's
- [ ] Popup stack: max 3 visible, "+N more", grouped by task (`x-priya-task` hint) or app, inline actions, replace-id updates (progress → done), never takes keyboard focus (layer-shell keyboard interactivity none)
- [ ] Approval actions call `v1.approval.decide`; `confirm`-tier approvals open the typed confirmation instead of deciding inline
- [ ] Centre panel (`ui/shell/NotificationCentre/`): history grouped by task/app, clear, open source, approvals pinned on top
- [ ] Do Not Disturb toggle and quiet hours (`notifications.quiet_hours`): popups suppressed; approvals become the top-bar needs-you dot; kill switch and failed system merges always show
- [ ] Reminder after 10 minutes for unanswered approvals (11 §5); unanswered approvals pause the task, never auto-allow
- [ ] `Notifier` desktop adapter extended with actions, grouping and replace-id; contract suite updated

**UI**
- Popup states: single, grouped, actionable, progress, approval (ask), approval (confirm → opens dialog), expired approval, overflow, DND (none shown), Hindi text, long body (3 lines + "More").
- Centre states: empty, list, grouped, DND on, quiet hours active.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced motion.
- Keyboard: Super+N opens the centre (bind checked for conflicts in 5.14); in the centre, arrows move, Enter activates the default action, Delete dismisses; popups are mouse-only by design (no focus stealing) and every popup action is also in the centre. Screen-reader announces new popups politely; approvals assertively.
- 👤 Mockup approval (`ui/mockups/5.6/`) before building; gallery sign-off after.

**Human in the loop:** approvals reach the user wherever they are; DND never hides a pending approval (it moves to the top bar); nothing is ever auto-allowed.

**Tests**
- Unit: grouping, overflow, DND/quiet-hours decision table, reminder timer.
- Contract: `Notifier` suite (actions, replace-id, grouping) against the Quickshell server and a mock bus.
- Integration (headless Hyprland): `notify-send` from another app appears; approval action decides the approval; focus stays in the focused window while popups appear.
- UI: screenshots per state × theme × scale; a11y lint; journey "task ready → notification → Review → Task center".
- Regression: focus-stealing test, approval action, DND table, journey.

**Done when**
- [ ] A task can be approved, reviewed and kept starting from a notification
- [ ] Typing in kitty is never interrupted by a popup (asserted keystroke-by-keystroke)
- [ ] 👤 Gallery signed off

---

### 5.7 — Agent workspace and review comments

**Outcome:** Super+P shows every running agent as a tile (goal, agent, model, last activity, progress, cost) with Pause, Stop and Steer; in the review, the owner comments on diff lines and the agent revises, producing a new revision to review.
**Features:** ORC-24, ORC-23.
**Depends on:** 4.9, 4.13.

**Tasks**
- [ ] Hyprland special workspace `priya` toggled by Super+P (07 §5) hosting `ui/shell/AgentWorkspace/`: grid of run tiles (agent glyph and hue, model, goal, last activity lines from normalised events — never raw ANSI —, step/elapsed, cost, mode chip) with Pause / Stop / Steer (inline input) / Open in Task center
- [ ] Expanded tile: live activity stream with tool calls collapsed and a raw-log toggle
- [ ] Finished runs stay 30 s as "Ready for review", then leave; stalled runs show Nudge / Stop
- [ ] Review comments: select one or more diff lines in the Task center Changes tab → comment → "Ask the agent to revise" sends all open comments; new API `v1.change.comment {changeset_id, path, lines, text}` and `v1.change.request_revision {changeset_id}`
- [ ] Revision run: same workspace (fork still unmerged, or live for direct mode), same agent and model by default, comments in the prompt with file/line anchors; result is a new ChangeSet with `revision_of` (extend the ChangeSet model in 03 §5 in the same PR); review shows "Revision 2", diff against revision 1 and against base; comments marked resolved/unresolved
- [ ] Only unmerged sandbox ChangeSets and not-yet-kept direct ChangeSets accept revision requests; merged ones offer "Start a follow-up task"

**UI**
- Agent workspace states: empty ("Nothing running. Agents you start show up here."), one run, several, many (> 6: compact tiles, scroll), paused, stuck, finished, offline.
- Review comment states: selecting lines, drafting, several open comments, revision requested (progress), revision ready, comment resolved, revision failed.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; reduced motion.
- Keyboard: arrows move between tiles, Enter expands, P pauses, X stops, S steers; in diffs, Shift+arrows select lines, C comments, Ctrl+Enter requests revision. Screen-reader: "Run: add health endpoint, Claude Code with Sonnet, running 3 min, 12 cents".
- 👤 Mockup approval (`ui/mockups/5.7/`) before building; gallery and journey sign-off after.

**Human in the loop:** the user watches without being interrupted, can stop or steer any run, and keeps full control of revisions: nothing is merged until they press Keep on the revision they want.

**Tests**
- Unit: tile reduction from events, comment anchoring after line shifts, revision eligibility rules.
- Integration (fake machine): fake agent ChangeSet → two comments → revision run receives both anchors → ChangeSet with `revision_of` → diff vs revision 1 correct.
- UI: screenshots per state × theme × scale; a11y lint; journeys "Super+P → steer a run" and "comment → revise → keep revision 2".
- Perf: 6 live tiles at < 5 % CPU on the Zenbook.
- Regression: revision flow, both journeys.

**Done when**
- [ ] A real Claude Code run is revised from two diff comments and the revision kept
- [ ] Super+P never takes focus from other workspaces unless the user pressed it
- [ ] 👤 Gallery and journey signed off

---

### 5.8 — Settings app complete

**Outcome:** one Settings app with every page from 11 §4, search across all settings, "Tell Priya what to change" at the top, Undo on every change and clear admin locks.
**Features:** UX-11.
**Depends on:** 1.16, 3.8, 4.7.

**Tasks**
- [ ] `ui/apps/Settings/` shell: sidebar (General, Appearance, AI & Models, Agents, Autonomy & Safety, Privacy, Memory, Voice, Vision, Automations, Devices, Updates, About), search, "Tell Priya what to change" input (wired in 5.9), deep links `priya://settings/<page>#<key>`
- [ ] Pages for features not built yet (Memory, Voice, Vision, Automations, Devices, Updates) appear only when their module is installed, contributed through the module manifest (`settings_schema` + a `settings_page` surface); document this in 02 §5.1
- [ ] Schema-driven section renderer: JSON Schema + `x-priya-ui` annotations (title, description, group, order, widget, advanced, synonyms) → switch, segmented control, select, slider/stepper, text, path picker, list editor; hand-designed pages compose generated sections
- [ ] General page sections: Desktop (top bar options, layouts list from 5.12), Notifications (DND, quiet hours), Keyboard shortcuts (read-only list with conflicts from 5.14)
- [ ] Search over titles, descriptions and synonyms, jumping to the key with a highlight
- [ ] Undo toast for every change (inverse patch applied as a new `setting.changed`; full settings Rewind is Phase 7)
- [ ] Admin locks: values set in `/etc/priya` with a lock marker show a lock icon and "Set by your administrator"; define the lock marker in 02 §6.1
- [ ] About page: version, priyad and helper status, modules, licences, `priya doctor` summary; "Ask Priya about this" input on every page

**UI**
- Every page: default, edited (toast with Undo), invalid (inline plain-language error), locked by admin, loading, module crashed (page shows "This part of Priya stopped. Restart it"), search with no results, long values, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; font scale 150 %.
- Keyboard: Ctrl+F search, Up/Down sidebar, Tab into controls, Ctrl+Z undo last change; screen-reader names from schema titles and descriptions.
- 👤 Mockup approval (`ui/mockups/5.8/`, every page) before building; gallery sign-off after.

**Human in the loop:** every change is visible, immediate and undoable; locked settings explain who set them.

**Tests**
- Unit: schema → control mapping table, search ranking, inverse-patch generation, lock resolution across layers.
- Contract: settings schema snapshots including `x-priya-ui` annotations.
- Integration: module providing a settings page appears/disappears with the module; change → file written → hot reload → Undo restores.
- UI: screenshots every page × state × theme × scale; a11y lint; keyboard-only journey "search 'quiet' → set quiet hours → undo".
- Regression: mapping table, module page lifecycle, journey.

**Done when**
- [ ] Every settings key reachable from search
- [ ] Every page passes screenshot and a11y tests; 👤 gallery signed off
- [ ] No page exists for an uninstalled module

---

### 5.9 — Natural-language settings

**Outcome:** "make answers shorter", "always do note edits directly", "only use local models for ~/Documents", "just do it for file edits" each become a validated change the owner confirms with one key, with Undo.
**Features:** CORE-08.
**Depends on:** 5.8, 5.1, 5.4.

**Tasks**
- [ ] `v1.settings.nl_change` per 02 §6.2: retrieve the relevant schema subset (keyword + embedding over titles/synonyms), current values, Priya AI role `smart` (local when values contain private paths) → JSON Patch
- [ ] Validation: schema, Cedar `ChangeSetting`, admin locks; invalid patch → plain explanation, never applied
- [ ] Confirm card: plain sentence + before/after per key ("Answer length: normal → brief"); Enter confirms; applied as `setting.changed {source: natural-language}` and recorded as a ChangeSet of kind `settings`
- [ ] Mode rules: inserts/edits `[[rule]]` entries in `modes.toml` (03 §1.1), respecting `lock`
- [ ] Privacy routing ("only local models for ~/Documents"): a settings key (`privacy.rules`) that `priya-policy` compiles to Cedar `UseModel` forbids, so this works before the Phase 14 policy builder — record as ADR; requests that need real policy authoring answer "That's a security policy; it comes with the policy builder"
- [ ] Autonomy per feature ("just do it for file edits") → 3.8 keys
- [ ] Entry points: bar (`settings` class), Settings input, `priya settings change "<text>"`
- [ ] NL-settings evaluation set: 120 phrase → expected patch pairs

**UI**
- Confirm card states: proposal, several keys, locked key (explained), invalid request, ambiguous (clarification card), applied (Undo), undone.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; Enter confirm, Esc cancel, E "edit in Settings" deep link; screen-reader reads the before/after list.
- 👤 Mockup approval with 5.4; gallery sign-off.

**Human in the loop:** nothing changes without confirmation; every applied change has Undo and appears in the settings history.

**Tests**
- Unit: patch validation (schema, locks, policy), modes-rule insertion, privacy-rule compilation to Cedar.
- Integration: "only use local models for ~/Documents" → task reading a Documents file routes to a local model (asserted on `model.called`).
- Eval: exact-patch accuracy ≥ 90 %; invalid patches applied = 0 (hard requirement).
- UI: confirm card screenshots; journey "bar → 'make answers shorter' → confirm → undo".
- Regression: zero-invalid-apply, privacy routing end to end, journey.

**Done when**
- [ ] The four example phrases work from the bar and from Settings
- [ ] Eval thresholds met; zero invalid patches applied
- [ ] ADR for `privacy.rules` accepted

---

### 5.10 — Natural-language shell

**Outcome:** in `priya sh` the owner types `ls -la` and it runs normally, types "find big files in Downloads" and gets a proposed command to confirm, and types `??` after a failure to have it explained.
**Features:** ADM-02.
**Depends on:** 5.1, 5.4, 3.3.

**Tasks**
- [ ] `priya sh` in `priya-cli`: runs the user's `$SHELL` in a pty and passes everything through; command boundaries and exit codes from OSC 133 shell-integration marks (kitty-compatible)
- [ ] Line classification before sending to the shell: `!` prefix = always a command, `?` prefix = always plain language, otherwise a deterministic check (first word is a builtin, alias, function or executable on PATH and the line parses) = command; anything else = plain language
- [ ] Plain language → Priya AI proposes command(s) with a one-line explanation, risk and mode (resolved by 3.3) → Command card rendered in the terminal (ratatui, tokens from the kitty theme) → Enter run, E edit, M change mode, Esc cancel
- [ ] Execution of proposed commands follows mode resolution: read-only → read-only run; sandbox → task with review in the Task center; direct → safety snapshot first. Commands the user typed always run normally in their shell (UX-14)
- [ ] `??` explains the last failed command using its captured output (UX-07); suggestions are proposed commands, never auto-run
- [ ] Proposed and confirmed commands are written to the shell history as the real command
- [ ] Offline: plain-language lines answer "Priya AI is offline — type commands normally" unless a local model is available

**UI** (terminal)
- States: proposal, editing, running, sandbox task started (link to Task center), denied by policy, failed with explanation, offline, Hindi input.
- Readable in light, dark, OLED-black and high-contrast kitty themes from 5.11; no colour-only meaning (mode shown as a word).
- 👤 Mockup approval (`ui/mockups/5.10/`, terminal screenshots) before building; recorded session sign-off after.

**Human in the loop:** plain language never runs anything without Enter on the proposed command; the mode is visible and changeable; typed commands are untouched.

**Tests**
- Unit: line classifier table (builtins, aliases, paths, quoting, Hindi), OSC 133 parser, history writing.
- Integration (fake machine): "delete all .tmp files in ~/x" → proposal in Sandbox → task → review; `ls` passes through byte-exact; `vim` (full-screen app) works through the pty.
- E2E (VM): scripted `priya sh` session over a pty.
- Regression: classifier table, pass-through byte-exactness, full-screen app test.

**Done when**
- [ ] Interactive programs (vim, htop, ssh) work unchanged inside `priya sh`
- [ ] Zero plain-language lines executed without confirmation (asserted over the classifier set)
- [ ] 👤 Recorded session signed off

---

### 5.11 — Whole-desktop theming, OLED black, high contrast, accents, auto schedule

**Outcome:** one choice in Appearance (or "switch to dark", "use the teal accent") re-themes the Priya UI, GTK and Qt apps, kitty, Hyprland borders, cursor, icons and fonts live; Auto switches at sunrise and sunset; OLED black and high-contrast themes are available.
**Features:** DS-12, DS-13, DS-14, DS-15, DS-16, DS-22.
**Depends on:** 0.10, 5.8.

**Tasks**
- [ ] Fix tokens before adding themes: the CI contrast checker currently fails `text.tertiary` (light 3.27:1 on `bg.base`; dark 4.05:1) and white `on.accent` on the light Marigold accent (3.99:1); adjust values or restrict usage to large text, and update the 11 §2.1 table
- [ ] Token variants in `ui/design/tokens.json`: light, dark, OLED black, high-contrast light, high-contrast dark (all text ≥ 7:1); accents Marigold, Indigo, Teal, Rose, Leaf, Sky with light/dark values; checker runs over all 5 themes × 6 accents; Leaf/Sky must stay distinguishable from Sandbox (`success`) and Look (`info`) colours, which also carry icon + word
- [ ] Generators in `ui/design/generate/` per 11 §3: `Theme.qml`; GTK (adw-gtk3 + generated `gtk.css` for GTK 3 and 4, `color-scheme` via gsettings and the portal); Qt (qt6ct colour scheme + config, `QT_QPA_PLATFORMTHEME` in our Hyprland snippet); kitty (included theme file + SIGUSR1); Hyprland (sourced `priya-theme.conf`, applied with `hyprctl keyword` to avoid a full reload); cursor (Bibata via `hyprctl setcursor` + gsettings); icons (Papirus with accent folder colours in a user-level icon theme, no root); fonts (fontconfig `60-priya.conf`: Inter, JetBrains Mono, Noto Sans Devanagari)
- [ ] `priya theme apply [--theme …] [--accent …]`, `priya theme auto on|off|schedule`, `priya theme status`; files written atomically and recorded as one ChangeSet so Undo/Rewind can restore the previous look; dependency check lists missing packages (Bibata is AUR) with an install offer
- [ ] Auto schedule (DS-12): sunrise/sunset computed locally (no network) from the timezone's representative coordinates or manual latitude/longitude, or fixed times; 400 ms crossfade in Quickshell; apps follow through the portal
- [ ] OLED black (DS-13): true-black surfaces, bar and panels dim after inactivity
- [ ] Battery saver turns blur off (11 §2.3)
- [ ] Appearance page: theme tiles with live previews, accent swatches, Auto schedule controls, "Apply to apps" status per target with plain errors ("kitty didn't reload — restart kitty")
- [ ] Backfill OLED-black and high-contrast screenshot baselines for every Phase 0–4 screen
- [ ] OLED burn-in protection: static shell elements dim after 2 min of inactivity and shift by 1 px every 10 min on OLED outputs (detected via EDID or set by the user)

**UI**
- Appearance states: each theme selected, each accent selected, auto by sun, auto by schedule, applying, partial failure, missing package, apps that ignore themes (listed, e.g. some Electron apps), Hindi text.
- Every theme × accent in the gallery; scale 1.0 and 2.0.
- Keyboard: arrows across theme tiles and swatches, Enter applies; screen-reader names "Theme: Dark, OLED black, selected", "Accent: Teal".
- 👤 Mockup approval (`ui/mockups/5.11/`) before building; whole-desktop screenshot set (bar, Task center, Firefox, a GTK app, a Qt app, kitty) signed off after.

**Human in the loop:** every theme change applies instantly and can be undone; Auto can be turned off at any time.

**Tests**
- Unit: generator goldens per target × theme × accent, sunrise/sunset against reference tables, schedule transitions.
- Contract: contrast checker over all theme × accent pairs (fails CI below AA; HC below 7:1).
- Integration (headless Hyprland): apply dark → GTK test app, Qt test app and kitty colours change (pixel sampling); Undo restores files byte-exact.
- UI: screenshots for every screen in all five themes (backfill included); reduced-transparency variants.
- Perf: full apply < 1 s; crossfade without dropped frames.
- Regression: generator goldens, contrast checker, apply/undo, sun schedule.

**Done when**
- [ ] One click switches every listed target live on the Zenbook
- [ ] Contrast checker green for 30 theme × accent combinations
- [ ] Auto switches at the computed sunset on the Zenbook; 👤 desktop screenshot set signed off

---

### 5.12 — Workspace layouts

**Outcome:** "set up my api project" opens the editor, a terminal in the project and the browser on the dev server on the right workspaces; "save this layout as writing" saves the current arrangement; layouts restore without duplicating windows.
**Features:** UX-12.
**Depends on:** 5.2.

**Tasks**
- [ ] Layout file `~/.config/priya/layouts/<name>.toml`: windows `{app, args, cwd, url, workspace, monitor, floating, size}`, optional `run` from the project's `.priya.toml` (dev server), project binding
- [ ] Capture: `priya layout save <name>` from `hyprctl clients -j` + `/proc` cwd; browser URLs asked for or left blank (no browser scraping)
- [ ] Restore: reuse matching open windows; otherwise `exec [workspace N silent]`, wait for the window by class/pid (timeout), move/resize with dispatchers; user-invoked restores focus the target workspace once, automation-invoked restores stay silent (UX-13)
- [ ] Plain language: project with a saved layout → restore; without → propose one from the project type (editor + kitty at the path + browser at the `run` port) as a card → Start → offer to save
- [ ] Undo: "Close what I opened" closes only windows the layout launched
- [ ] Layouts in bar local results and in Settings → General → Desktop; CLI `priya layout list|save|restore|delete`

**UI**
- Proposal card, restoring (progress per window), restored (Undo), partial (one app missing → "Install it?"), saved, list in Settings (empty, list, rename, delete).
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names for list actions.
- 👤 Mockup approval with 5.4; gallery sign-off.

**Tests**
- Unit: layout schema, window matching, proposal from project type.
- Integration (headless Hyprland): restore opens three windows on the right workspaces; second restore opens none; Undo closes only launched windows.
- E2E (VM): "set up my api project" from the bar.
- Regression: idempotent restore, undo scope.

**Done when**
- [ ] Restore is idempotent and completes in < 3 s for three windows on the Zenbook
- [ ] Automation-invoked restore never moves focus

---

### 5.13 — Accessibility, motion and performance pass

**Outcome:** everything is usable with the keyboard alone, with Orca, at 200 % text, with reduced motion and transparency; the bar opens in under 100 ms and the desktop idles at near-zero CPU.
**Features:** DS-17, DS-18, UX-13, UX-14.
**Depends on:** 5.3–5.12.

**Tasks**
- [ ] Motion system (DS-17): duration and easing tokens in `Theme.qml`, `PriyaUI/Motion` helpers, thinking shimmer, no layout jumps while streaming; reduced motion (Priya setting + GTK `enable-animations`) replaces movement with fades or nothing; reduced transparency makes surfaces opaque and turns blur off
- [ ] Font scaling 80–200 % (`ui.font_scale`) without clipping on every screen
- [ ] Orca journeys (DS-18): verify Quickshell layer-shell and floating windows expose AT-SPI; fix gaps (report upstream if needed); scripted AT-SPI journeys assert names, roles, focus order and announcements for bar, Task center, Settings, notifications, agent workspace
- [ ] Focus-order audit and visible 2 px accent focus ring everywhere; never colour alone (modes and risks have icon + word)
- [ ] Focus-stealing audit (UX-13): every surface checked against "background work never takes focus"; automated keystroke test while tasks start, finish and notify
- [ ] Normal-input audit (UX-14): Priya keybinds never intercept keys when surfaces are closed; with priyad stopped, keyboard, mouse, terminal and Hyprland binds keep working and the bar shows offline
- [ ] Performance: profile with the QML profiler; budgets on the Zenbook runner — bar open p95 < 100 ms, local results < 50 ms, Task center open < 300 ms, Settings open < 300 ms, theme apply < 1 s, idle Quickshell CPU < 0.5 % over 60 s, Quickshell RSS < 150 MiB; add the new budgets to 09 §2

**UI**
- Screenshot variants: reduced motion, reduced transparency, font 150 % and 200 % for every screen in light and dark (plus OLED/high-contrast for key screens).
- 👤 Owner tries the Orca journey and the 200 % font journey on the Zenbook.

**Tests**
- Unit: motion token mapping, reduced-motion switches.
- UI: a11y lint clean everywhere; AT-SPI journey suite; font-scale screenshots; keyboard-only journeys for every app.
- Integration: focus-stealing and stopped-priyad tests on headless Hyprland.
- Perf: all budgets above in the nightly `perf` suite.
- Regression: AT-SPI journeys, focus tests, perf budgets.

**Done when**
- [ ] Every screen passes a11y lint and its AT-SPI journey
- [ ] All perf budgets met for three nightly runs
- [ ] 👤 Owner completes the Orca and 200 % journeys

---

### 5.14 — `priya desktop install`

**Outcome:** on any Hyprland Arch — including Omarchy — one command adds the Priya bar, top bar, notifications, OSDs, agent workspace, keybinds and theming, shows exactly what will change first, and can be undone with `priya desktop uninstall`.
**Features:** UX-14 (integration never breaks the user's existing setup).
**Depends on:** 5.3, 5.5, 5.6, 5.11.

**Tasks**
- [ ] Environment detection: Hyprland version and config path, Omarchy (its install directory and config layout), running bar (Waybar), notification daemon (mako, dunst, swaync), launcher (walker, wofi, rofi), existing binds via `hyprctl binds -j`
- [ ] Plan card / CLI summary of every change before applying; applied as a direct-mode task with a safety snapshot so it is a reviewable, revertable ChangeSet
- [ ] Hyprland: one `source = /usr/share/priya/desktop/hyprland.conf` line in the user's config (Omarchy: in its user override file, after Omarchy's own sources); `exec-once` for the Quickshell shell
- [ ] Choices with sensible defaults: replace or keep Waybar (keep → add `priya status --waybar`), switch notifications to Priya's server or keep the existing daemon, theming follows PriyaOS or leaves the current theme
- [ ] Keybind conflict table (e.g. a launcher on Super+Space): user picks keep theirs / use Priya's / alternative key; result written to `~/.config/priya/keybinds.toml` and the generated snippet
- [ ] Idempotent re-run; `priya desktop status`; `priya desktop uninstall` reverts the ChangeSet (three-way, keeps later user edits); `priya doctor` desktop checks
- [ ] Human guide section in `docs/human/07-install-on-your-zenbook.md`

**UI**
- Install plan card and CLI output: plan, conflicts to resolve, applying, done (with "Press Super+Space"), partial failure, already installed, uninstall plan.
- Light and dark at minimum (installs before theming is chosen), OLED-black and high-contrast when active; keyboard path for conflict choices; screen-reader names.
- 👤 Mockup approval with 5.4; sign-off after a recorded Omarchy install.

**Human in the loop:** nothing is changed before the user sees the plan; every conflict is the user's choice; uninstall restores their previous setup.

**Tests**
- Unit: config editing on fixture configs (vanilla Hyprland default, Omarchy snapshot, heavily customised with conflicts), conflict detection, idempotency.
- Integration (fake home dirs on headless Hyprland): install → Super+Space opens the bar; re-run changes nothing; uninstall restores byte-exact when untouched and keeps later user edits.
- E2E (VM): vanilla Arch + Hyprland and an Omarchy VM, install → journey → uninstall.
- Regression: fixture configs, idempotency, uninstall, Omarchy VM journey.

**Done when**
- [ ] Installs and uninstalls cleanly on vanilla Hyprland and Omarchy VMs
- [ ] Zero overwritten user keybinds without an explicit choice
- [ ] Owner installs it on the Zenbook

---

### 5.15 — Phase gate (owner usability session)

**Outcome:** the owner, without help, completes a scripted session that covers the whole desktop and rates it.
**Features:** all Phase 5 features, QA-22.
**Depends on:** 5.1–5.14.

**Tasks**
- [ ] Session script (`docs/claude/phases/usability/phase-05.md`): (1) ask "why is my fan loud?"; (2) open Firefox from the bar; (3) "turn on bluetooth" then undo; (4) "set up my api project"; (5) "install btop" → approve from a notification → review → keep; (6) watch a task in Super+P and steer it; (7) comment on a diff and keep the revision; (8) "make answers shorter"; (9) switch to OLED black with the teal accent, then Auto; (10) in `priya sh`, "find files over 1 GB in Downloads"; (11) kill switch and resume; (12) do (2) and (5) with keyboard only
- [ ] Measures: completion without help per task, time per task, errors, System Usability Scale score; recorded screen session
- [ ] Every blocker found is fixed (with a regression test) before the gate; non-blockers become issues
- [ ] Docs updated (07, 11, 02, 03 changes from this phase); feature statuses; `docs/human/05-the-phases.md`
- [ ] Reusable usability-session kit: task script, observation sheet, and System Usability Scale (SUS) questionnaire under `docs/claude/usability/`; target SUS ≥ 80 for this gate; results recorded

**Done when**
- [ ] 11 of 12 session tasks completed without help; SUS ≥ 80
- [ ] All blockers fixed with tests
- [ ] 👤 Owner sign-off

## Phase gate

- [ ] Whole regression suite green on `main` (unit, contract, integration, UI including all five themes); nightly e2e, perf, chaos, security, eval and live green
- [ ] Owner usability session (5.15) passed and signed off 👤
- [ ] Docs match the code (07, 11 card list and tokens, 02 API/settings/manifest additions, 03 `revision_of`, 09 perf budgets, new ADRs); feature statuses updated in [features.md](../features.md)
- [ ] `docs/human/05-the-phases.md` status updated

## Out of scope

- Voice and the bar's listening state (Phase 10); screen and image understanding beyond attaching files to a vision model (Phase 11)
- Desktop restyling by plain language with preview on a nested Hyprland, generated mini-apps (Phase 16: UX-15, UX-16); wallpaper-derived accent and theme sharing (Phase 16: DS-21)
- Network, Bluetooth, audio, power and display panels, login/lock/boot styling, onboarding (Phase 9)
- Multi-agent views, scoreboards and plan DAG UI (Phase 12)
- Morning brief in the clock popover (Phase 13); Rewind UI and settings history beyond the Undo toast (Phase 7)
- Hindi UI translation (Phase 10: DS-20) — Hindi *text* must still render correctly everywhere now
- Full Omarchy layer distribution (Phase 16: ECO-07)
