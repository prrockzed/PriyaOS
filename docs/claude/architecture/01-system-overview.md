# 01 — System overview

## 1. Layers

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ SURFACES     Priya bar · voice · Priya Vision · CLI · review UI · Waybar ·      │
│              notifications · settings · phone (later)                         │
├──────────────────────────────────────────────────────────────────────────────┤
│ MODULES      desktop · voice · vision · updates · rewind · tool-index ·        │
│ (plugins)    memory · automation · notifiers · dev-lifecycle · …              │
├───────────────────────────── module API v1 (JSON-RPC) ───────────────────────┤
│ CORE         intent → plan → runs   (Priya Orchestration)                      │
│ (priyad,     model gateway + router (Priya AI)                                │
│  per user)   task engine · event store · policy engine · settings · IPC       │
├───────────────────────────── helper protocol (Unix socket) ──────────────────┤
│ PRIVILEGED   priya-helper (root): snapshots · sandbox launch · merge ·         │
│              rollback · privileged package ops. No network. No AI.            │
├──────────────────────────────────────────────────────────────────────────────┤
│ BASE OS      Arch Linux · btrfs · systemd · GRUB · Hyprland · PipeWire · NM    │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 2. Processes

| Process | Runs as | Unit | Responsibility | Network |
|---|---|---|---|---|
| `priya-helper` | root | `priya-helper.service` (system) | The only privileged component: btrfs snapshots, sandbox launch (nspawn), merge apply, system swap and rollback, privileged package transactions, mount of `@priya` | **None** (`PrivateNetwork=yes`, `RestrictAddressFamilies=AF_UNIX`) |
| `priyad` | the user | `priyad.service` (systemd **user** unit) | Core: event store, tasks, plans, policy, settings, module host, Priya AI, Priya Orchestration, IPC for surfaces | Yes (model APIs) |
| modules | the user | children of priyad, supervised | One feature each, talk to priyad over JSON-RPC on stdio | Declared in manifest, enforced |
| agent runs | the user (inside sandbox, or live in direct mode) | transient systemd scope per run (cgroup limits) | Claude Code / Codex / Gemini CLI / OpenCode / Priya native agent | Per-run egress policy |
| model proxy | inside priyad | — | Local OpenAI- and Anthropic-compatible endpoint on `127.0.0.1` for agents | Yes |
| `priya-egress` (Phase 14) | the user | child of priyad | Per-run egress proxy (CONNECT-level allow-lists) | Yes, filtered |
| `priya-trace` (Phase 14) | root | `priya-trace.service` | eBPF tracing of run cgroups only; read-only output to priyad | **None** |
| `priya-remote` (Phase 15) | the user | user unit | Device gateway on the tailnet/WireGuard interface only; forwards an allow-listed API subset | Private network only |

Helper socket access: `/run/priya/helper.sock` is group `priya` (every PriyaOS user; may request operations on their own home and forks) and system-level operations (system merge, rollback, doctor fixes, privileged package transactions) additionally require membership of `priya-admin`, checked by the helper from peer credentials.

