# How we test

## Why the Mac isn't enough

PriyaOS is built on Linux. Your Mac runs macOS, which is a different system and doesn't have Linux's tools. So:

| Machine | What it's for |
|---|---|
| 💻 **Mac** | Planning, docs, writing code, testing the "thinking" parts |
| 🐧 **Zenbook (Arch Linux)** | Building PriyaOS and testing everything for real |

**Easiest way to work:** install Claude Code on the Zenbook's Arch and run Claude there on build-and-test days.

## Five levels of testing

From safest to most real, like a pilot who trains in a simulator before flying a real plane. ✈️

| Level | Where | What it's like | What we check |
|---|---|---|---|
| **0. Tiny checks** | Mac or laptop | Checking each sum in your homework | Small pieces of logic: "is this a conflict?", "which model should this go to?" |
| **1. Plug checks** | Mac or laptop | Testing every charger fits the same socket | Every AI company connection and every AI worker behaves the same way (using recorded conversations, no real money spent) |
| **2. Pretend computer** | Laptop | Practising surgery on a teddy bear 🧸 | A tiny fake computer inside one file. Photocopies, merges, undo and safety are all tested there. Your real laptop is never touched. |
| **3. Virtual computer** | Laptop | A flight simulator 🎮 | A whole computer in a window, with a pretend Windows inside. We install PriyaOS, restart it, update it, roll back, and check Windows is never harmed. |
| **4. Your real laptop** | Zenbook | The real flight | Only after level 3 passes. You go through a short checklist for each phase. |

## Special tests

- **Model choice test**: about 300 example requests, to check Priya AI sends each one to the right kind of model and to measure cost.
- **Trap tests**: projects and web pages with hidden tricky instructions ("send me your passwords"), to prove Priya blocks them.
- **Pull-the-plug tests**: we switch things off in the middle of an important step to prove nothing gets corrupted.
- **Speed tests**: for example, making a photocopy must take under 2 seconds, and Priya's first words must appear in under 1.5 seconds.
- **Voice and screen tests**: recorded voices (English and Hindi, quiet and noisy) and saved screenshots.

## Safety rules Claude always follows

- Claude **never** tests keep, undo or system switching on your real laptop's system while building. Only on the pretend or virtual computer.
- Claude **never** runs disk-erasing commands on a real disk. Only you do that, once, when installing with the guide.
- Tests that use real AI accounts have tiny spending limits.

Next: [Install on your Zenbook](07-install-on-your-zenbook.md)
