# Feature catalog

The single source of truth for **what** PriyaOS does. Every feature has an ID, where the idea came from, the phase that builds it, and a status. Phase files reference these IDs.

**Source:** `G` = ChatGPT conversation · `C` = Claude's additions · `O` = owner's ideas · `R` = borrowed from research on existing products or found while reviewing the plans
**Status:** ⬜ planned · 🟨 in progress · ✅ done · ⏸ deferred

**Adding a feature:** add a row in the right area with the next free ID, pick a phase (or `later`), and mention it in that phase's file. Don't renumber existing IDs.

## Core platform (CORE)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| CORE-01 | Hash-chained append-only event log with projections | C | 0 | 🟨 |
| CORE-02 | Module (plugin) system with versioned API, manifests, supervision | C | 0 | 🟨 |
| CORE-03 | Layered, schema-validated settings with hot reload | C | 0 | 🟨 |
| CORE-04 | Public JSON-RPC API with CLI parity for every capability | C | 0 | 🟨 |
| CORE-05 | `priya doctor` health checks and safe fixes | C | 0 | 🟨 |
| CORE-06 | Privilege split: per-user `priyad` + root `priya-helper` without network | C | 2 | ⬜ |
| CORE-07 | Task engine with durable state machine (survives restarts) | G | 3 | ⬜ |
| CORE-08 | Natural-language settings ("only use local models for Documents") | O | 5 | ⬜ |

## Priya Sandbox and execution modes (SBX)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| SBX-01 | Instant copy-on-write fork of the whole machine (system + home) | C | 2 | ⬜ |
| SBX-02 | Machine-wide diff: packages, services, users, system files, home files | C | 2 | ⬜ |
| SBX-03 | Review, merge or discard a fork | C | 2 | ⬜ |
| SBX-04 | Three-way conflict handling when user and agent changed the same file | C | 2 | ⬜ |
| SBX-05 | System changes applied on reboot; rollback including the kernel | C | 2 | ⬜ |
| SBX-06 | Execution modes: read-only, sandbox, direct (with safety snapshot), direct raw | O | 2 | ⬜ |
| SBX-07 | Mode chosen per task and per feature by rules, request or AI risk suggestion | O | 3 | ⬜ |
| SBX-08 | Direct-mode keep/revert from the safety snapshot | O+C | 2 | ⬜ |
| SBX-09 | Preview changes before applying them (sandbox = simulation) | G | 2 | ⬜ |
| SBX-10 | External actions (push, send, deploy) detected and gated by approval | C | 3 | ⬜ |
| SBX-11 | Agents run without permission prompts inside the sandbox | C | 4 | ⬜ |
| SBX-12 | MicroVM / container tier for untrusted work | R | 14 | ⬜ |
| SBX-13 | Start-menu entry to boot the previous system if a new one fails | C | 2 | ⬜ |
| SBX-14 | Interactive shell inside a fresh sandbox (`priya sandbox shell`) | R | 2 | ⬜ |
| SBX-16 | Boot counting with automatic fallback to the previous system | R | 9 | ⬜ |

