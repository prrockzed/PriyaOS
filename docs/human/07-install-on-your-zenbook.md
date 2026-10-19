# Installing PriyaOS on your Zenbook

PriyaOS reaches your laptop in three steps:

1. **End of phase 0: as an app** on your current Arch. Ask questions with Super + Space. Nothing is erased, and any Arch works.
2. **Phase 2: PriyaOS gets its own space.** Priya's safe copies need a special disk layout (btrfs). You make room by shrinking Windows (you don't use it), then install an **early PriyaOS test version** into that room. **Your Arch stays exactly as it is.**
3. **Phase 9: PriyaOS 1.0.** A polished installer. You can do a fresh install or just keep updating.

**The result of the real install:** when you turn on the laptop, a menu lets you pick **PriyaOS**, **Arch** or **Windows**, and it remembers what you picked last time. Arch and Windows keep working as before.

> 🏠 Your laptop's disk is like a building with apartments. Windows lives in a big one with empty rooms, and Arch, where you live, has another. Windows gives up some empty rooms, and we build a new PriyaOS apartment there. Your Arch apartment and what's left of Windows stay locked, and nobody goes in.

This works on any normal laptop that can run Linux. The keys below are for ASUS laptops.

---

## Your laptop today

Checked with the laptop survey on 19 October 2026. The details are **private**: they're not in this repo (anyone can read it), only on your laptop and in Claude's notes on your Mac.

What they mean for you:
- **Phase 0:** Priya works as an app on your current Arch. Nothing is erased.
- **Phase 2:** your Arch stays. It's your main system, so we won't replace it. How PriyaOS gets its own space is being decided.
- **Windows:** kept, and not encrypted, so no recovery key is needed.

---

## Step 1 — Priya as an app (end of phase 0)

Works on any Arch. Claude gives you the exact commands at the time. They look roughly like this:

```bash
sudo pacman -U priya-*.pkg.tar.zst
```
```bash
systemctl --user enable --now priyad
```

Then press Super + Space.

## Step 2 — The PriyaOS disk setup (phase 2)

You keep using Arch every day. PriyaOS becomes a **third system** you can start whenever you like, and you switch fully only when it's good enough for you.

Follow "The real install" below with the **early test installer**. It looks simpler than the 1.0 installer, but it has the same protections: it never touches Windows or your Arch.

(Other people whose Arch already uses btrfs the right way can skip the installer: `priya doctor --fix` adjusts their Arch after asking.)

---

## The real install (phase 2 test installer, or phase 9 1.0 installer)

### Part 1 — Before you start (very important) ⚠️

**1. Back up anything you care about from Arch.** Your Arch isn't erased, but the installer changes the disk's map, so a backup is the safety net. Copy to a USB drive or the cloud:
- projects that aren't on GitHub,
- configs you like (`~/.config/hypr`, `~/.config/kitty`…). Priya can bring your Hyprland setup back,
- documents and downloads you need.

**2. Save your Windows BitLocker recovery key.** Windows may ask for a 48-digit key after startup settings change. Without it you can't get into Windows.
1. On your phone or Mac, open **https://account.microsoft.com/devices/recoverykey** and sign in with your Microsoft account.
2. Find the Zenbook and **write the key down** or take a photo.

If there's no key there, Windows may not be encrypted. Check in Windows: **Settings → Privacy & security → Device encryption**.

