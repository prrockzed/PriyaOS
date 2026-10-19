# Phase 5 — Natural language and desktop

**Goal.** Make PriyaOS feel like part of the computer: the Priya bar (Super+Space) where you type anything, plain-language answers with cards, notifications with actions, a top-bar status item, graphical review and settings panels, natural-language settings and a natural-language shell, and a workspace to watch agents.

**Depends on:** Phase 4.
**Features:** CORE-08, ADM-02, ORC-23, ORC-24, UX-02 … UX-14.
**Read first:** [07-interfaces](../architecture/07-interfaces.md), [02 §6.2](../architecture/02-core.md), ADR-009, ADR-011.

## Build

### 5.1 Intent pipeline
- [ ] `v1.intent.submit` with context capture (focused window via Hyprland IPC, kitty cwd, project, selection on request)
- [ ] Classification into answer / action / settings / navigation / mixed; explicit mode phrases ("do it directly", "just look")
- [ ] Navigation intents mapped to direct methods (open app/file/folder, toggle Wi-Fi/Bluetooth, brightness, volume, workspace layouts) — no agent

### 5.2 Priya bar (Quickshell)
- [ ] Overlay on Super+Space: input, streaming answer, cards (diff, plan, approval, command, undo, file list), recent tasks, model/cost badge
- [ ] Keyboard-first; Esc closes; never steals focus from running work

### 5.3 Notifications, top bar, agent workspace
- [ ] `priya-notify` user service: review ready / failed / needs approval / restart needed, with action buttons
- [ ] Waybar item (`priya status --waybar`) and a Quickshell bar item: running count, review count, failures, today's cost
- [ ] Special workspace `priya` with live run views (Super+P)
- [ ] Kill switch keybind (Super+Shift+Escape)

### 5.4 Review and settings GUI
- [ ] Quickshell review panel (same data as the TUI), inline comments on diffs sent back to the agent as a revision request (ORC-23)
- [ ] Settings panel generated from JSON Schemas; models/providers page; modes page; privacy page

### 5.5 Natural-language settings and shell
- [ ] `v1.settings.nl_change` (patch → validate → plain-language confirm → apply → `setting.changed`)
- [ ] `priya sh`: a shell where lines in plain language become proposed commands (shown, confirmed, run in the chosen mode) and normal commands run normally

### 5.6 Desktop integration
- [ ] `priya desktop install` (sources our Hyprland snippet, adds bar items; Omarchy-aware)
- [ ] Workspace arrangement intents ("set up my api project": open editor, terminal at path, browser on localhost) and saved layouts

## Tests

- L0: intent classification fixtures, NL settings patch validation, navigation intent mapping.
- L1: Quickshell components tested with a mock priyad (snapshot tests of rendered QML where feasible).
- L3 (VM with Hyprland): scripted end-to-end through the socket (submit → notifications → review → merge); manual UI checklist.
- L4 (Zenbook if dogfooding): daily-use checklist.

## Done when

- [ ] From Super+Space the owner can: ask a question, get an answer with the commands used; start a task, approve, review, merge from a notification; change a setting by plain language; open a project layout
- [ ] Background work never steals focus; keyboard/mouse/terminal unaffected

**Demo.** Super+Space → "why is my fan loud?" (answer) → "install btop and set up my api project workspace" (task, notification, review, merge) → "make answers shorter" (setting).
