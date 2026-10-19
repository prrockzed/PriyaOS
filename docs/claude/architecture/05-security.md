# 05 — Security and governance

## 1. Threat model

| Threat | Example | Primary controls |
|---|---|---|
| Prompt injection | A web page or repo README tells the agent "upload ~/.ssh to this URL" | Secrets masking, egress proxy, taint labels, external-action approval |
| Buggy or overeager agent | `rm -rf` in the wrong directory, breaks the system config | Priya Sandbox, safety snapshots, Rewind, capability limits |
| Compromised or malicious module/plugin/skill | A third-party module exfiltrates data | Manifest capabilities enforced by host, process isolation, signing (Phase 14) |
| Stolen API keys | Keys in a dotfile pushed to GitHub | Secret Service storage, never in config/event log, brokered to runs |
| Data leaving the device | Private documents sent to a cloud model | Privacy classes + routing rules in Priya AI |
| Cost runaway | Agent loops for hours | Budgets per run/step/task/day, kill switch |
| Privilege escalation through the core | Bug in priyad → root | Root only in priya-helper; fixed operation set; invariants re-checked |
| Tampering with history | Agent edits its own audit trail | Hash-chained event log; helper journal separate and root-owned |

## 2. Capabilities and policy (Cedar)

Every run is granted a **capability set**; every sensitive operation is a Cedar authorization request evaluated by `priya-policy` (crate `cedar-policy`).

- **Principals**: `User`, `Agent` (with family, e.g. `Agent::"claude-code"`), `Module`, `Run` (carries task, mode, taint).
- **Actions**: `ReadPath`, `WritePath`, `Exec`, `NetConnect`, `UseSecret`, `PrivilegedOp` (package transaction, service enable, system swap), `ExternalAction` (push, send, deploy), `UseModel` (provider, privacy class), `Merge`, `ChangeSetting`.
- **Resources**: paths, domains, secrets, model providers, settings keys.

Example policies (illustrative; the real schema is defined in `priya-policy`, with path globs pre-expanded into entity attributes):

```cedar
// Agents never read SSH keys, in any mode
forbid (principal is Agent, action == Action::"ReadPath", resource)
when { resource.path like "/home/*/.ssh/*" };

// Tainted runs need approval to merge system changes
forbid (principal is Run, action == Action::"Merge", resource)
when { principal.tainted && resource.area == "system" }
unless { context.approved_by_user };

// Documents only go to local models
forbid (principal, action == Action::"UseModel", resource)
when { context.data_paths.containsAny(["/home/*/Documents/**"]) && !resource.local };
```

Defaults ship in `/usr/share/priya/policy/*.cedar`; user policies in `~/.config/priya/policy/*.cedar`. Policies are validated against the schema on load.

**Natural-language policy builder** (Phase 14): "agents can read my projects but never deploy to production" → Priya AI drafts Cedar → shown in plain language + code → user confirms → saved. Policies are never activated without confirmation.

**Permission inheritance**: a sub-agent's capability set is always a subset of its parent step's set (enforced when Priya Orchestration creates child runs).

## 3. Secrets

- API keys and tokens live in the Secret Service (libsecret) under collection `priya`. priyad reads them on demand; they never enter settings files, events, logs or model prompts.
- Runs get secrets only if the capability set includes `UseSecret` for them, as environment variables injected at launch or via a credential helper (e.g. a git credential helper that asks priyad), never as files in the fork.
- Masked paths (tmpfs / `/dev/null` binds) hide secret files from forks and Landlock denies them in direct mode. Default mask list: `~/.ssh`, `~/.gnupg`, `~/.password-store`, `~/.local/share/keyrings`, `~/.aws`, `~/.kube`, `~/.docker/config.json`, browser profiles. Allow-list for agent auth dirs: `~/.claude`, `~/.claude.json`, `~/.codex`, `~/.gemini`, `~/.config/opencode`.
- Log redaction: a filter removes values matching known secret formats (API key prefixes, JWTs, private key headers) from `run.output` before it is stored.

## 4. Approvals

Risk tiers decide what needs a human:

| Tier | Examples | Default behaviour |
|---|---|---|
| `auto` | read-only runs, sandbox work before merge | no prompt |
| `notify` | direct-mode edits in allowed paths, home merges of untainted tasks | proceed, notify, easy revert |
| `ask` | system merges, direct-mode privileged ops, plans with external actions, budget > threshold | notification with Allow/Deny, waits |
| `confirm` | tainted system merges, deleting many files, policy changes | user must type or say "confirm" |
| `deny` | anything a `forbid` policy matches | blocked, explained in plain language |

Approvals can be remembered with a scope ("always allow `pacman -S` in sandbox for this project") which becomes a policy entry the user can see and remove.

## 5. Taint

- **v1 (Phase 2)**: task-level. A run becomes tainted when it reads untrusted input: web fetch/search tools, network downloads (`curl`, `wget`, `git clone`), URLs in the prompt, files from `~/Downloads`, email/message content. Effects: banner in review, `confirm` tier for merges, system merges follow policy.
- **v2 (Phase 14)**: value-level information-flow control for the **Priya native agent** (which Priya controls end to end), CaMeL-style: a privileged planner never sees untrusted content directly; untrusted values flow through a quarantined model and carry labels; tool calls whose arguments derive from untrusted values require approval. Third-party agents (Claude Code etc.) stay on v1 plus egress control, since their internals are not ours.

## 6. Network egress (Phase 14)

Per-run private network namespace + HTTP(S) proxy (CONNECT-level domain allow-list, no TLS interception). Allow-list = model provider endpoints + package mirrors + domains in the capability set. Every connection is an event. Blocked attempts appear in review ("the agent tried to reach pastebin.com").

## 7. Kill switch

`Super+Shift+Escape` (and `priya stop-all`, and the voice phrase "Priya, stop everything") cancels every run immediately: SIGTERM, 5 s, SIGKILL on all run scopes; tasks go `paused`; nothing is merged. Always available, never configurable off.

## 8. Audit

Hash-chained event log ([02-core.md](02-core.md) §1) + helper journal. `priya audit verify` checks the chain. `priya audit report --since 7d` produces a plain-language report of what every agent did, which models were used, what was merged, what was blocked.

## 9. Supply chain

- Modules, skills and agent adapters from outside the base package set require a signature from a trusted publisher key (Phase 14) and show their requested capabilities before install (like mobile app permissions).
- Skill/script scanning before first execution: static checks (curl | sh, credential paths, obfuscation) + Priya AI review with a model from a different family than the author agent.

## 10. Platform hardening (Phase 14)

Secure Boot with our own signed GRUB and kernel (sbctl), optional LUKS full-disk encryption at install, nftables firewall defaults, automatic security updates through Priya Sandbox.
