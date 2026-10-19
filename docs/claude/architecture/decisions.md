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
**Decision.** Ratatui TUI for review in early phases; Quickshell (QML, layer-shell) for the Priya bar (minimal from Phase 0, complete in Phase 5), panels, notifications UI and settings, all built from the shared `PriyaUI` component library and design tokens ([11-design-system-and-ux.md](11-design-system-and-ux.md)).
**Consequences.** Native-feeling Wayland surfaces, popular in the Hyprland community. The TUI remains the fallback and works over SSH.

## ADR-010 — Agent identity via policy principal + cgroup scope, not separate Unix users (for now)

**Context.** Windows gives each agent its own account. Separate Unix users per agent would strengthen isolation in direct mode.
**Decision.** In v1, runs execute as the requesting user inside forks (merged files must be user-owned), identified for policy/audit by `Agent::"<id>"` and a dedicated systemd scope; direct mode adds Landlock path limits.
**Consequences.** Simple ownership and merges. Revisit in Phase 14 (per-agent users with ID-mapped mounts) if direct-mode isolation proves insufficient.

## ADR-011 — Natural language compiles to the typed API

**Context.** Plain language must control everything, but behaviour must be inspectable and testable.
**Decision.** Every natural-language request becomes typed API calls (the same ones the CLI uses); settings and policies change via validated patches shown to the user before applying.
**Consequences.** One code path; everything the AI does is reproducible as commands; no hidden "AI-only" behaviour.

## ADR-012 — Walking skeleton first, then vertical slices

**Context.** The owner wants PriyaOS usable from day 1, and every phase is large.
**Decision.** Phase 0 ends with a thin end-to-end slice that is genuinely usable: Super+Space → Priya bar → priyad → one model → streamed answer, themed light/dark, installable on any Arch. Every later sub-phase thickens a slice and ends with something the owner can try. Phases are split into small sub-phases (each roughly one pull request or a few) with their own tests and gate.
**Consequences.** Feedback from real use starts immediately; integration problems surface early. Some components start minimal and are rebuilt to full spec later (e.g. the bar in Phase 0 vs Phase 5), which is planned work, not waste.

## ADR-013 — Regression protection is enforced by CI, not by discipline

**Context.** As features accumulate, changes must not silently break earlier ones.
**Decision.** Pull-request workflow with branch protection; required CI checks (unit, contract, integration on a fake machine, UI screenshots, accessibility, schema compatibility, traceability, diff coverage); every gated acceptance test joins the permanent regression suite; nightly VM end-to-end, performance, chaos, security and evaluation suites. Details in [09-testing.md](09-testing.md).
**Consequences.** Slower merges and CI cost; in exchange, "did this break something?" has a mechanical answer. A self-hosted runner on the Zenbook may be needed for KVM and hardware smoke tests.

## ADR-014 — One token file themes the whole desktop

**Context.** The OS must look consistent in light and dark, including third-party GTK/Qt apps, terminal, login and boot.
**Decision.** `ui/design/tokens.json` is the single source; generators produce the QML theme, GTK, Qt, kitty, Hyprland, greeter and GRUB themes; `priya theme apply` switches everything live. Contrast is verified in CI.
**Consequences.** One change restyles everything; third-party apps that ignore system themes (some Electron apps) stay partly inconsistent.

## ADR-015 — Strategies are a base plus filters

**Context.** Users want combinations like "only Claude, cheapest" or "local only, best".
**Decision.** A strategy is one base (`best`, `balanced`, `cheapest`, `fixed:<model>`) plus zero or more filters (`family:<name>`, `local_only`). Filters narrow candidates; the base decides scoring.
**Consequences.** Every combination is expressible and testable; the model picker shows the base and filters as separate controls.

## ADR-016 — Live actions are first-class, reversible where possible

