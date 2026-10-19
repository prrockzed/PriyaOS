# Phase 9 — PriyaOS 1.0 distribution

**Goal.** PriyaOS becomes a complete operating system anyone can install: signed packages in our own repository and a release pipeline; a full default desktop with normal Linux apps; every base-OS settings panel; app installs (pacman, AUR, Flatpak) through Priya Sandbox; firewall, printing and backups; multiple users; a graphical, dual-boot-safe installer with optional encryption; a "previous system" boot entry; first-boot onboarding; and one consistent look from boot menu to lock screen. The Phase 2 dev installer and disk layout are polished into this 1.0.

**Usable at the end:** boot the PriyaOS USB, install next to Windows with the graphical installer (Windows shown locked and untouched), optionally encrypt, restart, pick PriyaOS in a themed boot menu, finish onboarding (Wi-Fi, AI accounts, how careful Priya should be, folders, appearance), press Super+Space — on a fresh, branded PriyaOS 1.0 whose every earlier phase works. Windows still boots.

**Depends on:** Phases 0–8. In particular Phase 2 (dev installer and disk layout 2.5, system merge and rollback 2.12), Phase 5 (shell, Settings app 5.8, theming 5.11, accessibility 5.13, `priya desktop install` 5.14), Phase 6 (`priya update`, Health), Phase 7 (Rewind timeline), Phase 8 (file index folder picker, tool index).

**Features:** SBX-13, ECO-06, OS-02, OS-03, OS-04, OS-05, OS-06, OS-07, OS-08, OS-09, OS-10, OS-11, OS-12, OS-13, OS-14, OS-16, DS-19, QA-21.

**Read first:** [10-base-os-and-distro](../architecture/10-base-os-and-distro.md) (all), [03 §2 layout, §5.2 system merge, §5.4 rollback](../architecture/03-execution-and-sandbox.md), [01 §2 processes, §5 storage](../architecture/01-system-overview.md), [05 §3 secrets, §4 approvals, §10 platform hardening](../architecture/05-security.md), [09 §3 VM harness, §4 CI release stage ⑤, §8 L4](../architecture/09-testing.md), [11 §3 themes incl. login/lock/boot, §4 Settings, Onboarding, Installer, Lock and login, §7 accessibility, §9 design process](../architecture/11-design-system-and-ux.md), ADR-001, ADR-005, ADR-008, ADR-014, and `docs/human/07-install-on-your-zenbook.md`.

## Design notes for this phase

- **Safety.** Installer code that writes disks (`sgdisk`, `cryptsetup`, `mkfs.btrfs`, `grub-install`, `efibootmgr`) runs only in the QEMU VM during development, and on real hardware only when the owner runs the installer from the USB. The plan/validate logic is pure and tested on recorded disk layouts.
- **Installer architecture.** The Phase 2 `priya-install` is split into an engine (typed `InstallPlan` → validate → journaled apply) and frontends: the graphical one (Quickshell under the `cage` kiosk compositor on the live ISO), a TUI fallback, and an unattended answers file accepted only on the live ISO for VM tests. Add the engine crate to [01 §6](../architecture/01-system-overview.md).
- **System-part changes need a restart.** Today every system merge applies on restart ([03 §5.2](../architecture/03-execution-and-sandbox.md)). For app installs that is poor UX; 9.4 contains the decision (ADR) on a live path for additive installs. Until it lands, cards say "ready after restart" and Flatpak defaults to per-user installs, which merge live.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 9.1 | Packages, signed repo, release pipeline | Phase 8 | `pacman -S priyaos` from our signed `[priyaos]` repo; tagging `v*` produces signed packages, ISO and release notes |
| 9.2 | Default apps and shell | 9.1 | A fresh install has a complete desktop: shell, lock, idle, terminal, files, browser, viewers, fonts, portals |
| 9.3 | System panels complete | 9.2 | Settings has Network, Bluetooth, Sound, Power, Displays, Keyboard & input (incl. Hindi), Accessibility, Date & time, About — all also by plain language |
| 9.4 | App installs through the sandbox | 9.1 | "Install VLC/GIMP/an AUR tool" → sandbox → review → available; Flatpak apps work like any app |
| 9.5 | Firewall, printing, backup | 9.3 | Firewall on by default; printers added in a click; backups to a USB disk or cloud on a schedule |
| 9.6 | Multi-user | 9.3 | A second user with their own Priya, memory and sandbox; only admins change the system |
| 9.7 | Graphical installer | 9.1, 9.2 | Install from USB with a disk picture that locks Windows and refuses unsafe targets |
| 9.8 | Encryption option | 9.7 | Optional full-disk encryption with one passphrase at start-up and a recovery plan |
| 9.9 | Previous-system boot entry | 9.1 | Boot menu entry "PriyaOS (previous system)" after any system change; rollback from there |
| 9.10 | First-boot onboarding | 9.3, 9.7 | Six-step setup: Welcome, Wi-Fi, AI accounts, how careful, folders, appearance |
| 9.11 | Branding (boot, login, lock, wallpapers) | 9.2 | Boot menu, splash, login, lock screen and wallpapers all styled from the design tokens |
| 9.12 | Zenbook hardware validation | 9.2–9.11 | Hardware checklist passes on the Zenbook; quirks captured in a hardware profile |
| 9.13 | Phase gate (fresh install by the owner) | all | The owner installs 1.0 on the Zenbook from USB following the human guide, Windows still boots |

---

### 9.1 — Packages, signed repo, release pipeline
**Outcome:** all PriyaOS software ships as signed packages from our `[priyaos]` pacman repo, trusted through `priyaos-keyring`; pushing a `v*` tag runs the release stage that builds, signs and publishes packages, the ISO, checksums and release notes after every earlier CI stage is green on that commit.
**Features:** OS-02, OS-03, QA-21.
**Depends on:** Phase 8.

