# The 7-day plan

Claude writes most of the code. Your job each day is small: run a few commands on the laptop, try what was built, and say "looks good" or "this is wrong".

The detailed version for Claude is [docs/claude/08-build-plan.md](../claude/08-build-plan.md). This page is the simple version.

## Is one week realistic?

**Yes, for the prototype.** By day 7 you can install PriyaOS on your Zenbook, ask it to do things on a safe copy, keep or throw away the changes, and update safely.

**Not for everything in VISION.md.** Phone approvals, "teach by showing", the OS that "dreams" at night and so on come after week 1. Building with Claude is fast, but testing on real hardware still takes real time: restarts, USB sticks and fixing surprises.

## Day by day

| Day | What gets built | What you do | You'll see |
|---|---|---|---|
| **0** | Getting the laptop ready | Boot the Zenbook into Arch, install a few tools (Claude gives you one command), install Claude Code there | Claude knows exactly what your laptop has |
| **1** | The skeleton and the "health check" (`priya doctor`) | Nothing much; maybe run one command | A checklist that says whether a computer is ready for PriyaOS |
| **2** | Making photocopies (forks) and the "what changed" list | Run one test command | Priya runs something on a copy and lists what changed, while the real computer stays untouched |
| **3** | The manager (priyad), keeping changes, and going back | Try `priya run`, `priya merge`, `priya rollback` on a pretend computer | The full photocopy → review → keep loop working |
| **4** | Claude and Codex working inside the copy; time and money limits; safe updates | Give it a real small task | "Make a hello-world app" is done by the AI on a copy, tested, and kept |
| **5** | The desktop: Super+Space bar, top-bar icon, notifications | Try the shortcut | It feels like part of the computer, not a terminal tool |
| **6** | The phone book of tools, the notebook (history), the installable package | Maybe install it on your current Arch (only if your disk is already btrfs) | `priya history`, and the AI finding tools on its own |
| **7** | The installer USB (ISO), tested in a virtual computer first, then on the real laptop | Make the USB, install on the Zenbook (guide: [05](05-install-on-your-zenbook.md)) | PriyaOS and Windows both in the start menu |

## Status

Claude updates this line each day.

- Day 0: not started
- Day 1: not started
- Day 2: not started
- Day 3: not started
- Day 4: not started
- Day 5: not started
- Day 6: not started
- Day 7: not started

## What "done" means for the week

On your Zenbook:

1. You turn it on and pick **PriyaOS** (Windows is still there too).
2. You press **Super + Space**, ask for something, get a notification, review it, keep it.
3. You run `priya update`, restart, and everything works. `priya rollback` takes you back.

Next: [How we test](04-how-we-test.md)