**Context.** Some system fixes change runtime state, not files (restart a unit, switch the audio sink, unblock Wi-Fi), so they produce no ChangeSet.
**Decision.** A typed, allow-listed `LiveAction` set in the `sysadmin` module; each action has an approval tier, an inverse where one exists, and an event that appears in the Rewind timeline.
**Consequences.** Principle P4 is extended: the timeline shows ChangeSets and live actions; Rewind can undo live actions that have an inverse and explains those that don't.

## ADR-017 — Who owns notifications

**Context.** Only one program can own the desktop notification service; PriyaOS wants a notification centre with actions and grouping.
**Decision.** Until Phase 5, use the existing daemon (mako on PriyaOS installs). From Phase 5, the Quickshell notification centre owns it on PriyaOS; on existing setups `priya desktop install` asks before replacing another daemon and can revert.
**Consequences.** Notifications always work; no silent takeover of a user's setup.

## ADR-018 — Build output stays out of merges

**Context.** Running checks in a fork creates `target/`, `node_modules/` and similar, which would flood ChangeSets and merges.
**Decision.** Inside git repositories, paths ignored by the repository's own `.gitignore` are excluded from home merges by default and summarised as a count; the user can include them per task.
**Consequences.** Clean reviews; artifacts that a task deliberately produced in ignored paths must be included explicitly.

## ADR-019 — Forgetting uses redaction events

**Context.** Users must be able to delete memories and history (MEM-09), but the event log is append-only and hash-chained.
**Decision.** A `redaction.performed` event replaces the payloads of the targeted events with tombstones (type and time kept, content gone) and records the change in the chain, like compaction; derived projections, indexes and blobs are purged in the same transaction.
**Consequences.** Forgetting is real and verifiable; the audit trail shows that something was forgotten, not what.

## ADR-020 — Fluent for backend strings

**Context.** QML strings use `qsTr`, but priyad messages, CLI output, notifications and the phone app also need translation (Hindi first).
**Decision.** A shared Fluent (`fluent-rs`) message catalogue for everything outside QML, with the same locale setting.
**Consequences.** One translation workflow for backend and web; translators handle two formats (Qt `.ts` and Fluent `.ftl`).

## ADR-021 — Recipes and automations are TOML

**Context.** Recipes must be readable, shareable and consistent with other PriyaOS files (P10).
**Decision.** Recipes and automation definitions are TOML with a JSON Schema; they're scanned and signed like modules when shared.
**Consequences.** One config language across PriyaOS; no YAML footguns.

## ADR-022 — Apache-2.0 licence

**Context.** PriyaOS is open source and should be easy to adopt, embed and build on, including by companies, while protecting contributors and users from patent claims (0.2).
**Decision.** All PriyaOS code and docs are licensed under Apache-2.0, chosen by the owner on 2026-10-20. `LICENSE` holds the official text, `NOTICE` the copyright line, `THIRD_PARTY.md` outside components. Contributions are accepted under the same licence (inbound = outbound, no CLA). Cargo crates set `license = "Apache-2.0"` in `[workspace.package]`.
**Consequences.** Anyone may use, change and ship PriyaOS, including in closed products, if they keep the notices. Dependencies must have licences compatible with Apache-2.0; `cargo-deny` enforces this from 0.3. GPL-only code can't be linked into PriyaOS crates, but GPL programs can still be called as separate processes (pacman, btrfs-progs, Hyprland).

## Open questions (to decide in the named sub-phase)

- **Disk encryption with `/boot` in `@`** (9.8, 14.11): GRUB must unlock LUKS (PBKDF2 keyslot needed) and can't use the TPM. Options: two passphrase prompts; keyfile in the initramfs (must be masked from forks and the initramfs rebuilt by the helper at merge); or moving `/boot` to an unencrypted XBOOTLDR partition with a signed UKI (changes ADR-008).
- **GUI apps inside forks** (later): how agents get Wayland access to drive graphical apps in a sandbox.
- **Live apply of additive system changes** (9.4): installing an app without a reboot when the change only adds files.
