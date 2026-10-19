# Feature catalog

The single source of truth for **what** PriyaOS does. Every feature has an ID, where the idea came from, the phase that builds it, and a status. Phase files reference these IDs.

**Source:** `G` = ChatGPT conversation · `C` = Claude's additions · `O` = owner's ideas · `R` = borrowed from research on existing products
**Status:** ⬜ planned · 🟨 in progress · ✅ done · ⏸ deferred

**Adding a feature:** add a row in the right area with the next free ID, pick a phase (or `later`), and mention it in that phase's file. Don't renumber existing IDs.

## Core platform (CORE)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| CORE-01 | Hash-chained append-only event log with projections | C | 0 | ⬜ |
| CORE-02 | Module (plugin) system with versioned API, manifests, supervision | C | 0 | ⬜ |
| CORE-03 | Layered, schema-validated settings with hot reload | C | 0 | ⬜ |
| CORE-04 | Public JSON-RPC API with CLI parity for every capability | C | 0 | ⬜ |
| CORE-05 | `priya doctor` health checks and safe fixes | C | 0 | ⬜ |
| CORE-06 | Privilege split: per-user `priyad` + root `priya-helper` without network | C | 1 | ⬜ |
| CORE-07 | Task engine with durable state machine (survives restarts) | G | 2 | ⬜ |
| CORE-08 | Natural-language settings ("only use local models for Documents") | O | 5 | ⬜ |

## Priya Sandbox and execution modes (SBX)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| SBX-01 | Instant copy-on-write fork of the whole machine (system + home) | C | 1 | ⬜ |
| SBX-02 | Machine-wide diff: packages, services, users, system files, home files | C | 1 | ⬜ |
| SBX-03 | Review, merge or discard a fork | C | 1 | ⬜ |
| SBX-04 | Three-way conflict handling when user and agent changed the same file | C | 1 | ⬜ |
| SBX-05 | System changes applied on reboot; rollback including the kernel | C | 1 | ⬜ |
| SBX-06 | Execution modes: read-only, sandbox, direct (with safety snapshot), direct raw | O | 1 | ⬜ |
| SBX-07 | Mode chosen per task and per feature by rules, request or AI risk suggestion | O | 2 | ⬜ |
| SBX-08 | Direct-mode keep/revert from the safety snapshot | O+C | 1 | ⬜ |
| SBX-09 | Preview changes before applying them (sandbox = simulation) | G | 1 | ⬜ |
| SBX-10 | External actions (push, send, deploy) detected and gated by approval | C | 2 | ⬜ |
| SBX-11 | Agents run without permission prompts inside the sandbox | C | 4 | ⬜ |
| SBX-12 | MicroVM / container tier for untrusted work | R | 14 | ⬜ |
| SBX-13 | Start-menu entry to boot the previous system if a new one fails | C | 9 | ⬜ |

## Priya AI — models, routing, tokens (AI)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| AI-01 | Providers: Anthropic, OpenAI, Google, xAI, OpenAI- and Anthropic-compatible (Ollama, LM Studio, llama.cpp, vLLM, OpenRouter) | O | 3 | ⬜ |
| AI-02 | Model catalog with automatic discovery of new models and prices | C+R | 3 | ⬜ |
| AI-03 | Add a model or endpoint by one settings entry (no code) | O | 3 | ⬜ |
| AI-04 | Switch model, family or strategy by command or plain language | O | 3 | ⬜ |
| AI-05 | Routing within one family (e.g. Haiku / Sonnet / Opus / Fable by task) | O | 3 | ⬜ |
| AI-06 | Routing across families to the best model per task | O | 3 | ⬜ |
| AI-07 | Roles (fast, smart, coding, review, vision, embed, private) instead of model names | C | 3 | ⬜ |
| AI-08 | Strategies: best, balanced, cheapest, local-only, one family, fixed model | O+C | 3 | ⬜ |
| AI-09 | Local LLMs as first-class models; offline fallback | O+G | 3 | ⬜ |
| AI-10 | Privacy routing: control what leaves the device and to which provider | G | 3 | ⬜ |
| AI-11 | Token optimizer: cache-aware prompts, compaction, trimming, retrieval, output caps | O | 3 | ⬜ |
| AI-12 | Response cache (exact and semantic) for repeat questions | G | 3 | ⬜ |
| AI-13 | Cascades: cheap model first, verify, escalate | C+R | 3 | ⬜ |
| AI-14 | Learns which model is best on this machine from real outcomes | O+C | 3 | ⬜ |
| AI-15 | Explains every routing decision in plain language | C | 3 | ⬜ |
| AI-16 | Cost tracking and budgets per call, task, day, month | G | 3 | ⬜ |
| AI-17 | Secure API key storage | G | 3 | ⬜ |
| AI-18 | Local proxy so any agent can use any model (`priya-auto`) | C+R | 3 | ⬜ |
| AI-19 | Savings report (tokens and money saved per technique) | C | 3 | ⬜ |
| AI-20 | Batch APIs for background jobs | C+R | 13 | ⬜ |

