# Phase 16 — Ecosystem

**Goal.** Other people can build for PriyaOS and more people can use it: module SDKs in Rust, Python and TypeScript with templates and a docs site; a directory of verified modules, skills, recipes and themes that installs by asking; an SDK for apps to expose structured actions to people and agents alike; small generated mini-apps; a desktop that restyles itself by plain language with a preview; wallpaper-derived accents and shareable themes; and PriyaOS as a layer on Omarchy and any btrfs Arch.

**Usable at the end:** "get me a Spotify control skill" → directory result from a verified publisher with its permissions → install → "pause Spotify" works for the owner and for agents; "make my desktop warmer and add a focus timer" → live preview → keep; "build me a dashboard of today's agent costs" → a mini-app opens and is there tomorrow; PriyaOS installs onto a fresh Omarchy machine without breaking its setup.

**Depends on:** Phase 5 (theming, `priya desktop install`), Phase 8 (tool index), Phase 9 (signed repo, app installs through the sandbox), Phase 11 (app actions in the tool index), Phase 13 (recipes), Phase 14 (scanning, signing, MCP registry, egress proxy).

**Features:** ECO-01, ECO-02, ECO-03, ECO-04, ECO-05, ECO-07, TOOL-05, UX-15, UX-16, DS-21.

**Read first:** [02 §5 module system, §6 settings](../architecture/02-core.md), [05 §9 supply chain](../architecture/05-security.md), [08 §3 tool index](../architecture/08-rewind-memory-tools.md), [07 §5 desktop integration](../architecture/07-interfaces.md), [10 §5 other installation targets](../architecture/10-base-os-and-distro.md), [11 §2 visual language, §3 themes, §8 UI architecture, §9 design process](../architecture/11-design-system-and-ux.md), [09 §6 schema checks](../architecture/09-testing.md), [decisions ADR-002, ADR-014](../architecture/decisions.md).

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 16.1 | Module SDKs and templates | Phase 14 | `priya module new hello --lang python` → working, testable module in minutes |
| 16.2 | Developer docs site | 16.1 | Public docs site with tutorials and a generated API reference |
| 16.3 | Directory backend | 16.1 | Signed, static directory index with verified publishers and a submission pipeline |
| 16.4 | Directory UI and install by asking | 16.3 | Browse the directory or say "get me a …" and install with a permissions prompt |
| 16.5 | App action SDK | 16.1 | Apps ship action manifests; actions work from the bar and for agents under one permission model |
| 16.6 | Generated mini-apps | 16.1 | "Build me a dashboard of …" → a small, permission-limited app that reopens and can be edited by language |
| 16.7 | Desktop restyling with preview | Phase 5 | "Make the bar smaller and add a pomodoro timer" → preview → keep or discard |
| 16.8 | Wallpaper accents and theme sharing | 16.7 | Accent picked from the wallpaper; themes exported, imported and shared |
| 16.9 | Omarchy layer | 16.7 | PriyaOS installs onto Omarchy or any btrfs Arch and integrates with its desktop |
| 16.10 | Phase gate | all | Third-party-style module from the directory, restyle demo, Omarchy install |

---

### 16.1 — Module SDKs and templates
**Outcome:** stable SDKs for Rust, Python and TypeScript speak the `v1` module protocol with typed bindings generated from the JSON Schemas; `priya module new` creates a module that builds, passes the module contract suite and can be signed.
**Features:** ECO-01.
**Depends on:** Phase 14.

**Tasks**
- [ ] Rust: stabilise `priya-module-sdk` (Phase 0.8) — public API review, docs, examples; published to crates.io.
- [ ] Python SDK `priya-module` (PyPI): stdio JSON-RPC with `Content-Length` framing, async host calls (`host.emit_event`, `host.call`, `host.ai.ask`, `host.task.create`, `host.settings.get`, `host.secret.get`), event subscriptions, tool exposure; typed models generated from the schemas.
- [ ] TypeScript SDK `@priyaos/module-sdk` (npm, Node runtime): same surface; types generated from the schemas.
- [ ] Code generation in `cargo xtask schemas` emits Python and TS bindings so SDKs can't drift from `docs/claude/api/` (schema compatibility check covers them).
- [ ] `priya module new <name> --lang rust|python|ts`: manifest, example method/intent/tool, settings schema, tests, CI workflow, signing setup (14.6), README.
- [ ] `priya module test <path>`: runs the module contract suite (handshake, manifest validity, declared-capability use only, crash/restart behaviour, settings schema) against any module; `priya module dev` runs it in developer mode with reload.
- [ ] Module UI guidance: modules contribute surfaces only through manifest `surfaces` rendered by PriyaUI (no custom styling), documented.

