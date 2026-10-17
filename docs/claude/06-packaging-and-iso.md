# 06 — Packaging and the ISO

## 1. Arch package — `packaging/arch/PKGBUILD`

- `pkgname=priya`, builds the workspace with `cargo build --release --locked`.
- `depends=(btrfs-progs systemd sqlite bubblewrap fuzzel libnotify kitty)`; `optdepends=(claude-code codex waybar mako)`.
- Installs: `/usr/bin/priya`, `/usr/bin/priyad`, `/usr/lib/systemd/system/priyad.service`, `/usr/lib/systemd/user/priya-notify.service`, `/usr/share/libalpm/hooks/priya-tools.hook`, `/usr/share/priya/desktop/*`, default `/etc/priya/{config,policy}.toml` (as `backup=()`), `/usr/lib/sysusers.d/priya.conf` (group `priya`), `/usr/lib/tmpfiles.d/priya.conf` (`/run/priya`).
- `priya.install`: post-install message: "Run `sudo priya doctor` next."

Install on an existing Arch (week 1, day 6):

```
cd packaging/arch && makepkg -si
sudo priya doctor --fix        # creates @priya if missing, fixes fstab subvolid, checks layout
sudo systemctl enable --now priyad
sudo usermod -aG priya $USER   # then log out and in
systemctl --user enable --now priya-notify
priya desktop install
```

`priya doctor --fix` creating `@priya`: create subvolume at top level, copy any existing `/var/lib/priya` into it, add an fstab line `UUID=<fs> /var/lib/priya btrfs subvol=@priya,<same options as /> 0 0`, mount it.

## 2. ISO — `iso/`

Based on archiso's `releng` profile (copy from `/usr/share/archiso/configs/releng/` and modify; don't vendor unneeded files).

- `packages.x86_64` adds: `priya` (from a local repo), `hyprland`, `waybar`, `kitty`, `fuzzel`, `mako`, `xdg-desktop-portal-hyprland`, `pipewire`, `wireplumber`, `pipewire-pulse`, `networkmanager`, `bluez`, `btrfs-progs`, `grub`, `efibootmgr`, `os-prober`, `sof-firmware`, `linux-firmware`, `intel-ucode`, `amd-ucode`, `nodejs`, `npm`, `archinstall`, `bubblewrap`, `sqlite`, `git`.
- Local repo: `scripts/build-iso.sh` runs `makepkg` for `priya`, creates `iso/localrepo/` with `repo-add`, and adds `[priya-local]` to the profile's `pacman.conf` (`SigLevel = Optional TrustAll`, `Server = file:///…`).
- Build: `sudo mkarchiso -v -w /tmp/priya-work -o out/ iso/` → `out/priyaos-<date>-x86_64.iso`. Build must run on x86_64 Arch (the Zenbook).
- Live session autologins to a tty with a banner: "Welcome to PriyaOS. Run `priya-install` to install."

## 3. Installer — `iso/airootfs/usr/local/bin/priya-install`

Bash script, interactive, **dual-boot safe**. It never touches partitions the user did not pick.

1. Show `lsblk -o NAME,SIZE,FSTYPE,LABEL,PARTLABEL,MOUNTPOINT`. Detect Windows partitions (ntfs, `Microsoft basic data`) and the ESP (`vfat`, type `EFI System`) and label them clearly.
2. Ask for the **target partition** (to be wiped and become PriyaOS) and the **ESP** (reused, never formatted). Refuse if the target is the ESP, is NTFS with a Windows label, or is smaller than 40 GiB. Require typing the target's name to confirm.
3. Check the ESP has ≥ 30 MiB free (GRUB only needs a few MiB because kernels live in `@/boot`).
4. `mkfs.btrfs -f -L PriyaOS <target>`; create `@ @home @log @pkg @priya @priya-forks`; mount at `/mnt` with `compress=zstd:1,noatime`; mount ESP at `/mnt/efi`.
5. `archinstall --config iso/installer/archinstall.json --creds <generated> --silent` with `disk_config.config_type = "pre_mounted_config"` and `mountpoint = "/mnt"`. Ask the user for hostname, username, password, timezone before running (write a temp creds file, delete after).
6. In `arch-chroot /mnt`: write fstab with `subvol=` only (no `subvolid=`); `grub-install --target=x86_64-efi --efi-directory=/efi --bootloader-id=PriyaOS`; enable `GRUB_DISABLE_OS_PROBER=false`; `grub-mkconfig`; enable `priyad`, `NetworkManager`, `bluetooth`; add user to `priya` and `wheel`; copy desktop configs to the user's `~/.config`; set Hyprland to auto-start from tty1 login.
7. Post-install note: "Install Claude Code after first boot: `npm install -g @anthropic-ai/claude-code` then `claude` to log in." (Verify the current official install method at build time.)

## 4. Hardware notes — ASUS Zenbook 14 OLED

The exact model is unknown (UX3405MA Intel Meteor Lake, UX3405CA Intel Lunar Lake, and UM3406 AMD all exist). Check with `cat /sys/class/dmi/id/product_name` on the existing Arch install and record it in `docs/human/05-install-on-your-zenbook.md`.

- Audio needs `sof-firmware` (Intel SOF).
- Lunar Lake needs a recent kernel; Arch's current `linux` is fine.
- Secure Boot must be off for week 1 (our GRUB is unsigned). Disabling Secure Boot can trigger a BitLocker recovery prompt in Windows — the human guide covers getting the recovery key first.
