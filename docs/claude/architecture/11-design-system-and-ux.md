# 11 — Design system and UX

PriyaOS must look calm and beautiful, feel instant, and be obvious to use for someone who has never seen a terminal. The backend and the AI are only half the product; this file is the other half.

## 1. UX principles

| Principle | Means in practice |
|---|---|
| **One place to ask** | The Priya bar (Super+Space) does everything: questions, actions, settings, app launching, undo. Every other screen also has an "Ask Priya about this" input. |
| **Show, then do** | Before anything changes, Priya shows what it's about to do in one plain sentence plus a mode chip the user can change. How often it asks is the user's **autonomy level** (§5). |
| **Always undoable, always stoppable** | Every result card has Undo. Esc stops what's running. The kill switch is always one shortcut away. |
| **Plain first, details on demand** | Plain-language summary on top; commands, diffs, logs, model choice behind "Details". Never jargon in a headline. |
| **Honest** | Show cost, model used, risk, and whether something can be undone. Never fake progress. |
| **Calm** | Background work never steals focus. At most 3 notifications on screen. No modal traps. Quiet by default. |
| **Fast** | Bar opens in < 100 ms; local results (apps, files, history) appear while the AI is still thinking. |
| **Keyboard-first, mouse-friendly, voice-ready** | Every action has a key; every screen works with mouse only; every action can be spoken (Phase 10). |
| **Consistent** | One component library, one token set, one way of showing tasks, changes, approvals and errors everywhere. |

## 2. Visual language

### 2.1 Colour tokens

A single token file `ui/design/tokens.json` is the only place colours, type, spacing, radius, motion and elevation are defined. Values below are starting points; the CI contrast checker (§9) is authoritative and may adjust them.

| Token | Light | Dark | Dark (OLED black) |
|---|---|---|---|
| `bg.base` | `#FAFAF7` | `#121214` | `#000000` |
| `bg.surface1` (cards) | `#FFFFFF` | `#1A1A1F` | `#0B0B0D` |
| `bg.surface2` (raised) | `#F3F3EF` | `#222229` | `#151518` |
| `bg.surface3` (hover) | `#EAEAE4` | `#2B2B33` | `#1E1E23` |
| `border.subtle` (dividers only) | `#DCDCD4` | `#34343D` | `#26262C` |
| `border.strong` (input and control outlines, ≥ 3:1) | `#8C8C84` | `#6E6E7A` | `#5E5E6A` |
| `text.primary` | `#18181B` | `#EDEDF0` | `#EDEDF0` |
| `text.secondary` | `#55555E` | `#A9A9B4` | `#A9A9B4` |
| `text.tertiary` | `#6B6B74` | `#9A9AA5` | `#9A9AA5` |
| `accent` (Marigold, default) | `#9A5B00` | `#F2B544` | `#F2B544` |
| `accent.subtle` | `#FFF1D6` | `#3A2C10` | `#2A1F08` |
| `on.accent` | `#FFFFFF` | `#1A1300` | `#1A1300` |
| `success` | `#1E7F4F` | `#3FB97A` | `#3FB97A` |
| `warning` | `#B35400` | `#F08A24` | `#F08A24` |
| `danger` | `#C42B1C` | `#F0564A` | `#F0564A` |
| `info` | `#1F6FB8` | `#4AA3F0` | `#4AA3F0` |

- **Accent options** (user choice): Marigold (default), Indigo, Rose, Plum, Slate. Each defines light and dark values that pass contrast. Green and blue accents are deliberately not offered because they would be confused with the Sandbox (`success`) and Look (`info`) mode colours. Wallpaper-derived accent comes later (Phase 16).
- **Mode colours** are always the same meaning: Look (read-only) = `info`, Sandbox = `success`, Direct = `warning`, Direct raw = `danger`. Untrusted label = `danger` outline.
- **Agent families** get distinct neutral hues and generic glyphs with a text label (no company logos): e.g. Claude Code = clay, Codex = slate, Gemini CLI = azure, Grok = graphite, OpenCode = moss, Priya native agent = accent.

### 2.2 Typography

- UI: **Inter** (variable). Code: **JetBrains Mono**. Hindi and other Indic scripts: **Noto Sans Devanagari** (fallback chain configured in fontconfig).
- Scale (px at 1.0): 12 caption · 13 small · 14 body (base) · 16 body-large · 20 title · 24 headline · 32 display. Weights 400 / 500 / 600. Line height 1.45 body, 1.25 titles.
- Numbers in tables and costs use tabular figures.

