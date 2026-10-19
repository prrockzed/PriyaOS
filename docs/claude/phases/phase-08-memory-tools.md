# Phase 8 — Memory, files and tools

**Goal.** Priya remembers what matters (projects, preferences, past work), finds files by description, and every program on the machine becomes a tool agents can discover — all local, inspectable and deletable.

**Depends on:** Phases 3–5.
**Features:** MEM-02 … MEM-11, TOOL-02, TOOL-03.
**Read first:** [08 §2–3](../architecture/08-rewind-memory-tools.md).

## Build

### 8.1 Tool index (`tool-index` module)
- [ ] pacman hook + `priya tools index --stdin` (bubblewrap, 2 s timeout, always exit 0, skipped in forks), `--all` background indexing
- [ ] `.desktop` files and D-Bus introspection for apps
- [ ] FTS + embeddings over tool descriptions; `priya tools search`

### 8.2 `priya mcp` (full)
- [ ] Tools: `search_tools`, `describe_tool`, `task_history`, `project_memory`, `personal_memory`, `search_files`, `system_info`, `rewind_log`
- [ ] Relevant-tools-only injection per step (token optimizer integration)

### 8.3 Memory (`memory` module)
- [ ] Project memory files (`<project>/.priya/memory.md` or central), auto-updated after tasks with a visible diff, user-editable
- [ ] Personal memory ("remember that…", "forget…"), preferences applied to answers
- [ ] Profiles (personal / work) selected by path or manually; separate stores
- [ ] `priya memory list/show/edit/forget/export`; Quickshell memory page
- [ ] "Continue where I left off": last tasks, open files, layouts per project

### 8.4 File intelligence (`files` module)
- [ ] User-chosen permitted folders (none by default); exclusions (mask list, `.env`, keys, binaries over size)
- [ ] Local embeddings (role `embed`, local by default) into `sqlite-vec`; incremental re-index on change (inotify) with low priority and battery awareness
- [ ] Search by description; summarise folder/document/codebase; find duplicates (hash) and likely-obsolete files (age, references); relationship graph between files, tasks, projects

## Tests

- L0: exclusion rules, profile selection, memory patch generation.
- L1: embedding provider contract (local + cloud), MCP server contract.
- L2: install a package → its tools appear in search; agent in sandbox finds `jq` via `search_tools`; indexed test corpus answers 20 "find the file where…" queries with ≥ 85 % top-3 accuracy; secrets never appear in the index.

## Done when

- [ ] Owner can find a real document by description and gets correct folder summaries
- [ ] Every memory item is visible and deletable; deleting removes it from indexes
- [ ] Agents use project memory and tool search without being told

**Demo.** "Find the PDF where I compared bike prices" → result; "remember I use pnpm" → next task uses pnpm; install `ripgrep` → agent uses it in the next task.