## Priya Orchestration and agents (ORC)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| ORC-01 | Agent adapters: Claude Code, Codex CLI, Gemini CLI, OpenCode | G+O | 4 | ⬜ |
| ORC-02 | Priya native agent that works with any model, including local | C | 4 | ⬜ |
| ORC-03 | Agent registry: detect installed agents, versions, logins | C | 4 | ⬜ |
| ORC-04 | Best (agent, model) pair chosen per step | O | 4 | ⬜ |
| ORC-05 | Task statuses and live progress (queued, running, blocked, failed, done) | G | 4 | ⬜ |
| ORC-06 | Pause, resume, stop, restart a run | G | 4 | ⬜ |
| ORC-07 | Live steering of a running agent | C | 4 | ⬜ |
| ORC-08 | Background agents | G | 4 | ⬜ |
| ORC-09 | One normalised event stream for every agent | R | 4 | ⬜ |
| ORC-10 | Per-project setup / run / check scripts | R | 4 | ⬜ |
| ORC-11 | Basic verifiers (tests, build, boot check) | G | 4 | ⬜ |
| ORC-12 | Multi-step plans (DAG), plan approval and editing in plain language | C | 12 | ⬜ |
| ORC-13 | Parallel agents in separate forks / git worktrees | G | 12 | ⬜ |
| ORC-14 | Sub-agents with permission inheritance | G | 12 | ⬜ |
| ORC-15 | Handoffs, shared blackboard and agent messages | G+R | 12 | ⬜ |
| ORC-16 | Conflict manager across parallel agents | G | 12 | ⬜ |
| ORC-17 | Independent cross-family verifier | G+C | 12 | ⬜ |
| ORC-18 | Resource scheduler (CPU, RAM, battery, rate limits) | G | 12 | ⬜ |
| ORC-19 | Stuck detection, auto-retry, escalation to other models/agents | C | 12 | ⬜ |
| ORC-20 | Reliable resume after crash or reboot | G | 12 | ⬜ |
| ORC-21 | More adapters: Grok Build, Aider, Goose, Cursor CLI, Copilot CLI | O+R | 12 | ⬜ |
| ORC-22 | Agent and model success/cost dashboards | C | 12 | ⬜ |
| ORC-23 | Review loop: comment on a diff, agent revises | R | 5 | ⬜ |
| ORC-24 | Watch agents work in their own desktop workspace | C | 5 | ⬜ |
| ORC-25 | Agents on another machine / remote server | G | 15 | ⬜ |
| ORC-26 | A2A protocol for external agents | R | 15 | ⬜ |
| ORC-27 | Portable tasks between models and machines | G | 15 | ⬜ |

## Software development lifecycle (DEV)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| DEV-01 | Run tests, lints, builds automatically after changes | G | 4 | ⬜ |
| DEV-02 | Understand a repository and its architecture | G | 12 | ⬜ |
| DEV-03 | Branch + worktree per agent | G | 12 | ⬜ |
| DEV-04 | Agents review each other's code | G | 12 | ⬜ |
| DEV-05 | Create pull requests with summaries | G | 12 | ⬜ |
| DEV-06 | Approval before merging important changes | G | 12 | ⬜ |
| DEV-07 | Run CI and diagnose failures | G | 12 | ⬜ |
| DEV-08 | Deploy after authorization; monitor; rollback | G | 12 | ⬜ |