**3. Make room for PriyaOS (in Windows, about 10 minutes).**
1. Start **Windows**. Copy off anything in Windows you want to keep, just in case.
2. Right-click the Start button → **Disk Management**.
3. Right-click the big **C:** drive → **Shrink Volume**.
4. Type **204800** (that's 200 GB) in "Enter the amount of space to shrink" → **Shrink**.
5. You'll see a black bar marked **Unallocated**. Leave it empty: the installer uses it.

If Windows offers less than 200 GB, Claude gives you two small settings to turn off for a moment, then you try again.

> ✂️ Windows is a big room with a lot of empty floor. We move its wall in a bit. Everything in Windows stays; it just has less empty space.

**4. Get a USB stick**, at least 4 GB. Everything on it will be erased.

### Part 2 — Make the installer USB

Claude builds the installer file. It's called something like `priyaos-<date>-x86_64.iso`.

**Easiest (on the Mac):**
1. Install **balenaEtcher** from https://etcher.balena.io.
2. **Flash from file** → pick the `.iso` → **Select target** → your USB → **Flash**.

### Part 3 — Laptop settings (BIOS)

You probably did this when you installed Arch, but check:

1. **Windows Fast Startup off**: Control Panel → Power Options → *Choose what the power buttons do* → *Change settings that are currently unavailable* → untick **Turn on fast startup** → Save.
2. **Secure Boot off** (we turn it back on in phase 14):
   - Shut down. Hold **F2** and press power until the ASUS setup screen appears.
   - **F7** (Advanced Mode) → **Security** → **Secure Boot** → **Secure Boot Control** → **Disabled**.
   - **F10** → **OK**.

> Secure Boot is a guard who only lets in programs signed by Microsoft. Our early PriyaOS isn't signed yet, so the guard takes a break. In phase 14 we sign PriyaOS and bring the guard back.

### Part 4 — Install

1. Plug in the USB. Turn on the laptop and **tap Esc** until a boot menu appears. Pick the USB (**UEFI: …**).
2. Connect to Wi-Fi: type `nmtui` and pick your network.
3. Type `priya-install` and press Enter.
4. The installer shows your disk and marks **Windows** and **EFI** clearly:
   - **Target** (becomes PriyaOS): the **free space** you made in Part 1, shown as **new partition here**.
   - **Arch** and **Windows** are shown as **locked**. It **refuses** to erase them, even if you pick them by mistake.
   - **EFI** (shared start-up area, **not erased**): the one your Arch already uses. PriyaOS only adds its own folder there.
   - Type the target's name to confirm. **Read it twice.**
5. Choose whether to **encrypt** the disk (recommended; you'll type a password at every start).
6. Answer: computer name, username, password, timezone (e.g. `Asia/Kolkata`).
7. Wait 5–15 minutes. When it says **Done**, remove the USB and restart.

### Part 5 — First start

(With the early phase-2 test installer, first start is simpler and text-based. The friendly setup wizard below arrives with PriyaOS 1.0 in phase 9.)

1. The menu shows **PriyaOS**, **Arch Linux** and **Windows Boot Manager**. Pick PriyaOS. From now on the menu starts whatever you picked last time.
2. The **setup wizard** helps you:
   - connect Wi-Fi,
   - connect your AI accounts (Claude, ChatGPT/Codex, Gemini, Grok, or a free AI on the laptop),
   - choose a mode (best / balanced / cheapest / local only),
   - choose how careful Priya should be by default,
   - choose which folders Priya may search.
3. Press **Super + Space** and try: *"install htop and show me how to use it"*.
4. Restart and pick **Arch**: everything should be exactly as you left it. Then restart once more and pick **Windows** to check it still works. If it asks for the BitLocker key, type the one you saved.

### If something goes wrong

| Problem | Fix |
|---|---|
| Laptop goes straight to Windows | Tap **Esc** at start and pick PriyaOS. To fix for good: F2 → Boot → put PriyaOS first → F10. |
| Menu doesn't show Windows | In PriyaOS, Super + Space → "add Windows to the start menu". |
| Windows asks for a recovery key | Type the BitLocker key you saved. |
| No sound or no Wi-Fi | Super + Space → "my sound isn't working". Or tell Claude your laptop model. |
| PriyaOS won't start after an update | In the start menu pick **PriyaOS (previous system)**, then run `sudo priya rollback` to make it permanent. |
| Menu doesn't show Arch | Tap **Esc** at start and pick Arch's own menu (**GRUB**). Then in PriyaOS: Super + Space → "add Arch to the start menu". |
| You want to remove PriyaOS | Claude gives you the steps: delete PriyaOS's partition and its start-up folder. Arch and Windows stay as they were, and Windows can take the space back. |

Next: [Words explained](08-words-explained.md)
