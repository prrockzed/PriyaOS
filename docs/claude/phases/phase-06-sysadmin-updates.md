# Phase 6 — System administration and safe updates

**Goal.** Priya runs the machine well: updates are tested on a copy and booted before they're applied, broken things are fixed by an agent in the sandbox, and everyday problems (slow, battery, Wi-Fi, failed services, broken packages) are diagnosed and fixed in plain language.

**Depends on:** Phases 1–5.
**Features:** ADM-03 … ADM-13.
**Read first:** [03 §4](../architecture/03-execution-and-sandbox.md) (boot checks), [10 §2](../architecture/10-base-os-and-distro.md).

## Build

### 6.1 `updates` module
- [ ] `priya update`: sandbox fork → `pacman -Syu` (+ AUR/Flatpak updates if enabled) → boot check → new failed units vs live → agent fix loop (max 2) → review with upgraded packages, fixes and boot result → system merge on reboot
- [ ] Arch news check (archlinux.org news feed) for manual-intervention notices, summarised in the review
- [ ] Kernel handling per `/boot` location ([03 §2](../architecture/03-execution-and-sandbox.md))
- [ ] Update availability check (daily, read-only) → top-bar badge

### 6.2 `sysadmin` module (diagnostics, read-only first)
- [ ] Tools (exposed to agents and Priya AI): processes and resource usage, disk usage and SMART, journal queries, failed units, network state (NetworkManager), Bluetooth, audio (PipeWire), displays (Hyprland), power/battery, package integrity (`pacman -Qk`), dependency issues
- [ ] Diagnosis flow: read-only run gathers facts → Priya AI explains in plain language → proposes a fix task (mode by rules) → verify step re-checks the original symptom

### 6.3 Health and maintenance
- [ ] `priya health`: scored checklist (disk space, failed units, orphaned packages, pacnew files, old kernels, SMART, battery health, timeline/blob space)
- [ ] Maintenance actions as safe tasks (clean caches, merge `.pacnew` with AI help in sandbox, remove orphans)

### 6.4 Dev environments
- [ ] "Set up a dev environment for <project>" → detects stack, installs toolchains in sandbox, writes `.priya.toml` setup/run/check

## Tests

- L2: update on the fake machine with a deliberately broken service (unit that fails after upgrade) → boot check catches it → agent fix → re-check passes → merge.
- L3 (VM): real `pacman -Syu` against a VM snapshot pinned to an older Arch archive date (Arch Linux Archive) so updates exist; reboot; rollback; reboot.
- Diagnosis fixtures: recorded command outputs for common problems → expected explanations (judged by rubric).

## Done when

- [ ] A full real update in the VM goes through boot check, merges, survives reboot, and rolls back cleanly
- [ ] Five scripted problems (full disk, failed service, broken package dependency, Wi-Fi misconfig, runaway process) are diagnosed correctly and fixed with verification
- [ ] `priya health` is understandable by the owner without explanation

**Demo.** `priya update` with a forced breakage → watch Priya fix it in the sandbox → merge → reboot → healthy.
