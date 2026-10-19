# 00 — Architecture principles

These rules decide every design question. When a spec and a principle disagree, the principle wins and the spec gets fixed.

## P1. Small core, everything else is a module

The core does only what everything else depends on: the event log, tasks and plans, policy, the module host, the IPC API, Priya AI (the model gateway and router) and Priya Orchestration. Every user-facing capability — desktop bar, voice, vision, updates, Rewind, tool index, memory, notifiers, phone — is a **module** that plugs into the core through the versioned module API ([02-core.md](02-core.md) §5).

Test: adding a feature should mean adding a module and a feature-catalog row, not editing the core. If a feature needs a core change, the core is missing an extension point; add the extension point, not the feature.

## P2. Ports and adapters for every external dependency

Each subsystem talks to the outside world through a trait (a "port") with swappable implementations ("adapters"):

| Port | First adapter(s) | Later |
|---|---|---|
| `SnapshotBackend` | btrfs | ZFS, overlayfs-only (no btrfs) |
| `SandboxRunner` | systemd-nspawn | bubblewrap-only, microVM (Firecracker / cloud-hypervisor) |
| `ModelProvider` | Anthropic, OpenAI, Google, xAI, OpenAI-compatible (Ollama, LM Studio, llama.cpp, vLLM, OpenRouter) | Bedrock, Vertex, Azure |
| `AgentAdapter` | Claude Code, Codex CLI, Gemini CLI, OpenCode, Priya native agent | Aider, Goose, Cursor CLI, Copilot CLI, Grok |
| `PackageManager` | pacman (+ AUR helper) | Flatpak |
| `Notifier` | desktop (libnotify) | phone, email |
| `SpeechToText` / `TextToSpeech` | whisper.cpp / Piper | cloud STT/TTS |
| `ScreenSource` | grim + Hyprland IPC + AT-SPI | other compositors |
| `SecretStore` | Secret Service (libsecret) | systemd-creds, KeePassXC, TPM-sealed |
| `SystemProbe` | live allow-listed commands | recorded fixtures (tests) |
| `AudioSource` | PipeWire | — |
| `Connectivity` | Tailscale, WireGuard | — |

Every adapter passes the same **contract test suite** for its port ([09-testing.md](09-testing.md)).

## P3. Everything is an event

Every meaningful thing that happens is an immutable event in an append-only log: task created, plan made, model called, file changed, merge applied, setting changed, approval given. Current state is a projection of the log. History, audit, memory, cost reports, learning for Priya AI and Rewind (undo by prompt) are all **readers of the same log**, not separate systems.

## P4. One change model for every way of acting

Whether an agent works in a sandbox, directly on the machine, or the user changes something by hand, the result is the same object: a **ChangeSet** (what changed, before and after, who and why). Review, merge, conflict detection, Rewind and audit all work on ChangeSets. See [03-execution-and-sandbox.md](03-execution-and-sandbox.md).

## P5. The user chooses how much safety, per task and per feature

Execution mode (read-only, sandbox, direct with safety snapshot, direct raw) is a setting resolved per task from explicit request → per-feature rule → Priya AI risk suggestion → default. Nothing is forced. Irreversible **external** actions (sending, publishing, deploying, paying) always follow approval policy regardless of mode, because no snapshot can undo them.

## P6. Least privilege, enforced outside the model

- The model proposes; deterministic code decides. Safety never depends on a prompt.
- Only a tiny root helper has root, and it has no network and no AI ([01-system-overview.md](01-system-overview.md) §2).
- Every task runs with explicit capabilities evaluated by the policy engine ([05-security.md](05-security.md)).

## P7. Natural language is the primary interface, not the only one

Every capability is reachable by plain language (typed or spoken) and also by a CLI command and an API method. Plain language is translated into the same typed API calls the CLI uses, so there is one code path, and everything the AI does is inspectable as normal commands. Answers come back in plain language plus structured "cards" (diffs, plans, approvals).

## P8. Model-agnostic and local-first

No feature hard-codes a model. Features ask Priya AI for a **role** (`fast`, `smart`, `coding`, `vision`, `private`…), and the user's settings decide which models fill it. Local models are first-class; a feature must say what happens offline.

## P9. Built to fail safely

- Every operation that changes the live machine is journaled before and after, idempotent, and recoverable after a crash.
- Every long-running thing (task, plan, agent run) is durable: it survives reboot and resumes or fails cleanly.
- Every limit is explicit: time, money, tokens, CPU, RAM, disk, concurrency.

## P10. Boring technology for the foundation

SQLite, JSON-RPC, TOML, systemd, btrfs, Rust. No server databases, no message brokers, no Kubernetes. The machine is one laptop.

## P11. Names

Native flagship features carry the Priya name, used sparingly so it stays meaningful:

| Name | What it is |
|---|---|
| **Priya AI** | The OS's own AI: routes every request to the best enabled model, optimizes tokens, learns |
| **Priya Sandbox** | Machine-wide copy-on-write sandbox with review and merge |
| **Priya Orchestration** | Planning and running many agents across model families |
| **Priya Rewind** | Timeline of every change plus undo by prompt |
| **Priya Vision** | Understanding images and the screen |

Everything else gets a plain descriptive name (model gateway, tool index, memory, settings). Don't add new "Priya X" names without updating this table.