**Tasks**
- [ ] PKGBUILDs under `packaging/`: `priya` (priyad, priya-helper, CLI, units, sysusers, tmpfiles), `priya-modules` (first-party modules; split later if needed), `priyaos-shell` (Quickshell config, PriyaUI, apps), `priyaos-defaults`, `priyaos-branding`, `priyaos-keyring`, `priyaos-installer` (live ISO only), meta-package `priyaos`; built in a clean chroot (`makechrootpkg`, devtools) with pinned toolchains.
- [ ] Key management ADR: packager primary key kept offline by the owner; a signing subkey stored as a secret of the GitHub environment `release` that requires owner approval; revocation procedure; `priyaos-keyring` ships `priyaos.gpg`, `priyaos-trusted`, `priyaos-revoked` and runs `pacman-key --populate priyaos`.
- [ ] 👤 Owner creates the primary key and subkey following a step-by-step note Claude writes (Claude never handles the private key).
- [ ] Repo hosting ADR (stable URL needed for the database: GitHub Pages or a static bucket vs a fixed GitHub Release tag); `scripts/build-repo.sh` → `repo-add --sign --verify`; channels `[priyaos]` (stable) and `[priyaos-testing]`; `/etc/pacman.d/priyaos-mirrorlist`; `pacman.conf` adds `[priyaos]` after Arch's repos with `SigLevel = Required`.
- [ ] Release workflow (stage ⑤ in [09 §4](../architecture/09-testing.md)): verify stages ①–③ green on the tagged commit → build packages → sign → publish to `[priyaos-testing]` → VM upgrade test from the previous release → promote to `[priyaos]` → build ISO (`scripts/build-iso.sh`, archiso profile `iso/` based on `releng`, records the Arch mirror date) → SHA-256 + BLAKE2 checksums and detached GPG signature → draft GitHub Release with notes.
- [ ] Release notes: generated from merged PR titles and `features.md` status changes, drafted by Priya AI, edited and published by the owner (👤).
- [ ] Versioning: `priyaos 1.0.0`; `os-release` `VERSION_ID`; packages follow semver with `pkgrel` for packaging-only changes; upgrade path N-1 → N tested every release.
- [ ] Reproducibility check: build each package twice in clean chroots and compare (`diffoscope` report on mismatch, not yet a hard gate).
- [ ] ISO size budget < 2.5 GiB; UEFI-only boot (documented).

**Tests**
- Unit: release-notes generator on fixture PR lists; version bump checks.
- Integration: `build-repo.sh` on CI with a throwaway test key → a container with `priyaos-keyring` installs `priyaos` from the repo; tampered package or database → pacman refuses; revoked key → refused.
- E2E (VM): previous release installed → `priya update` pulls the new `[priyaos]` packages through the sandbox → restart → all regression journeys pass.
- Security: release workflow runs only on tags from this repository; signing secret unavailable to PR workflows (workflow policy test); no private key material in the repo (secret scan).
- Regression: `repo_signed_install`, `repo_rejects_tampered`, `release_upgrade_n_minus_1`.

**Done when**
- [ ] A dry-run tag (`v1.0.0-rc.1`) produces signed packages, ISO, checksums and draft notes with no manual steps except the owner's approvals.
- [ ] Unsigned or tampered artefacts are refused by pacman in the integration test.

---

### 9.2 — Default apps and shell
**Outcome:** a fresh PriyaOS has a complete, themed desktop session: Hyprland with the Quickshell shell (top bar, Priya bar, notifications, OSDs), lock and idle, terminal, file manager, browser, everyday apps, portals, fonts and icons — all normal Linux apps working alongside the AI-native parts.
**Features:** OS-06, ECO-06.
**Depends on:** 9.1.

**Tasks**
- [ ] `priyaos-defaults`: Hyprland config in `/usr/share/priya/desktop/` with `/etc/skel/.config/hypr/hyprland.conf` sourcing it (user overrides stay in the user file); greetd as the display manager launching the Hyprland session.
- [ ] Session services: Quickshell shell (top bar, Priya bar, notifications, OSDs from Phase 5), hyprlock, hypridle (dim 3 min, lock 5 min, suspend 15 min on battery / 30 min on AC; settings-backed), hyprpolkitagent, xdg-desktop-portal-hyprland + -gtk, PipeWire/WirePlumber, NetworkManager (iwd backend), bluez, power-profiles-daemon, hyprsunset, wl-clipboard + clipboard history, grim/slurp.
- [ ] 👤 Default app set approved by the owner (proposal: kitty, Nautilus, Firefox, image viewer, PDF viewer, text editor, archive manager, calculator, system monitor); file-manager choice recorded as an ADR (10 §2 lists Nautilus or Thunar).
- [ ] MIME defaults (`/etc/xdg/mimeapps.list`), fonts (Inter, JetBrains Mono, Noto Sans Devanagari, Noto Color Emoji with a fontconfig fallback chain), Papirus icons with accent folders, Bibata cursor; whole-desktop theme applied by `priya theme apply` at first login (5.11).
- [ ] ECO-06: normal apps from pacman and Flatpak appear in the launcher and tool index (8.2), follow light/dark via the portal, and open files via MIME defaults; a compatibility list of apps tested (GTK3, GTK4, Qt6, Electron, Flatpak).
- [ ] Session robustness: shell crash → restarted by systemd user unit; priyad down → bar shows "Priya is starting" and the desktop still works (UX-14).
- [ ] `priyaos-defaults` never overwrites user files on upgrade (configs in `/usr/share` or `/etc/skel`, `.pacnew` for `/etc`).

**UI**
- Whole session as shipped: top bar, launcher (Priya bar), notifications, OSDs, lock and idle behaviour, default wallpapers (from 9.11).
- States: first login (theme being applied), normal, priyad unavailable, offline, battery saver (blur off), long app names, Hindi text in app names and notifications.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0 (Zenbook OLED native 2880×1800 at 2.0).
- Keyboard path: every default keybind listed in a cheat sheet (Super+/); screen-reader names verified for shell surfaces with Orca.
- 👤 Mockup approval of the default desktop (`ui/mockups/9.2/`) before building; journey sign-off ("first login → open files → open a PDF → lock → unlock").

**Tests**
- Unit: config generators (hypridle timings from settings), MIME list validation.
- Integration (headless Hyprland): session starts with every default service active; killing Quickshell restarts it; priyad stopped → bar degraded state, apps still launch.
- UI: screenshots of the default desktop in every theme and scale; a11y lint on shell components.
- E2E (VM): fresh install → login → open each default app → GTK and Qt apps follow dark mode → Flatpak test app follows dark mode.
- Perf: login to usable desktop < 4 s on the Zenbook; idle session RAM (shell + priyad) < 400 MiB.
- Regression: `session_default_services`, `session_survives_priyad_down`, `default_apps_open`, `theme_follows_portal`.