**Tests**
- Unit: framing codec in each language (partial reads, large messages), generated bindings round-trip against schema fixtures.
- Contract: the module contract suite passes for each template in all three languages, and fails clearly for a deliberately broken module (undeclared capability, bad manifest).
- Integration (fake machine): a template module per language installs (developer mode), registers an intent and a tool, answers through the bar and through `priya mcp`.
- Regression: `sdk_template_<lang>_contract` ×3, `sdk_bindings_match_schemas`, `module_test_detects_undeclared_capability`.

**Done when**
- [ ] A new module in each language goes from `priya module new` to working in the bar in < 10 minutes following the template README (timed by the owner for Python).

---

### 16.2 — Developer docs site
**Outcome:** a public developer docs site explains how to build modules, app actions, recipes and themes, with an API reference generated from the schemas and examples tested in CI.
**Features:** ECO-01.
**Depends on:** 16.1.

**Tasks**
- [ ] mdBook site from `docs/dev/` (new folder; separate from `docs/claude` specs and `docs/human` guides) built in CI and published to GitHub Pages on tags; versioned per API version (`v1`).
- [ ] Generated reference: API methods, events, settings sections, module manifest, MCP tools and action manifests from `docs/claude/api/` JSON Schemas; SDK references (rustdoc, pdoc, TypeDoc) linked.
- [ ] Tutorials: first module in each language; exposing app actions (16.5); writing a recipe (13.9); making a theme (16.8); publishing to the directory (16.3); security model for developers (capabilities, signing, scanning, what reviewers check).
- [ ] Design guidelines for module surfaces: tokens, PriyaUI cards, states, copy rules (11 §6), accessibility requirements.
- [ ] Examples compiled and run in CI (doctest-style extraction); link checker; search.

**Tests**
- Integration: docs build in CI; every tutorial's code extracted and run against the fake machine; reference pages exist for every schema (generator test).
- UI: docs site passes axe accessibility checks and renders in light/dark.
- Regression: `docs_examples_run`, `docs_reference_complete`, `docs_links_ok`.

**Done when**
- [ ] Someone other than the owner (or Claude in a fresh session with only the site) builds a working module from the tutorial alone.

---

### 16.3 — Directory backend
**Outcome:** a static, signed directory index lists modules, skills, recipes, action packs and themes with publisher verification levels, hashes, signatures, capabilities and scan verdicts; a submission pipeline checks every version before it is listed.
**Features:** ECO-02, ECO-03.
**Depends on:** 16.1.

**Tasks**
- [ ] ADR: directory as a git repository + static hosting (P10 — no server database): entries `entries/<publisher>/<name>.toml` (kind, versions, artifact URLs, content hashes, publisher signatures, declared capabilities, licence, min API version); CI builds `index.json` signed with the directory key and publishes it with the artifacts.
- [ ] Publisher registry: publisher keys (14.6 format) with proof of control (signed commit from a GitHub org/user, or DNS TXT for a domain); levels `verified`, `known`, `unverified` (hidden by default in clients).
- [ ] Submission pipeline (PR to the directory repo): signature check, scanner (14.5) with cross-family review, capability diff vs the previous version (increases flagged for reviewers), module contract suite (16.1) for modules, size/licence checks; first submission of a publisher needs maintainer review.
- [ ] Integrity on the client: index signature, artifact hash, publisher signature, revocation list and yanked versions checked on install and on update.
- [ ] Directory domains added to the default egress allow-list only for the directory client.
- [ ] No usage tracking: install counts and ratings are not collected (privacy); entries show maintainer-provided info and verification only.

