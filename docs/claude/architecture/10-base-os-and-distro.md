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

Hosting: the pacman repository needs a stable URL, so the repo database and packages are published to a static host (GitHub Pages or object storage) by the release pipeline; GitHub Releases keep the ISO and checksums. Package signing: a PriyaOS packager GPG key (primary key offline, a signing subkey available only to the release job); `priyaos-keyring` package installs it into pacman's keyring. Phase 9 sets this up; before that, local unsigned repos for testing.

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
| Power | power-profiles-daemon, suspend; zram swap; hibernation optional (needs the `@swap` subvolume) |
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
- `priya-install` (dual-boot safe): shows disks, detects Windows partitions and the ESP, asks for the target partition (wiped) and the ESP (reused, never formatted), refuses Windows/ESP/too-small targets, typed confirmation, optional LUKS, creates the btrfs layout, installs with `pacstrap` directly (archinstall's config API changes too often to depend on), writes fstab without `subvol=`/`subvolid=` on `/` (root comes from `rootflags`), installs GRUB to `/efi` with os-prober, enables services, creates the user.
- The Phase 2 dev installer is text-based; the Phase 9 installer is graphical (same design language) on the same `priya-install` backend.
- **First-boot setup** (Quickshell wizard + CLI fallback), the same steps as [11 §4](11-design-system-and-ux.md) Onboarding: Welcome & language · Wi-Fi · AI accounts (keys, agent CLI logins, local model detection) · How careful Priya should be (autonomy level and default mode) · Privacy & folders · Appearance (light/dark/auto, accent) · Voice (optional) · Import from a previous install (optional). Everything later changeable by plain language.

## 4. Hardware: ASUS Zenbook 14 OLED

The owner's exact model and disk layout were surveyed in 0.1 and are kept private, since this repo is public. General notes for this laptop family (UX3405 Intel and UM3406 AMD variants exist): `sof-firmware` for audio, current `linux` kernel for Lunar Lake, OLED: prefer dark themes and enable Hyprland's `misc:vrr` carefully; Secure Boot off until Phase 14; Windows BitLocker recovery key needed before changing boot settings.

## 5. Other installation targets

- **Any existing Arch**: from Phase 0 the package runs Priya AI answers (no btrfs needed).
- **Existing Arch on btrfs with the right layout**: `priya doctor --fix` enables Priya Sandbox from Phase 2.
- **Dev installer (Phase 2)**: a minimal ISO that installs Arch + the PriyaOS disk layout + `priya` next to Windows, so the owner dogfoods on real hardware long before the polished Phase 9 distro.
- **Omarchy**: same as existing Arch; `priya desktop install` integrates with Omarchy's Hyprland config instead of replacing it (Phase 16 distribution strategy).
- **VMs**: QEMU/KVM, UTM (x86 emulation is slow on Apple Silicon; prefer the Zenbook).
