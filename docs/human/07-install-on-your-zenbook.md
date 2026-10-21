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

## Step 1 — Try the Priya bar (Phase 0) 💬

Priya comes to your current Arch as an app. Nothing is erased, nothing outside Priya's own files changes, and one command takes it all away again. Think of it as **lending your laptop a guest room**: Priya moves in with its own things, and when it leaves the room is empty again.

You do these steps yourself, in a terminal, as you (not as root, the all-powerful admin account). Claude reads the output of `priya doctor` with you if anything looks wrong.

### 1. Build and install

The recipe for the package is in the repo. `makepkg` is Arch's own tool for turning a recipe into an installable box; it needs Arch's build tools (`sudo pacman -S --needed base-devel`) and Rust. You already have Rust through `rustup` if you build PriyaOS on this laptop; don't also install the `rust` package, the two clash. It makes two boxes, `priya` (the brain) and `priya-shell` (the bar you see):

```bash
cd ~/path/to/PriyaOS/packaging/arch
```
```bash
makepkg -si
```

This can take 10 minutes or more the first time, and asks for your password to install. Afterwards `git checkout packaging/arch/PKGBUILD` tidies the version number `makepkg` writes into it.

### 2. Start Priya's background service

Priya's **background service** (`priyad`) is like a receptionist that sits quietly in the back office: it's always there to take a question and pass it to the right model. `systemctl --user` is the on/off switchboard for background programs that run as you:

```bash
systemctl --user enable --now priyad
```

### 3. Choose a model

Pick at least one (both is fine):

- **Gemini, Google's model** (free, cleverer, uses the internet). Google may read what you send on the free tier, so keep it for ordinary questions; private ones are better with the laptop model. Get a free key at aistudio.google.com/apikey. A key is like a library card that lets Priya borrow from your Google allowance. Then:
  ```bash
  priya ai key set gemini
  ```
  Paste the key when it asks; it won't show on screen, and it goes into your laptop's locked key store (the "keyring"), never into a file. If it says there's no key store, install one (`sudo pacman -S gnome-keyring`), log out and in, and try again.
  ```bash
  priya settings set ai.strategy fixed:gemini/gemini-2.5-flash
  ```
- **A model on this laptop** (free, private, less clever), with Ollama, a free program that runs AI models on your own laptop. Install it and switch it on first:
  ```bash
  sudo pacman -S ollama
  ```
  ```bash
  sudo systemctl enable --now ollama
  ```
  Then fetch a small model (about 400 MB) and choose it:
  ```bash
  ollama pull qwen2.5:0.5b
  ```
  ```bash
  priya settings set ai.strategy fixed:local/qwen2.5:0.5b
  ```

Check it works: `priya ask "what is btrfs?"` should answer in the terminal.

### 4. Add the shortcut

Tell Hyprland about **Super + Space** (Super is the Windows/logo key) by adding **one line** to your Hyprland settings. Hyprland can be set up in two ways of writing the same settings; use the file you have:

- If you have `~/.config/hypr/hyprland.lua` (Hyprland 0.55 and later; if you have both files, this one), add:
  ```lua
  pcall(require, "/usr/share/priya/desktop/hyprland.lua")
  ```
  It means "load Priya's shortcut file; if it's gone, carry on quietly", so removing Priya can never break your settings.
- If you only have `~/.config/hypr/hyprland.conf`, add:
  ```text
  source = /usr/share/priya/desktop/hyprland.conf
  ```

That line is like a **speed-dial button**: one press now rings Priya. It also starts the bar (drawn by a program called Quickshell) every time you log in, so you never start it yourself. Save your work, then log out and back in.

### 5. Ask

Press **Super + Space**, type a question, press **Enter**. The answer appears word by word. **Esc** stops an answer; a second **Esc** closes the bar. **Ctrl + N** starts a new conversation.

If something doesn't work, run:

```bash
priya doctor
```

It checks the pieces it knows about (the service, the model and its key, Quickshell, the shortcut, the fonts) and says what to do for anything missing. If Super + Space already does something else, such as another launcher, it says so.

### 6. Remove it again

First delete the Hyprland line you added, then:

```bash
systemctl --user disable --now priyad
```
```bash
sudo pacman -Rns priya-shell priya
```

Log out and back in. Your questions, answers and settings stay in `~/.local/share/priya` and `~/.config/priya`, like a box of your things left in the hall: delete those two folders too if you want everything gone.

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