Why per-user core plus root helper: the part that holds API keys, talks to the internet and runs model output never has root. A compromised or confused priyad can only ask the helper for a fixed set of validated operations, and the helper re-checks invariants (never delete `@`, `@home`, `@priya`; only the requesting user's home; system operations only for users in `priya-admin`).

## 3. Core domain model

```
Intent ──(Priya AI: understand)──▶ Plan ──▶ Step* ──▶ Run* ──▶ ChangeSet* ──▶ Merge / Revert
  │                                  │         │         │
  text / voice / image / screen      DAG     agent +   workspace: read-only | sandbox | direct
                                             model     budget, capabilities, events
```

| Entity | Meaning |
|---|---|
| **Intent** | What the user asked, with attachments (text, transcript, image, screenshot, selected files) and context (focused window, cwd, project) |
| **Task** | The user-visible unit of work created from an intent. Has mode, budget, capabilities, state |
| **Plan** | A DAG of steps produced by Priya AI for a task (a single step for simple tasks) |
| **Step** | One unit of the plan: goal, assigned agent + model role, dependencies, mode, budget |
| **Run** | One execution attempt of a step by one agent with one model, in one workspace |
| **Workspace** | Where a run executes: `ReadOnly`, `Sandbox(fork)`, `Direct(safety snapshot?)` |
| **ChangeSet** | The diff a run (or a manual change, or a settings change) produced, with before/after content addresses |
| **Approval** | A decision by the user (or a policy) about an action or a merge |
| **Event** | Immutable record of anything above happening |

Full definitions and state machines: [02-core.md](02-core.md).

## 4. Request lifecycle (typical)

1. User presses Super+Space (or says "Hey Priya", or drops an image) → surface sends `intent.submit`.
2. Priya AI classifies the intent: answer-only, action, or both; category; risk; required modalities; privacy class.
3. **Answer-only** (e.g. "why is my laptop slow?"): Priya AI runs a read-only run with system tools, answers in plain language. No ChangeSet.
4. **Action**: Priya Orchestration makes a plan, resolves execution mode per step, checks policy, asks for plan approval if policy says so.
5. Steps run in their workspaces under budgets; events stream to surfaces.
6. Verifiers check results (tests, boot check, cross-family review).
7. ChangeSets are shown for review → merge (sandbox) or keep/revert (direct).
8. Outcome is recorded; Priya AI's routing statistics update.

## 5. Storage

| Data | Where | Notes |
|---|---|---|
| Event store, projections, settings history | `~/.local/share/priya/priya.db` (SQLite, WAL) | Per user |
| Content-addressed blobs (file versions for Rewind) | User-owned content: `$XDG_DATA_HOME/priya/blobs` (written by priyad); root-owned system file versions: `/var/lib/priya/blobs/` on `@priya` (written by the helper, hashes only for files the user can't read) | BLAKE3, zstd, retention policy |
| System-level state (helper journal, prev roots index) | `/var/lib/priya/system.db` | Helper-owned |
| Forks | `@priya-forks` subvolume | Helper-owned |
| Timeline snapshots | `@priya-timeline/<ts>-{root,home}` | Rewind module; retention |
| Secrets (API keys) | Secret Service via libsecret | Never in config files or the event log |
| Model catalog | `/usr/share/priya/models/*.toml` (shipped) + `~/.config/priya/models.toml` (user) | Updatable by package |

## 6. Crate layout

```
crates/
  priya-api/        wire types for JSON-RPC (surfaces, modules, helper), versioned, schemars JSON Schema
  priya-core/       domain types, state machines, plan DAG, ChangeSet model, diff/merge decisions (pure; builds on macOS)
  priya-store/      event store, projections, blob store, migrations
  priya-policy/     Cedar policies, capabilities, approvals, risk scoring, taint
  priya-ai/         model gateway: providers, catalog, classifier, router, optimizer, learning, local proxy
  priya-orchestra/  planner, scheduler, supervisor, verifier, conflict manager
  priya-agents/     agent adapters + Priya native agent loop
  priya-sandbox/    copy engine logic (pure parts: dump parsing, classification, 3-way decisions)
  priya-sys/        Linux system calls and CLI wrappers (btrfs, nspawn, pacman, mounts); #[cfg(target_os = "linux")]
  priya-helper/     root helper binary
  priyad/           user daemon binary: wires everything, module host, IPC server
  priya-cli/        `priya` binary: CLI, TUI, `priya mcp`
  priya-module-sdk/ helper crate for writing modules in Rust
  priya-testkit/    fakes, fixtures, golden files, FakeMachine, fake agent/provider (dev-dependency only)
  priya-theme/      design-token generator (Theme.qml, GTK, Qt, kitty, Hyprland, greeter, GRUB) and contrast checker
xtask/              `cargo xtask` developer commands (test suites, coverage, trace, schemas, gallery, fake-machine)
  priya-install/    dev installer (Phase 2) and the 1.0 installer backend (Phase 9)
modules/            first-party modules (each its own crate or package)
```

Dependency direction: `priya-api` and `priya-core` depend on nothing internal; adapters depend on core; binaries depend on everything. `priya-sys` is only used by `priya-helper` (and by tests).