**Done when**
- [ ] Every default app opens and follows the theme in the VM journey; owner approves the app set.

---

### 9.3 — System panels complete
**Outcome:** Settings has every panel a desktop OS needs — Network, Bluetooth, Sound, Power & battery, Displays, Keyboard & input (layouts and Hindi input), Accessibility, Date & time, About — each also reachable by plain language and quick panels in the top bar.
**Features:** OS-07, OS-08, OS-11, OS-19.
**Depends on:** 9.2.

**Tasks**
- [ ] Settings sidebar additions (update [11 §4](../architecture/11-design-system-and-ux.md)): Network, Bluetooth, Sound, Power, Displays, Keyboard & input, Accessibility, Date & time, Users (9.6), Printers (9.5), Backup (9.5), Firewall (9.5); About shows `os-release`, hardware, versions.
- [ ] Network: Wi-Fi list (signal, security), connect with password prompt, hidden networks, forget, airplane mode, wired, VPN (WireGuard import, OpenVPN via NM plugin), metered flag; backed by NetworkManager D-Bus.
- [ ] Bluetooth: on/off, scan, pair (PIN/confirm prompts), connect, device battery; BlueZ D-Bus.
- [ ] Sound: output/input devices, volume and mute per device and per app, profiles (A2DP/HFP), test sound, mic level; PipeWire via `wpctl`/`pw-dump`.
- [ ] Power & battery: profiles (power-profiles-daemon), battery %, health and time estimates (UPower), charge limit 80 % when the firmware supports `charge_control_end_threshold`, lid and power-button actions, idle timings (9.2), suspend/hibernate options (hibernate shown only when a swap target exists).
- [ ] Displays: arrangement drag, resolution, refresh, scale, rotation, mirror, VRR toggle (off by default on OLED), night light schedule (hyprsunset); writes the user's Hyprland monitor file as a home change (undoable with Rewind) with a 15 s "Keep these settings?" revert timer.
- [ ] Keyboard & input: layouts and switching shortcut, fcitx5 with Hindi (Devanagari via m17n, phonetic/InScript) and other IMEs, key repeat, touchpad (tap, natural scroll, gestures), mouse speed.
- [ ] Accessibility (OS-11): Orca on/off (Super+Alt+S), text scaling 80–200 %, high-contrast theme, reduced motion and transparency, larger cursor, sticky/slow keys; all surfaces tested with Orca.
- [ ] Date & time: timezone (manual list; no location lookup by default), NTP, 24 h clock.
- [ ] Plain language for every panel: navigation intents and settings mappings ("turn on Bluetooth", "make text bigger", "add Hindi typing", "set the external screen to the left") through 5.2 and 5.9.
- [ ] Firmware updates through fwupd/LVFS, run as a sandboxed-check + approval task, listed on the Updates page

**UI**
- Each panel: default, edited, invalid, locked by admin (11 §4), plus loading, device absent (no Bluetooth adapter / no battery), permission needed, error, offline, long content (50 Wi-Fi networks), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; font scaling at 150 % without clipping.
- Keyboard path for every control; focus order tested; screen-reader names for all controls (e.g. "Wi-Fi network Home, secured, signal strong, connected").
- 👤 Mockup approval per panel group (`ui/mockups/9.3/`) before building; gallery sign-off; Orca journey sign-off ("connect to Wi-Fi with a screen reader only").

**Tests**
- Unit: display-arrangement to Hyprland monitor-line generator; charge-limit capability detection; IME config writer.
- Contract: panel backends against fakes (NetworkManager, BlueZ and UPower D-Bus mocks via `python-dbusmock`; recorded `pw-dump`).
- Integration (headless Hyprland + dbusmock): every panel action produces the expected D-Bus call or file change; display change auto-reverts after 15 s without confirmation.
- UI: every panel × state × theme × scale screenshots; a11y lint; keyboard-only and Orca journeys.
- E2E (VM): virtual Wi-Fi (`mac80211_hwsim`) connect from the panel; Hindi IME types Devanagari into a GTK text field.
- Regression: `panel_network_hwsim_connect`, `panel_display_auto_revert`, `panel_ime_hindi`, `panels_a11y_journeys`, `panels_nl_intents`.

**Done when**
- [ ] Each panel works in the VM and on the Zenbook (9.12) and has a plain-language path.
- [ ] 👤 Owner completes the Orca Wi-Fi journey.

---

### 9.4 — App installs through the sandbox
**Outcome:** installing apps from official repos, the AUR or Flathub — by plain language, by `priya install`, or from an Apps page — goes through Priya Sandbox with a review showing source, size and permissions; apps can be removed the same way and undone with Rewind.
**Features:** OS-09, ECO-06.
**Depends on:** 9.1.

**Tasks**
- [ ] Flatpak adapter for the `PackageManager` port (P2 "later" → now) with the port's contract suite; Flathub remote added by `priyaos-defaults`.
- [ ] Source resolver: official repo first, then Flathub (verified apps preferred), then AUR; the card states the source and why; the user can choose another source in place.
- [ ] Install task `priya install <name|description>` / "install VLC": sandbox fork → install → for AUR: `paru` build as the user in the fork, PKGBUILD shown and reviewed by a `review`-role model from a different family, run tainted → review (source, version, size, new services, Flatpak permissions from metadata) → merge.
- [ ] Flatpak installs default to `--user` (home part, live merge); system Flatpaks on request.
- [ ] ADR + implementation spike: live apply for additive-only system installs (no existing file modified or deleted outside the new packages, no kernel/initramfs/bootloader/unit changes) by replaying the same pacman transaction live from the cached packages and checking the resulting ChangeSet matches the fork's; otherwise "ready after restart".
- [ ] Remove: `priya remove <app>` / "uninstall GIMP" as a sandbox task (`pacman -Rns` / `flatpak uninstall`), with dependents listed.
- [ ] Apps page in Settings: installed apps (pacman explicit, AUR, Flatpak) with source, size, remove; "Install an app…" opens the bar.
- [ ] Terminal installs (`sudo pacman -S` typed by the user) keep working normally (UX-14) and are captured by Rewind's pacman hooks.

