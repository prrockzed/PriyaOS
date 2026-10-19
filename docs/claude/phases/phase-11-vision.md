# Phase 11 — Priya Vision

**Goal.** PriyaOS understands what the user sees — screenshots, dropped images and the structure of app windows — explains it, answers questions about it, and can act on visible UI with approval. Nothing is captured without an explicit request, private windows are never captured, and every capture is previewed before it is sent anywhere.

**Usable at the end:** an error dialog is on screen → Super+Shift+S over it → thumbnail in the bar → "what does this mean?" → plain explanation → "fix it" → task in the sandbox. "Click Export in this window" → highlighted button, 3-second cancel window → clicked after approval.

**Depends on:** Phase 3 (approvals, untrusted labels), Phase 5 (Priya bar attachments, context capture), Phase 8 (tool index, D-Bus registration 8.2), Phase 9 (Hyprland/Quickshell defaults, Orca/AT-SPI enabled).

**Features:** VIS-01, VIS-02, VIS-03, VIS-04, VIS-05, VIS-06, VIS-07, VIS-08, TOOL-04.

**Read first:** [07 §4 Priya Vision, §5 desktop](../architecture/07-interfaces.md), [04 §4 roles, §10 multimodal](../architecture/04-priya-ai.md), [05 §4 approvals, §5 taint](../architecture/05-security.md), [08 §3 tool index](../architecture/08-rewind-memory-tools.md), [11 §4 Priya bar and cards, §5 human in the loop, §7 accessibility](../architecture/11-design-system-and-ux.md).

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 11.1 | Screen capture and window metadata | Phase 9 | `priya vision capture --window` saves a capture with window metadata; "what's on my screen?" works with a vision model |
| 11.2 | Region capture and preview | 11.1 | Super+Shift+S → select → thumbnail in the bar → ask about it |
| 11.3 | Image inputs | 11.2 | Drop or paste an image into the bar and ask about it |
| 11.4 | Vision routing and OCR | 11.1 | Best enabled vision model chosen automatically; OCR answers when no vision model is enabled |
| 11.5 | Accessibility-tree reader | 11.1 | "What does this window say?" answered from the app's UI tree, without pixels |
| 11.6 | Private-window protection | 11.1, 11.5 | Password managers, banking and private browsing windows are blacked out or refused |
| 11.7 | Acting on the screen | 11.5, 11.6 | "Click Export", "fill this form with my address" — highlighted, cancellable, approved |
| 11.8 | App actions in the tool index | 11.5, 11.7 | Agents find and call app actions (D-Bus and accessibility) through `priya mcp` |
| 11.9 | Phase gate | all | Vision demo on the Zenbook with cloud and local vision models |

---

### 11.1 — Screen capture and window metadata
**Outcome:** the `vision` module can capture the focused window, an output or all outputs with grim, and attach Hyprland window metadata; "what's on my screen?" sends a previewed capture to a vision model.
**Features:** VIS-01.
**Depends on:** Phase 9.

**Tasks**
- [ ] `modules/vision` process module: manifest (methods `vision.capture`, `vision.describe`, `vision.windows`; capabilities `screen:capture`, `ai.role:vision`; settings `[vision]`; intents "what's on my screen", "explain this").
- [ ] `ScreenSource` port (`windows()`, `capture(target) -> Capture {png, geometry, scale, window_meta}`) and contract suite.
- [ ] grim + Hyprland IPC adapter: window list/active window/monitors via the Hyprland socket (`j/clients`, `j/activewindow`, `j/monitors`); capture by geometry with `grim -g`; scale-aware (2× outputs).
- [ ] Capture only on explicit request (bar intent, shortcut, CLI or a user-created automation); every capture emits `vision.captured {target, window_class, bytes}` — metadata only, never pixels.
- [ ] Captures held in `$XDG_RUNTIME_DIR/priya/vision/` and deleted when the task ends unless "keep" is chosen (then stored as a task attachment blob).
- [ ] Intent wiring: "what's on my screen?" → capture focused output → preview card (11.2 component) → `v1.intent.submit {screen}`.
- [ ] Screen content marks the task as **untrusted** (taint reason `screen_content`), because on-screen text can carry injected instructions.

**UI**
- Capture preview card (thumbnail, window title/class, Send / Retake / Cancel); states: capturing, preview, sent, discarded, error (compositor unavailable), long titles, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: Enter sends, Esc cancels, R retakes. Screen-reader names: "Screenshot of <window title>", "Send screenshot".
- 👤 Mockup approval (`ui/mockups/11.1/`) before building; gallery sign-off after.

