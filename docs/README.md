# PriyaOS docs

Two folders, two audiences.

## 🙂 [human/](human/) — start here if you're a person

Simple explanations with everyday examples. No coding knowledge needed.

1. [What is PriyaOS?](human/01-what-is-priyaos.md)
2. [How it works](human/02-how-it-works.md)
3. [The 7-day plan](human/03-the-7-day-plan.md) (with daily status)
4. [How we test](human/04-how-we-test.md)
5. [Install on your Zenbook](human/05-install-on-your-zenbook.md)
6. [Words explained](human/06-words-explained.md)

## 🤖 [claude/](claude/) — specs Claude builds from

Precise technical specs. Claude reads these (via [CLAUDE.md](../CLAUDE.md)) before writing code.

| File | Covers |
|---|---|
| [00-overview.md](claude/00-overview.md) | Architecture, tech choices, repo layout, machines |
| [01-fork-diff-merge.md](claude/01-fork-diff-merge.md) | btrfs layout, forks, diff, merge, rollback, /boot |
| [02-daemon-and-cli.md](claude/02-daemon-and-cli.md) | priyad, task states, SQLite schema, JSON-RPC, CLI, review TUI |
| [03-agents-and-policy.md](claude/03-agents-and-policy.md) | Agent adapters, budgets, secrets masking, taint, checkers, `priya update` |
| [04-desktop.md](claude/04-desktop.md) | Hyprland command bar, Waybar, notifications |
| [05-tools-and-memory.md](claude/05-tools-and-memory.md) | pacman tools hook, `priya mcp`, task history |
| [06-packaging-and-iso.md](claude/06-packaging-and-iso.md) | PKGBUILD, archiso, dual-boot-safe installer, Zenbook notes |
| [07-testing.md](claude/07-testing.md) | Unit, fake machine, VM, real laptop |
| [08-build-plan.md](claude/08-build-plan.md) | 7-day plan with checkboxes (progress tracker) |

The product vision is in [VISION.md](../VISION.md).