**UI**
- Install card (source chip, version, size, permissions, "ready now" or "ready after restart"), review, Apps page.
- States: resolving, several sources (choose), installing, AUR building (log behind Details), ready for review, installed, needs restart, failed (with Ask why), untrusted (AUR) banner, removing, offline (cached packages only), long content (500 apps), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard path (source chip with arrow keys, Enter installs); screen-reader names ("VLC from Arch repository, 45 MB, ready now").
- 👤 Mockup approval (`ui/mockups/9.4/`) before building; journey sign-off ("install GIMP from Flathub → open it → remove → undo removal").

**Human in the loop:** source, permissions and untrusted label are shown before merge; AUR system merges are `confirm` tier.

**Tests**
- Unit: source resolver ranking, Flatpak permission summary, additive-install classifier.
- Contract: `PackageManager` suite for pacman, paru (recorded outputs) and Flatpak (local test remote with a tiny app).
- Integration (fake machine): pacman install from the local test repo merges; Flatpak user install from a local OSTree remote merges live; AUR fixture PKGBUILD builds in the fork, run tainted, merge needs confirm; additive-install live path matches the fork ChangeSet or falls back.
- E2E (VM): "install VLC" → review → available in launcher and tool index; Flathub GIMP (nightly, network) → opens; remove → Rewind revert restores.
- Security: AUR PKGBUILD with `curl | sh` in `build()` → flagged by review model and scanner rules from 3.11; injection text in an AUR comment never reaches a tool call.
- Regression: `install_repo_sandbox`, `install_flatpak_user_live`, `install_aur_tainted_confirm`, `remove_then_undo`.

**Done when**
- [ ] All three sources install and remove through the sandbox in the VM; the live-apply ADR is decided and its outcome documented in [03 §5.2](../architecture/03-execution-and-sandbox.md).

---

### 9.5 — Firewall, printing, backup
**Outcome:** a default-deny inbound firewall is on with a simple panel; printers and scanners work driverless when the optional printing group is installed; scheduled, encrypted off-device backups of home run to a USB disk or a remote target and can be restored through the sandbox.
**Features:** OS-10, OS-14, OS-18.
**Depends on:** 9.3.

**Tasks**
- [ ] Firewall: `nftables.service` enabled with `/etc/nftables.conf` from `priyaos-defaults` (inbound drop except established/related, loopback, required ICMPv6, DHCPv6 client; mDNS opened only when printing is enabled); Firewall panel (on/off, allowed incoming services); hardening and per-app rules are Phase 14 (SEC-19).
- [ ] Printing (optional group `priyaos-printing`): `cups`, `cups-browsed`, `avahi` + `nss-mdns`, `sane`, `sane-airscan`, a scanning app; Printers panel: discovered IPP Everywhere printers added with one click, queue, default printer; plain language "print this PDF two-sided" via `lp` options.
- [ ] Backup module `modules/backup` with restic (ADR restic vs borg): targets USB disk, SFTP, S3-compatible, REST server; repository password and target credentials in Secret Service; schedule (daily default when enabled), source = the latest Rewind timeline snapshot of the user's home (consistent, read-only), excludes caches and user patterns.
- [ ] Off-device backup is an external action: enabling a target creates a visible remembered approval scoped to that target; data goes nowhere else.
- [ ] Restore: `priya backup restore <path> --at <date>` into a sandbox fork → review → merge; full-home restore option for a new machine.
- [ ] Backup health: last success, `restic check --read-data-subset=2%` monthly; Health page (6.10) item when backups are older than 3 days or failing.
- [ ] Backup panel: target setup wizard, schedule, last runs, restore entry.
- [ ] Firewall defaults (nftables: deny inbound except explicitly allowed, allow outbound) shipped and shown on a Firewall settings page

**UI**
- Firewall, Printers and Backup panels; backup setup wizard; restore picker (shares the Rewind time picker).
- States: firewall on/off; printers none/discovered/added/offline/error; backup not set up, setting up, running (progress), last success, failing, target unreachable, restoring, long content, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard path through wizards; screen-reader names ("Backup to USB disk Samsung T7, last backup today 09:10").
- 👤 Mockup approval (`ui/mockups/9.5/`) before building; gallery sign-off.

**Human in the loop:** the user chooses targets and sees exactly where data goes; restores go through review.

**Tests**
- Unit: nftables ruleset generator, restic argument builder (no secrets on the command line; via env from Secret Service), schedule logic.
- Integration (fake machine): firewall rules loaded, inbound TCP to a test port blocked, outbound allowed; backup to a local directory target and a containerised REST server; restore a deleted file via sandbox review.
- E2E (VM): IPP Everywhere virtual printer (`ippeveprinter`) discovered and a test page printed; scheduled backup runs after a timer tick; target removed → health warning.
- Security: backup credentials never appear in events or logs (redaction test).
- Regression: `firewall_default_deny_inbound`, `printer_ipp_everywhere`, `backup_restore_via_sandbox`, `backup_health_warning`.

**Done when**
- [ ] VM: firewall blocks inbound by default, a virtual printer prints, a backup and restore round trip succeeds.

---

### 9.6 — Multi-user
**Outcome:** several people can use one PriyaOS machine: each user has their own priyad, memory, file index, budgets and sandbox runs; only members of `priya-admin` can change the system; users can't see each other's homes through Priya.
**Features:** OS-12.
**Depends on:** 9.3.

**Tasks**
- [ ] Users panel: add user (name, username, password typed by the person or the admin, admin or standard), remove user (home kept or deleted, `confirm`), change account type; implemented through a `users` module with helper ops `users.create`, `users.delete`, `users.set_admin` (`priya-admin` only).
- [ ] Mode rule shipped in defaults: `when = { module = "users" } mode = "direct"` (safety snapshot), placed before the locked privileged→sandbox rule, so account changes apply immediately and are revertible.
- [ ] Per-user services: `priyad.service` and Quickshell per session; first-login onboarding (9.10) for every new user; shared system-wide model catalog and tool index; per-user secrets, memory, indexes, budgets.
- [ ] Isolation checks: helper peer-uid on every op; forks expose only the requester's home; Rewind restores only the requester's home; timeline and manual ChangeSets hide other users' home paths; socket `0600` per user.
- [ ] System merges by standard users → refused with "ask an admin"; a pending system merge by one user blocks another user's system merge (drift) with a plain explanation.
- [ ] Kill switch scope: `priya stop-all` stops the caller's runs; admins get `priya stop-all --all-users` (`confirm`).
- [ ] Session switching: log out / log in through greetd (fast user switching is out of scope).

