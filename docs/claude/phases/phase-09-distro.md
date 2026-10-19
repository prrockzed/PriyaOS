# Phase 9 — Distro v0.1 (installable PriyaOS)

**Goal.** PriyaOS becomes a complete operating system: our signed package repo, base OS defaults (everything a normal desktop OS needs), branding, an installer USB, a dual-boot-safe installer, a first-boot setup — installed on the owner's Zenbook next to Windows.

**Depends on:** Phases 0–8.
**Features:** OS-02 … OS-14, SBX-13, ECO-06.
**Read first:** [10-base-os-and-distro](../architecture/10-base-os-and-distro.md), ADR-001, ADR-008.

## Build

### 9.1 Packages and repo
- [ ] Split packages: `priya` (core + helper + CLI), `priya-modules-*`, `priyaos-shell` (Quickshell config), `priyaos-defaults`, `priyaos-branding`, `priyaos-keyring`
- [ ] Packager GPG key; `[priyaos]` repo built by `scripts/build-repo.sh`, published to GitHub Releases (or static hosting); `repo-add` signed DB

### 9.2 Base OS defaults (`priyaos-defaults`)
- [ ] Hyprland + Quickshell shell (bar, launcher = Priya bar, notifications, hyprlock, hypridle), kitty, file manager, Firefox
- [ ] Panels: network, Bluetooth, audio, power, displays, night light, keyboard/IME (fcitx5, Hindi), accessibility (Orca, scaling)
- [ ] App installs (pacman / paru / Flatpak) routed through Priya Sandbox by default
- [ ] nftables firewall defaults; printing/scanning optional group; optional restic backup module
- [ ] Multi-user correctness (per-user priyad, `priya-admin` for system ops)

### 9.3 ISO and installer
- [ ] archiso profile (releng-based) with `[priyaos]`, `scripts/build-iso.sh`
- [ ] `priya-install`: disk listing with Windows/ESP detection, target and ESP selection with refusals, typed confirmation, optional LUKS2, btrfs layout, archinstall `pre_mounted_config`, fstab `subvol=` only, GRUB to `/efi` + os-prober, services, user, groups
- [ ] Unattended answers file (live ISO only) for VM tests
- [ ] GRUB entry "PriyaOS (previous system)" that boots the newest `@priya-prev-*` via `rootflags=subvol=`, regenerated after every system swap (SBX-13)

### 9.4 First-boot setup
- [ ] Wizard (Quickshell + CLI fallback): Wi-Fi, language, AI providers (keys into Secret Service; agent CLI installs and logins; local model detection and optional Ollama install), strategy, default mode, privacy defaults, permitted folders, voice on/off (Phase 10 later)

### 9.5 Branding
- [ ] GRUB theme, Plymouth (optional), wallpapers, `os-release` (`NAME="PriyaOS"`, `ID=priyaos`, `ID_LIKE=arch`)

## Tests

- L3 (VM, main gate): disk with fake Windows (ESP 260 MiB + NTFS "Windows") → install → GRUB lists PriyaOS and the fake Windows entry → first boot wizard (scripted) → Phase 4–8 scenarios → update → reboot → rollback → reboot. Also an encrypted (LUKS) install run.
- Installer refusal tests: picking the ESP, an NTFS Windows partition, a too-small partition → refused.
- L4 (Zenbook): install checklist from `docs/human/07-install-on-your-zenbook.md` — Windows still boots (BitLocker key ready), Wi-Fi, audio, brightness, suspend/resume, external display, battery reporting.

## Done when

- [ ] VM suite green, including LUKS variant
- [ ] Owner installs from USB on the Zenbook following the human guide without help beyond it
- [ ] Hardware checklist passes on the Zenbook; Windows boots
- [ ] All earlier phase demos work on the installed system

**Demo.** Boot from USB, install next to Windows, first-boot setup, Super+Space works on a fresh PriyaOS.