## Priya AI — models, routing, tokens (AI)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| AI-01 | Providers: Anthropic, OpenAI, Google, xAI, OpenAI- and Anthropic-compatible (Ollama, LM Studio, llama.cpp, vLLM, OpenRouter) | O | 1 | ⬜ |
| AI-02 | Model catalog with automatic discovery of new models and prices | C+R | 1 | ⬜ |
| AI-03 | Add a model or endpoint by one settings entry (no code) | O | 1 | ⬜ |
| AI-04 | Switch model, family or strategy by command or plain language | O | 1 | ⬜ |
| AI-05 | Routing within one family (e.g. Haiku / Sonnet / Opus / Fable by task) | O | 1 | ⬜ |
| AI-06 | Routing across families to the best model per task | O | 1 | ⬜ |
| AI-07 | Roles (fast, smart, coding, review, vision, embed, private) instead of model names | C | 1 | ⬜ |
| AI-08 | Strategies: best, balanced, cheapest, local-only, one family, fixed model | O+C | 1 | ⬜ |
| AI-09 | Local LLMs as first-class models; offline fallback | O+G | 1 | ⬜ |
| AI-10 | Privacy routing: control what leaves the device and to which provider | G | 1 | ⬜ |
| AI-11 | Token optimizer: cache-aware prompts, compaction, trimming, retrieval, output caps | O | 1 | ⬜ |
| AI-12 | Response cache (exact and semantic) for repeat questions | G | 1 | ⬜ |
| AI-13 | Cascades: cheap model first, verify, escalate | C+R | 1 | ⬜ |
| AI-14 | Learns which model is best on this machine from real outcomes | O+C | 1 | ⬜ |
| AI-15 | Explains every routing decision in plain language | C | 1 | ⬜ |
| AI-16 | Cost tracking and budgets per call, task, day, month | G | 1 | ⬜ |
| AI-17 | Secure API key storage | G | 1 | ⬜ |
| AI-18 | Local proxy so any agent can use any model (`priya-auto`) | C+R | 1 | ⬜ |
| AI-19 | Savings report (tokens and money saved per technique) | C | 1 | ⬜ |
| AI-20 | Batch APIs for background jobs | C+R | 13 | ⬜ |
| AI-21 | Minimal model access for the walking skeleton (one cloud + one local provider) | C | 0 | 🟨 |
| AI-22 | Costs shown in the user's currency (e.g. INR) | R | 1 | ⬜ |
| AI-23 | Deprecated-model warnings and moving pins to the successor | R | 1 | ⬜ |

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
| ORC-28 | Install and log in to agents from the Agents page | R | 4 | ⬜ |

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
| ADM-14 | Full-kernel VM boot check for kernel, initramfs and bootloader updates | R | 6 | ⬜ |

