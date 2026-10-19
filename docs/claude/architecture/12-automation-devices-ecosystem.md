# 12 — Automation, devices and teams, ecosystem

Short architecture for the later phases (13, 15, 16). The phase files hold the detail; this file fixes the shapes they must share.

## 1. Automation (Phase 13)

- **Module** `automation` owns automation definitions (TOML, JSON Schema, ADR-021): `{id, name, trigger, action, mode, autonomy, limits, notify, enabled, created_by}`.
- **Action** is one of: an intent (plain language), a recipe with parameters, or a typed module method. Every firing becomes a normal task through `v1.intent.submit` with `origin = automation:<id>`, the lowest scheduling priority, and the automation's mode as a locked rule.
- **Triggers**: time (generated systemd **user** timers, `Persistent=true`, catch-up runs once), file changes (inotify, debounced, within the automation's read capabilities), PriyaOS events (subscriptions, [02 §1.3](02-core.md)), and conditions checked at fire time (on AC, idle, network up, time window, not in Do Not Disturb). No inbound webhooks (see the backlog in [features.md](../features.md)).
- **Loop prevention**: changes made by an automation's own task never re-trigger it; automation chains are limited to depth 2.
- **Limits** (AUTO-10): per-run budget, daily cap per automation and for all automations, max runs per day, max duration, concurrency 1 per automation, runaway detection (too many firings or consecutive failures → auto-pause with a notification). External and `confirm`-tier actions always wait for the user; their approvals can expire.
- **Recipes**: TOML with parameters, steps and sub-recipes; shared recipes are scanned and signed like modules ([05 §9](05-security.md)). Teach-by-showing produces a recipe draft the user edits.
- **Dreaming**: overnight jobs (on AC power, idle) use batch APIs where the provider offers them; outputs are memory diffs the user can see.

## 2. Devices and teams (Phase 15)

- **No central PriyaOS server** (P10). Machines and phones talk directly over an encrypted private network: Tailscale (preferred, gives HTTPS names for the phone app) or plain WireGuard.
- **Gateway** `priya-remote`: a user process listening only on the private-network interface; mutual authentication with device keys; forwards an allow-listed subset of `v1.*` methods to priyad according to the device's role; every call is an event with `actor = device:<id>`. The root helper keeps `PrivateNetwork=yes`: remote work goes priyad → gateway → priyad, never to a remote helper.
- **Pairing**: QR code with a one-time code and the gateway key fingerprint; the device creates its own key (non-extractable on phones); the user confirms matching fingerprint words on the laptop. Revocation is immediate.
- **Roles** become Cedar `Device` principals: `phone` (monitor, approve, kill switch), `peer` (own machine: run steps, receive tasks), `teammate` (shared workspaces), `fleet-admin`.
- **Phone companion**: a PWA served by the gateway, built from the same design tokens (a CSS target of the token generator), the same component rules, states and copy as the desktop ([11](11-design-system-and-ux.md)). Push notifications carry **no task content** (only "something needs you"); content is fetched over the private network.
- **Portable tasks**: task state from the event log plus the workspace via `btrfs send | receive` over the private network.
- **Teams**: sharing is per project (a shared workspace), never personal files. Team policies are signed Cedar bundles every member enforces locally; audit is aggregated by members pushing signed audit summaries to each other, not to a server.

## 3. Ecosystem (Phase 16)

- **Directory**: a git repository plus static hosting (no server database, P10). Entries per publisher list versions, artifact URLs, content hashes, publisher signatures, declared capabilities, licence and minimum API version; CI builds a signed `index.json`. Submissions go through the scanning pipeline ([05 §9](05-security.md)). No user ratings (privacy); verification levels instead.
- **Install by asking**: "get me a Spotify control skill" searches the directory index locally and shows publisher, verification level and permissions before install.
- **App action SDK**: apps ship action manifests (MCP servers or declarative manifests); the tool index registers them; every action runs under the same capability model as modules.
- **Mini-apps**: generated small apps run as Quickshell windows or a restricted web view with **declared data permissions only** (read specific API methods, no shell, no network unless declared); stored, reopenable, editable by plain language, shareable through the directory (signed like modules).
- **Themes**: a theme bundle is a token file plus optional wallpaper and metadata, validated by the contrast checker before it can be applied or shared.