**UI**
- Users panel and account-type control; "ask an admin" states on system actions.
- States: single user, several users, adding, removing, last admin (cannot remove), standard user viewing a locked panel, error, long names, Hindi names.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard path; screen-reader names ("Ravi, standard user, remove").
- 👤 Mockup approval (`ui/mockups/9.6/`) before building; gallery sign-off.

**Tests**
- Unit: user-op validation (last admin, reserved names), mode-rule ordering.
- Integration (fake machine, two users): concurrent sandbox runs; user B's run can't read user A's home; B's Rewind can't list or restore A's paths; standard user's system merge refused; A's pending system merge → B's refused as drift; stop-all scoped.
- E2E (VM): create a second user from the panel → log in → onboarding → task → merge; delete user.
- Security: helper fuzzing with forged uids and paths into another home; all refused.
- Regression: `multiuser_home_isolation`, `multiuser_rewind_isolation`, `standard_user_system_merge_refused`, `stop_all_scoped`.

**Done when**
- [ ] Every isolation test passes; two users work concurrently in the VM without interference.

---

### 9.7 — Graphical installer
**Outcome:** the ISO boots into a graphical installer in the PriyaOS design language that shows the disk as a picture with Windows locked, installs next to Windows into an existing Linux partition or free space, refuses every unsafe target, never formats the EFI partition, and leaves a boot menu with PriyaOS and Windows.
**Features:** OS-03, OS-04, OS-16, DS-19.
**Depends on:** 9.1, 9.2.

**Tasks**
- [ ] Engine refactor of the Phase 2 `priya-install`: typed `InstallPlan` (target, ESP, encryption, user, locale, hostname, timezone), pure `validate(plan, DiskInventory) -> Vec<Refusal | Warning>`, journaled idempotent `apply` steps; disk inventory from `lsblk -J -O`, `blkid`, `sfdisk -J`; recorded inventories (Zenbook survey from 0.1, synthetic layouts) as fixtures.
- [ ] Refusals (OS-04, each with a plain reason): target is the ESP; target is NTFS/ReFS/exFAT with Windows files or a BitLocker signature (`-FVE-FS-`); Microsoft reserved or Windows recovery partition type GUIDs; target smaller than 40 GiB; target on the install medium; target with mounted partitions; ESP not FAT32 or < 5 MiB free; disk is MBR/BIOS-only; Intel RST/RAID mode hiding NVMe (explains the BIOS change); creating a partition only in existing free space (no resizing of Windows).
- [ ] Warnings: Secure Boot on (explained; PriyaOS signing is Phase 14), Windows Fast Startup/hibernation flag on NTFS, BitLocker detected → recovery-key reminder screen with a checkbox before anything is written.
- [ ] Apply: create/format target (btrfs, or LUKS2 per 9.8), flat subvolume layout from [03 §2](../architecture/03-execution-and-sandbox.md), install with `pacstrap` (the same backend as the Phase 2 dev installer) from the ISO's package set + `[priyaos]`, fstab without `subvol=` on `/`, GRUB to `/efi` with `--bootloader-id=PriyaOS`, os-prober enabled plus an explicit chainload entry for `/EFI/Microsoft/Boot/bootmgfw.efi` as a fallback, `efibootmgr` entry first in order, services, user and groups (`priya-admin`), Rewind timeline enabled, previous-system entry hooks (9.9).
- [ ] Graphical frontend `ui/apps/Installer` on the live ISO under `cage` (auto-started on tty1), talking to the engine over JSON-RPC; TUI fallback `priya-install --tui`; Orca available on the live ISO (Super+Alt+S) and announced at boot.
- [ ] Screens (11 §4): Welcome (language, keyboard, accessibility), Network (optional; offline install from the ISO's packages possible, agent CLIs later), Disk (picture: Windows locked, EFI "shared start-up area — kept", recovery locked, target highlighted), Encryption (9.8), Account (name, username, password, computer name, timezone from a list), Summary (exact actions; type the target's name to confirm), Progress (cancel allowed until formatting starts), Done (remove USB, restart), Error (plain message, save log to the USB).
- [ ] Unattended answers file `priya-install --answers <file>` accepted only when booted from the live ISO with `priya.unattended=1` (VM tests).
- [ ] Update `docs/human/07-install-on-your-zenbook.md` Part 4 for the graphical installer (TUI kept as a fallback).

**UI**
- All installer screens; states: no disks, Windows found, no Windows, several disks, no valid target (explains), BitLocker reminder, Secure Boot warning, offline, installing, error, done, long disk names, Hindi text.
- Light, dark (default at boot), OLED-black, high-contrast; scale 1.0 and 2.0 (auto 2.0 on HiDPI panels).
- Keyboard-only path through every screen; screen-reader names for the disk picture ("Partition 3, Windows, 300 GB, locked — won't be touched").
- 👤 Mockup approval (`ui/mockups/9.7/`) before building; journey sign-off on a recorded VM install.

**Human in the loop:** the user picks the target, sees what will be erased in plain words, confirms by typing its name, and is reminded to have the BitLocker key before anything is written.