**Human in the loop:** the user sees the exact image before it is sent; Cancel discards it immediately.

**Tests**
- Unit: geometry/scale maths for mixed-DPI outputs; Hyprland JSON parsing on fixtures; taint added for screen captures.
- Contract: `ScreenSource` suite (fake source + grim/Hyprland adapter in headless Hyprland).
- Integration: headless Hyprland with a test window → capture → PNG dimensions and metadata match; capture file removed after task end.
- UI: preview card states × theme × scale; a11y lint.
- Perf: shortcut → preview visible < 300 ms for one 2880×1800 output.
- Regression: `vision_capture_window_meta`, `vision_capture_deleted_after_task`, `vision_capture_taints_task`.

**Done when**
- [ ] "What's on my screen?" works on the Zenbook with a cloud vision model and with a local Ollama vision model.
- [ ] No capture happens without a user action (event-log audit over a day of use shows captures only with a causing user intent).

---

### 11.2 — Region capture and preview
**Outcome:** Super+Shift+S lets the user drag a region; the thumbnail drops into the bar ready for a question.
**Features:** VIS-08, VIS-02.
**Depends on:** 11.1.

**Tasks**
- [ ] Region selection via `slurp` (themed with tokens: accent border, dimmed backdrop) → `grim -g`; window snapping (click a window to capture just it).
- [ ] Hyprland bind `bind = SUPER SHIFT, S, exec, priya vision region` in the desktop snippet.
- [ ] Bar opens with the region attached and suggested prompts ("Explain this", "What does this error mean?", "Summarise", "Copy the text").
- [ ] "Explain this error" path: classifier sees `screen` attachment + error-like OCR text → answer with `UX-07`-style fix suggestions and a "Fix it" action that starts a sandbox task with the capture as context.
- [ ] Redact before sending: drag to blur a rectangle on the thumbnail (applied locally before upload).

**UI**
- Region selector overlay, bar attachment chip, redact tool; states: selecting, selected, redacting, attached, removed, too small, error.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: Super+Shift+S, arrow keys nudge the selection, Enter confirms, Esc cancels; screen-reader names "Select a screen area", "Attached screenshot, remove".
- 👤 Mockup approval before building; journey sign-off ("error dialog → region → explain → fix it").

**Tests**
- Unit: redaction applied to pixels before encoding (blurred region hash differs, rest identical).
- Integration: scripted region selection in headless Hyprland → attachment arrives in bar subscription.
- Eval: 30 screenshot fixtures (terminal errors, dialogs, charts, web pages) → answers judged by rubric (`review` role + owner spot check); ≥ 85 % correct with a frontier vision model, ≥ 65 % with the local model.
- UI: selector and chip screenshots; journey test.
- Regression: `region_capture_attaches`, `redaction_applied_before_send`, `explain_error_screenshot_eval`.

**Done when**
- [ ] Owner explains three real errors from screenshots and starts one fix task from a capture.

---

### 11.3 — Image inputs
**Outcome:** dropping, pasting or attaching an image file in the bar works like a capture.
**Features:** VIS-03.
**Depends on:** 11.2.

**Tasks**
- [ ] Bar drop target (Quickshell `DropArea`) for image files and image URLs from browsers; paste from clipboard via `wl-paste --type image/png`.
- [ ] Attach button → file picker (xdg-desktop-portal FileChooser).
- [ ] Normalisation in the vision module: decode PNG/JPEG/WebP/HEIC (where codecs exist), strip EXIF/GPS metadata, downscale to the model's limit, max 10 images per request.
- [ ] Images from `~/Downloads` or URLs mark the task untrusted (existing taint rules).
- [ ] Image attachments listed in the answer history with thumbnail and file name.

**UI**
- Attachment chips, drop highlight, multi-image strip; states: dragging over, attached, unsupported type, too large (auto-resized notice), many images (strip scrolls), error.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard-accessible attach button and chip removal (Delete key); screen-reader names include file names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: EXIF/GPS stripping (fixture with GPS tags → none after), resize maths, format detection.
- Integration: paste image → intent with `images[1]`; HEIC without codec → plain error.
- UI: drop/attach states screenshots.
- Regression: `image_drop_attaches`, `exif_stripped_before_send`.