### 2.3 Space, shape, depth

- Spacing on a 4 px grid: 2, 4, 8, 12, 16, 24, 32, 48.
- Radius: 6 controls · 10 cards · 16 panels and the Priya bar · full for pills and chips.
- Depth: light theme uses soft shadows; dark theme uses lighter surfaces plus a 1 px `border.subtle`. Optional background blur behind the bar and panels (Hyprland blur), automatically off on battery saver.

### 2.4 Motion

- Durations: 120 ms (micro: hover, press), 200 ms (standard: expand, fade), 320 ms (panels, bar open/close).
- Easing: standard `cubic-bezier(0.2, 0, 0, 1)`; emphasised `cubic-bezier(0.3, 0, 0, 1)` for entering panels.
- "Thinking" is a soft shimmer on the status line, never a spinning wheel. Streaming text appears word by word with no layout jumps (reserve space, grow smoothly).
- **Reduced motion** setting (and the system preference) replaces movement with fades or nothing.

### 2.5 Icons and imagery

- UI icons: **Lucide** (ISC licence), stroke 1.75, sizes 16 / 20 / 24.
- App icons: Papirus, folder colour follows the accent.
- The Priya mark: a simple rounded glyph whose subtle animation shows state (idle, listening, thinking, needs you, done, error).

## 3. Themes

The token file defines **all** theme variants (light, dark, OLED black, high contrast) from Phase 0. Screenshot baselines cover light and dark from Phase 0; sub-phase 5.11 adds OLED-black and high-contrast baselines for every existing component, and from then on every new component is tested in all four. Phase 5 also adds the user-facing choices (auto schedule, OLED, high contrast, accents) and applies them to the whole desktop.


| Theme | Details |
|---|---|
| Light | Default during the day |
| Dark | Default at night |
| Dark (OLED black) | True black backgrounds for OLED screens (the Zenbook 14 OLED); static elements (top bar, bar items) dim after inactivity and shift by a pixel periodically to avoid burn-in |
| High contrast | Light and dark variants; all text ≥ 7:1 |
| **Auto** | Switches at sunrise/sunset (location from timezone, or set manually) or on a fixed schedule; smooth 400 ms crossfade |

**Whole-desktop consistency**: `priya theme apply` generates every app's theme from the same tokens and reloads them live:

| Target | How |
|---|---|
| Priya UI (Quickshell) | Generated `Theme.qml` singleton; live binding, no restart |
| GTK 3/4 apps | adw-gtk3 + generated colour overrides; `color-scheme` set through gsettings / xdg-desktop-portal so apps follow light/dark |
| Qt apps | Generated qt6ct colour scheme (Fusion style) |
| kitty | Generated theme file + `SIGUSR1` reload |
| Hyprland | Border, shadow, blur colours; `hyprctl reload` |
| Cursor, icons, fonts | Cursor theme packaged in `[priyaos]` (Bibata rebuilt from source, since it is AUR-only in Arch); Papirus icons through a generated **user-level** icon theme that inherits Papirus and overrides folder colours with the accent (no root needed); fontconfig defaults |
| Login, lock, boot | greetd/regreet, hyprlock, GRUB theme, Plymouth from the same palette (Phase 9) |

## 4. Surfaces and screens

All built with Quickshell (QML) and the shared `PriyaUI` component library (ADR-009). Every screen lists its states; every state has screenshot tests in all themes.