**Tests**
- Unit: entry schema validation, index signing/verification, revocation/yank logic, capability diff.
- Integration: local directory repo + CI script → index built and signed; client verifies; modified artifact (one byte) → refused; revoked publisher → install refused with explanation; yanked version not offered.
- Security: index replay (older signed index) detected via monotonic version; mirror serving a different artifact → hash mismatch refusal.
- Regression: `directory_index_signature`, `directory_tampered_artifact_refused`, `directory_revoked_publisher`, `directory_index_rollback_detected`.

**Done when**
- [ ] Pipeline lists a sample module, skill, recipe and theme from a verified test publisher; every negative integrity case is refused.

---

### 16.4 — Directory UI and install by asking
**Outcome:** a Directory screen lets the user browse and install verified items with their permissions in plain language; saying "get me a Spotify control skill" or "I need an app to edit PDFs" finds candidates in the directory and in normal Linux app sources and installs the chosen one safely.
**Features:** ECO-02, ECO-05.
**Depends on:** 16.3.

**Tasks**
- [ ] Directory app (`ui/apps/Directory`): categories, search, entry detail (description, publisher + verification badge, capabilities in plain language, versions, scan verdict, licence), Install / Update / Remove; installed items list.
- [ ] Install by asking: intent category `find_and_install` → local semantic search over the cached index (role `embed`, local) + system app sources (pacman, Flathub appstream, AUR marked "community, less checked") → up to 3 candidate cards with reasons and trust level → install through the 14.6 permissions prompt (directory items) or the 9.4 sandboxed package install (apps).
- [ ] ECO-05 ranking rules: prefer official repos and verified publishers; explain trade-offs ("Flatpak, sandboxed, 120 MB" vs "pacman, smaller"); never install without confirmation.
- [ ] Updates of directory items join the normal update pipeline with capability-increase prompts.
- [ ] Remove: uninstall + revoke registry/MCP entries; Rewind captures the removal.

**UI**
- Directory app, candidate cards in the bar, install prompt; states: loading index, offline (cached index, install disabled), empty search, results, entry detail, installing, installed, update available, capability increase, removed, unverified hidden notice, long descriptions, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard: search focus on open, arrows through results, Enter details, I installs; screen-reader reads trust level and capability count first.
- 👤 Mockup approval (`ui/mockups/16.4/`) before building; journey sign-off ("get me a …" → candidates → install → use).

**Human in the loop:** every install shows publisher, trust level and permissions first; unverified items are never suggested by default; removal is one click and undoable.

**Tests**
- Unit: candidate ranking rules, trust filtering, capability rendering.
- Eval: 40 "find me an app/skill for …" requests → correct candidate in top 3 ≥ 85 %; never an unverified directory item or AUR package ranked first when an official option exists.
- Integration: ask → install module from the local test directory → works; ask for a PDF editor → Flatpak/pacman candidate → sandboxed install → review → merge.
- UI: screenshots for every state × theme × scale; a11y lint; journey.
- Regression: `install_by_asking_directory`, `install_by_asking_system_app`, `unverified_not_suggested`.

**Done when**
- [ ] Owner installs one directory item and one normal app by asking.

---

### 16.5 — App action SDK
**Outcome:** apps ship an action manifest (backed by an MCP server or a D-Bus interface); the tool index registers it on install; actions appear as bar commands for people and as tools for agents, with one permission model.
**Features:** TOOL-05, ECO-04.
**Depends on:** 16.1.

**Tasks**
- [ ] Action manifest format (`/usr/share/priya/actions/<app-id>.toml`, or bundled with a directory item): app id, transport (`mcp_stdio` command or `dbus` interface mapping), actions `{name, title, description, params schema, result schema, effect: read | write | external, confirm?}`; schema in `priya-api`, validated by `priya actions lint`.
- [ ] Registration: pacman hook (Phase 8) and directory installs register actions in the tool index and the MCP registry (14.8) with default per-agent access; third-party manifests signed and scanned.
- [ ] SDK helpers in all three SDKs to expose actions as an MCP server with schema validation; GTK and Qt example apps; MPRIS-style wrappers for common app types.
- [ ] People side (ECO-04): actions appear in bar local results and the command palette ("Pause Spotify"), with parameter prompts; same Cedar checks and approval tiers as agents (`effect` maps to tier: read → `auto`, write → `notify`/`ask`, external → `ask`).
- [ ] Sample "both-sided" app: a small notes app whose UI and actions share one code path, used in docs.