**Done when**
- [ ] Owner asks about a pasted chart and a dropped photo; no metadata leaves the device (asserted on outgoing request fixtures).

---

### 11.4 — Vision routing and OCR
**Outcome:** image requests go to the best enabled vision model under the user's strategy and privacy rules; with no vision model enabled, tesseract OCR extracts text so a text model can still answer.
**Features:** VIS-02, VIS-06.
**Depends on:** 11.1.

**Tasks**
- [ ] Router: requests with images require `modalities.input ⊇ ["image"]`; role `vision`; candidates filtered by `UseModel` policy for data class `screen`.
- [ ] Default privacy rule: data class `screen` is `local_preferred` (cloud allowed unless the user picks local-only for screens); setting in Settings → Vision with plain explanation.
- [ ] OCR fallback via tesseract (`eng` + `hin` traineddata) with layout preserved (`--psm 3`, TSV output → lines with positions); used when no vision model passes filters, offline, or when the user asks "copy the text".
- [ ] Hybrid mode: OCR text + AT-SPI text (11.5, when available) sent alongside the image to cut tokens and improve accuracy on small text.
- [ ] Token optimisation: crop to the region of interest, downscale per model's tiling rules, reuse cached image tokens within a task (provider caching where supported).
- [ ] `priya ai why` shows image-related routing reasons ("used the local vision model because Vision privacy is set to local only").

**Tests**
- Unit: router filter with image modality; privacy class `screen`; OCR TSV → text assembly.
- Contract: vision request path through the fake provider and recorded cassettes for one cloud and one local (Ollama) vision model.
- Integration: no vision model enabled → OCR path → answer; offline → local vision or OCR with notice.
- Eval: OCR character accuracy ≥ 97 % on 40 English UI screenshots, ≥ 90 % on 20 Hindi screenshots; vision-vs-OCR answer quality comparison recorded.
- Perf: OCR on a 1920×1080 capture < 2 s on the Zenbook.
- Regression: `vision_routes_to_image_model`, `ocr_fallback_no_vision_model`, `screen_privacy_local_only`.

**Done when**
- [ ] Same question answered with a cloud model, a local vision model and OCR-only, each explained by `priya ai why`.

---

### 11.5 — Accessibility-tree reader
**Outcome:** the vision module reads the focused app's UI as structured text through AT-SPI2 — roles, names, values, states and available actions — and prefers it over pixels.
**Features:** VIS-04.
**Depends on:** 11.1.

**Tasks**
- [ ] AT-SPI2 client (`atspi` crate over zbus): find the accessible application for the focused Hyprland window (PID match), walk the tree with depth/size limits, collect role, name, description, value, states, actions, bounding boxes.
- [ ] Tree trimmer: drop invisible/offscreen nodes, collapse decorative containers, cap to N nodes (default 400) with a summary of what was dropped; stable node ids for later actions.
- [ ] Enable accessibility for common toolkits in `priyaos-defaults`: `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`, GTK default on, Electron/Chromium `--force-renderer-accessibility` via flags files where supported; `priya doctor` reports apps that expose no tree.
- [ ] Route: questions about the focused window use the tree first; fall back to pixels when the tree is empty or the question is visual ("what colour…", charts).
- [ ] Text selection: AT-SPI `Text` interface used for "summarise the selected text".

**Tests**
- Unit: trimmer on recorded trees (GTK4 app, Qt6 app, Firefox page, VS Code/Electron, LibreOffice) → golden output; node cap respected.
- Contract: `UiTreeSource` port suite (fake + AT-SPI adapter in a headless session with test apps).
- Integration: headless Hyprland with a GTK test app → "what does this window say" answered from the tree; no screenshot taken (no `vision.captured` event).
- Perf: tree read for a typical app < 500 ms; tree under 4k tokens after trimming for 95 % of fixtures.
- Regression: `atspi_tree_gtk`, `atspi_tree_qt`, `atspi_tree_electron`, `tree_preferred_over_pixels`.

**Done when**
- [ ] Trees read correctly from GTK, Qt, Firefox and one Electron app on the Zenbook; apps without a tree are listed by `priya doctor`.

---

### 11.6 — Private-window protection
**Outcome:** windows matching the private list (password managers, banking, private browsing, user additions) are never captured or read — they are blacked out in full-screen captures and refused for window and tree reads.
**Features:** VIS-07.
**Depends on:** 11.1, 11.5.