| Surface | Anatomy | States | Phase |
|---|---|---|---|
| **Priya bar** | Top-centre overlay, 720 px (max 60 % width), radius 16. Input ("Ask or tell Priya anything…"), Priya mark on the left; on the right chips for **mode** (Auto / Look / Sandbox / Direct), **model** (Auto · Balanced), attach, mic. Results below: local matches (apps, files, recent tasks) instantly, then the streaming answer and cards. Footer key hints. | idle, typing, local results, thinking, streaming, needs approval, done, error, offline, listening (Phase 10) | 0 (minimal), 5 (full) |
| **Cards** | Answer · Command (copyable, with "ran in Look mode") · Plan preview · Progress (live steps of a running task) · Clarification (2–3 choices when Priya isn't sure what you meant) · Approval · ChangeSet summary · Diff · Undo · File list · Chart · Cost · Error with "Try again / Ask why" | each: compact, expanded, loading, error | 0–5 |
| **Task center** | Floating window. Left: groups **Needs you · Running · Ready for review · Done · Failed**. Right: task detail with tabs Overview (plain summary, step timeline), Changes, Checks, Activity (live, agent/tool events), Cost & models. Sticky actions: Keep / Discard / Revert / Ask about this. | empty, list, running, review, conflicts, failed | 4 |
| **Review** | Plain summary first ("Installed Docker and set it up for your api project"), then groups (Apps, Services, System files, Your files by project) with icons and counts; file diffs (unified or side-by-side, syntax highlighted); conflict resolution inline; untrusted banner; checks; comment on a line → agent revises | clean, conflicts, tainted, partial (budget stop) | 2 (TUI), 4 (GUI) |
| **Approval** | Verb-first title ("Restart nginx?"), why (agent's reason), exactly what (command or change), risk badge, reversibility line ("Undo with Rewind" / "Can't be undone: leaves your computer"), buttons **Allow once · Always allow here · Deny**, keys Y / A / N | pending, approved, denied, expired | 3 |
| **Approvals inbox** | List of pending approvals across tasks with batch actions | empty, list | 3 |
| **Top bar** | Floating 32 px bar: workspaces (left), clock (centre; click → calendar + morning brief), right: Priya status ring (running count, review badge, needs-you dot), optional cost today, network, audio, battery, tray | normal, busy, needs you, offline, battery saver | 5 |
| **Notifications** | Top-right stack, max 3 visible, grouped by task, inline actions; centre with history; Do Not Disturb and quiet hours | single, grouped, actionable, DND | 3 (minimal), 5 (full) |
| **Agent workspace** | Super+P special workspace: grid of run tiles (agent glyph, model, goal, last lines, progress, elapsed, cost) with Pause / Stop / Steer | empty, running, stuck, many runs | 5, 12 (multi-agent view) |
| **Settings** | Sidebar: General (incl. notifications, top bar, keybinds, layouts), Appearance, AI & Models, Agents, Autonomy & Safety, Privacy, Memory, Rewind, Voice, Vision, Automations, Devices; system pages Network, Bluetooth, Sound, Power, Displays, Keyboard & input, Date & time, Users, Printers, Backup, Firewall, Apps, Updates; About. Pages for modules that aren't installed are hidden. Search. "Tell Priya what to change" input at the top. Every change shows an Undo toast. | each page: default, edited, invalid, locked by admin | 1 (AI page), 3 (Safety), 5 (complete) |
| **Model picker** | From the bar's model chip: strategy (Best / Balanced / Cheapest / Local only), family filter, pin a model, "why this model?" | — | 1 |
| **Usage & cost** | Today / week / month, by model, family, agent, task; savings by technique; budgets | — | 1 |
| **Rewind** | Horizontal scrubbable timeline + list; filters (who, what, where); change detail; actions Undo this change · Restore file · Return system here | empty, list, detail, undo in progress | 7 |
| **Health** | Score ring + checklist with "Fix" buttons | healthy, warnings, critical | 6 |
| **Memory** | Projects, preferences, files index, each item viewable, editable, forgettable | — | 8 |
| **Onboarding** | Welcome & language · Wi-Fi · AI accounts · How careful should Priya be? · Privacy & folders · Appearance (light/dark/auto, accent) · Voice (optional) · Import from a previous install (optional) | each step: default, skipped, error | 9 |
| **Installer** | Graphical, same design language; disk picture showing Windows (locked) and the target | — | 9 |
| **Kill switch overlay** | Dimmed screen: "Stopped everything", list of what stopped, Resume buttons | — | 3 |
| **OSDs** | Volume, brightness, mic mute, keyboard layout | — | 5 |
| **Lock and login** | hyprlock and greeter styled from tokens | — | 9 |

## 4a. Keybinds

| Key | Action |
|---|---|
| Super+Space | Priya bar |
| Super+J | Task center ("jobs") |
| Super+P | Agent workspace |
| Super+V (hold) | Push-to-talk |
| Super+Shift+S | Priya Vision on a region |
| Super+Shift+Escape | Kill switch |

All keybinds are settings. `priya desktop install` detects conflicts with the existing Hyprland config (for example Omarchy binds Super+Space to its launcher) and asks which to keep, offering an alternative (e.g. Super+Alt+Space).

## 5. Human in the loop and autonomy

**Autonomy level** is a user setting, global and per feature (updates, file edits, installs, code, automations), shown as a three-step control in Settings and as a chip in the plan preview:

| Level | Behaviour |
|---|---|
| **Ask me first** | Plan preview for every task; approval for every change, even in the sandbox |
| **Ask when it matters** (default) | Plan preview only for multi-step, direct-mode, privileged, external or expensive work; approvals per risk tier |
| **Just do it** | No previews; still sandboxed by mode rules; still asks for external actions and `confirm`-tier actions (these can never be skipped) |

Autonomy maps onto approval tiers and mode rules ([05-security.md](05-security.md) §4, [03 §1.1](03-execution-and-sandbox.md)); it never overrides `forbid` policies.

Interaction rules:
- **Interrupt and steer anywhere**: Esc stops; typing while a task runs sends guidance to it ("use postgres instead").
- **Preview cards are editable in place**: change the mode chip, the agent/model chip, the budget, or say "do it differently".
- **Approvals arrive where the user is**: bar, notification, task center, voice (Phase 10), phone (Phase 15). Unanswered approvals pause the task (never auto-allow); a reminder comes after 10 minutes.
- **Batching**: related approvals in one task are grouped ("Allow these 3 package installs?").

## 6. Writing (UI copy)

- Short, plain, verb-first buttons ("Keep changes", "Undo", "Allow once").
- Priya speaks briefly in the first person: "I installed Docker and turned it on." Errors say what happened and what to do next, never blame.
- Technical detail lives behind "Details".
- Costs always with currency and two decimals; times as "about 3 min".
- Every UI string goes through `qsTr` from day one so Hindi (and others) can be added without code changes. Strings produced by the backend (priyad `user_message`s, CLI output, notifications, the phone app) use a shared Fluent message catalogue (ADR-020).

## 7. Accessibility

- WCAG 2.2 AA contrast for all text in all themes (AAA in high contrast), enforced by CI.
- Full keyboard operation; visible focus ring (2 px accent); logical focus order tested.
- `Accessible.name`, `role` and descriptions on every interactive QML item (lint-enforced) so Orca can read everything.
- Font scaling 80–200 % without clipping (screenshot tests at 1.0 and 2.0 scale and at 150 % font).
- Reduced motion and reduced transparency settings.
- Never use colour alone: modes and risks always have an icon and a word.

## 8. UI architecture

```
ui/
  design/tokens.json        the single source of design tokens
  design/generate/          generators: Theme.qml, GTK, Qt, kitty, Hyprland, GRUB, greeter
  PriyaUI/                  QML module: Theme singleton, components, cards, PriyaClient (JSON-RPC over Quickshell Socket)
  shell/                    Quickshell config: bar, Priya bar, notifications, OSDs, agent workspace
  apps/                     Task center, Settings, Rewind, Memory, Health, Onboarding (Quickshell FloatingWindows)
  gallery/                  component gallery: every component × state × theme × scale
  tests/                    Qt Quick tests, screenshot baselines, journey scripts
  mockups/                  approved design mockups (HTML/PNG) per sub-phase
```

- Data comes from priyad over the same JSON-RPC API as the CLI (P7, ADR-011); QML type helpers are generated from the API JSON Schemas so UI and backend can't drift.
- The UI never decides policy; it renders state and sends user decisions.
- Performance: Quickshell stays resident; the Priya bar is pre-created and only shown; lists are virtualised; idle UI CPU ≈ 0.

## 9. Design process (per sub-phase with UI)

1. **Mockup**: Claude makes a mockup of every new screen and state in light and dark (HTML or gallery screenshots) and saves it under `ui/mockups/<phase>.<sub>/`.
2. **Owner review**: the owner approves or comments. Nothing is built before approval.
3. **Build** with `PriyaUI` components only (new components go into the library and the gallery first).
4. **Baselines**: screenshot baselines for every state × theme × scale.
5. **Journey test** for the main flow.
6. **Owner sign-off** on the gallery or a recorded journey; it's part of the sub-phase gate.
