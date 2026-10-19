# PriyaOS

An AI-native operating system built on Arch Linux.

Talk to your computer by typing, speaking or showing it something, and it does the work using the best AI for each job, safely.

- **Priya AI**: one AI over every model you enable (Claude, OpenAI, Gemini, Grok, local LLMs). It routes each job to the best model within and across families, optimizes tokens, and learns from outcomes.
- **Priya Orchestration**: plans work and runs many agents (Claude Code, Codex, Gemini CLI, OpenCode, a native agent for any model) in parallel, with cross-family verification.
- **Priya Sandbox**: an optional copy-on-write fork of the whole machine. Agents work on the fork; you review a machine-wide diff and merge or discard. You choose the mode per task and per feature: read-only, sandbox, or direct with a safety snapshot.
- **Priya Rewind**: a timeline of every change, with undo of any single change, including by plain language ("undo whatever broke my Wi-Fi on Tuesday").
- **Priya Vision** and voice: ask about your screen or an image; talk with push-to-talk or "Hey Priya".

Docs:
- [Vision](VISION.md)
- [Simple guides](docs/human/README.md)
- [Architecture, feature catalog and phase plans](docs/claude/README.md)

Status: planning complete, phase 0 next. See [the phases](docs/human/05-the-phases.md).
