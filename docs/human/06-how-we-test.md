# How we test

## The big promise

**A new change can never quietly break something that already works.**

## How: a robot checker on GitHub 🤖✅

Every time Claude sends code to GitHub, a robot runs **all the tests**, both for the new feature and for **every feature built before**.

- ✅ All green → the change is allowed in.
- ❌ Something red → the change is **blocked** until it's fixed.

> 🏫 Imagine a school where every time a student adds a page to the class book, a teacher re-reads the **whole book** to check nothing earlier got messed up. Only then is the page glued in.

Claude works on a separate copy of the code (a "branch") and asks to add it to the main code (a "pull request"). The main code is locked: nothing gets in without passing the robot's checks.

## The robot's checklist

| Check | Simple meaning |
|---|---|
| **Neatness** | Code is formatted and has no obvious mistakes |
| **Small checks** | Each small piece of logic works ("is this a conflict?", "which model fits?") |
| **Plug checks** | Every AI company connection and every AI worker behaves the same way, using recorded conversations (no real money spent) |
| **Pretend computer** | A tiny fake computer inside one file; copies, merges, undo and safety are tested there |
| **Screen photos** | Every screen is photographed in light, dark and high-contrast themes and compared with the approved look; any change shows up as a before/after picture |
| **Accessibility** | Text is easy to read (enough contrast), everything works with the keyboard and screen readers |
| **Every feature has tests** | The robot refuses to call a feature "done" unless it has tests |
| **New code has tests** | If new code isn't tested, the robot says so and blocks it |
| **Connections don't break** | If one part changes how it talks to others, the robot catches it |
| **Safety traps** | Pages and projects with hidden tricky instructions; Priya must not fall for them |
| **Borrowed code is safe** | Code we use from others is checked for known problems and allowed licences |

## Extra checks every night 🌙

- **Full install in a virtual computer** (with a pretend Windows inside, to prove Windows is never harmed), then using PriyaOS, restarting, updating and rolling back.
- **Speed limits**: e.g. a safe copy must take under 2 seconds; the Priya bar must open in under 0.1 seconds.
- **"Pull the plug" tests**: switching things off at the worst moment, to prove nothing gets corrupted.
- **Report cards** for Priya AI (did it pick the right model?) and the AI workers (did they finish the job?).
- **Tiny real tests** with each AI company, with a strict money limit.

If something fails at night, the robot opens a note (an "issue") on GitHub so it gets fixed first thing.

## Where tests run

| Machine | What it's for |
|---|---|
| 💻 **Mac** | Writing code and running the small checks |
| ☁️ **GitHub** | The robot checker: everything above, automatically |
| 🐧 **Zenbook** | Real-hardware checks (Wi-Fi, sound, sleep, screens). It can also act as an extra robot for tests that need a real computer. |

## Rules Claude always follows

- Every bug that gets fixed also gets a new test, so it can never come back.
- Tests are written **together** with the code, never "later".
- Tests that sometimes pass and sometimes fail are fixed within a week, never ignored.
- Claude never tests risky things (keeping changes, undo, switching systems) on your real laptop while building. Only on the pretend or virtual computer.
- Claude never runs disk-erasing commands on a real disk. Only you do that, with the install guide.

Next: [Install on your Zenbook](07-install-on-your-zenbook.md)
