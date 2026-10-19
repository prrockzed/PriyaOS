# Architecture decision records

Short records of decisions that are expensive to reverse. Add a new ADR instead of editing an accepted one; mark the old one `Superseded by ADR-NNN`.

Format: **Context** → **Decision** → **Consequences**.

---

## ADR-001 — Build on Arch packages, don't fork Arch

**Context.** We want a complete OS without maintaining a kernel or base system.
**Decision.** Use Arch's official repositories unchanged; ship our software, defaults and branding as packages in our own signed `[priyaos]` repo; build installer media with archiso.
**Consequences.** Low maintenance, fast updates from Arch, same model as EndeavourOS/CachyOS/Omarchy. Arch's rolling updates can break things — mitigated by routing updates through Priya Sandbox with boot checks. We depend on Arch's packaging decisions.

## ADR-002 — Rust core; modules in any language over JSON-RPC

**Context.** The core touches the filesystem, processes and privileges, and must be reliable; features should be addable by anyone.
**Decision.** Core, helper and CLI in Rust (one Cargo workspace). Modules are out-of-process by default, speaking JSON-RPC over stdio with a versioned API (`v1`); built-in modules use the same API in-process.
**Consequences.** Memory safety and single binaries for the core; module crashes can't take down the core; features can be written in Python/TypeScript where libraries are better (e.g. ML tooling). Cost: IPC boundaries and schema versioning discipline.

## ADR-003 — Event sourcing on SQLite

**Context.** History, audit, memory, cost reporting, routing statistics and Rewind all need the same facts.
**Decision.** Append-only, hash-chained event log in SQLite (WAL) with projections updated transactionally.
**Consequences.** One source of truth, replayable, tamper-evident; projections can be rebuilt. Cost: schema evolution via versioned event types; large payloads go to the blob store.

## ADR-004 — btrfs + systemd-nspawn for Priya Sandbox; one ChangeSet model

**Context.** Agents need a full copy of the machine cheaply; all ways of acting must be reviewable and undoable the same way.
**Decision.** btrfs snapshots for forks, safety snapshots and timeline; systemd-nspawn for running inside forks; `btrfs send --no-data` + stat/hash for diffs; every mode produces a ChangeSet.
**Consequences.** Instant forks, exact diffs, kernel included in rollbacks (with `/boot` in `@`). Requires btrfs root with a specific layout. `SnapshotBackend` and `SandboxRunner` ports keep ZFS/overlayfs/microVMs possible later.

## ADR-005 — Per-user core, tiny root helper

**Context.** The component talking to the internet and running model output must not be root.
**Decision.** `priyad` runs as a systemd user service; `priya-helper` is the only root component, has no network, exposes a fixed set of validated operations and re-checks invariants.
**Consequences.** A compromised core can't get root directly; multi-user works naturally. Cost: a second protocol and process to maintain.

## ADR-006 — Cedar for policy

**Context.** Permissions must be deterministic, explainable, analysable and editable by users (including via plain language).
**Decision.** Use Cedar (`cedar-policy` crate) for authorization of runs, agents, modules and model use.
**Consequences.** Mature, fast, Rust-native, has a validator; policies are readable text Priya AI can draft. Cost: users rarely read Cedar directly — the UI shows plain-language summaries.

## ADR-007 — Priya AI as gateway + router with two levers over agents

**Context.** Users want one AI that uses the best model per job across families and within a family, minimises tokens, and works with local models; agent CLIs have their own model clients and subscriptions.
**Decision.** Priya AI routes all of Priya's own model calls. For agents it (1) chooses agent + model flag by default, and (2) optionally serves a local proxy (`/v1/messages`, `/v1/responses`, `/v1/chat/completions`) so any agent can use any model with `priya-auto` routing.
**Consequences.** Works with subscriptions and API keys alike. Some vendors don't support their CLI against other vendors' models (e.g. Anthropic's docs say Claude Code to non-Claude models via gateways is unsupported) → proxy is opt-in per agent and we track quality per (agent, model).

## ADR-008 — GRUB with /boot inside the root subvolume

**Context.** Kernel updates must be part of sandboxed system changes and rollbacks; Windows laptops have small ESPs.
**Decision.** GRUB (reads btrfs) with `/boot` in `@`; ESP mounted at `/efi` holds only the GRUB EFI binary; os-prober for Windows.
**Consequences.** Swapping `@` swaps the kernel too; tiny ESP footprint. systemd-boot is not used. Secure Boot needs our own signing (Phase 14).

## ADR-009 — Quickshell for desktop surfaces, TUI first

**Context.** We need an overlay bar, panels and review UI on Hyprland quickly, and a fallback that always works.
**Decision.** Ratatui TUI for review in early phases; Quickshell (QML, layer-shell) for the Priya bar, panels, notifications UI and settings from Phase 5.
**Consequences.** Native-feeling Wayland surfaces, popular in the Hyprland community. The TUI remains the fallback and works over SSH.

## ADR-010 — Agent identity via policy principal + cgroup scope, not separate Unix users (for now)

**Context.** Windows gives each agent its own account. Separate Unix users per agent would strengthen isolation in direct mode.
**Decision.** In v1, runs execute as the requesting user inside forks (merged files must be user-owned), identified for policy/audit by `Agent::"<id>"` and a dedicated systemd scope; direct mode adds Landlock path limits.
**Consequences.** Simple ownership and merges. Revisit in Phase 14 (per-agent users with ID-mapped mounts) if direct-mode isolation proves insufficient.

## ADR-011 — Natural language compiles to the typed API

**Context.** Plain language must control everything, but behaviour must be inspectable and testable.
**Decision.** Every natural-language request becomes typed API calls (the same ones the CLI uses); settings and policies change via validated patches shown to the user before applying.
**Consequences.** One code path; everything the AI does is reproducible as commands; no hidden "AI-only" behaviour.
