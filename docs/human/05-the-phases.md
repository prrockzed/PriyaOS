# The phases

We build PriyaOS in **17 phases**, and every phase is cut into **small steps** (sub-phases). Think of building a house room by room, where each room is finished, cleaned and inspected before the next one starts.

There are no fixed dates. Claude builds fast, but a step only counts as done when it's tested and works.

## Usable from day 1

You don't wait until the end to use it. Phase 0 already ends with something real:

> Press **Super + Space** on your current Arch, ask anything, and Priya answers, in a light or dark theme.

After that, every small step adds something you can try. It's like a phone that gets a new feature in every update.

## How every small step goes

1. **Plan**: Claude re-reads the plan. If the step has screens, Claude first shows you a **picture of how it will look** (light and dark), and you say yes or suggest changes.
2. **Build**: Claude writes the code in small pieces.
3. **Test**: the tests are written together with the code. GitHub runs **all** tests automatically, including every older feature's tests, so nothing old breaks quietly. → [How we test](06-how-we-test.md)
4. **Check**: everything ticked. If there's a screen, you look at it and approve (marked 👤 in the plans).

## The phases

| # | Phase | What you can do at the end | Status |
|---|---|---|---|
| 0 | **Foundations + first taste** | Super + Space → ask → answer, on your current Arch. Light and dark theme. Automatic testing on GitHub is set up. | ⬜ not started |
| 1 | **Priya AI** | Use all your AI accounts and local AIs; Priya picks the best one; switch with a button; see what it costs. | ⬜ |
| 2 | **Priya Sandbox** | Your Zenbook gets PriyaOS's special disk setup (early test installer, with care for Windows). Run anything on a safe copy, review, keep or throw away, go back. | ⬜ |
| 3 | **Jobs and safety** | Jobs with live status; Priya asks before important things; choose how much it asks; secrets hidden; big red stop button. | ⬜ |
| 4 | **AI workers** ⭐ | "Do this" → see the plan → the best AI worker does it → review in the Task center → keep. | ⬜ |
| 5 | **Complete desktop** ⭐ | A complete, beautiful desktop: top bar, notifications, settings by talking, themes for everything (auto light/dark, OLED black, accent colours). | ⬜ |
| 6 | **Looking after the computer** | Safe updates, finding and fixing problems, health page. | ⬜ |
| 7 | **Priya Rewind** | Undo anything, even by saying what went wrong. | ⬜ |
| 8 | **Memory and tools** | Project memory, your preferences, find files by describing them. | ⬜ |
| 9 | **PriyaOS 1.0** ⭐ | Beautiful installer, all normal OS settings, branded start and login screens. A fresh, polished install on your Zenbook. | ⬜ |
| 10 | **Voice** | Talk to Priya in English or Hindi; Priya screens in Hindi too. | ⬜ |
| 11 | **Priya Vision** | Priya understands your screen and pictures when you ask. | ⬜ |
| 12 | **AI team work** | Many AI workers on one big job, checking each other, through to pull requests and publishing. | ⬜ |
| 13 | **Automation** | Scheduled jobs, morning summary, overnight tidy-up, teach by showing. | ⬜ |
| 14 | **Extra-strong safety** | Internet limits per job, rules in plain words, signed add-ons, Secure Boot. | ⬜ |
| 15 | **Phone and teams** | Approve from your phone, continue on another computer, share with teammates. | ⬜ |
| 16 | **Ecosystem** | Add-on store, custom mini-apps, the desktop restyles itself, themes from your wallpaper. | ⬜ |

⭐ = big moments for you.

Each phase has between 9 and 18 small steps. You can see them all in [docs/claude/phases](../claude/phases/README.md), one file per phase, each step with its tasks, its look (if it has screens), its tests and its "done" checklist.

## When does it reach your laptop?

| When | What happens on your Zenbook |
|---|---|
| End of phase 0 | Install Priya as an app on your current Arch. Ask questions with Super + Space. |
| Phase 2 | Your Arch is either adjusted (if its disk is already the right kind) or replaced by an early PriyaOS test install, next to Windows, following a careful guide. From then on you use PriyaOS daily and every step arrives as an update. |
| Phase 9 | The polished 1.0 installer. You can do a fresh install, or just update. |

## Why this order?

- **0**: the foundations *and* a first taste, so you use it from day 1.
- **1**: make that taste smart (all models, best choice, costs). It only answers, so nothing on your computer changes yet.
- **2–3**: the safety machinery (copies, rules, approvals) **before** any AI is allowed to change anything.
- **4–5**: AI workers act, and the desktop becomes complete and beautiful.
- **6–8**: looking after the computer, undo, memory.
- **9**: polish into PriyaOS 1.0.
- **10–16**: voice, vision, AI teams, automation, stronger safety, phone and teams, and an add-on store.

Next: [How we test](06-how-we-test.md)
