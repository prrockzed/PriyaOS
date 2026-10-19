# Phase 14 — Advanced security

**Goal.** Harden PriyaOS for real-world hostility: per-run network firewall, value-level information-flow control for the native agent, plain-language policy builder, scanning and signing of everything that runs, eBPF tracing of agents, a system MCP registry with per-agent access, stronger isolation tiers, Secure Boot and encryption.

**Depends on:** Phases 2, 4, 9, 12.
**Features:** SEC-11 … SEC-20, SBX-12.
**Read first:** [05-security](../architecture/05-security.md) (all), ADR-010.

## Build

- [ ] Egress: per-run private network namespace + veth + CONNECT proxy with domain allow-lists from capabilities; external actions blocked until approved; blocked attempts in review
- [ ] Value-level IFC for the Priya native agent (privileged planner never sees untrusted content; quarantined model for untrusted data; labelled values; approval on tool calls with untrusted-derived arguments)
- [ ] Plain-language policy builder: Priya AI drafts Cedar → plain-language summary + code → confirm → activate; policy linting and conflict detection
- [ ] Skill/script/module scanning before first run (static rules + cross-family AI review)
- [ ] Signing: publisher keys, signature verification for modules/skills/adapters, permission prompt at install
- [ ] eBPF agent tracing (exec, file, network, LLM endpoints) attached to runs; hang detection; anomaly alerts (e.g. reading outside scope)
- [ ] System MCP registry: every MCP server registered with an owner, capabilities and per-agent access lists; `priya mcp-registry` CLI
- [ ] Isolation tier: microVM runner (cloud-hypervisor/Firecracker) behind `SandboxRunner` for untrusted work
- [ ] Re-evaluate per-agent Unix users (ADR-010) with ID-mapped mounts; implement if direct-mode isolation needs it
- [ ] Platform: Secure Boot with sbctl-signed GRUB/kernel, LUKS2 + TPM2 unlock option, nftables hardening

## Tests

- Full prompt-injection corpus (web pages, repos, documents, images with hidden text) across all agents: no secret leaves, no unapproved external action, taint/IFC labels correct.
- Red-team scenarios: malicious module tries undeclared capability; agent tries to reach pastebin; skill with `curl | sh`.
- Secure Boot install in the VM (OVMF with enrolled keys) and on the Zenbook.

## Done when

- [ ] Injection corpus passes 100 % on blocked exfiltration and unapproved external actions
- [ ] Secure Boot enabled on the Zenbook with Windows still booting
- [ ] Owner can write a policy in plain language and see it enforced

**Demo.** A repo with a poisoned README asks the agent to upload `~/.ssh`: the request is blocked at the proxy, labelled, explained in plain language.