**Tests**
- Unit: `validate` over ≥ 30 fixture inventories (Zenbook recording, single NVMe with Windows + Arch, Windows + free space, BitLocker, two disks, MBR disk, RAID mode, tiny ESP, install medium only) with expected refusals; journal step idempotency.
- Integration: engine `apply` against loop devices in a CI VM only (never host disks), resumed after a kill at every step.
- E2E (VM, nightly): `scripts/vm-test.sh` disk with 260 MiB ESP containing a dummy `EFI/Microsoft/Boot/bootmgfw.efi` + NTFS "Windows" + a target partition → graphical install driven by the answers file → GRUB lists PriyaOS and Windows → boots PriyaOS → the "Windows" NTFS partition and the ESP's Microsoft files are byte-identical before/after (hash check).
- E2E refusal suite: picking the ESP, the NTFS Windows partition, a BitLocker-signature partition, a too-small partition, the install medium → each refused, nothing written (disk hash unchanged).
- UI: installer screens × states × themes × scales; a11y lint; keyboard-only journey; Orca smoke test.
- Chaos: VM power-off during each install step → rerun the installer → completes; Windows partition unchanged.
- Regression: `installer_refusals_fixtures`, `installer_vm_dual_boot`, `installer_windows_untouched_hash`, `installer_refusal_e2e`, `installer_chaos_rerun`.

**Done when**
- [ ] Nightly VM dual-boot install green; Windows partition and Microsoft ESP files unchanged by hash in every run.
- [ ] Every refusal case refused with nothing written.

---

### 9.8 — Encryption option
**Outcome:** the installer offers full-disk encryption (LUKS2) of the PriyaOS partition with one passphrase at start-up, a recovery plan, and no loss of sandbox, update, rollback or Rewind features.
**Features:** OS-16 (installer encryption step; TPM unlock and hardening stay in Phase 14), OS-17.
**Depends on:** 9.7.

**Tasks**
- [ ] ADR "LUKS with `/boot` inside `@`": GRUB must unlock the container (`GRUB_ENABLE_CRYPTODISK=y`); upstream GRUB can't read Argon2id keyslots, so the keyslot GRUB uses is LUKS2 with PBKDF2 (iteration time tuned so GRUB unlocks in ≤ 10 s on the Zenbook); avoid a second prompt with a keyfile in the initramfs (`/etc/cryptsetup-keys.d/`, initramfs mode 0600) — or accept two prompts. Recommendation and consequences recorded.
- [ ] If the keyfile option is chosen: add the keyfile and initramfs images to the fork mask list and the ChangeSet ignore list (agents in forks have passwordless sudo), and regenerate initramfs in the helper at system-merge time instead of inside forks; tests prove no fork can read the keyfile.
- [ ] Installer step: "Encrypt this computer?" with a plain explanation, passphrase + strength meter, keyboard-layout note (GRUB prompt uses US layout), recovery: second passphrase keyslot and an optional LUKS header backup saved to the USB.
- [ ] Engine: `cryptsetup luksFormat --type luks2 --pbkdf pbkdf2`, open, btrfs inside, `crypttab`/kernel parameters (`rd.luks.name=`), mkinitcpio `sd-encrypt` hook, GRUB cryptodisk config.
- [ ] Themed passphrase prompt where possible (GRUB prompts before its theme loads; documented), Plymouth prompt for the initramfs stage if two prompts remain.
- [ ] Swap/hibernation: zram by default; hibernation not offered on encrypted installs in 1.0 (documented; needs a swap subvolume decision).
- [ ] `docs/human/07-install-on-your-zenbook.md` encryption section (passphrase at every start, where to keep the recovery passphrase).
- [ ] The encryption choice is offered in the installer and onboarding with a plain explanation and recovery-passphrase handling (print/save prompt, header backup to a user-chosen location)

**UI**
- Encryption screen and the start-up passphrase prompt(s); states: off, on, weak passphrase, mismatch, recovery saved, error, Hindi text (installer UI; the GRUB prompt is English-only).
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard path; screen-reader names ("Encryption passphrase, strength good").
- 👤 Mockup approval (`ui/mockups/9.8/`) before building; journey sign-off on a recorded encrypted VM install.

**Human in the loop:** the owner chooses encryption and keeps the recovery passphrase; Claude never sees it.

**Tests**
- Unit: encryption plan validation (passphrase rules, keyslot plan), crypttab/kernel-parameter generator.
- E2E (VM, nightly): encrypted install → passphrase typed by the harness at GRUB (and initramfs if two prompts) → boots → `priya update` with a kernel update → restart → unlocks → `priya rollback` → unlocks → previous system boots.
- Security: sandbox run in a fork tries to read the keyfile and initramfs → denied/masked; ChangeSets never contain them.
- Perf: GRUB unlock time on the Zenbook ≤ 10 s (L4).
- Regression: `encrypted_install_vm`, `encrypted_update_rollback_vm`, `fork_cannot_read_luks_keyfile`.

**Done when**
- [ ] Nightly encrypted VM install, update and rollback green; keyfile isolation proven.

---

### 9.9 — Boot counting, automatic fallback and recovery UI
**Outcome:** if a newly applied system fails to reach the desktop twice, the next boot falls back to the previous system automatically and explains what happened; the previous-system entry from 2.12 gets the themed menu and a graphical recovery banner.
**Features:** SBX-16, SBX-13.
**Depends on:** 9.1, 9.11, 2.12.

**Tasks**
- [ ] Boot counter in `grubenv`: the helper sets `priya_tries=2` after every system swap; a `grub.d` snippet decrements it at each boot and, at 0, makes "PriyaOS (previous system)" the default for that boot
- [ ] `priya-boot-ok.service` (after `graphical.target` and a successful user login, or `multi-user.target` + 2 min on headless) clears the counter; journaled
- [ ] After an automatic fallback: a graphical banner "Your last update didn't start properly, so I started the previous system" with **Keep using it (roll back)**, **Try the new one again**, **Show what changed** (opens the update review)
- [ ] Encrypted installs (9.8): fallback entry unlocks the same container without a second unexpected prompt
- [ ] Themed boot-menu entries (9.11) and plain-language entry titles with dates ("PriyaOS — before update of 24 Oct")
- [ ] `priya doctor` shows the boot counter state and the last fallback

**UI**
- Recovery banner and rollback dialog; states: normal boot, booted previous manually, booted previous automatically, rolling back, done (restart needed), error.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard path; screen-reader names for every banner action.
- 👤 Mockup approval (`ui/mockups/9.9/`) before building; journey sign-off.

**Human in the loop:** nothing is rolled back permanently without the user choosing it; the banner explains in one sentence what happened and offers the review of the failed update.

