# 05 — Tools registry and memory

## 1. Every installed program becomes a tool

### pacman hook — `packaging/hooks/priya-tools.hook`

```ini
[Trigger]
Operation = Install
Operation = Upgrade
Type = Path
Target = usr/bin/*

[Action]
Description = Priya: indexing new tools...
When = PostTransaction
Exec = /usr/bin/priya tools index --stdin
NeedsTargets
```

`priya tools index --stdin`:
- Exits immediately (status 0) if `/run/priya/fork` exists (we're inside a fork) or priyad is not reachable. **A hook failure must never break a pacman transaction**: always exit 0, log errors.
- For each path: owning package (`pacman -Qoq`), package description (`pacman -Qi`), and the first 60 lines of `--help` (fallback `-h`), captured in a sandbox:
  `bwrap --ro-bind / / --dev /dev --proc /proc --tmpfs /tmp --unshare-all --die-with-parent --new-session timeout 2 <bin> --help`
- Sends results to priyad (`tools.upsert`), stored in a `tools` table (name, path, package, description, help, indexed_at) + FTS5 index.
- `priya tools index --all` indexes everything in `/usr/bin` (run once at install; may take a few minutes, run in background with low priority).

### `priya mcp` — MCP stdio server

Started by agents (`claude mcp add priya -- priya mcp`; the ISO preconfigures this for new users). Tools:

| Tool | Does |
|---|---|
| `search_tools(query)` | FTS search over indexed programs → name, package, one-line description |
| `describe_tool(name)` | description + captured help |
| `task_history(query)` | search past tasks (what was done, when, what changed) |
| `system_info()` | distro, kernel, CPU, RAM, disk free, GPU, desktop, installed agent CLIs |

Read-only in week 1. It talks to priyad over the socket; inside a fork, the socket is not reachable, so bind it read-only into forks for these read-only methods only (add `--bind-ro=/run/priya/priyad-ro.sock`, a second socket that only serves `tools.*`, `history.search`, `system.info`).

## 2. Memory (week-1 scope)

Memory in week 1 = **task history**, because it is already structured and private:

- Every finished task writes title, prompt, summary text and the agent's final message into the `history` FTS table.
- `priya history "docker"` and the MCP `task_history` tool search it.
- Agents are told in the preamble: "Use the priya MCP tool `task_history` to find how similar things were done on this machine before."

The opt-in activity journal (what the user worked on, local and encrypted) is after week 1.
