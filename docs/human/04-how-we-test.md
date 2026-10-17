# How we test

## Why we can't test everything on the Mac

PriyaOS is built on Linux. Your Mac runs macOS, which is a different system and doesn't have the btrfs "magic floor" or Linux's tools.

So we split the work:

| Machine | What it's for |
|---|---|
| 💻 **Mac** | Planning, writing docs, writing code, and testing the "brain" parts that don't need Linux |
| 🐧 **Zenbook (Arch Linux)** | Building PriyaOS and testing everything for real |

**Easiest way to work:** install Claude Code on the Zenbook's Arch and run Claude there for build and test days. Claude can then build, test and fix in one place. You can still use the Mac to read docs and plan.

## Four levels of testing

We test from safest to most real, like a pilot training in a simulator before flying a real plane. ✈️

### Level 0: tiny checks (on Mac or laptop)

Claude tests small pieces of logic by themselves. *"If the user changed a file and the AI changed it too, is that a conflict?"* These run in seconds.

> 🧮 Like checking each sum in your homework before checking the whole page.

### Level 1: a pretend computer (on the laptop)

Claude creates a **fake computer inside a single file** on the laptop: a small btrfs disk with a tiny Arch inside. All the photocopy, keep, throw-away and go-back tests run on that fake computer.

Your real laptop is never touched by these tests.

> 🧸 Like practising surgery on a teddy bear.

### Level 2: a virtual computer (on the laptop)

A **virtual machine** is a whole computer running inside a window on your laptop. Claude builds the PriyaOS installer and installs it into this virtual computer, including a pretend Windows partition, to check that the installer never harms Windows. Then it restarts the virtual computer, tests updates and rollback, and restarts again.

> 🎮 Like a flight simulator: everything is real except the crash.

### Level 3: your real laptop

Only after Level 2 passes. You install PriyaOS on the Zenbook and we go through a checklist:

- [ ] The health check (`priya doctor`) is all green
- [ ] Super + Space → ask for something → review → keep → it works
- [ ] A real coding task in one of your real projects
- [ ] `priya update` → restart → works → `priya rollback` → restart → back to before
- [ ] Windows still starts from the menu
- [ ] Wi-Fi, sound, brightness, and sleep/wake still work

## Safety rules Claude follows

- Claude **never** tests "keep changes" or "go back" on your real laptop's system while building. Only on the pretend computer or the virtual computer.
- Claude **never** runs disk-erasing commands on a real disk. Only you do that, on day 7, by following the install guide, after checking the disk name twice.

Next: [Install on your Zenbook](05-install-on-your-zenbook.md)
