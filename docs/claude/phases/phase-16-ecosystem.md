# Phase 16 — Ecosystem

**Goal.** Others can build for PriyaOS and PriyaOS reaches more people: module SDK and docs, a directory of trusted skills and integrations, an SDK for apps to expose structured actions, generated mini-apps, a desktop that restyles itself by plain language, and PriyaOS as a layer on Omarchy and any btrfs Arch.

**Depends on:** Phases 9, 14.
**Features:** ECO-01 … ECO-05, ECO-07, TOOL-05, UX-15, UX-16.
**Read first:** [02 §5](../architecture/02-core.md), [05 §9](../architecture/05-security.md), [10 §5](../architecture/10-base-os-and-distro.md).

## Build

- [ ] Module SDK packages for Rust, Python, TypeScript; templates (`priya module new`); docs site generated from API schemas
- [ ] Skill and module directory: publishing flow, verified publishers, integrity checks, capability display, ratings; install via plain language ("get me a Spotify control skill")
- [ ] App action SDK: apps ship MCP servers/manifests; tool index registers them automatically; consistent permission model
- [ ] Generated mini-apps: "make me a dashboard of my deploys" → Priya AI generates a small Quickshell/web app with declared data permissions → saved, reopenable, editable by plain language
- [ ] Self-restyling desktop: plain-language desktop changes in sandbox with live preview (nested Hyprland / special workspace) → merge
- [ ] Omarchy layer: `priya desktop install` integrates with Omarchy config conventions; install docs for Omarchy users

## Tests

- SDK templates build and pass the module contract suite in all three languages.
- Directory: unsigned/modified package rejected; capability prompt shown.
- Mini-app generation: 10 scripted requests produce working apps within declared permissions.

## Done when

- [ ] A third-party-style module written from the template in Python installs from the directory and works
- [ ] Owner restyles the desktop by plain language and keeps or discards it from a preview
- [ ] PriyaOS installs as a layer on a fresh Omarchy VM

**Demo.** "Make my desktop warmer and add a focus timer" → preview → keep; "build me a dashboard of today's agent costs" → mini-app opens.