## Security and governance (SEC)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| SEC-01 | Helper invariants: no privilege escalation, protected subvolumes | G | 2 | ⬜ |
| SEC-02 | Agent identity for policy and audit | G | 3 | ⬜ |
| SEC-03 | Capability permissions with Cedar policies | G | 3 | ⬜ |
| SEC-04 | Secrets hidden from agents; encrypted key store; log redaction | G+C | 3 | ⬜ |
| SEC-05 | Approval tiers (auto, notify, ask, confirm, deny); remembered approvals | G | 3 | ⬜ |
| SEC-06 | Audit log and plain-language audit reports | G | 3 | ⬜ |
| SEC-07 | Kill switch: stop every agent instantly | C | 3 | ⬜ |
| SEC-08 | Time, money and resource limits | G | 3 | ⬜ |
| SEC-09 | "Untrusted" label (task-level taint) with extra confirmation | C | 3 | ⬜ |
| SEC-10 | Prompt-injection test corpus (basic) | C | 3 | ⬜ |
| SEC-11 | Per-run network firewall with domain allow-lists | G | 14 | ⬜ |
| SEC-12 | Value-level information-flow control for the native agent | C | 14 | ⬜ |
| SEC-13 | Plain-language policy builder | G | 14 | ⬜ |
| SEC-14 | Scan skills, scripts and tools before running | G | 14 | ⬜ |
| SEC-15 | Signed modules/skills with permission prompts at install | G | 14 | ⬜ |
| SEC-16 | Detect attempts to access resources outside scope | G | 14 | ⬜ |
| SEC-17 | eBPF tracing of agent processes | R | 14 | ⬜ |
| SEC-18 | System MCP registry with per-agent access lists | R | 14 | ⬜ |
| SEC-19 | Secure Boot with signed boot chain, TPM-backed unlock, hardened firewall | C | 14 | ⬜ |
| SEC-20 | Per-agent Unix users (re-evaluate ADR-010) | R | 14 | ⬜ |
| SEC-21 | Find secret-looking files outside masked folders (`priya secrets doctor`) | R | 3 | ⬜ |
| SEC-22 | Flag ChangeSets that add or expose a secret | R | 3 | ⬜ |
| SEC-23 | "Mark as trusted" flow for untrusted tasks, with confirmation | R | 3 | ⬜ |
| SEC-24 | Security-advisory tracking for installed packages | R | 6 | ⬜ |
| SEC-25 | Security status page (Secure Boot, encryption, firewall, advisories) | R | 14 | ⬜ |

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
| UX-01 | Review screen in the terminal (TUI) | C | 2 | ⬜ |
| UX-02 | Priya bar (Super+Space), minimal: ask and get a streamed answer | G | 0 | 🟨 |
| UX-03 | Plain-language answers with cards (diff, plan, approval, command, undo) | O | 5 | ⬜ |
| UX-04 | Automatically choose between a quick answer and a task | G | 5 | ⬜ |
| UX-05 | Open apps, files, folders and settings by plain language | G | 5 | ⬜ |
| UX-06 | Ask about the current app, terminal or project | G | 5 | ⬜ |
| UX-07 | Explain errors and suggest fixes | G | 5 | ⬜ |
| UX-08 | Show plans, progress and approval requests | G | 5 | ⬜ |
| UX-09 | Notifications with action buttons | C | 5 | ⬜ |
| UX-10 | Top-bar status (running, review, failed, today's cost) | C | 5 | ⬜ |
| UX-11 | Complete Settings app | C | 5 | ⬜ |
| UX-12 | Arrange windows and workspaces for a task; restore layouts | G | 5 | ⬜ |
| UX-13 | Background work never steals focus | G | 5 | ⬜ |
| UX-14 | Keyboard, mouse and terminal always work normally | G | 5 | ⬜ |
| UX-15 | Generated mini-apps and task dashboards | G | 16 | ⬜ |
| UX-16 | Desktop restyles itself by plain language, with preview | C | 16 | ⬜ |
| UX-17 | Priya bar complete: local results while thinking, mode and model chips, attachments, launcher | G+C | 5 | ⬜ |
| UX-18 | Minimal notifications for approvals and finished tasks | C | 3 | ⬜ |
| UX-19 | Task center: task list, live detail, review GUI | C | 4 | ⬜ |
| UX-20 | Plan preview before acting, editable in place (mode, agent, model, budget) | C | 4 | ⬜ |

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
| DEVC-10 | Optional two-person approval rule for team actions | R | 15 | ⬜ |

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
| ECO-08 | Share generated mini-apps through the directory | R | 16 | ⬜ |

## Base operating system (OS)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| OS-01 | Install as a package on any existing Arch | C | 0 | 🟨 |
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
| OS-15 | Dev installer: minimal ISO with the PriyaOS disk layout for early dogfooding | C | 2 | ⬜ |
| OS-16 | Graphical installer in the PriyaOS design language | C | 9 | ⬜ |
| OS-17 | Full-disk encryption option at install, with recovery handling | R | 9 | ⬜ |
| OS-18 | Firewall defaults and Firewall settings page | R | 9 | ⬜ |
| OS-19 | Firmware updates through fwupd | R | 9 | ⬜ |
| OS-20 | Import configs from a previous install during onboarding | R | 9 | ⬜ |

## Design system and UI (DS)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| DS-01 | One design-token file generating every theme | C | 0 | ⬜ |
| DS-02 | Light and dark themes | O | 0 | 🟨 |
| DS-03 | PriyaUI component library (shared by every screen) | C | 0 | 🟨 |
| DS-04 | Component gallery: every component in every state and theme | C | 0 | ⬜ |
| DS-05 | Translatable UI strings from day one | C | 0 | ⬜ |
| DS-06 | Accessibility baseline: keyboard, screen-reader names, AA contrast | C | 0 | 🟨 |
| DS-07 | Model picker chip and "why this model?" | O+C | 1 | ⬜ |
| DS-08 | Usage and cost view | C | 1 | ⬜ |
| DS-09 | Autonomy levels: ask me first / ask when it matters / just do it, per feature | O | 3 | ⬜ |
| DS-10 | Approval card and approvals inbox | O+C | 3 | ⬜ |
| DS-11 | Kill-switch overlay | C | 3 | ⬜ |
| DS-12 | Automatic light/dark by sunrise, sunset or schedule | O | 5 | ⬜ |
| DS-13 | OLED true-black theme | C | 5 | ⬜ |
| DS-14 | High-contrast themes | C | 5 | ⬜ |
| DS-15 | Accent colour choice | C | 5 | ⬜ |
| DS-16 | Whole-desktop theming: GTK, Qt, terminal, window borders, cursor, icons, fonts | O+C | 5 | ⬜ |
| DS-17 | Motion system with reduced-motion and reduced-transparency options | C | 5 | ⬜ |
| DS-18 | Accessibility complete: font scaling, Orca journeys, focus order | C | 5 | ⬜ |
| DS-19 | Login, lock screen, boot menu and installer styled from the same tokens | C | 9 | ⬜ |
| DS-20 | Hindi UI translation | O | 10 | ⬜ |
| DS-21 | Wallpaper-derived accent and theme sharing | C | 16 | ⬜ |
| DS-22 | OLED burn-in protection (dimming and pixel shift of static elements) | R | 5 | ⬜ |

## Quality engineering and CI (QA)

| ID | Feature | Src | Phase | Status |
|---|---|---|---|---|
| QA-01 | CI on every push and pull request: lint, build, unit, contract tests | O | 0 | 🟨 |
| QA-02 | Branch protection and pull-request workflow | C | 0 | 🟨 |
| QA-03 | Local pre-commit hooks | C | 0 | 🟨 |
| QA-04 | Test kit: fakes, fixtures, golden files, fake machine handle | C | 0 | ⬜ |
| QA-05 | Feature traceability check (every done feature has tests) | O+C | 0 | 🟨 |
| QA-06 | Diff coverage gate on changed code | O+C | 0 | ⬜ |
| QA-07 | Permanent regression suite | O | 0 | 🟨 |
| QA-08 | Schema compatibility checks (API, events, settings, manifests) | C | 0 | 🟨 |
| QA-09 | UI screenshot and accessibility tests in CI | O+C | 0 | ⬜ |
| QA-10 | Dependency licence and advisory audit | C | 0 | 🟨 |
| QA-11 | Flaky-test quarantine policy | C | 0 | ⬜ |
| QA-12 | Router and agent evaluation suites | C | 1 | ⬜ |
| QA-13 | Live provider smoke tests with budgets | C | 1 | ⬜ |
| QA-14 | Mutation testing on core crates | C | 1 | ⬜ |
| QA-15 | Fake-machine integration tests in CI | C | 2 | ⬜ |
| QA-16 | Nightly VM end-to-end tests | C | 2 | ⬜ |
| QA-17 | Performance budgets | C | 2 | ⬜ |
| QA-18 | Chaos tests | C | 2 | ⬜ |
| QA-19 | Self-hosted Zenbook runner for KVM and hardware smoke tests | C | 2 | ⬜ |
| QA-20 | Security suites (fast on PRs, full nightly) | C | 3 | ⬜ |
| QA-21 | Release pipeline: signed packages, ISO, release notes | C | 9 | ⬜ |
| QA-22 | Reusable usability-session kit with SUS score as a gate artifact | R | 5 | ⬜ |

## Count by phase

| Phase | Count | Features |
|---|---|---|
| 0 Foundations and walking skeleton | 25 | CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, AI-21, UX-02, OS-01, DS-01, DS-02, DS-03, DS-04, DS-05, DS-06, QA-01, QA-02, QA-03, QA-04, QA-05, QA-06, QA-07, QA-08, QA-09, QA-10, QA-11 |
| 1 Priya AI | 26 | AI-01, AI-02, AI-03, AI-04, AI-05, AI-06, AI-07, AI-08, AI-09, AI-10, AI-11, AI-12, AI-13, AI-14, AI-15, AI-16, AI-17, AI-18, AI-19, AI-22, AI-23, DS-07, DS-08, QA-12, QA-13, QA-14 |
| 2 Priya Sandbox | 19 | CORE-06, SBX-01, SBX-02, SBX-03, SBX-04, SBX-05, SBX-06, SBX-08, SBX-09, SBX-13, SBX-14, SEC-01, UX-01, OS-15, QA-15, QA-16, QA-17, QA-18, QA-19 |
| 3 Tasks, policy and safety | 20 | CORE-07, SBX-07, SBX-10, SEC-02, SEC-03, SEC-04, SEC-05, SEC-06, SEC-07, SEC-08, SEC-09, SEC-10, SEC-21, SEC-22, SEC-23, UX-18, DS-09, DS-10, DS-11, QA-20 |
| 4 Agents and first actions | 19 | SBX-11, ORC-01, ORC-02, ORC-03, ORC-04, ORC-05, ORC-06, ORC-07, ORC-08, ORC-09, ORC-10, ORC-11, ORC-28, DEV-01, ADM-01, MEM-01, TOOL-01, UX-19, UX-20 |
| 5 Desktop experience | 26 | CORE-08, ORC-23, ORC-24, ADM-02, UX-03, UX-04, UX-05, UX-06, UX-07, UX-08, UX-09, UX-10, UX-11, UX-12, UX-13, UX-14, UX-17, DS-12, DS-13, DS-14, DS-15, DS-16, DS-17, DS-18, DS-22, QA-22 |
| 6 Looking after the computer | 13 | ADM-03, ADM-04, ADM-05, ADM-06, ADM-07, ADM-08, ADM-09, ADM-10, ADM-11, ADM-12, ADM-13, ADM-14, SEC-24 |
| 7 Priya Rewind | 10 | RWD-01, RWD-02, RWD-03, RWD-04, RWD-05, RWD-06, RWD-07, RWD-08, RWD-09, RWD-10 |
| 8 Memory, files and tools | 12 | MEM-02, MEM-03, MEM-04, MEM-05, MEM-06, MEM-07, MEM-08, MEM-09, MEM-10, MEM-11, TOOL-02, TOOL-03 |
| 9 PriyaOS 1.0 distribution | 22 | SBX-16, ECO-06, OS-02, OS-03, OS-04, OS-05, OS-06, OS-07, OS-08, OS-09, OS-10, OS-11, OS-12, OS-13, OS-14, OS-16, OS-17, OS-18, OS-19, OS-20, DS-19, QA-21 |
| 10 Voice | 8 | VOX-01, VOX-02, VOX-03, VOX-04, VOX-05, VOX-06, VOX-07, DS-20 |
| 11 Priya Vision | 9 | TOOL-04, VIS-01, VIS-02, VIS-03, VIS-04, VIS-05, VIS-06, VIS-07, VIS-08 |
| 12 Orchestration v2 and software lifecycle | 18 | ORC-12, ORC-13, ORC-14, ORC-15, ORC-16, ORC-17, ORC-18, ORC-19, ORC-20, ORC-21, ORC-22, DEV-02, DEV-03, DEV-04, DEV-05, DEV-06, DEV-07, DEV-08 |
| 13 Automation | 12 | AI-20, MEM-12, AUTO-01, AUTO-02, AUTO-03, AUTO-04, AUTO-05, AUTO-06, AUTO-07, AUTO-08, AUTO-09, AUTO-10 |
| 14 Advanced security | 12 | SBX-12, SEC-11, SEC-12, SEC-13, SEC-14, SEC-15, SEC-16, SEC-17, SEC-18, SEC-19, SEC-20, SEC-25 |
| 15 Devices and teams | 13 | ORC-25, ORC-26, ORC-27, DEVC-01, DEVC-02, DEVC-03, DEVC-04, DEVC-05, DEVC-06, DEVC-07, DEVC-08, DEVC-09, DEVC-10 |
| 16 Ecosystem | 11 | TOOL-05, UX-15, UX-16, ECO-01, ECO-02, ECO-03, ECO-04, ECO-05, ECO-07, ECO-08, DS-21 |

Total: 275 features.

## Ideas backlog (not yet assigned to a phase)

Suggestions collected while planning. Promote one by giving it an ID above and a sub-phase.

- Reconcile estimated AI spend with provider billing APIs
- Provider data-retention and training-use metadata used in privacy routing
- Retry on another model when a model refuses
- Compare two models' answers side by side
- Scrub personal data from prompts before they go to the cloud
- Queue background model calls while offline
- Per-module AI usage reporting
- Battery-aware routing between local and cloud models
- Export and import learned model statistics
- Agent Client Protocol (ACP) adapters for richer live steering
- One automatic fix attempt when checks fail (before Phase 12's escalation)
- Selective revert of a merged sandbox change before Rewind exists
- `??` in `priya sh` to explain the last error
- Undo for navigation actions
- Option to follow the Omarchy theme
- Flat subvolumes for containers (`@containers`, `@machines`) and hibernation with a swap subvolume
- LUKS header and recovery-key management page
- Lightweight Apps page for installed apps
- Fast user switching and guest sessions
- Screen-reader-capable lock screen
- Multi-turn follow-up questions in the Priya bar (minimal version exists from Phase 0)
- Pseudo-locale and clipping tests for translations
- Opt-in camera input for Priya Vision
- Inbound webhooks and internet-service triggers for automations
- Calendar and email integration for the morning brief and triggers
- Voice input in the phone companion
- Speaker verification for voice approvals
- Plain-language CI and deploy monitors as ready-made automations
- External security audit before a public release