**Tests**
- Unit: grubenv counter logic on fixtures (fresh swap, one failure, two failures, success clears).
- E2E (VM): seeded update whose kernel panics → two failed boots → third boot lands in the previous system automatically → banner → roll back → healthy; same on the encrypted VM.
- Chaos: power-off during the boot-ok service and during rollback → system still boots one of the two roots.
- Regression: `boot_counter_fallback_vm`, `boot_ok_clears_counter`, `recovery_banner_states`.

**Done when**
- [ ] The kernel-panic VM scenario recovers with no user action beyond restarting, and the banner explains it.

---

### 9.10 — First-boot onboarding
**Outcome:** the first login runs a six-step setup — Welcome, Wi-Fi, AI accounts, How careful should Priya be?, Folders Priya may search, Appearance — covering language, providers and keys, agent CLIs, local models, strategy, default mode and autonomy, privacy defaults and permitted folders; every step can be skipped and changed later by plain language.
**Features:** OS-05, OS-20.
**Depends on:** 9.3, 9.7.

**Tasks**
- [ ] `ui/apps/Onboarding` (Quickshell) + CLI fallback `priya setup` (TUI) sharing one `onboarding` state machine in priyad; resumable after logout; `priya setup --again`.
- [ ] Welcome: language (English now; Hindi UI in Phase 10), keyboard layout confirmation, accessibility shortcuts (Orca, high contrast, larger text) on the first screen.
- [ ] Wi-Fi: reuses the Network panel component; skippable (local-only path offered).
- [ ] AI accounts: connect providers (keys typed by the user into the secure field → Secret Service, 1.5 flow), agent CLIs (Claude Code, Codex, Gemini CLI, OpenCode) installed through a sandbox task from their official sources with review, logins done by the user in a kitty window opened for them; local models: detect Ollama/LM Studio, offer `ollama` install and one small model sized to RAM with download size shown; strategy (Best / Balanced / Cheapest / Local only); privacy default ("keep my documents on local models").
- [ ] How careful should Priya be?: autonomy level (Ask me first / Ask when it matters / Just do it) and default execution mode, explained with one example each.
- [ ] Folders Priya may search: 8.7 picker, nothing preselected.
- [ ] Appearance: light / dark / auto, OLED black, accent; live preview.
- [ ] Finish: "Press Super+Space and try: install htop and show me how to use it"; skipped items become gentle reminders in the bar (max one per day, dismissible forever).
- [ ] Update [11 §4](../architecture/11-design-system-and-ux.md) and [10 §3](../architecture/10-base-os-and-distro.md) so both list the same steps (language in Welcome, strategy and privacy in AI accounts, mode in How careful; voice added in Phase 10).
- [ ] Import from the previous install: onboarding offers to bring back a backed-up `~/.config/hypr`, `kitty` and other configs; Priya merges them with PriyaOS defaults in a sandbox and shows the review

**UI**
- Six steps with progress dots; states per step: default, filled, skipped, validating key, key invalid, installing agent CLI (progress), login waiting, offline, error, long content, Hindi text; final screen.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard-only path through every step; Orca journey; screen-reader names ("Step 3 of 6, AI accounts").
- 👤 Mockup approval (`ui/mockups/9.10/`) before building; journey sign-off on a recorded VM first boot.

**Human in the loop:** keys and logins are entered by the user only; every download shows its size first; everything is skippable.

**Tests**
- Unit: onboarding state machine (skip, resume, re-run), RAM-to-model-size table.
- Integration: scripted onboarding with fake provider and fake agent installer → settings, secrets and permitted folders set as chosen; skip-all → working local-only (or no-AI) state with reminders.
- UI: every step × state × theme × scale; a11y lint; keyboard-only and Orca journeys.
- E2E (VM): fresh install → onboarding via the scripted path (fake provider key from fixtures, recorded agent installer) → Super+Space works.
- Regression: `onboarding_full_path`, `onboarding_skip_all`, `onboarding_resume_after_logout`, `onboarding_orca_journey`.

**Done when**
- [ ] A fresh VM install reaches a working Priya bar through onboarding with no terminal use.

---

### 9.11 — Branding (boot, login, lock, wallpapers)
**Outcome:** the boot menu, splash, login, lock screen, wallpapers and system identity all look like PriyaOS, generated from `ui/design/tokens.json`, in light, dark and OLED-black where the surface allows.
**Features:** OS-13, DS-19.
**Depends on:** 9.2.

**Tasks**
- [ ] GRUB theme generator in `ui/design/generate/grub`: `theme.txt`, PNG assets rendered at native resolution (2880×1800 on the Zenbook, 1920×1080 fallback), fonts via `grub-mkfont` from Inter; `GRUB_GFXMODE` auto; entries "PriyaOS", "PriyaOS (previous system)", "Windows".
- [ ] Plymouth theme (optional, on by default) with the Priya mark animation on true black (OLED-friendly); passphrase prompt styled for encrypted installs.
- [ ] Login: greetd + regreet with GTK CSS generated from tokens (follows the system default theme or the auto schedule); user list, session choice hidden unless more than one; Orca toggle on the greeter.
- [ ] Lock screen: hyprlock config generated from tokens (clock, user avatar, password field, battery, keyboard layout, notifications count without content); OLED burn-in guard (slow content shift, dim after 30 s).
- [ ] Wallpapers: light/dark pairs (OLED-friendly dark variants) in `priyaos-branding`, licensed and credited; default follows the theme.
- [ ] Identity: `/etc/os-release` (`NAME="PriyaOS"`, `PRETTY_NAME="PriyaOS 1.0"`, `ID=priyaos`, `ID_LIKE=arch`, `VERSION_ID`, `HOME_URL`, `LOGO=priyaos`), logo in hicolor, `/etc/issue`, installer and About page branding.
- [ ] `priya theme apply` regenerates GRUB, greeter and lock themes on accent or theme change (GRUB through the helper, since it writes `/boot`).

**UI**
- Boot menu, splash, greeter, lock screen, wallpapers; states: boot menu (with/without previous system and Windows), splash, passphrase prompt, greeter (one user / several / wrong password / caps lock / layout switch), lock (locked, wrong password, battery low, notifications waiting), long user names, Hindi user names.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path on greeter and lock; screen-reader names on the greeter; known limitation recorded if the lock screen can't speak with Orca.
- 👤 Mockup approval (`ui/mockups/9.11/`) before building; sign-off on screenshots from a VM boot and the Zenbook.

