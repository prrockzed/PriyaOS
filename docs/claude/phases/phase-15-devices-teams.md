# Phase 15 — Devices and teams

**Goal.** PriyaOS beyond one laptop: approve and watch from the phone, continue tasks on another computer, use a more powerful remote machine, share workspaces with teammates, assign work to people and agents, and manage shared policies and several machines.

**Depends on:** Phases 9, 12, 14.
**Features:** DEVC-01 … DEVC-09, ORC-25, ORC-26, ORC-27.
**Read first:** [06 §7](../architecture/06-orchestration.md), [05-security](../architecture/05-security.md).

## Build

- [ ] Secure connectivity: Tailscale or WireGuard integration; device pairing with keys; per-device permissions
- [ ] Phone companion: PWA (status, approvals with biometric confirmation via the phone, review summaries, kill switch) + push via ntfy (self-hostable)
- [ ] Remote machines: register another PriyaOS/Linux host; run steps there (remote helper with the same protocol over the secure link); results returned as ChangeSets
- [ ] Portable tasks: serialise task state + workspace (btrfs send/receive of forks) to continue on another machine
- [ ] A2A: expose selected Priya agents as A2A agents and call external A2A agents, under policy
- [ ] Teams: shared workspaces (selected projects only), task assignment to people or agents, shared Cedar policy bundles, central audit/usage view; personal files never shared
- [ ] Fleet: list and manage several machines (updates, health, policies) from one place

## Tests

- L3: two VMs paired; task started on one, continued on the other; remote step execution; phone approval simulated via the PWA API.
- Security: unpaired device rejected; revoked device loses access immediately; shared workspace cannot read personal paths.

## Done when

- [ ] Owner approves a merge from the phone and continues a task on a second machine
- [ ] A teammate (second user) collaborates on a shared project with separate personal data

**Demo.** Start a long task on the laptop, leave, get a phone notification, review and approve on the phone; it merges at home.
