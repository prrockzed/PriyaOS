# Phase 3 — Priya AI

**Goal.** The OS's own AI: one gateway over every enabled model and provider, with roles, strategies, routing within and across families, local models, privacy rules, token optimization, cost tracking, learning from outcomes, plain-language explanations, and a local proxy any agent can use.

**Depends on:** Phase 2 (policy for `UseModel`, secrets, budgets).
**Features:** AI-01 … AI-19.
**Read first:** [04-priya-ai](../architecture/04-priya-ai.md), ADR-007.

## Build

### 3.1 Providers (`priya-ai`)
- [ ] Unified request/response/stream types (messages, tools, images, audio refs, cache hints, effort, max tokens, usage)
- [ ] Adapters: `anthropic`, `openai` (Responses + Chat), `google`, `xai`, `openai_compatible`, `anthropic_compatible`
- [ ] Per-provider capabilities (caching kind, batch, compaction, modalities), pricing function
- [ ] Circuit breakers, retries with jitter, timeouts, health in `priya doctor`

### 3.2 Catalog and discovery
- [ ] Catalog TOML loading (shipped + user), `data/model-priors.toml` with prior skill scores
- [ ] Discovery job: provider model-list endpoints + OpenRouter catalog → new models with limits/prices; plain-language notification "New model available: …"
- [ ] Local discovery: probe Ollama / LM Studio / llama.cpp default ports and user-listed endpoints
- [ ] `priya ai models` (enabled, available, local, prices), `priya ai add-endpoint <name> <url>`

### 3.3 Roles, strategies, control
- [ ] Roles from [04 §4](../architecture/04-priya-ai.md); strategy settings global / project / task
- [ ] `priya ai use <strategy|family|model>`; `v1.ai.use` (plain-language control lands with Phase 5, but the API is final now)
- [ ] Privacy classes and Cedar `UseModel` checks

### 3.4 Router
- [ ] Classifier (role `fast` + heuristics), candidate filtering, scoring, Thompson-sampling exploration (bounded), stickiness per task
- [ ] Cascades with verifiers (schema validation, self-consistency, `review`-role check)
- [ ] `model.called` events with candidates and reasons; `priya ai why <call-id>` plain-language explanation

### 3.5 Learning
- [ ] Outcome recorder (verifier, escalation, merge/discard, later revert, feedback), `model_stats` projection (Beta per model × category × difficulty, cost/latency EMAs)
- [ ] `priya ai stats` league table; reset on new model id

### 3.6 Token optimizer and costs
- [ ] Cache-aware context builder, provider cache controls, `prompt_cache_key`
- [ ] Tool-output trimming, compaction (provider endpoints or `fast` summary), output caps by role
- [ ] Exact + semantic response cache for read-only answers, invalidated by relevant ChangeSets
- [ ] Budgets per call/task/day/month (soft/hard), `costs_daily` projection, `priya ai costs`, `priya ai savings`

### 3.7 Local proxy
- [ ] `127.0.0.1` endpoints: `/v1/messages`, `/v1/responses`, `/v1/chat/completions`, `/v1/models`; virtual model `priya-auto`; per-agent tokens so usage is attributed
- [ ] Streaming, tool calls and images pass-through; errors mapped to each API's format

### 3.8 First user-facing use
- [ ] `priya ask "<question>"`: read-only answer using system tools in a `read_only` run where needed, routed by Priya AI, plain-language answer + the commands used

## Tests

- L0: scoring and filtering, strategy constraints, privacy denials, Thompson sampling bounds, cascade logic, cost math, cache-key stability, compaction triggers.
- L1: provider contract suite with recorded HTTP cassettes for every adapter (streaming, tools, images, errors, rate limits); proxy contract tests in all three API formats.
- Router evaluation suite (~300 labelled prompts): accuracy and simulated cost reported; baseline stored.
- Live smoke (`PRIYA_LIVE=1`): one tiny call per enabled provider, incl. local Ollama.

## Done when

- [ ] With only Claude enabled, different task types route to different Claude models, and `priya ai why` explains each choice
- [ ] With Claude + OpenAI + Gemini + local enabled, `cheapest`, `best`, `local_only`, `family:claude` all behave as specified
- [ ] Adding an Ollama model requires one settings entry (or zero, via discovery)
- [ ] Pulling the network cable switches to local models with a notice
- [ ] OpenCode (via OpenAI-compatible) and Claude Code (via `ANTHROPIC_BASE_URL`) both work against the local proxy with `priya-auto`
- [ ] `priya ai savings` shows cache and trimming savings on a repeated workload

**Demo.** Ask three different questions with only Claude enabled, then switch to `cheapest` with a local model enabled; show `priya ai why`, `priya ai stats`, `priya ai costs`.