**Tests**
- Unit: manifest validation, effect → tier mapping, schema validation of params/results.
- Contract: action manifest schema snapshot; SDK action-server helpers pass an MCP conformance subset.
- Integration: install sample app → actions registered → "add a note: buy milk" from the bar and from an agent run → same result, same approval behaviour; uninstall removes actions.
- Security: an action declaring `read` that writes (detected by eBPF trace/ChangeSet) → flagged and disabled; unsigned third-party manifest refused.
- Regression: `app_actions_registered_on_install`, `app_action_same_for_person_and_agent`, `action_effect_mismatch_flagged`.

**Done when**
- [ ] Two real apps (one sample, one wrapper for an existing app) work by plain language for the owner and for an agent.

---

### 16.6 — Generated mini-apps
**Outcome:** "build me a dashboard of today's agent costs" generates a small app from PriyaUI components with a manifest declaring exactly which data it can read; it opens in a restricted runtime, is saved, reopens from the bar and can be changed by plain language.
**Features:** UX-15, ECO-08.
**Depends on:** 16.1.

**Tasks**
- [ ] ADR: mini-app runtime — QML using only `PriyaUI` components, loaded in a separate restricted Quickshell process (no `Process`, no file or network access in QML; Landlock + seccomp on the process); data only through a restricted client that enforces the manifest.
- [ ] Mini-app manifest: `{name, data: [read-only API methods and event filters], refresh, actions: [allowed API calls or app actions], network: []}`; capabilities shown before first open.
- [ ] Generator (role `coding`): request → manifest + QML from a component allow-list; QML lint (accessibility names, tokens only, no hard-coded colours); failing lint → regenerate (max 2) → scanned (14.5).
- [ ] Storage `~/.local/share/priya/miniapps/<id>/`; each edit by language is a ChangeSet (Rewind can undo); listed in the bar ("Open: agent costs dashboard") and Settings.
- [ ] Task dashboards: "show me a dashboard for this migration" generates a mini-app bound to a task's events.
- [ ] Share/export a mini-app as a file (manifest + QML) and via the directory (kind `miniapp`, scanned and signed like other items).

**UI**
- Mini-app window chrome (title, "Made by Priya" badge, Edit by asking, permissions), generation progress, capability prompt; states: generating, lint retry, preview, saved, editing, data error, permission denied, empty data, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0 (generated apps inherit tokens automatically).
- Keyboard and screen-reader names required by lint in generated code.
- 👤 Mockup approval of the chrome and generation flow before building; gallery sign-off with 5 generated examples.

**Human in the loop:** the user sees what data a mini-app can read before it opens; edits are reviewable and undoable; mini-apps cannot change anything except through declared actions with normal approvals.

**Tests**
- Unit: manifest enforcement in the restricted client, QML lint rules, component allow-list check.
- Eval: 20 scripted requests (costs, tasks, deploys, CI, disk, battery) → working app within declared permissions ≥ 85 %; zero apps with undeclared data access.
- Security: generated QML trying `Process`, `XMLHttpRequest`, file URLs or undeclared API methods → refused at lint and at runtime.
- UI: screenshots of generated examples in all themes/scales; a11y lint on generated output.
- Perf: generation < 30 s; mini-app open < 300 ms; idle CPU ≈ 0.
- Regression: `miniapp_permissions_enforced`, `miniapp_lint_required`, `miniapp_eval_set`.

**Done when**
- [ ] Owner keeps at least two generated mini-apps in daily use for a week.

---

### 16.7 — Desktop restyling with preview
**Outcome:** plain-language desktop changes ("make the bar smaller, warmer colours, add a pomodoro timer, move the clock left") run as a sandbox task on `~/.config`, show a live preview, and are kept or discarded; accessibility guardrails always hold.
**Features:** UX-16.
**Depends on:** Phase 5.