**Tasks**
- [ ] Deny list `/usr/share/priya/vision/private-windows.toml` (class and title regex: KeePassXC, Bitwarden, 1Password, Proton Pass, GNOME Keyring prompts, polkit/pinentry dialogs, browser private windows, common banking title patterns) + user list `~/.config/priya/vision/private-windows.toml`.
- [ ] Enforcement inside the `ScreenSource` adapter (not the UI): window capture of a private window refused; output/region captures black out private windows by geometry before the image leaves the module; AT-SPI reads of private apps refused; password-role fields (`ROLE_PASSWORD_TEXT`) always masked.
- [ ] Re-check at capture time (windows can move between listing and capture): capture → re-query clients → mask; if geometry changed during capture, discard and retry once.
- [ ] Event `vision.private_blocked {window_class}`; preview card shows "1 private window hidden".
- [ ] "Never capture this app" action on the preview card and in Settings → Vision (adds a class rule).

**UI**
- Preview card "private window hidden" state; Settings → Vision private-apps list (add/remove, test against open windows); states: empty user list, list, rule matches now, invalid regex.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: rule matching (class, title regex, case), masking rectangles with fractional scaling.
- Integration: headless Hyprland with a window titled "KeePassXC" overlapping a terminal → full capture has the area black (pixel assertion), window capture refused, AT-SPI read refused; window moved mid-capture → retried and masked.
- Security: 20 private-window scenarios (minimised, other workspace, fullscreen, floating over, scaled output) → zero private pixels in any captured image; password fields never appear in tree output.
- UI: settings and preview states.
- Regression: `private_window_never_captured`, `private_window_masked_in_full_capture`, `password_field_masked_in_tree`.

**Done when**
- [ ] Security scenarios 100 % pass; owner adds an app to the list in Settings and sees it hidden.

---

### 11.7 — Acting on the screen
**Outcome:** "click Export", "fill this form with my address", "turn on dark mode in this app" act on the visible app — AT-SPI actions first, ydotool input only as a fallback — with the target highlighted, a 3-second cancel window and approval.
**Features:** VIS-05.
**Depends on:** 11.5, 11.6.

**Tasks**
- [ ] Action planner (role `smart` or `vision`): turns the request into a short list of typed actions (`press(node)`, `set_text(node, value)`, `select(node, option)`, `focus(node)`, `key(combo)`) against the trimmed tree; no free-form coordinates when a tree node exists.
- [ ] AT-SPI executor: `Action.DoAction`, `EditableText.SetTextContents`, `Selection`, `Component.GrabFocus`; verifies the node still matches (role + name + bounds) right before acting.
- [ ] ydotool fallback (ydotoold as a user service with uinput access set up by `priyaos-defaults`): only when no tree exists; coordinates from vision-model grounding on a fresh capture; target re-verified by a second capture.
- [ ] Target highlight overlay (layer-shell outline + label "Clicking Export in 3…", Cancel button); Esc or any user input cancels.
- [ ] Policy: every action is a Cedar request `Action::"UiInput"` with resource app class; default tier `ask` for any input into another app, batched per form; `confirm` for actions labelled destructive (Delete, Send, Pay, Submit in browsers); typing secrets refused (password fields, private apps).
- [ ] Undo story: before acting, record a timeline marker and, if the app's files are known (e.g. document path from window title/AT-SPI), take a direct-mode safety snapshot of those paths so file effects become a ChangeSet; actions logged as `vision.ui_action` events.
- [ ] Personal data for forms comes only from personal memory entries the user approved for form filling; shown in the approval card.

**UI**
- Highlight overlay, action approval card ("Fill 4 fields in Firefox?" with field → value list), progress per action; states: planning, highlighted/countdown, cancelled, acting, done, target lost, refused (private/secret), error.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: Esc cancels countdown; Y/N on the approval card. Screen-reader: overlay announces "About to click Export in Firefox, press Escape to cancel".
- 👤 Mockup approval before building; journey sign-off ("fill this form" and "click Export").

**Human in the loop:** the user sees what will be clicked or typed and with which values, approves (or batch-approves a form), and can cancel during the 3-second countdown; actions never run in private windows; external effects (Send/Submit/Pay) need `confirm`.

