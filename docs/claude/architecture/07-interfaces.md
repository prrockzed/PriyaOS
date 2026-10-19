# 07 — Interfaces: natural language, voice, Priya Vision, desktop

Principle P7: every capability is reachable by plain language and by a typed API; plain language is translated into the same API calls.

## 1. Natural language pipeline

```
input (text | speech transcript | image | screenshot | selection)
  → context capture (focused window, cwd, project, selected files, clipboard on request)
  → Priya AI: classify → {answer | action | settings | navigation | mixed}
  → answer: read-only run(s) with tools → plain-language answer + cards
  → action: task + plan → runs → review card
  → settings: v1.settings.nl_change
  → navigation: open apps / files / settings directly (no task, logged)
```

**Answers** are short plain language first, details on demand. Every answer can carry **cards**: `diff`, `plan`, `approval`, `command` (the exact command that was run, copyable), `chart`, `file list`, `undo` (one-click revert of what just happened).

**OS control by language**: navigation intents ("open my api project in kitty and the browser on the docs", "turn on bluetooth", "make the screen dimmer") map to module methods and Hyprland IPC directly, not to agents — fast and deterministic. Only when no direct method matches does it become an agent task.

## 2. Surfaces

| Surface | Tech | Phase |
|---|---|---|
| CLI `priya …` | Rust, clap | 0+ |
| Review TUI | ratatui | 2 |
| Priya bar (Super+Space overlay: input, streaming answer, cards) | Quickshell (QML) layer-shell overlay on Hyprland, talks to priyad socket | 0 (minimal), 5 (full) |
| Top-bar status item (running / review / failed, cost today) | Waybar module early, Quickshell top bar from Phase 5 | 1 (Waybar), 5 |
| Notifications with actions | libnotify / mako first, Quickshell notification centre later | 3 (minimal), 5 (centre) |
| Review GUI (Task center) and Settings | Quickshell windows | 4 (Task center), 1 (AI page), 5 (Settings complete) |
| Agent workspace view | Hyprland special workspace `priya` with live run logs and (for GUI runs) their windows | 5 |
| Voice | voice module | 10 |
| Priya Vision | vision module | 11 |
| Phone companion | PWA served over Tailscale/WireGuard + push (ntfy) | 15 |

Decision record for Quickshell: [decisions.md](decisions.md) ADR-009. Visual design, components, themes and screen states: [11-design-system-and-ux.md](11-design-system-and-ux.md).

## 3. Voice (Phase 10)

- **Push-to-talk** (hold Super+V) and optional **wake word** "Hey Priya" (openWakeWord, local, off by default).
- **STT**: whisper.cpp (local, default; model size by hardware) via `SpeechToText` port; cloud STT optional per privacy settings.
- **TTS**: Piper (local), short spoken answers; full answer stays in the bar.
- Voice confirmations for `ask` tier approvals ("Allow?" → "yes"); `confirm` tier requires saying the confirmation word shown on screen.
- Languages: English first; Hindi supported by whisper (multilingual model) — UI language setting decides TTS voice.
- Audio never leaves the device unless the user enabled cloud STT; recordings are deleted after transcription unless "keep voice history" is on.

## 4. Priya Vision (Phase 11)

Inputs:
- **Screen**: `grim` screenshot of the focused window/region/all outputs + Hyprland IPC (window list, classes, titles, geometry).
- **Accessibility tree**: AT-SPI2 for GTK/Qt/Electron apps → structured text of UI elements (preferred over pixels).
- **Images**: dropped files, clipboard images, camera (opt-in).
- **OCR**: tesseract for local text extraction when routed to a non-vision model.

Uses:
- "What's on my screen?", "explain this error", "summarise this page", "what does this chart say".
- Act on what's visible: "fill this form with my address" (approval tier `ask`), "click the export button" — via AT-SPI actions first, pixel-level input (ydotool) only as fallback, always in the visible session with a highlighted target and a cancel window.
- Routing: Priya AI role `vision` → best enabled vision model; `private` screens (password managers, banking — window-class deny list) are never captured.

Privacy: no continuous capture. Screenshots are taken only on explicit request (or by an automation the user created), shown as a thumbnail in the bar before sending, and deleted after the task unless kept.

## 5. Desktop integration

- Hyprland config snippet at `/usr/share/priya/desktop/hyprland.conf` (keybinds, window rules, special workspace), sourced from the user's config.
- Keybinds: see [11 §4a](11-design-system-and-ux.md) (configurable, conflict-checked on install).
- Session environment: the Hyprland snippet runs `systemctl --user import-environment WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP` (and `dbus-update-activation-environment`) at startup so priyad and modules see the graphical session.
- Notifications: only one program can own `org.freedesktop.Notifications`. Before Phase 5 PriyaOS uses the existing daemon (mako on PriyaOS installs); from Phase 5 the Quickshell notification centre owns it on PriyaOS, and `priya desktop install` asks before replacing another daemon on existing setups (ADR-017).
- GUI apps inside forks (agents driving graphical apps in a sandbox) are out of scope until a later ADR defines Wayland access for forks.
- The desktop configuration itself is editable by plain language ("make the bar smaller, add a pomodoro timer") → task in sandbox mode for `~/.config` → preview by launching the changed config on a temporary nested Hyprland or a special workspace → merge.