## AI system administration (ADM)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| ADM-01 | Install and configure software by plain language | G | 4 | ⬜ |
| ADM-02 | Natural-language shell (plain language and normal commands together) | G+R | 5 | ⬜ |
| ADM-03 | Safe updates: update a copy, boot-test it, AI fixes breakage, then apply | C | 6 | ⬜ |
| ADM-04 | Diagnose CPU, memory, disk, network problems | G | 6 | ⬜ |
| ADM-05 | Explain why a service or container failed | G | 6 | ⬜ |
| ADM-06 | Fix broken dependencies and package conflicts | G | 6 | ⬜ |
| ADM-07 | Read and explain logs and errors | G | 6 | ⬜ |
| ADM-08 | Set up development environments | G | 6 | ⬜ |
| ADM-09 | Help with Wi-Fi, Bluetooth, displays, audio, peripherals | G | 6 | ⬜ |
| ADM-10 | Battery-life and performance suggestions | G | 6 | ⬜ |
| ADM-11 | Explain kernel, driver and update issues | G | 6 | ⬜ |
| ADM-12 | Health checks and maintenance recommendations | G | 6 | ⬜ |
| ADM-13 | Verify a repair actually fixed the problem | G | 6 | ⬜ |

## Security and governance (SEC)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| SEC-01 | Helper invariants: no privilege escalation, protected subvolumes | G | 1 | ⬜ |
| SEC-02 | Agent identity for policy and audit | G | 2 | ⬜ |
| SEC-03 | Capability permissions with Cedar policies | G | 2 | ⬜ |
| SEC-04 | Secrets hidden from agents; encrypted key store; log redaction | G+C | 2 | ⬜ |
| SEC-05 | Approval tiers (auto, notify, ask, confirm, deny); remembered approvals | G | 2 | ⬜ |
| SEC-06 | Audit log and plain-language audit reports | G | 2 | ⬜ |
| SEC-07 | Kill switch: stop every agent instantly | C | 2 | ⬜ |
| SEC-08 | Time, money and resource limits | G | 2 | ⬜ |
| SEC-09 | "Untrusted" label (task-level taint) with extra confirmation | C | 2 | ⬜ |
| SEC-10 | Prompt-injection test corpus (basic) | C | 2 | ⬜ |
| SEC-11 | Per-run network firewall with domain allow-lists | G | 14 | ⬜ |
| SEC-12 | Value-level information-flow control for the native agent | C | 14 | ⬜ |
| SEC-13 | Plain-language policy builder | G | 14 | ⬜ |
| SEC-14 | Scan skills, scripts and tools before running | G | 14 | ⬜ |
| SEC-15 | Signed modules/skills with permission prompts at install | G | 14 | ⬜ |
| SEC-16 | Detect attempts to access resources outside scope | G | 14 | ⬜ |
| SEC-17 | eBPF tracing of agent processes | R | 14 | ⬜ |
| SEC-18 | System MCP registry with per-agent access lists | R | 14 | ⬜ |
| SEC-19 | Secure Boot, full-disk encryption option, firewall defaults | C | 14 | ⬜ |
| SEC-20 | Per-agent Unix users (re-evaluate ADR-010) | R | 14 | ⬜ |

## Priya Rewind (RWD)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| RWD-01 | One timeline of every change (agents, settings, manual) | C | 7 | ⬜ |
| RWD-02 | Automatic timeline snapshots (hourly and around package installs) | C | 7 | ⬜ |
| RWD-03 | Undo one specific change, keep everything after it | O+C | 7 | ⬜ |
| RWD-04 | Restore one file or folder to any time | C | 7 | ⬜ |
| RWD-05 | Undo by prompt ("undo whatever broke my Wi-Fi on Tuesday") | O | 7 | ⬜ |
| RWD-06 | Return the whole system to a point in time | C | 7 | ⬜ |
| RWD-07 | See which agent or person changed each file and why | G | 7 | ⬜ |
| RWD-08 | Check that an undo restored the expected state | G | 7 | ⬜ |
| RWD-09 | Undo settings changes | C | 7 | ⬜ |
| RWD-10 | Retention rules and disk-space management | C | 7 | ⬜ |