**Tests**
- Unit: generators produce valid GRUB theme syntax, regreet CSS and hyprlock config for every theme; contrast checker on generated pairs.
- UI: screenshots of greeter and lock in every theme and scale (headless); GRUB and Plymouth captured from the VM via QEMU screendump.
- E2E (VM): boot shows the themed menu with all expected entries; accent change → regenerated GRUB theme on next boot.
- Regression: `branding_generators_valid`, `grub_theme_vm_screendump`, `greeter_lock_screenshots`, `os_release_identity`.

**Done when**
- [ ] Every boot-to-desktop surface uses token colours (checked by the contrast checker and screenshots); 👤 owner signs off on the Zenbook.

---

### 9.12 — Zenbook hardware validation
**Outcome:** PriyaOS 1.0 is validated on the owner's Zenbook model: automated hardware smoke tests pass on the self-hosted runner, the manual checklist passes, and model-specific quirks live in a hardware profile.
**Features:** OS-07, OS-06, OS-11.
**Depends on:** 9.2–9.11.

**Tasks**
- [ ] Hardware profile `data/hardware/<dmi_product_name>.toml` (kernel parameters, firmware packages such as `sof-firmware`, backlight interface, charge-limit support, known-bad options like VRR on OLED), applied by `priyaos-defaults` when the DMI name matches.
- [ ] Automated smoke tests on the Zenbook runner (QA-19, on demand, on an installed PriyaOS the owner agrees to use for testing): Wi-Fi associates and gets an address, Bluetooth adapter powers on and scans, audio sink and source present (sof), brightness changes via the backlight interface, keyboard backlight, suspend/resume ×20 (`rtcwake -m freeze`) with Wi-Fi and audio back after each, battery reported by UPower, power profiles switch, webcam device present, GPU video acceleration (`vainfo`), lid switch event.
- [ ] Manual checklist (owner): speakers and headphone jack, built-in mic, Fn keys and OSDs, touchpad gestures, external display over USB-C and HDMI (hotplug, scale, arrangement), sleep drain over 8 h (target ≤ 2 % per hour), idle power draw, fan noise under a build, lid close/open.
- [ ] Phase 6 hardware playbooks re-run against the installed 1.0 (6.6 L4 scenarios).
- [ ] Results recorded in this file and in the hardware profile's notes; failures become issues with fixes in the profile or packages.

**Tests**
- E2E (Zenbook runner): the automated smoke set above, tagged `hardware`, run before every release.
- Regression: `zenbook_smoke_suspend_resume`, `zenbook_smoke_wifi_audio_backlight` (hardware-tagged, release-gating).

**Done when**
- [ ] Automated smoke set green 3 runs in a row; manual checklist passed and recorded; sleep drain within target or documented.

---

## Phase gate

### 9.13 — Phase gate (fresh install by the owner)
**Outcome:** the owner installs PriyaOS 1.0 on the Zenbook from USB, next to Windows, following only the human guide, and every earlier phase works on the installed system.
**Features:** all Phase 9 features.
**Depends on:** 9.1–9.12.

**Tasks**
- [ ] Run the full regression suite and the nightly suites (VM install plain and encrypted, previous-system recovery, multi-user, upgrade N-1 → N); fix failures.
- [ ] Cut `v1.0.0` through the release pipeline (owner approves signing and publishes).
- [ ] 👤 Owner fresh install on the Zenbook (below); Claude is available but the owner follows `docs/human/07-install-on-your-zenbook.md` only.
- [ ] Update `features.md` statuses for every Phase 9 feature; update [10](../architecture/10-base-os-and-distro.md), [03 §2 and §5.2](../architecture/03-execution-and-sandbox.md), [11 §4](../architecture/11-design-system-and-ux.md) and [01 §6](../architecture/01-system-overview.md) for everything that changed; ADRs added in this phase listed in [decisions.md](../architecture/decisions.md).
- [ ] Update `docs/human/05-the-phases.md`, `docs/human/04-everything-priyaos-will-do.md` and `docs/human/07-install-on-your-zenbook.md`.

**Done when**
- [ ] Whole regression suite green on `main`, plus nightly e2e (dual-boot install, LUKS install, previous-system recovery, multi-user, N-1 → N upgrade), perf, chaos, security and eval suites, all on the `v1.0.0` commit.
- [ ] Before the install the owner has: backed up anything needed from the current Arch, saved the Windows BitLocker recovery key (photo or paper), turned off Windows Fast Startup, checked Secure Boot is off, flashed the signed and checksum-verified ISO.
- [ ] Fresh install by the owner from USB with the graphical installer, following the guide without help beyond it; encryption chosen per the owner's preference; onboarding completed.
- [ ] Boot menu shows PriyaOS and Windows; Windows boots afterwards (BitLocker key entered if asked) and its files are intact; PriyaOS boots again after Windows.
- [ ] Hardware checklist on the Zenbook: Wi-Fi, Bluetooth, audio (speakers, headphones, mic), brightness, keyboard backlight and Fn keys, suspend/resume (×5 by hand), external display, battery reporting and charge limit, power profiles, touchpad, webcam.
- [ ] Owner demo on the installed system: Super+Space question; "install htop and show me how to use it" through the sandbox; an app from Flathub; `priya update` → restart → "everything works"; "PriyaOS (previous system)" visible in the boot menu after the update; undo by prompt; find a file by description; a second user account created and removed.
- [ ] Docs and feature statuses updated; `docs/human/05-the-phases.md` status updated.

## Out of scope

- Secure Boot signing, TPM2 unlock, firewall hardening and per-app network rules (Phase 14, SEC-19).
- Voice, Hindi UI translation and Priya Vision (Phases 10–11); onboarding gains a voice step in Phase 10.
- Resizing Windows partitions, BIOS/legacy boot, non-x86_64 architectures, and hardware beyond the Zenbook (best-effort only).
- Fast user switching and guest accounts.
- Installing as a layer on Omarchy or other existing Arch systems beyond what Phase 0/5 already support (Phase 16, ECO-07).
- A full app store with ratings and a directory of skills (Phase 16).
- Hibernation on encrypted installs (needs a swap-subvolume decision).
