# 03 — Agents, budgets, policy, update

## 1. Agent adapters

Trait in `priya-core`, implementations in `priyad`:

```rust
trait AgentAdapter {
    fn name(&self) -> &'static str;
    fn command(&self, prompt: &str, opts: &RunOpts) -> Vec<String>;   // argv run inside the fork
    fn parse_line(&self, line: &str) -> Vec<AgentEvent>;              // stdout line → events
}
enum AgentEvent { Text(String), ToolUse { name: String, input: serde_json::Value },
                  Usage { tokens_in: u64, tokens_out: u64, usd: Option<f64> },
                  Final { message: String, usd: Option<f64> }, Error(String) }
```

Because the agent only ever runs inside a fork, it runs with **all of its own permission prompts disabled**. The fork is the safety net.

| Agent | Command (verify flags with `--help` on the target machine before coding) |
|---|---|
| Claude Code | `claude -p "<prompt>" --output-format stream-json --verbose --dangerously-skip-permissions` |
| Codex CLI | `codex exec --json --dangerously-bypass-approvals-and-sandbox "<prompt>"` |
| Command | `bash -lc "<cmd>"` (for `priya run --cmd`) |

The prompt is wrapped with a short preamble:

```
You are running inside a PriyaOS fork: a disposable copy of this computer.
Nothing you do affects the real machine until the user reviews and merges your changes.
You may install packages with pacman (you have passwordless sudo here), edit any file, and enable services.
Do not try to reach the real machine. When done, reply with a short summary of what you changed and how to verify it.
Task: <user prompt>
```

Passwordless sudo inside forks: at fork creation, write `/etc/sudoers.d/90-priya-fork` (`<user> ALL=(ALL) NOPASSWD: ALL`) **into the fork's root only**, and add it to the diff ignore list so it can never be merged.

Store every `AgentEvent` in `events`. The final message goes into the `history` FTS table.

## 2. Budgets

- `budget_secs` (default 30 min): when exceeded, `SIGTERM` the nspawn process, wait 10 s, `SIGKILL`; state `failed`, reason `time budget`. The fork is kept for review (partial work can still be merged).
- `budget_usd` (default 2.00): from `Usage`/`Final` events. Claude Code's final `result` event carries `total_cost_usd`; while running, estimate from per-message usage. Exceeding it stops the agent the same way.
- Defaults live in `/etc/priya/config.toml`; flags override.

## 3. Policy file

`/etc/priya/policy.toml`:

```toml
# Paths hidden from every fork (empty tmpfs mounted on top). ~ = the task user's home.
mask = [
  "~/.ssh", "~/.gnupg", "~/.password-store", "~/.local/share/keyrings",
  "~/.aws", "~/.kube", "~/.docker/config.json",
  "~/.mozilla", "~/.config/chromium", "~/.config/google-chrome", "~/.config/BraveSoftware",
]
# Never masked, so agents can authenticate.
allow = ["~/.claude", "~/.claude.json", "~/.codex"]

[taint]
tools = ["WebFetch", "WebSearch"]                       # agent tool names that read untrusted content
bash_patterns = ['\bcurl\b', '\bwget\b', 'git clone', 'https?://']
prompt_urls = true                                      # a URL in the prompt taints the task

[merge]
tainted_system_merge = "confirm"                        # "confirm" | "deny"
```

Masking files (not directories) uses `--bind-ro=/dev/null:<path>`; directories use `--tmpfs=<path>`. Skip mask entries that don't exist.

## 4. Taint labels (week-1 version)

We cannot see inside the agent's context, so week 1 tracks taint **per task**, not per value:

- A task becomes `tainted` when any rule in `[taint]` fires: a matching `ToolUse` event, a matching Bash command in a `ToolUse` input, or a URL in the prompt. Record each reason.
- Effects: the review UI shows the reasons; merging requires typed confirmation; system merge follows `tainted_system_merge`.

Write this limitation in the code comment and the docs. Real per-value information-flow control (a CaMeL-style planner that PriyaOS controls) is after week 1.

## 5. Checkers

- `--check "<cmd>"` runs `<cmd>` inside the fork (not booted) after the agent finishes, as the task user, in the task cwd. Exit code 0 → `passed`. Output stored.
- Auto-checks when no `--check` is given (week 1): if the cwd contains `Cargo.toml` → `cargo test`; `package.json` with a `test` script → `npm test`; otherwise none.

## 6. `priya update`

1. Create a fork (system scope).
2. In the fork (not booted): `pacman -Syu --noconfirm` (plus `--ignore` list from [01 §6](01-fork-diff-merge.md) when `/boot` is not inside `@`). Log output.
3. **Boot check**: boot the fork with nspawn (01 §3), collect failed units. Compare with `systemctl --failed` on the host right now; only *new* failures count.
4. If there are new failures and `--fix` (default on): run Claude Code in the fork with a prompt listing the failed units and their `journalctl -u <unit> -b --no-pager -n 80` output, then boot-check again. At most 2 fix rounds.
5. State `review`. The summary shows upgraded packages, fixes the agent made, and the final boot check result.
6. Merge = system merge (reboot to apply). `priya rollback` undoes it.

Units that cannot work in a container (hardware, `systemd-udevd`-dependent, `NetworkManager` wifi, `bluetooth`, etc.) must be filtered out of the comparison; keep the list in config with sane defaults and log which ones were skipped.
