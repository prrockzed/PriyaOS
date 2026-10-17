# PriyaOS

An AI-native operating system built on Arch Linux.

**Your whole computer is version-controlled. Agents work on copies of it. You review and merge.**

Ask for something (Super + Space). An AI agent like Claude Code does it on an instant btrfs copy of your machine. You get a diff of the whole machine (packages, services, config, files) and decide what to keep. System updates are tested on a copy and booted before you accept them, and one command rolls them back.

- Vision: [VISION.md](VISION.md)
- Docs: [docs/](docs/README.md) — simple guides in [docs/human](docs/human/), technical specs in [docs/claude](docs/claude/)

Status: week-1 prototype in progress, see [the 7-day plan](docs/human/03-the-7-day-plan.md).
