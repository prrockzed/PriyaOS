# 10 — Base OS and distribution

## 1. How PriyaOS is built on Arch (no fork)

We do **not** clone or fork Arch's repositories or the kernel. PriyaOS = Arch's official packages, unchanged + our packages + our defaults + our installer.

| Part | Source | Delivery |
|---|---|---|
| Kernel, drivers, firmware, base system, desktop apps | Arch official repos (`core`, `extra`, `multilib`) | Arch mirrors, as on any Arch |
| PriyaOS software (`priya`, `priya-helper`, modules, Quickshell shell, configs) | This repository | **Our pacman repo** `[priyaos]` (signed), served from GitHub Releases or a static host |
| Agent CLIs not in Arch repos | Upstream installers / AUR | Installed by first-boot setup from their official sources |
| Defaults and branding | `priyaos-defaults`, `priyaos-branding` packages | `[priyaos]` repo |
| Installer media | archiso profile in `iso/` | ISO on GitHub Releases |

Updates: `pacman -Syu` (through Priya Sandbox) updates both Arch's and our packages. Same model as EndeavourOS, CachyOS and Omarchy.

Package signing: a PriyaOS packager GPG key; `priyaos-keyring` package installs it into pacman's keyring. Phase 9 sets this up; before that, local unsigned repos for testing.

## 2. Base OS features (what any good desktop OS needs)

Shipped and configured by `priyaos-defaults` (Phase 9), each also controllable by plain language:

| Area | Choice |
|---|---|
| Boot | GRUB with `/boot` inside `@`, os-prober for Windows, Plymouth splash (optional) |
| Filesystem | btrfs (zstd:1, noatime), layout from [03](03-execution-and-sandbox.md) §2; optional LUKS2 encryption at install |
| Desktop | Hyprland + Quickshell shell (bar, launcher = Priya bar, notifications, lock screen via hyprlock, idle via hypridle) |
| Terminal / files / browser | kitty, Nautilus or Thunar, Firefox (user can swap) |
| Network | NetworkManager (+ nm-applet / Quickshell panel), iwd backend |
| Bluetooth | bluez + panel |
| Audio | PipeWire + WirePlumber, sof-firmware |
| Power | power-profiles-daemon, TLP-free defaults, suspend/hibernate |
| Display | Hyprland monitor config UI (Quickshell panel), night light (hyprsunset) |
| Input | keyboard layouts, IME via fcitx5 (Hindi and others) |
| Printing / scanning | CUPS + sane (optional group) |
| Apps | pacman + AUR helper (paru) + Flatpak (Flathub), all installs routed through Priya Sandbox by default |
| Security | nftables firewall defaults, automatic security-update tasks, Secure Boot (Phase 14) |
| Backup | Priya Rewind timeline + optional off-device backup (restic/borg) module |
| Accessibility | screen reader (Orca with AT-SPI), scaling, high contrast theme |
| Users | multi-user supported; each user has their own priyad; `priya-admin` group for system operations |

## 3. ISO and installer

- archiso profile based on `releng`, with our `[priyaos]` repo and package list.
- Live environment: Wi-Fi via `iwctl` or `nmtui`, then `priya-install`.
- `priya-install` (dual-boot safe): shows disks, detects Windows partitions and the ESP, asks for the target partition (wiped) and the ESP (reused, never formatted), refuses Windows/ESP/too-small targets, typed confirmation, optional LUKS, creates the btrfs layout, installs via archinstall in `pre_mounted_config` mode, writes fstab with `subvol=` only, installs GRUB to `/efi` with os-prober, enables services, creates the user.
- **First-boot setup** (Quickshell wizard + CLI fallback): Wi-Fi, language, connect AI providers (API keys / subscription logins for agent CLIs / local Ollama detection), pick default strategy (best / balanced / cheapest / local-only), default execution mode, permitted folders for the file index, voice on/off, privacy defaults. Everything later changeable by plain language.

## 4. Hardware: ASUS Zenbook 14 OLED

Exact model unknown until Phase 0 (`cat /sys/class/dmi/id/product_name`): UX3405MA (Intel Meteor Lake), UX3405CA (Intel Lunar Lake) and UM3406 (AMD) all exist. Notes: `sof-firmware` for audio, current `linux` kernel for Lunar Lake, OLED: prefer dark themes and enable Hyprland's `misc:vrr` carefully; Secure Boot off until Phase 14; Windows BitLocker recovery key needed before changing boot settings.

## 5. Other installation targets

- **Existing Arch on btrfs**: `pacman -S priya` from `[priyaos]` + `priya doctor --fix` (Phase 4 onward, for dogfooding before the ISO).
- **Omarchy**: same as existing Arch; `priya desktop install` integrates with Omarchy's Hyprland config instead of replacing it (Phase 16 distribution strategy).
- **VMs**: QEMU/KVM, UTM (x86 emulation is slow on Apple Silicon; prefer the Zenbook).