**Tasks**
- [ ] Intent category `desktop_style` → task in `sandbox` mode limited to `~/.config/priya/`, `~/.config/quickshell/`, `~/.config/hypr/` (07 §5).
- [ ] Change targets: token overrides (`~/.config/priya/theme.toml`), shell layout overlays (bar size/position, widget order), Hyprland appearance (gaps, borders, rounding, animations), widgets from a curated set (pomodoro, CPU/RAM meter, now playing, clock variants) or generated via 16.6.
- [ ] Preview: the changed config runs in a nested Hyprland window (or a special workspace when nesting isn't possible) with the real shell; before/after screenshots side by side; "Adjust" by language iterates in the same fork.
- [ ] Guardrails: contrast checker on token overrides (refuse below AA, AAA in high contrast), accessibility lint on shell changes, locked keybinds and elements (kill switch, Priya bar, Esc, focus ring) can't be removed or rebound.
- [ ] Keep → merge → `priya theme apply` live reload; Undo toast; Rewind entry "desktop style change".

**UI**
- Preview window (before/after, Keep / Discard / Adjust), restyle progress; states: generating, preview ready, adjusting, guardrail refused (with reason), kept, discarded, preview unavailable (fallback screenshots), Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard K/D/A; screen-reader describes changes in words ("Bar height 32 → 28 px, accent Marigold → Rose").
- 👤 Mockup approval before building; journey sign-off ("warmer and add a timer" → preview → keep → undo).

**Human in the loop:** nothing changes on the live desktop until Keep; every kept change is undoable; guardrails can't be overridden by plain language.

**Tests**
- Unit: guardrail checks (contrast, locked binds), change-target classification.
- Integration (headless Hyprland): "make the bar smaller" → fork edits → preview screenshot differs as expected → keep → live config updated; "remove the kill switch shortcut" → refused with explanation.
- Eval: 30 restyle requests → owner-judged correct ≥ 80 %; 0 guardrail violations.
- UI: preview window states; journey test.
- Regression: `restyle_preview_then_keep`, `restyle_guardrails_hold`, `restyle_undo`.

**Done when**
- [ ] Owner restyles the desktop three times by plain language, keeping two and discarding one from the preview.

---

### 16.8 — Wallpaper accents and theme sharing
**Outcome:** the accent can follow the wallpaper (always passing contrast), and full themes (token overrides + wallpaper) can be exported, imported and shared through files or the directory.
**Features:** DS-21.
**Depends on:** 16.7.

**Tasks**
- [ ] Accent extraction: k-means over the wallpaper in OKLab, candidates ranked by chroma and distance from semantic colours (success/warning/danger), lightness/chroma adjusted per theme until `on.accent` and text pairs pass the CI contrast rules; 3 suggestions shown.
- [ ] Setting `appearance.accent = "wallpaper"` (opt-in): re-derives on wallpaper change, with crossfade (reduced-motion aware).
- [ ] Theme bundle `.priya-theme` (archive: `theme.toml` token overrides only — no code — wallpaper(s), previews, manifest, optional signature); `priya theme export|import|list`.
- [ ] Import: schema validation, contrast check for all themes (light, dark, OLED-black, high-contrast) — failing pairs auto-adjusted or import refused with explanation; preview via 16.7 before applying.
- [ ] Directory kind `theme` (16.3) with previews; install by asking ("find me a calm green theme").

**UI**
- Appearance page accent-from-wallpaper control and suggestions, theme import/export sheet; states: suggestions ready, no good accent found (falls back), importing, preview, invalid bundle, contrast adjusted, applied, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names (colour names read in words).
- 👤 Mockup approval before building; gallery sign-off with 10 wallpapers.

**Tests**
- Unit: extraction determinism (seeded), contrast adjustment always yields passing pairs (property test over 1,000 random wallpapers' palettes), bundle schema.
- Integration: import a bundle with a low-contrast accent → adjusted or refused; export → import round-trip identical.
- UI: screenshots for 10 fixture wallpapers × themes; contrast checker over every generated theme.
- Regression: `wallpaper_accent_contrast_property`, `theme_bundle_roundtrip`, `theme_import_no_code`.

**Done when**
- [ ] Owner uses a wallpaper accent for a week; a theme exported on one machine imports identically on the VM.

---

### 16.9 — Omarchy layer
**Outcome:** PriyaOS installs as a layer on Omarchy (and on any btrfs Arch): it detects the existing desktop, integrates without overwriting it, reports keybind conflicts, bridges themes, and uninstalls cleanly.
**Features:** ECO-07.
**Depends on:** 16.7.

**Tasks**
- [ ] Detection of Omarchy (its config and theme directories; verify the conventions against the current Omarchy release when implementing) and of generic Hyprland setups.
- [ ] `priya desktop install` Omarchy mode (extends Phase 5.14): sources our Hyprland snippet after Omarchy's, never edits its files in place; keybind conflict report (e.g. Super+Space, Super+V) with suggested alternatives chosen by the user; Waybar item when Omarchy's Waybar is the bar, Quickshell overlays (Priya bar, notifications) layered on top.
- [ ] Theme bridge: Omarchy theme → PriyaOS token overrides (background, accent) and PriyaOS theme → Omarchy theme files on request; follows Omarchy theme switches.
- [ ] Any btrfs Arch: guided `priya doctor` layout check and fixes (Phase 2), clear answers-only mode on non-btrfs with an explanation of what's missing.
- [ ] `priya desktop uninstall` restores the previous config exactly (backup taken at install, recorded as a ChangeSet).
- [ ] Human guide section for Omarchy users and a docs-site page.

**UI**
- Install report (what was added, conflicts, choices), theme bridge toggle; states: Omarchy detected, generic Hyprland, conflicts found, resolved, answers-only (non-btrfs), uninstalled, Hindi.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** the user chooses how every keybind conflict is resolved; nothing in Omarchy's own files is changed; uninstall is complete.

**Tests**
- Unit: conflict detection on Hyprland config fixtures (Omarchy defaults, user variants), theme bridge mapping.
- E2E (VM): fresh Omarchy install → `pacman -S priya` + `priya desktop install` → bar, notifications, approvals work; Omarchy keybinds still work; theme switch in Omarchy reflected; uninstall → config byte-identical to before.
- E2E (VM): plain Arch on btrfs with a non-PriyaOS layout → doctor guidance → sandbox works after fixes; ext4 Arch → answers-only mode explained.
- Regression: `omarchy_layer_install`, `omarchy_uninstall_restores`, `generic_btrfs_arch_doctor`.

**Done when**
- [ ] PriyaOS installs as a layer on a fresh Omarchy VM and uninstalls cleanly; conflicts are reported, not silently overridden.

---

### 16.10 — Phase gate
**Outcome:** the ecosystem works for a third-party-style developer and for the owner.
**Features:** all Phase 16 features.
**Depends on:** 16.1–16.9.

**Tasks**
- [ ] Full regression and nightly suites green (including Omarchy VM e2e and directory integrity suites).
- [ ] Third-party-style exercise: build a Python module from the template using only the docs site, publish it through the directory pipeline under a test publisher, install it by asking.
- [ ] Update `features.md`, `docs/human/05-the-phases.md`, `docs/human/04-everything-priyaos-will-do.md`.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green, including nightly e2e (Omarchy layer, generic btrfs Arch), security (directory integrity, mini-app sandbox, action effect mismatch) and eval (install by asking, mini-apps, restyling) suites.
- [ ] Owner demo: "get me a Spotify control skill" → verified item → permissions → install → "pause Spotify" from the bar and from an agent; "make my desktop warmer and add a focus timer" → preview → keep → undo → redo; "build me a dashboard of today's agent costs" → mini-app reopened the next day; wallpaper accent applied; theme shared to the VM.
- [ ] Third-party-style Python module built from the docs alone, published through the pipeline and installed from the directory.
- [ ] L4 (Zenbook): restyle preview in a nested Hyprland performs smoothly on the OLED display; wallpaper accents look right on OLED-black.
- [ ] Docs: [02 §5](../architecture/02-core.md) (SDKs, action manifests), [05 §9](../architecture/05-security.md) (directory, publisher verification), [11](../architecture/11-design-system-and-ux.md) (mini-app runtime, wallpaper accents, theme bundles), [10 §5](../architecture/10-base-os-and-distro.md) (Omarchy layer) updated; ADRs for directory hosting and mini-app runtime merged; feature statuses and human status updated.

## Out of scope

- Paid items, payments or revenue sharing in the directory.
- Ratings, reviews or install counts (no usage tracking).
- Mobile-native or web-hosted versions of mini-apps.
- Distribution layers for non-Arch distributions.
- Themes that ship code (themes are token overrides and images only).