## Memory, files, tools (MEM, TOOL)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| MEM-01 | Searchable task history | C | 4 | ⬜ |
| MEM-02 | Project memory (decisions, conventions, working commands) | G | 8 | ⬜ |
| MEM-03 | Personal preferences and remembered facts | G | 8 | ⬜ |
| MEM-04 | Find files by description, not filename | G | 8 | ⬜ |
| MEM-05 | Summarise folders, documents and codebases | G | 8 | ⬜ |
| MEM-06 | Find duplicate and obsolete files | G | 8 | ⬜ |
| MEM-07 | Connect related files, tasks and projects | G | 8 | ⬜ |
| MEM-08 | Separate personal and work profiles | G | 8 | ⬜ |
| MEM-09 | Inspect, correct, export and delete any memory | G | 8 | ⬜ |
| MEM-10 | Sensitive indexing stays local; secrets never indexed | G | 8 | ⬜ |
| MEM-11 | "Continue where I left off" | G | 8 | ⬜ |
| MEM-12 | Overnight consolidation ("dreaming") and morning brief | C | 13 | ⬜ |
| TOOL-01 | `priya mcp` server for agents (basic) | C | 4 | ⬜ |
| TOOL-02 | Every installed program becomes a searchable tool (pacman hook) | C | 8 | ⬜ |
| TOOL-03 | Agents discover tools without hard-coded integrations | G | 8 | ⬜ |
| TOOL-04 | Use apps through D-Bus and accessibility actions | G | 11 | ⬜ |
| TOOL-05 | SDK for apps to expose structured actions | G | 16 | ⬜ |

