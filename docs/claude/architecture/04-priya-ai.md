# 04 — Priya AI: model gateway, router, token optimizer

Priya AI is the OS's own AI. It is not a model we train. It is a **router and gateway** over every model the user has enabled — across families (Anthropic, OpenAI, Google, xAI, local) and within a family (e.g. Haiku / Sonnet / Opus / Fable and their versions) — that sends each job to the model best suited to it at the lowest token cost, and learns from outcomes on this machine.

## 1. Components

```
                ┌───────────────────────────── Priya AI (crate priya-ai, inside priyad) ─────────────────────────────┐
 callers ──────▶│ classifier ─▶ router ─▶ optimizer ─▶ provider adapter ─▶ HTTP ─▶ model                          │
 (core, modules,│     │            │          │                                                                     │
  agents via    │     │       model catalog   context builder (cache-aware layout, compaction, retrieval)           │
  local proxy)  │     │       + stats         response cache · cascade/verify · budget guard                         │
                │     └──────── outcome recorder ◀── verifiers, merges, reverts, user feedback                       │
                └──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## 2. Providers (port `ModelProvider`)

| Adapter | Covers | Notes |
|---|---|---|
| `anthropic` | Claude models | Messages API, prompt caching (`cache_control`, 5 min / 1 h TTL), batches, model list (`GET /v1/models` returns limits and capabilities) |
| `openai` | GPT / Codex models | Responses API + Chat Completions, automatic caching (`prompt_cache_key`), batches |
| `google` | Gemini | `generateContent`, implicit caching, `models.list` |
| `xai` | Grok | OpenAI-compatible + `/v1/language-models` metadata, compaction endpoint |
| `openai_compatible` | Ollama, LM Studio, llama.cpp server, vLLM, OpenRouter, any compatible endpoint | One adapter, configured by base URL |
| `anthropic_compatible` | Ollama, LM Studio, llama.cpp, vLLM (all now expose `/v1/messages`) | Lets local models serve Anthropic-format callers |

Adding a model on an existing provider = a catalog entry (often auto-discovered). Adding a new kind of provider = one adapter implementing:

```rust
#[async_trait]
trait ModelProvider {
    fn id(&self) -> ProviderId;
    async fn list_models(&self) -> Result<Vec<DiscoveredModel>>;
    async fn complete(&self, req: ModelRequest) -> Result<ModelStream>;   // unified request: messages, tools, images, audio, cache hints, max tokens, effort
    fn capabilities(&self) -> ProviderCaps;                                // caching kind, batch, compaction, modalities
    fn price(&self, model: &ModelId, usage: &Usage) -> Money;
    async fn health(&self) -> Health;                                      // a free call (`GET /models`), 2 s; never spends tokens
}
```

Keys: Secret Service; never logged. Subscription logins for agent CLIs (Claude Code, Codex) stay with those CLIs; Priya AI doesn't need them to orchestrate those agents (§7).

## 3. Model catalog

`/usr/share/priya/models/*.toml` (shipped, updated by package) + auto-discovery (daily: provider model-list endpoints and OpenRouter's catalog for pricing/limits) + `~/.config/priya/models.toml` (user overrides and custom endpoints).

```toml
[[model]]
id = "anthropic/claude-sonnet-5-5"
family = "claude"
tier = "mid"                         # small | mid | large | frontier
context = 1_000_000
modalities = { input = ["text", "image"], output = ["text"] }
tools = true
price = { input = 3.00, output = 15.00, cache_read = 0.15, unit = "usd_per_mtok" }   # illustrative; real values come from discovery
local = false
privacy = "cloud"                    # cloud | local
# prior skill scores 0..1 per category; replaced over time by measured outcomes
skills = { code_edit = 0.92, code_plan = 0.85, analysis = 0.86, summarize = 0.88, chat = 0.85, vision = 0.80, classify = 0.90, extract = 0.90, sysadmin = 0.85 }

[[model]]
id = "ollama/qwen3-coder:30b"
family = "local"
tier = "mid"
endpoint = "http://127.0.0.1:11434"
local = true
privacy = "local"
skills = { code_edit = 0.70, classify = 0.80, summarize = 0.72 }
```

Prior skill scores come from a shipped table maintained in this repo (`data/model-priors.toml`), built from public benchmarks and our own router evaluation suite, and versioned. They are only priors.

## 4. Roles, strategies, user control

**Roles** are what callers ask for; they never name models:

| Role | Used for |
|---|---|
| `fast` | classification, intent parsing, short answers, titles |
| `smart` | planning, analysis, explanations, undo-by-prompt reasoning |
| `coding` | code edits inside agents |
| `review` | verifying another model's work — must prefer a **different family** than the worker |
| `vision` | Priya Vision |
| `embed` | memory and file index |
| `private` | anything touching private data classes — local only by default |
| `transcribe` | voice (when cloud STT is allowed) |

**Strategies** (global, per project, per task). A strategy is a **base** (`best`, `balanced`, `cheapest`, `fixed:<model>`) plus optional **filters** (`family:<name>`, `local_only`), so "only Claude, cheapest" is `cheapest` + `family:claude` (ADR-015):

| Strategy | Behaviour |
|---|---|
| `best` | maximise expected quality within budget |
| `balanced` (default) | quality vs cost trade-off, cascade cheap → strong |
| `cheapest` | lowest cost that clears a minimum quality bar |
| `local_only` | never leave the device |
| `family:<name>` | only one family (e.g. "only Claude today") — Priya AI still picks the best model **within** that family |
| `fixed:<model>` | pin one model |

**Privacy classes**: every endpoint is `local` (same machine), `trusted_local` (LAN machine the user explicitly marked as theirs), or `cloud`. LAN endpoints default to `cloud` until marked. Requests carry a data class (`secret` never leaves the process, `private` → `local`/`trusted_local` only by default, `normal` → any allowed endpoint).

User control is by plain language and by settings: "use only Claude today", "use Opus for this", "cheapest mode", "keep my documents on local models", "turn on Grok", "add my LM Studio at 192.168.1.20:1234". All become `setting.changed` events.

## 5. Routing algorithm

For each request:

1. **Classify** (role `fast`, or heuristics when the caller already knows): category, difficulty (1–5), required modalities, estimated input size, privacy class (from data paths and taint), latency need (interactive vs background).
2. **Filter** candidates: enabled + healthy (circuit breaker), modality support, context fits, privacy allowed (Cedar `UseModel`), strategy constraints, budget remaining, provider rate limits.
3. **Score** each candidate:
   `score = Q̂(model, category, difficulty) − λ_cost · Ĉ(model, request) − λ_lat · L̂(model)`
   where `Q̂` is the expected success probability (Bayesian estimate: prior from catalog skills, updated by local outcomes), `Ĉ` the expected cost including cache-hit probability for this prefix, `L̂` expected latency; `λ` set by strategy.
4. **Explore a little**: Thompson sampling on `Q̂` (bounded by budget and never for `confirm`-tier work) so new models get tried and stale beliefs get corrected — a contextual bandit, as in recent routing research (LinUCB / Thompson variants).
5. **Cascade** (strategy `balanced`/`cheapest`, when a verifier exists): run the cheapest candidate whose `Q̂` clears the bar; verify (tests pass, schema valid, self-consistency, or a `review`-role check for high-stakes outputs); escalate to the next tier on failure. Escalations are recorded as outcomes.
6. **Stickiness**: within a task, keep the same model for a conversation unless escalating, to preserve cache hits.

Every decision emits `model.called` with the candidates, scores and reason, so `priya ai why <call>` can explain in plain language: "I used Sonnet because this was a code edit; Haiku failed this kind of task 3 of the last 10 times; Opus would have cost 5× more."

## 6. Learning from outcomes

Outcome signals per call/run, attributed to (model, category, difficulty):
- verifier pass/fail (tests, boot check, schema), escalation needed or not;
- merge accepted vs discarded; later reverted with Rewind within 7 days (negative);
- explicit feedback (👍/👎 on answers, "that was wrong");
- tokens, cost, latency.

`model_stats` projection keeps Beta-distribution parameters per (model, category, difficulty bucket) plus cost and latency moving averages. Stored locally only. `priya ai stats` shows a plain-language league table per category. Reset per model on major version change (new model ids start from priors).

## 7. Two levers for agents

Agents (Claude Code, Codex, Gemini CLI, Grok Build, OpenCode…) have their own model clients. Priya AI influences them in two ways:

1. **Agent + model selection** (always works, uses the user's agent subscriptions): Priya Orchestration asks Priya AI which (agent, model) pair fits the step, then launches the agent with its own model flag (e.g. Claude Code `--model`, Codex `-m`, Gemini `-m`). This is the default for first-party agents with subscriptions.
2. **Local proxy** (any model behind any agent): priyad serves on `127.0.0.1:<port>`:
   - Anthropic Messages (`/v1/messages`) — for Claude Code via `ANTHROPIC_BASE_URL` (+ `ANTHROPIC_AUTH_TOKEN`);
   - OpenAI Responses (`/v1/responses`) — required by Codex custom providers (`wire_api = "responses"`);
   - OpenAI Chat Completions (`/v1/chat/completions`) — OpenCode, Aider, Goose and most others.
   The proxy listens on `127.0.0.1:47800` only, rejects requests with a browser `Origin` or a `Host` other than `127.0.0.1`/`localhost` (DNS-rebinding protection), requires a per-agent token, and keeps Responses-API `previous_response_id` state locally for 1 hour.
   The virtual model `priya-auto` means "Priya AI decides per request"; named models pass through. The proxy applies routing, caching, budgets, privacy rules and logging for every agent uniformly.
   Caveat recorded in [decisions.md](decisions.md) ADR-007: some vendors don't support their CLI against other vendors' models; agent quality may drop off-family. The proxy is opt-in per agent.

## 8. Token optimizer

Applied to every request Priya AI makes itself, and to proxied agent traffic where possible:

| Technique | How |
|---|---|
| Cache-aware layout | Stable prefix first (system prompt, tool schemas, project memory), volatile content last; explicit `cache_control` breakpoints for Anthropic; `prompt_cache_key` per task for OpenAI; sticky model per task |
| Tool-schema compression | Send only tools relevant to the step (retrieved from the tool index), compact JSON schemas, shared definitions |
| Tool-output trimming | Truncate/structure long command output (head + tail + error lines), summarise logs before sending |
| Context compaction | Rolling summaries for long runs (provider compaction endpoints when available, else a `fast`-role summary), drop stale tool results |
| Retrieval over stuffing | Memory/files fetched through MCP tools on demand, not pasted |
| Response cache | Exact-match cache for deterministic calls (classification, titles); semantic cache (embedding similarity ≥ threshold) for repeated questions, only for read-only answers, TTL-bound, invalidated by relevant ChangeSets |
| Output caps | Role-specific `max_tokens`, "answer briefly" style for `fast` |
| Batch | Non-urgent background jobs (overnight memory consolidation, indexing) use provider batch APIs |
| Cascades | §5 step 5 |
| Budget guard | Per call, run, task, day and month; soft limit warns, hard limit stops; shown in the top-bar item (and the bar's notice banner before the Phase 5 top bar exists) |

Provider constraints the optimizer must respect (verify against current docs when implementing): some models reject edited history that contains replayed thinking blocks, so providers declare `history_edit: AppendOnly | Editable` and compaction prefers server-side endpoints for append-only models; caches are per model, so cascades and switches trade cache hits for cheaper tokens (the scorer accounts for it); some models reject forced `tool_choice`, so the proxy downgrades it to `auto` with an instruction.

`priya ai savings` reports tokens and money saved per technique (estimated against a no-optimizer baseline).

## 9. Reliability

Per-provider circuit breakers, retries with jitter on retryable errors, fallback to the next candidate, provider health in `priya doctor`, offline mode → local models only with a notice, all timeouts explicit.

Phase 0 (`priya_ai::reliability`): connect 5 s, first token 60 s (local models may still be loading), 30 s idle between chunks, 5 minutes in all. One retry with jitter on `overloaded` or `network`, only before the first token; none once streaming has started.

Thinking: a provider's optional `reasoning_effort` setting (`minimal`, `low`, `medium`, `high`) is sent with each request as the OpenAI API's `reasoning_effort`; unset, or `default`, sends nothing (settings merge key by key, so `default` is how a user undoes a shipped value, for a server that doesn't know the field). Gemini 3 always thinks, and its thinking counts against `max_output_tokens`, so the shipped `gemini` provider sets `minimal`: quicker answers, with room left for the text. The routing engine (1.9) will set effort per task.

## 10. Multimodal

Requests carry images (Priya Vision), audio refs (voice), and files; the router filters by modality. Local vision via Ollama vision models; OCR fallback when only text models are enabled.
