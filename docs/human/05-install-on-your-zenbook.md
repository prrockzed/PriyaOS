# Installing PriyaOS on your Zenbook

This guide is for **day 7**, when the installer USB is ready. Read it once now so nothing is a surprise.

**The result:** when you turn on the laptop, a menu lets you pick **PriyaOS** or **Windows**. Windows is not touched.

**The cost:** PriyaOS **replaces your current Arch Linux**. Everything on the Arch partition is erased. Windows and its files stay.

> 🏠 Your laptop's disk is like a building with apartments. Windows lives in one, Arch lives in another. We empty the Arch apartment and move PriyaOS in. The Windows apartment keeps its door locked and nobody goes in.

This works on any normal laptop that can run Linux, not just the Zenbook. The steps below use the Zenbook's keys.

---

## Your laptop today

Claude fills this in on day 0.

- Model: _(unknown yet)_
- Arch disk format: _(unknown yet: btrfs or ext4?)_
- Boot menu program: _(unknown yet: GRUB or systemd-boot?)_
- Path: _(A or B, see below)_

**Path A**: your current Arch already uses btrfs in the right way. You can try PriyaOS on day 6 by installing it as an app, without reinstalling. You still do the full install on day 7 for the real experience.

**Path B**: your current Arch uses something else (most likely ext4). PriyaOS is tested in the virtual computer until day 7, then installed with this guide.

---

## Part 1 — Before you start (very important) ⚠️

### 1. Back up anything you care about from Arch

Everything on the Arch partition will be erased. Copy to a USB drive or the cloud:
- your projects that aren't on GitHub,
- `~/.config/hypr`, `~/.config/kitty` and other configs you like (Priya can bring your Hyprland setup back later),
- anything in `~/Documents` and `~/Downloads` you need.

Backing up Windows files too is a good habit, even though we don't touch Windows.

### 2. Save your Windows BitLocker recovery key

Many Windows laptops lock the disk with **BitLocker** (also called "Device encryption"). Changing startup settings can make Windows ask for a 48-digit **recovery key**. Without it, you can't get into Windows.

1. On your phone or Mac, go to **https://account.microsoft.com/devices/recoverykey** and sign in with the Microsoft account you use on the laptop.
2. Find the Zenbook and **write the key down on paper** or take a photo.

If there is no key there, Windows may not be encrypted. In Windows, check **Settings → Privacy & security → Device encryption**.

### 3. Get a USB stick

At least 4 GB. **Everything on it will be erased.**

---

## Part 2 — Make the installer USB

Claude builds the installer file on the Zenbook. It is called something like `out/priyaos-2026.10.25-x86_64.iso`.

**Easiest way (on the Mac):**
1. Copy the `.iso` file to the Mac.
2. Install **balenaEtcher** (free) from https://etcher.balena.io.
3. Open Etcher → **Flash from file** → pick the `.iso` → **Select target** → pick your USB stick → **Flash**.

**On the Zenbook's Arch (only if you're comfortable):**

Plug in the USB and list the disks:

```bash
lsblk
```

Find the USB by its size (for example `sda`, 16G). **Your laptop's main disk is called `nvme0n1`. Never pick that.** Then, replacing `sdX` with your USB's name:

```bash
sudo dd if=out/priyaos-2026.10.25-x86_64.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

---

## Part 3 — Laptop settings (BIOS)

You probably did this already when you installed Arch, but check:

1. **Turn off Windows Fast Startup.** In Windows: Control Panel → Power Options → *Choose what the power buttons do* → *Change settings that are currently unavailable* → untick **Turn on fast startup** → Save.
2. **Turn off Secure Boot.**
   - Shut down the laptop.
   - Hold **F2** and press the power button. Keep holding F2 until the blue ASUS setup screen appears.
   - Press **F7** for Advanced Mode → **Security** tab → **Secure Boot** → **Secure Boot Control** → **Disabled**.
   - Press **F10** → **OK** to save and restart.

> Secure Boot is like a guard who only lets in programs signed by Microsoft. Our week-1 PriyaOS isn't signed yet, so we ask the guard to take a break. We can sign it later.

---

## Part 4 — Install

1. Plug in the USB. Turn on the laptop and **tap Esc** repeatedly. A boot menu appears.
2. Pick the USB (it says **UEFI:** and the USB's name).
3. PriyaOS's installer starts and shows: *"Welcome to PriyaOS. Run `priya-install` to install."*
4. **Connect to Wi-Fi:**
   ```bash
   iwctl
   ```
   then type, one line at a time (replace `YourWiFiName`):
   ```
   station wlan0 scan
   station wlan0 get-networks
   station wlan0 connect "YourWiFiName"
   exit
   ```
5. **Start the installer:**
   ```bash
   priya-install
   ```
6. It shows your disk's partitions and marks **Windows** and **EFI** clearly.
   - **Target partition** (becomes PriyaOS, **erased**): pick the one your **current Arch** is on. It's usually the big one that is *not* labelled Windows.
   - **EFI partition** (shared startup area, **not erased**): pick the small one (100–300 MB) marked EFI.
   - The installer refuses to erase Windows or the EFI partition, even if you pick them by mistake.
   - Type the target partition's name to confirm. **Read it twice.**
7. Answer the questions: computer name, your username, a password, and your timezone (for example `Asia/Kolkata`).
8. Wait about 5–15 minutes.
9. When it says **Done**, remove the USB and restart.

---

## Part 5 — First start

1. A menu (GRUB) appears with **PriyaOS** and **Windows Boot Manager**. Pick PriyaOS.
2. Log in. The Hyprland desktop starts.
3. Connect Wi-Fi: press Super + Enter for a terminal, then:
   ```bash
   nmtui
   ```
4. Install Claude Code and log in. The installer prints the exact command; it is usually:
   ```bash
   npm install -g @anthropic-ai/claude-code
   ```
   ```bash
   claude
   ```
5. Check that everything is healthy:
   ```bash
   priya doctor
   ```
6. Try it: press **Super + Space** and type *"install htop and show me how to use it"*.
7. Restart and pick **Windows** once to make sure Windows still works. If it asks for the BitLocker key, type the one you saved.

---

## If something goes wrong

| Problem | Fix |
|---|---|
| The laptop goes straight to Windows, no menu | Tap **Esc** at startup and pick **PriyaOS**. To fix it for good: F2 → Boot tab → put **PriyaOS** first → F10. |
| The menu doesn't show Windows | In PriyaOS: `sudo grub-mkconfig -o /boot/grub/grub.cfg`, then restart. |
| Windows asks for a recovery key | Type the BitLocker key you saved in Part 1. |
| No sound | Tell Claude your laptop model (`cat /sys/class/dmi/id/product_name`). It's usually a missing package. |
| PriyaOS doesn't start after an update | Pick PriyaOS, log in on a text screen if needed, run `sudo priya rollback`, restart. |
| You want your old Arch back | The old Arch was erased, so install Arch again from the official Arch USB and restore your backup. That's why Part 1 matters. |

Next: [Words explained](06-words-explained.md)