## Natural language and desktop (UX)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| UX-01 | Review screen in the terminal (TUI) | C | 4 | ⬜ |
| UX-02 | Priya bar (Super+Space) for typing requests | G | 5 | ⬜ |
| UX-03 | Plain-language answers with cards (diff, plan, approval, command, undo) | O | 5 | ⬜ |
| UX-04 | Automatically choose between a quick answer and a task | G | 5 | ⬜ |
| UX-05 | Open apps, files, folders and settings by plain language | G | 5 | ⬜ |
| UX-06 | Ask about the current app, terminal or project | G | 5 | ⬜ |
| UX-07 | Explain errors and suggest fixes | G | 5 | ⬜ |
| UX-08 | Show plans, progress and approval requests | G | 5 | ⬜ |
| UX-09 | Notifications with action buttons | C | 5 | ⬜ |
| UX-10 | Top-bar status (running, review, failed, today's cost) | C | 5 | ⬜ |
| UX-11 | Graphical review and settings panels | C | 5 | ⬜ |
| UX-12 | Arrange windows and workspaces for a task; restore layouts | G | 5 | ⬜ |
| UX-13 | Background work never steals focus | G | 5 | ⬜ |
| UX-14 | Keyboard, mouse and terminal always work normally | G | 5 | ⬜ |
| UX-15 | Generated mini-apps and task dashboards | G | 16 | ⬜ |
| UX-16 | Desktop restyles itself by plain language, with preview | C | 16 | ⬜ |

## Voice (VOX)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| VOX-01 | Push-to-talk (hold Super+V) | O | 10 | ⬜ |
| VOX-02 | Optional wake word "Hey Priya" | O | 10 | ⬜ |
| VOX-03 | Local speech-to-text | O | 10 | ⬜ |
| VOX-04 | Spoken answers (local text-to-speech) | O | 10 | ⬜ |
| VOX-05 | Voice approvals and confirmations | C | 10 | ⬜ |
| VOX-06 | English and Hindi | O | 10 | ⬜ |
| VOX-07 | Audio stays on the device by default | C | 10 | ⬜ |

## Priya Vision (VIS)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| VIS-01 | "What's on my screen?" | O | 11 | ⬜ |
| VIS-02 | Explain an error, chart or page from a screenshot | O | 11 | ⬜ |
| VIS-03 | Understand dropped or pasted images | O | 11 | ⬜ |
| VIS-04 | Read app interfaces through the accessibility tree | C | 11 | ⬜ |
| VIS-05 | Act on what's visible (click, fill) with approval | G | 11 | ⬜ |
| VIS-06 | OCR fallback when no vision model is enabled | C | 11 | ⬜ |
| VIS-07 | Private windows are never captured | C | 11 | ⬜ |
| VIS-08 | Region capture shortcut (Super+Shift+S) | C | 11 | ⬜ |

## Automation and proactive help (AUTO)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| AUTO-01 | Scheduled tasks ("every morning…") | G | 13 | ⬜ |
| AUTO-02 | Triggers: file changes, failed tests, events | G | 13 | ⬜ |
| AUTO-03 | Monitor chosen services and report failures | G | 13 | ⬜ |
| AUTO-04 | Safe predefined maintenance | G | 13 | ⬜ |
| AUTO-05 | Daily project summaries | G | 13 | ⬜ |
| AUTO-06 | Suggest the next step when a task is blocked | G | 13 | ⬜ |
| AUTO-07 | Spot repetitive work and offer to automate it | G | 13 | ⬜ |
| AUTO-08 | User recipes (shareable workflow files) | G+R | 13 | ⬜ |
| AUTO-09 | Teach by showing | C | 13 | ⬜ |
| AUTO-10 | Limits on automations (time, money, resources, external actions) | G | 13 | ⬜ |

## Devices and teams (DEVC)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| DEVC-01 | Approve and monitor from the phone | G+C | 15 | ⬜ |
| DEVC-02 | Continue a task on another computer | G | 15 | ⬜ |
| DEVC-03 | Use a more powerful remote machine | G | 15 | ⬜ |
| DEVC-04 | Share selected workspaces with teammates | G | 15 | ⬜ |
| DEVC-05 | Assign tasks to people and agents | G | 15 | ⬜ |
| DEVC-06 | Shared team policies | G | 15 | ⬜ |
| DEVC-07 | Central usage, audit and access management | G | 15 | ⬜ |
| DEVC-08 | Personal files isolated from shared workspaces | G | 15 | ⬜ |
| DEVC-09 | Manage several computers from one place | G | 15 | ⬜ |

## Ecosystem (ECO)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| ECO-01 | Module SDK and developer docs | C | 16 | ⬜ |
| ECO-02 | Directory of trusted skills and integrations | G | 16 | ⬜ |
| ECO-03 | Publisher verification and package integrity | G | 16 | ⬜ |
| ECO-04 | Apps that work for both people and agents | G | 16 | ⬜ |
| ECO-05 | Find apps by plain-language request | G | 16 | ⬜ |
| ECO-06 | Normal Linux apps alongside AI-native ones | G | 9 | ⬜ |
| ECO-07 | Install PriyaOS as a layer on Omarchy / any btrfs Arch | C | 16 | ⬜ |

## Base operating system (OS)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| OS-01 | Install as a package on existing Arch (dogfooding) | C | 4 | ⬜ |
| OS-02 | Our signed package repository | C | 9 | ⬜ |
| OS-03 | Bootable installer USB (ISO) | G | 9 | ⬜ |
| OS-04 | Dual-boot-safe installer (never touches Windows) | C | 9 | ⬜ |
| OS-05 | First-boot setup: providers, keys, strategy, modes, privacy | C | 9 | ⬜ |
| OS-06 | Desktop defaults: Hyprland, Quickshell shell, lock screen, idle | G | 9 | ⬜ |
| OS-07 | Network, Bluetooth, audio, power, display panels | C | 9 | ⬜ |
| OS-08 | Keyboard layouts and input methods (incl. Hindi) | C | 9 | ⬜ |
| OS-09 | App installs (pacman, AUR, Flatpak) routed through Priya Sandbox | C | 9 | ⬜ |
| OS-10 | Printing and scanning (optional) | C | 9 | ⬜ |
| OS-11 | Accessibility (screen reader, scaling, contrast) | C | 9 | ⬜ |
| OS-12 | Multiple users | C | 9 | ⬜ |
| OS-13 | PriyaOS branding (boot, login, wallpapers) | C | 9 | ⬜ |
| OS-14 | Optional off-device backups | C | 9 | ⬜ |

## Count by phase

| Phase | Features |
|---|---|
| 0 Foundations | CORE-01..05 |
| 1 Priya Sandbox | CORE-06, SBX-01..06, 08, 09, SEC-01 |
| 2 Tasks, policy, security base | CORE-07, SBX-07, 10, SEC-02..10 |
| 3 Priya AI | AI-01..19 |
| 4 Agents and first end-to-end | ORC-01..11, SBX-11, DEV-01, ADM-01, MEM-01, TOOL-01, UX-01, OS-01 |
| 5 Natural language and desktop | CORE-08, ADM-02, ORC-23, 24, UX-02..14 |
| 6 System admin and safe updates | ADM-03..13 |
| 7 Priya Rewind | RWD-01..10 |
| 8 Memory, files, tools | MEM-02..11, TOOL-02, 03 |
| 9 Distro v0.1 | OS-02..14, SBX-13, ECO-06 |
| 10 Voice | VOX-01..07 |
| 11 Priya Vision | VIS-01..08, TOOL-04 |
| 12 Orchestration v2 and dev lifecycle | ORC-12..22, DEV-02..08 |
| 13 Automation | AUTO-01..10, MEM-12, AI-20 |
| 14 Advanced security | SEC-11..20, SBX-12 |
| 15 Devices and teams | DEVC-01..09, ORC-25..27 |
| 16 Ecosystem | ECO-01..05, 07, TOOL-05, UX-15, 16 |
