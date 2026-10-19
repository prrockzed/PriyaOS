# Phase 11 — Priya Vision

**Goal.** PriyaOS understands what you see: screenshots, images and app interfaces — explains them, answers questions about them, and can act on visible UI with approval. Never captures anything without being asked.

**Depends on:** Phases 3, 5 and 9.
**Features:** VIS-01 … VIS-08, TOOL-04.
**Read first:** [07 §4](../architecture/07-interfaces.md), [04 §10](../architecture/04-priya-ai.md).

## Build

- [ ] `vision` module with `ScreenSource` port: grim captures (window, region, outputs), Hyprland IPC window metadata
- [ ] AT-SPI2 reader: structured UI tree for the focused app (roles, names, values, actions), trimmed for tokens
- [ ] Image inputs: drag-and-drop into the Priya bar, clipboard images, files; camera opt-in
- [ ] Routing via Priya AI role `vision` (cloud or local vision models); OCR (tesseract) fallback
- [ ] Shortcut Super+Shift+S: select region → ask about it
- [ ] Private window deny list (password managers, banking — by class/title rules) + thumbnail preview before sending + delete after task by default
- [ ] Acting on UI: AT-SPI actions first (press button, set text), `ydotool` pixel input only as fallback; highlighted target and 3 s cancel window; approval tier `ask` for any input into other apps
- [ ] D-Bus app actions registered in the tool index (TOOL-04)

## Tests

- L1: screenshot and AT-SPI fixtures → expected extractions; OCR fixtures.
- L3 (VM with Hyprland): open a sample GTK app, ask "what does this window say", "click Export" → AT-SPI action happens after approval.
- Privacy: deny-listed window never captured (assert no capture event), thumbnail shown before send.

## Done when

- [ ] "What's on my screen?", "explain this error" (terminal screenshot), "summarise this chart" work with cloud and with a local vision model
- [ ] Acting on UI works in at least GTK, Qt and Electron apps via AT-SPI
- [ ] Privacy rules hold in all tests

**Demo.** Error dialog on screen → Super+Shift+S on it → plain explanation + "fix it" → task in sandbox.