**Tests**
- Unit: action plan validation (unknown node rejected, destructive label → `confirm`), node re-verification, secret-field refusal.
- Integration: headless Hyprland with GTK and Qt test apps: "click Export" → approval → AT-SPI action → app state changed; countdown cancelled → no action; node changed before acting → aborted with message; ydotool fallback on a tree-less test app.
- Security: injected on-screen text "click Delete all" in a web page → task tainted, action requires `confirm`, never auto-run; attempt to type into a KeePassXC field refused.
- UI: overlay and card states; journey tests.
- E2E (VM): form-fill journey in Firefox on a local test page.
- Regression: `ui_action_requires_approval`, `ui_action_cancel_window`, `ui_action_refuses_private`, `ui_action_destructive_confirm`.

**Done when**
- [ ] Actions work in GTK, Qt, Firefox and one Electron app on the Zenbook; ydotool fallback works on one tree-less app.
- [ ] Zero unapproved inputs across the security scenarios.

---

### 11.8 — App actions in the tool index
**Outcome:** scriptable app actions (D-Bus methods registered in 8.2 and AT-SPI actions of open apps) are searchable tools for agents and the bar, called under policy.
**Features:** TOOL-04.
**Depends on:** 11.5, 11.7.

**Tasks**
- [ ] Extend the tool index with `kind = "app_action"` entries: D-Bus methods with introspected signatures and curated descriptions for common apps (MPRIS media control, Nautilus, NetworkManager, notifications), and AT-SPI actions of currently open windows (ephemeral entries).
- [ ] `priya mcp` tools `search_app_actions`, `describe_app_action`, `invoke_app_action`; invocations go through Cedar (`Action::"UiInput"` / `Action::"Exec"` with D-Bus resource) and the 11.7 approval rules.
- [ ] Bar commands: app actions appear as local results ("Pause music", "Next track") without a model call.
- [ ] Safety: D-Bus methods from a deny list (session management, polkit, secret service) never exposed; read-only methods (`Get*`, properties) tier `auto`, others `ask`.
- [ ] Inside forks, app actions are unavailable (no live session) — tools report this plainly.

**Tests**
- Unit: D-Bus introspection XML → tool schema; deny-list filtering; tier assignment.
- Contract: MCP tool definitions snapshot (schema compatibility check).
- Integration: session bus with a test MPRIS service → "pause music" in the bar → method called; agent run calls `invoke_app_action` → approval requested.
- Security: agent tries `org.freedesktop.secrets` → not listed, direct call refused.
- Regression: `app_action_dbus_mpris`, `app_action_policy_gate`, `app_action_denylist`.

**Done when**
- [ ] An agent completes "pause the music and open the Downloads folder" via app actions with correct approvals.

---

### 11.9 — Phase gate
**Outcome:** Priya Vision is dependable on the Zenbook and private by construction.
**Features:** all Phase 11 features.
**Depends on:** 11.1–11.8.

**Tasks**
- [ ] Full regression and nightly suites green; fix failures.
- [ ] L4 checklist on the Zenbook (below); record results here.
- [ ] Update `features.md`, `docs/human/05-the-phases.md`, `docs/human/04-everything-priyaos-will-do.md`.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green, including nightly e2e (VM with Hyprland headless + test apps), security (private-window, injection-via-screen) and eval suites.
- [ ] Owner demo: error dialog → Super+Shift+S → explanation → "fix it" → sandbox task → keep; "what's on my screen?" with a local vision model; "click Export" in a real app with the countdown; a password manager open during a full-screen capture is blacked out.
- [ ] L4 checklist (Zenbook, 2880×1800 OLED at fractional scale): captures sharp and correctly cropped at the configured scale; external monitor at different scale; AT-SPI works in Firefox, a GTK app, a Qt app, one Electron app; ydotool fallback works; capture-to-preview < 300 ms.
- [ ] Privacy audit: one day of use → every `vision.captured` event has a causing user intent or a user-created automation.
- [ ] Docs: [07 §4](../architecture/07-interfaces.md) updated (taint for screen content, `UiInput` action, cancel window length); new Cedar action documented in [05 §2](../architecture/05-security.md); feature statuses and human status updated.

## Out of scope

- Continuous screen watching or recording ("remember everything I saw").
- Camera input (mentioned in 07 §4 as opt-in; no feature ID yet).
- Acting on the screen inside forks or other sessions (vision acts only in the visible session).
- Compositors other than Hyprland (port allows them later).
- Teach by showing (Phase 13 uses this phase's AT-SPI reader).
