# Phase 1 — Priya AI

**Goal.** Turn the walking skeleton's single-model call into the OS's own AI: one gateway over every enabled model and provider, with roles, strategies, privacy routing, automatic best-model choice within and across families, local models and offline fallback, cost tracking and budgets, a token optimizer, learning from real outcomes, a plain-language explanation of every routing decision, and a local proxy any agent can use. Still answer-only: nothing in this phase changes the machine (the sandbox arrives in Phase 2).

**Usable at the end:** in the Priya bar (or `priya ask`) the owner asks anything and Priya AI picks the best enabled model — Haiku for a quick definition, Sonnet for a code question, Opus or Fable for a hard design question, Gemini or GPT when they score better, a local Ollama model when offline or when the question touches private folders. The model chip shows what was used and why; one click switches strategy (Best / Balanced / Cheapest / Local only), family or a pinned model; "use only Claude today" works as typed. Settings → AI & Models connects providers and local endpoints; the Usage view shows spend, budgets and savings. Claude Code and OpenCode can point at the local proxy and get `priya-auto` routing.

**Depends on:** Phase 0 (event store 0.5, settings 0.6, priyad API and CLI 0.7, module host 0.8, schema checks 0.9, design tokens 0.10, PriyaUI and gallery 0.11, minimal model access and `priya ask` 0.12, minimal Priya bar 0.13, package 0.14).

**Features:** AI-01, AI-02, AI-03, AI-04, AI-05, AI-06, AI-07, AI-08, AI-09, AI-10, AI-11, AI-12, AI-13, AI-14, AI-15, AI-16, AI-17, AI-18, AI-19, DS-07, DS-08, QA-12, QA-13, QA-14.

**Read first:** [04-priya-ai](../architecture/04-priya-ai.md) (primary spec), [00-principles](../architecture/00-principles.md) (P2 ports, P3 events, P8 model-agnostic and local-first), [01 §2, §5, §6](../architecture/01-system-overview.md), [02 §1 events, §4 API, §6 settings](../architecture/02-core.md), [05 §2 `UseModel`, §3 secrets](../architecture/05-security.md), [09-testing](../architecture/09-testing.md) (suites, `covers:`/`suite:` tags, cassettes, eval and live suites), [11 §4 model picker, usage & cost, settings, §6 writing, §9 design process](../architecture/11-design-system-and-ux.md), [decisions](../architecture/decisions.md) ADR-007, ADR-012, ADR-013.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 1.1 | Unified model types and provider port | Phase 0 | `priya ask` runs through the new `ModelProvider` port (skeleton provider re-wrapped); nothing visibly new, everything still works |
| 1.2 | Anthropic and OpenAI adapters | 1.1 | `priya ask --model anthropic/claude-sonnet-5-5 …` or `--model openai/<id>` streams answers, with tools, images and cache usage reported |
| 1.3 | Google and xAI adapters | 1.1 | `--model google/<gemini-id>` and `--model xai/<grok-id>` work the same way |
| 1.4 | OpenAI/Anthropic-compatible adapter and local discovery | 1.1 | Ollama / LM Studio / llama.cpp / vLLM found automatically; OpenRouter and any endpoint added with one command |
| 1.5 | Key storage and "Connect a provider" flow | 1.1 | Keys go into the Secret Service through a dialog or `priya ai connect`; never in files, logs or events |
| 1.6 | Model catalog, priors, discovery | 1.2, 1.3, 1.4 | `priya ai models` lists every model with price, context, local badge; new models announced in plain language |
| 1.7 | Roles, strategies, control | 1.6 | `priya ai use cheapest`, `… family claude --for today`, `… model <id>`; privacy rules keep chosen folders on local models |
| 1.8 | Classifier | 1.7 | Each question gets a category, difficulty, modality and privacy class; "use only Claude today" typed in the bar becomes a setting change |
| 1.9 | Router | 1.8 | Questions are routed to the best model automatically, within and across families; `priya ai why last` explains the choice |
| 1.10 | Reliability and offline mode | 1.9 | A failing provider is skipped silently; pulling the network switches to local models with a notice |
| 1.11 | Costs and budgets | 1.9 | `priya ai costs` shows spend; daily/monthly budgets warn and stop cloud spend |
| 1.12 | Token optimizer I (cache layout, provider caching, trimming, caps) | 1.9 | Follow-up questions hit provider caches; long pasted logs are trimmed; cache savings visible in `priya ai costs --detail` |
| 1.13 | Token optimizer II (compaction, response cache, savings) | 1.11, 1.12 | Long conversations stay cheap; repeated questions answer instantly from cache; `priya ai savings` reports savings per technique |
| 1.14 | Learning from outcomes | 1.9 | 👍/👎 and verifier results update per-model statistics; `priya ai stats` shows the league table |
| 1.15 | Local proxy for agents | 1.5, 1.9, 1.10, 1.11 | Claude Code (via `ANTHROPIC_BASE_URL`), Codex and OpenCode run against `127.0.0.1` with `priya-auto` |
| 1.16 | AI & Models settings, model picker, usage view | 1.5, 1.7, 1.9, 1.11, 1.13, 1.14 | Model chip and picker in the bar, "Why this model?", Settings → AI & Models, Usage & cost view |
| 1.17 | Router evaluation suite | 1.9, 1.14 | ~300 labelled prompts give accuracy and simulated cost numbers every night; live smoke and mutation jobs run |
| 1.18 | Phase gate | all | Owner demo of the whole phase; baseline stored |

---

### 1.1 — Unified model types and provider port
**Outcome:** every model call in priyad goes through one `ModelProvider` port with one request/stream/error vocabulary; the Phase 0 skeleton client (AI-21) is re-implemented as adapters behind it and `priya ask` plus the bar keep working unchanged.
**Features:** AI-01 (infrastructure for all adapters).
**Depends on:** Phase 0.

**Tasks**
- [ ] `priya-ai::types`: `ModelRequest { messages, system, tools, tool_choice, response_format, max_tokens, effort, stop, cache_hints, metadata }`, `Message { role, content: Vec<ContentBlock> }`, `ContentBlock::{Text, Image { source: ImageSource::{Base64, Blob(BlobRef), Url}, media_type }, AudioRef, ToolUse { id, name, input }, ToolResult { tool_use_id, content, is_error }, Reasoning { opaque, provider_tag }}`, `ToolSpec { name, description, input_schema, strict }`, `ResponseFormat::{Text, JsonSchema}`, `Effort::{Low, Medium, High, Max}`, `CacheHint::Breakpoint { ttl: CacheTtl::{Short, Long} }`. All `serde` + `schemars`, in `priya-api` where they cross the wire.
- [ ] Streaming vocabulary: `ModelStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, ModelError>> + Send>>`; `StreamEvent::{Start { model, request_id }, TextDelta, ReasoningDelta, ToolCallStart { id, name }, ToolCallDelta { id, json_fragment }, ToolCallEnd { id }, Usage(Usage), Stop { reason: StopReason }}`; `StopReason::{EndTurn, MaxTokens, ToolUse, StopSequence, Refusal { category }, ContentFilter, Other(String)}`; a `collect()` helper that assembles a final `ModelResponse` and validates tool-call JSON.
- [ ] `Usage { input, output, cache_read, cache_write_short, cache_write_long, reasoning }` and `Money` as integer micro-USD (`MicroUsd(i64)`), never floats in storage.
- [ ] `ModelError` taxonomy with `retryable()`, `retry_after()`, `user_message()`: `Auth`, `PermissionDenied`, `QuotaExhausted`, `ModelNotFound`, `InvalidRequest`, `ContextTooLong { limit }`, `ContentFiltered`, `RateLimited { retry_after }`, `Overloaded`, `Timeout { phase }`, `Network`, `Server { status }`, `StreamInterrupted { after_tokens }`, `Unsupported { feature }`, `Cancelled`. Maps into the JSON-RPC error `data.kind` from [02 §4](../architecture/02-core.md).
- [ ] The `ModelProvider` trait from [04 §2](../architecture/04-priya-ai.md) plus three default-implemented methods this phase needs: `embed(&self, EmbedRequest) -> Result<Vec<Embedding>>` (semantic cache, 1.13), `count_tokens(&self, &ModelRequest) -> Result<TokenCount>` (provider endpoint or local estimate), and cancellation through a `CancellationToken` in `complete`. Update [04 §2](../architecture/04-priya-ai.md) in the same PR.
- [ ] `ProviderCaps { caching: CachingKind::{None, Explicit { ttls, max_breakpoints, min_prefix_tokens }, Automatic { key_param }, Implicit}, batch, server_compaction, count_tokens, embeddings, modalities_in, tools, parallel_tools, strict_tools, forced_tool_choice, json_schema, reasoning, history_edit: HistoryEdit::{Free, AppendOnly} }` — `history_edit` records that some current models reject edited history with replayed reasoning blocks, which the optimizer must respect (1.12, 1.13).
- [ ] Shared HTTP layer `priya-ai::http`: `HttpTransport` trait (real `reqwest` with rustls; `CassetteTransport` for tests), SSE parser (`sse.rs`: `event:`/`data:`/`id:`/comments, multi-line data, `[DONE]`), explicit connect/first-byte/idle/total timeouts per call, user agent `PriyaOS/<version>`.
- [ ] `priya-testkit::cassette`: record/replay of HTTP exchanges including chunk timing; request matcher on method + path + canonical JSON body hash; scrubbers for `authorization`, `x-api-key`, `x-goog-api-key`, `api-key` query params and key-shaped strings; `PRIYA_RECORD=1` records; every fixture dir gets `RECORDED.md`; pre-commit secret scan covers cassettes.
- [ ] `FakeProvider` in `priya-testkit` (scripted streams, scripted errors, latency injection, usage, deterministic per seed) — used by every later sub-phase.
- [ ] Re-home the Phase 0 skeleton (AI-21): its cloud and local clients become thin adapters behind the port; `v1.ai.ask` and `priya ask` call `Gateway::ask(role, request)` (a pass-through until the router exists).

**Tests**
- Unit: SSE parser edge cases (split chunks, CRLF, comments, empty data, `[DONE]`), `collect()` reassembly of interleaved tool-call deltas, invalid tool JSON → `InvalidRequest`, `MicroUsd` arithmetic overflow guards, error classification table (`retryable()` for every variant).
- Contract: the **provider contract suite** skeleton (`priya-ai/tests/contract/provider_contract.rs`, macro `provider_contract!(adapter, cassettes_dir)`) with scenario names fixed now: `text_stream_basic`, `system_prompt`, `multi_turn`, `tool_call_single`, `tool_call_parallel`, `tool_result_roundtrip`, `image_input`, `json_schema_output`, `max_tokens_stop`, `usage_reported`, `cache_usage_reported`, `reasoning_effort`, `list_models`, `count_tokens`, `embed`, `error_auth`, `error_model_not_found`, `error_context_too_long`, `error_rate_limited`, `error_overloaded`, `error_quota`, `stream_error_mid_stream`, `stream_connection_dropped`, `refusal_or_filter`, `cancel_mid_stream`. Adapters skip scenarios their `ProviderCaps` exclude and must then return `Unsupported`. `FakeProvider` passes the whole suite.
- Integration: priyad with `FakeProvider` — `v1.ai.ask` streams to a subscriber in order; cancel from the client stops the upstream stream within 100 ms.
- Regression: all Phase 0 `priya ask` / bar acceptance tests stay green unchanged; new: `fake_provider_passes_contract`, `ask_cancel_stops_upstream`.

**Done when**
- [ ] `cargo test -p priya-ai` green on Mac and Linux; `priya-ai` diff coverage ≥ 90 %.
- [ ] Phase 0 walking-skeleton journeys pass with no behaviour change.
- [ ] Schema snapshots for the new `priya-api` types committed under `docs/claude/api/`.

---

### 1.2 — Anthropic and OpenAI adapters
**Outcome:** Claude models (Messages API) and OpenAI models (Responses API, with Chat Completions as a second wire format) stream answers with tools, images, structured output, effort and cache accounting.
**Features:** AI-01.
**Depends on:** 1.1.

The first Anthropic adapter was planned for 0.12; it moved here on 2026-10-20, since Phase 0 tests with free tiers through the OpenAI-compatible adapter.

**Tasks**
- [ ] `providers::anthropic` wire layer: `POST /v1/messages` with `anthropic-version` header and `stream: true`; map SSE `message_start`, `content_block_start/delta/stop` (`text_delta`, `input_json_delta`, `thinking_delta`), `message_delta` (usage, `stop_reason`, `stop_details`), `message_stop`, `ping`, `error` to `StreamEvent`; images as base64 `image` blocks; `tools` + `input_schema` + `strict`; `output_config.format` for JSON schema; `output_config.effort` from `Effort`; adaptive thinking where the model requires it, reasoning blocks preserved opaquely and replayed only to the same model.
- [ ] Anthropic caching: translate `CacheHint::Breakpoint` into `cache_control: {type: "ephemeral", ttl}` (5 min / 1 h), enforce max 4 breakpoints, skip breakpoints below the model's minimum cacheable prefix; read `cache_creation_input_tokens` / `cache_read_input_tokens` into `Usage`.
- [ ] Anthropic model list and counting: `GET /v1/models` with `after_id` pagination → `DiscoveredModel { id, display_name, created_at, max_input_tokens, max_output_tokens, capabilities }`; `POST /v1/messages/count_tokens` for `count_tokens`.
- [ ] Anthropic error mapping: `invalid_request_error` (incl. prompt-too-long → `ContextTooLong`), `authentication_error`, `permission_error`, `not_found_error`, `request_too_large`, `rate_limit_error` (+ `retry-after`, `anthropic-ratelimit-*` headers parsed into `RateLimitState`), `api_error`, `overloaded_error` (529); `stop_reason: "refusal"` → `StopReason::Refusal { category }`; 400 on forced `tool_choice` for models whose caps say `forced_tool_choice = false` is prevented before sending.
- [ ] `providers::openai` Responses wire layer: `POST /v1/responses` with `stream: true`, `store: false`, `input` items (messages, `function_call`, `function_call_output`, `input_image`), `reasoning.effort`, `max_output_tokens`, `text.format` json_schema, `prompt_cache_key`; SSE `response.created`, `response.output_item.added/done`, `response.output_text.delta`, `response.function_call_arguments.delta/done`, `response.reasoning_summary_text.delta`, `response.completed` (usage incl. `input_tokens_details.cached_tokens`, `output_tokens_details.reasoning_tokens`), `response.failed`, `error`.
- [ ] `providers::openai` Chat Completions wire layer (`POST /v1/chat/completions`, `stream_options.include_usage`, `tools`/`tool_calls` deltas, `response_format`) — selected per model by catalog field `wire = "responses" | "chat"`; shared by 1.3 (xAI) and 1.4 (compatible).
- [ ] OpenAI error mapping: 401, 403, 404 `model_not_found`, 400 `context_length_exceeded`, 429 split into `RateLimited` (with `retry-after`, `x-ratelimit-remaining-requests/tokens`, `x-ratelimit-reset-*`) vs `insufficient_quota` → `QuotaExhausted` (not retryable), 500/502/503 → `Server`.
- [ ] OpenAI model list: `GET /v1/models` (ids only — limits and prices come from the shipped catalog and OpenRouter, 1.6); embeddings via `POST /v1/embeddings`.
- [ ] `ProviderCaps` for both adapters; `price()` reads the catalog entry (cache read/write multipliers, reasoning billed as output, long-context price tiers via `price.tiers`).
- [ ] Record cassettes for every contract scenario with cheap models (`max_tokens` ≤ 32) and document them in `crates/priya-ai/tests/fixtures/anthropic/RECORDED.md` and `…/openai/RECORDED.md`.
- [ ] `priya ask --model <provider/model>` flag (bypasses routing until 1.9; still logs `model.called`).

**Tests**
- Unit: request builders (golden JSON with `insta`) for each `ModelRequest` shape; breakpoint placement and the 4-breakpoint cap; reasoning blocks dropped when the target model differs; Responses ↔ Chat item conversion round-trip; header parsing into `RateLimitState`.
- Contract: full provider contract suite for `anthropic`, `openai` (responses) and `openai` (chat) against recorded cassettes, including `error_rate_limited` with `retry-after`, `error_quota` (OpenAI only), `refusal_or_filter`, `stream_connection_dropped` (cassette truncated mid-stream → `StreamInterrupted`).
- Live (`PRIYA_LIVE=1`, nightly): one 16-token streamed call, one tool call and one `list_models` per provider; per-run hard budget $0.05 per provider.
- Regression: `anthropic_contract`, `openai_responses_contract`, `openai_chat_contract`, `anthropic_cache_breakpoints_golden`.

**Done when**
- [ ] Both adapters pass every applicable contract scenario from cassettes on Mac and Linux CI.
- [ ] `priya ask --model anthropic/claude-haiku-5-5 "hi"` and an OpenAI equivalent stream on the Zenbook; `Usage` matches the provider-reported numbers exactly.
- [ ] Cassettes contain no secrets (pre-commit scan and a CI grep for key prefixes).

---

### 1.3 — Google and xAI adapters
**Outcome:** Gemini and Grok models work through the same port, so cross-family routing has four cloud families.
**Features:** AI-01.
**Depends on:** 1.1.

**Tasks**
- [ ] `providers::google`: `POST /v1beta/models/{model}:streamGenerateContent?alt=sse` with `x-goog-api-key`; `contents[].parts` (`text`, `inlineData`, `functionCall`, `functionResponse`), `systemInstruction`, `tools.functionDeclarations`, `toolConfig`, `generationConfig { maxOutputTokens, responseMimeType, responseSchema, thinkingConfig }`; effort → thinking budget/level per model from the catalog.
- [ ] Google usage and caching: `usageMetadata { promptTokenCount, candidatesTokenCount, cachedContentTokenCount, thoughtsTokenCount }`; implicit caching only (`CachingKind::Implicit`) — explicit `cachedContents` is out of scope.
- [ ] Google model list and extras: `GET /v1beta/models` (paginated, `inputTokenLimit`, `outputTokenLimit`, `supportedGenerationMethods`) → `DiscoveredModel`; `countTokens`; `embedContent` for `embed`.
- [ ] Google error mapping: `INVALID_ARGUMENT` (400; token-limit messages → `ContextTooLong`), `PERMISSION_DENIED`/`UNAUTHENTICATED`, `NOT_FOUND`, `RESOURCE_EXHAUSTED` (429, `RetryInfo` delay), `UNAVAILABLE`/`INTERNAL`, `DEADLINE_EXCEEDED`; `finishReason` `SAFETY`/`RECITATION`/`PROHIBITED_CONTENT` → `ContentFilter`; `MAX_TOKENS` → `MaxTokens`.
- [ ] `providers::xai`: reuse the OpenAI wire layers with base URL `https://api.x.ai/v1`; model metadata (prices, modalities, context) from `GET /v1/language-models`; conversation-affinity header for cache hits if the recorded API confirms it; `server_compaction` capability set only if confirmed by recording (do not assume [04 §2](../architecture/04-priya-ai.md) — fix the doc if it differs).
- [ ] Record cassettes and `RECORDED.md` for both; xAI and Google specifics added to the contract suite's per-adapter skip lists.
- [ ] Image input normalisation shared by all adapters: max dimension and size per provider from caps, downscale with `image` crate, convert HEIC/WebP when unsupported.

**Tests**
- Unit: Gemini request golden files (function declarations from JSON Schema, removal of unsupported schema keywords such as `$defs` via inlining), `finishReason` mapping table, `RetryInfo` parsing; image downscaling keeps aspect ratio.
- Contract: provider contract suite for `google` and `xai` from cassettes.
- Live: 16-token streamed call, `list_models`, one image call (vision-capable model) per provider; budget $0.05 each.
- Regression: `google_contract`, `xai_contract`, `image_normalisation_limits`.

**Done when**
- [ ] Both adapters pass all applicable contract scenarios.
- [ ] `priya ask --model google/<gemini-flash-id> "hi"` and `--model xai/<grok-id> "hi"` stream on the Zenbook.

---

### 1.4 — OpenAI/Anthropic-compatible adapter and local discovery
**Outcome:** local servers (Ollama, LM Studio, llama.cpp server, vLLM) are found automatically and their models used like any other; OpenRouter or any compatible endpoint is added with one command or one settings entry.
**Features:** AI-01, AI-03, AI-09.
**Depends on:** 1.1.

**Tasks**
- [ ] `providers::openai_compatible`: Chat Completions (and Responses where the server supports it) against a configured `base_url`; optional bearer key; per-server **quirk profile** (`ollama`, `lmstudio`, `llamacpp`, `vllm`, `openrouter`, `generic`) declaring support for `stream_options.include_usage`, tool calls, `response_format`, image input, parallel tools; unknown servers probed once and the result cached.
- [ ] `providers::anthropic_compatible`: the Anthropic wire layer against `base_url` without `x-api-key` when none is configured; used when a local server exposes `/v1/messages` (verified by probe, not assumed) so local models can serve Anthropic-format callers (1.15).
- [ ] Local model listing per profile: Ollama `GET /api/tags` + `POST /api/show` (context length, capabilities: tools, vision, embedding), `GET /api/ps` (loaded models); LM Studio `GET /api/v0/models` (loaded/not-loaded, `max_context_length`) falling back to `/v1/models`; llama.cpp `GET /v1/models` + `GET /props` (`n_ctx`) + `GET /health`; vLLM `GET /v1/models` (`max_model_len`).
- [ ] OpenRouter profile: base URL `https://openrouter.ai/api/v1`, `GET /models` (pricing per token, `context_length`, input modalities, supported parameters), `usage: {include: true}` so the response carries actual cost; attribution headers `HTTP-Referer`/`X-Title` set to PriyaOS.
- [ ] `LocalDiscovery` job: probe `127.0.0.1:11434`, `:1234`, `:8080`, `:8000` (300 ms timeout) at priyad start, every 60 s while any local endpoint is configured, and on NetworkManager connectivity change; found endpoints become `ai.endpoint.discovered` events and catalog entries with `local = true`, `privacy = "local"`, price 0.
- [ ] One-entry endpoints (AI-03): `[[ai.endpoints]] name, kind = "openai_compatible" | "anthropic_compatible", base_url, key_ref?, profile?, trusted_local = false` in `~/.config/priya/models.toml`; `priya ai add-endpoint <name> <url> [--profile …] [--key]` writes it through `v1.ai.endpoint.add` (a `setting.changed` event). Non-loopback endpoints (e.g. LM Studio at `192.168.1.20:1234`) default to `privacy = "cloud"` until the user marks them as their own machine (`trusted_local = true`).
- [ ] Cold-start handling for local models: longer first-byte timeout (60 s default) and a `ModelLoading` status from Ollama/LM Studio load state so the bar can say "Loading Qwen3 Coder… (first use)".
- [ ] Embeddings for local models: Ollama `POST /api/embed`, OpenAI-compatible `/v1/embeddings`.
- [ ] Record cassettes against real Ollama, LM Studio and llama.cpp servers on the Zenbook (small models, e.g. a 0.5–1.5 B instruct model) and an OpenRouter account; `RECORDED.md` lists server versions.

**Tests**
- Unit: quirk-profile selection; probe result caching; endpoint TOML validation (bad URL, duplicate name, `trusted_local` only meaningful for non-loopback); privacy default for LAN addresses.
- Contract: provider contract suite for `openai_compatible` with each profile's cassettes and for `anthropic_compatible` against Ollama's `/v1/messages` cassette.
- Integration: priyad with a mock Ollama HTTP server (wiremock) appearing and disappearing → `ai.endpoint.discovered` / health change events, catalog updates within 60 s; `priya ai add-endpoint` → model usable without restart.
- Live: if Ollama is running on the runner, list models and one 8-token call (cost 0); OpenRouter one 16-token call (budget $0.02).
- Perf: discovery probe of four closed ports completes < 400 ms and never blocks a request.
- Regression: `compat_contract_ollama`, `compat_contract_lmstudio`, `compat_contract_llamacpp`, `anthropic_compat_contract_ollama`, `add_endpoint_one_entry`, `lan_endpoint_defaults_cloud_privacy`.

**Done when**
- [ ] With Ollama running and nothing configured, `priya ask --model ollama/<model> "hi"` works (zero settings entries).
- [ ] LM Studio on another machine works after exactly one `priya ai add-endpoint` command.
- [ ] OpenRouter models are callable and report actual cost.

---

### 1.5 — Key storage and "Connect a provider" flow
**Outcome:** the owner connects Anthropic, OpenAI, Google, xAI or OpenRouter by pasting a key into a dialog (or `priya ai connect anthropic`); the key is verified, stored in the Secret Service, and never appears in settings files, events, logs or prompts.
**Features:** AI-17.
**Depends on:** 1.1.

**Tasks**
- [ ] `SecretStore` port in `priya-core` (`get`, `put`, `delete`, `list(kind)`) with the Secret Service adapter (libsecret D-Bus via the `oo7` or `secret-service` crate), collection `priya`, item attributes `{priya-kind = "provider-key", provider = "<id>"}`; in-memory adapter for tests; plain-language error when no Secret Service is running ("Install gnome-keyring or KeePassXC with Secret Service enabled").
- [ ] `KeyRef` type used by catalog/settings (`key_ref = "secret:provider/anthropic"`); adapters resolve keys at call time and hold them only in `secrecy::SecretString`.
- [ ] Key verification: `ModelProvider::list_models` (no token cost) with a 10 s timeout; outcomes `valid`, `invalid key`, `no permission`, `quota exhausted`, `unreachable`.
- [ ] API: `v1.ai.provider.list` (status per provider: connected / needs key / invalid / unreachable / offline), `v1.ai.provider.connect {provider, key?, base_url?}`, `v1.ai.provider.test`, `v1.ai.provider.disconnect`. Keys travel only over the 0600 priyad socket.
- [ ] CLI: `priya ai connect <provider>` reads the key from a no-echo TTY prompt or stdin (never argv); `priya ai keys list` shows provider, fingerprint (last 4 chars + BLAKE3 prefix) and added date; `priya ai disconnect <provider>`.
- [ ] Environment import: detect `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`/`GOOGLE_API_KEY`, `XAI_API_KEY`, `OPENROUTER_API_KEY` in priyad's environment and offer import (never used silently once the store exists); migrate any Phase 0 skeleton key location into the store and delete the old copy.
- [ ] Events: `ai.provider.connected {provider, key_fingerprint}`, `ai.provider.disconnected {provider}` — fingerprints only.
- [ ] Redaction layer for `tracing` and error messages: known key formats (`sk-ant-…`, `sk-…`, `AIza…`, `xai-…`, `sk-or-…`, bearer tokens) replaced with `«redacted»` before any log or event write (full log redaction for runs remains SEC-04, Phase 3).
- [ ] `ConnectProviderDialog` (QML, Quickshell `FloatingWindow`) opened from the bar ("Connect a provider…" when no cloud provider is connected) and later embedded in Settings → AI & Models (1.16).

**UI**
- `ConnectProviderDialog`: provider list (Anthropic, OpenAI, Google, xAI, OpenRouter, "Other compatible endpoint"), key field (password-style, paste button, "Where do I get a key?" link text), privacy line ("Your questions go to Anthropic only when Priya picks a Claude model"), Connect button. States: empty, typing, verifying (loading), connected (success with model count), invalid key, no permission, quota exhausted, unreachable, offline, no Secret Service, long provider error text, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: Tab order provider → key → Connect; Enter connects; Esc closes. Screen-reader names: "Provider: Anthropic", "API key, hidden", "Connect", status announced in a live region.
- 👤 Mockup approval (`ui/mockups/1.5/`) before building; gallery sign-off after.

**Human in the loop:** the user decides which providers to connect and sees, before connecting, where their questions will go. Disconnecting removes the key from the Secret Service immediately; routing stops using that provider on the next call. Nothing is connected implicitly.

**Tests**
- Unit: fingerprint function; redaction regexes (positive and negative corpus, incl. keys split across log fields); `KeyRef` parsing.
- Contract: `SecretStore` port suite against the in-memory adapter and the Secret Service adapter (Linux CI with `gnome-keyring-daemon --unlock` in a D-Bus session).
- Integration: connect with `FakeProvider` (valid/invalid scripts) → events carry fingerprint only; `grep` of `priya.db`, settings files and logs after connect finds no key bytes (`secret_never_persisted_outside_store`).
- Security: key never appears in `model.called`, `setting.changed`, tracing output, crash reports or JSON-RPC error `data` (fuzzed error paths).
- UI: dialog component tests; screenshots for every state × theme × scale; a11y lint; journey "open bar with no provider → Connect → paste → connected → ask".
- Regression: `connect_provider_stores_in_secret_service`, `secret_never_persisted_outside_store`, `connect_dialog_states`, `cli_connect_reads_stdin_not_argv`.

**Done when**
- [ ] Owner connects at least Anthropic plus one other provider on the Zenbook via the dialog; `priya ai keys list` shows fingerprints only.
- [ ] The security test proves no key bytes in db, settings, logs or events.
- [ ] 👤 Dialog signed off in the gallery.

---

### 1.6 — Model catalog, priors, discovery
**Outcome:** one catalog knows every enabled and available model — limits, modalities, prices, local or cloud, prior skill scores — kept fresh automatically; new models are announced in plain language.
**Features:** AI-02, AI-03, AI-23.
**Depends on:** 1.2, 1.3, 1.4.

**Tasks**
- [ ] `priya-ai::catalog`: load `/usr/share/priya/models/*.toml` (shipped), `~/.config/priya/models.toml` (user overrides + endpoints) and discovered data; schema per [04 §3](../architecture/04-priya-ai.md) plus `wire`, `aliases`, `successor_of`, `deprecated_after`, `price.as_of`, `price.tiers`, `caps_override`. Merge precedence per field: user > shipped curated fields (`skills`, `tier`, `family`) > discovered facts (`context`, `max_output`, modalities, prices) > shipped facts; conflicts logged.
- [ ] `data/model-priors.toml` (versioned, `priors_version`): per-model skill scores for the categories `code_edit, code_plan, analysis, summarize, chat, vision, classify, extract, sysadmin`; per-tier defaults for models without an entry; per-tier difficulty adjustment table (`[difficulty_adjust] small = [1.0, 0.95, 0.85, 0.70, 0.55]`, etc.); sources cited per row. Shipped catalog files generated from it by `cargo xtask catalog`.
- [ ] `model_catalog` projection fed by `ai.model.discovered`, `ai.model.changed {fields}` (price/limit change), `ai.model.retired` events; rebuildable from the log.
- [ ] Discovery job (daily timer inside priyad, on provider connect, and `priya ai models --refresh`): each connected provider's `list_models` + OpenRouter's public `/models` for prices and limits of models whose provider doesn't publish them (setting `ai.discovery.price_source_openrouter = true`; no user data is sent). Price changes > 10 % produce a notice.
- [ ] New-model policy (setting `ai.discovery.auto_enable`): `"successors"` (default) — a newer model with `successor_of` (or same family + tier + newer `created_at`) of an enabled model is enabled with its predecessor's priors; `"none"`; `"all"`. Everything else is listed as *available*.
- [ ] Notices: `ai.notice` events (`new_model`, `price_changed`, `model_retiring`, `provider_problem`) with plain text such as "New model available: Claude Haiku 5.5 — similar to Haiku 4.5, about 90 % cheaper. Enabled for quick questions." and actions `enable`, `ignore`, `pin`; `priya ai notices [--dismiss <id>]`; `v1.ai.notices`.
- [ ] `NoticeBanner` PriyaUI component in the bar (one line, action buttons, dismiss), reused by 1.10 and 1.11.
- [ ] CLI/API: `priya ai models [--enabled|--available|--local] [--json]` (id, family, tier, context, price in/out per Mtok, local badge, health) via `v1.ai.models`; `priya ai models enable|disable <id>` (`setting.changed`).
- [ ] Catalog validation in CI (`cargo xtask catalog --check`): every shipped model has family, tier, context, price or `local = true`, skills or tier default; aliases unique; no dangling `successor_of`.
- [ ] Deprecation tracking: when a provider marks a model deprecated or a successor appears, notify in plain language and offer to move pins and project settings to the successor (one confirmation)

**UI**
- `NoticeBanner` in the bar: states new model, price changed, model retiring, long model names, two actions, dismissed, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: banner reachable with Tab; Enter triggers the primary action; Esc dismisses. Screen-reader names: "Notice: new model available, Claude Haiku 5.5", buttons "Enable", "Ignore".
- 👤 Mockup approval (`ui/mockups/1.6/`) before building; gallery sign-off after.

**Human in the loop:** new models never replace a pinned model; the user sees each new or retiring model once and can enable, ignore or pin it. Auto-enable can be switched off.

**Tests**
- Unit: merge precedence table (each field from each source), alias resolution, successor inheritance of priors, difficulty adjustment lookup, price-change threshold.
- Contract: discovery parsers against recorded `list_models` cassettes from 1.2–1.4 and an OpenRouter `/models` cassette.
- Integration: discovery job with `FakeProvider` returning a new successor model → `ai.model.discovered` + notice + model enabled; projection rebuild equals incremental state.
- UI: `NoticeBanner` screenshots for every state × theme × scale; a11y lint.
- Regression: `catalog_merge_precedence`, `discovery_announces_successor`, `catalog_projection_rebuild`, `shipped_catalog_valid`.

**Done when**
- [ ] `priya ai models` on the Zenbook lists all connected providers' models with prices and context sizes, plus local models.
- [ ] A recorded "new model appears" scenario produces exactly one plain-language notice.
- [ ] `cargo xtask catalog --check` is a required CI check.

---

### 1.7 — Roles, strategies, control
**Outcome:** callers ask for roles, never model names; the owner controls routing with strategies, family filters, pins and privacy rules — globally, per project, per conversation or "for today" — by CLI, API and settings.
**Features:** AI-04, AI-07, AI-08, AI-10.
**Depends on:** 1.6.

**Tasks**
- [ ] `Role` enum (`fast`, `smart`, `coding`, `review`, `vision`, `embed`, `private`, `transcribe`) with `RoleProfile` defaults: output cap, effort, latency class (interactive/background), required modalities, privacy default (`private` = local only), `review` family-diversity rule. Profiles overridable under `[ai.roles.<role>]`.
- [ ] Strategy model: a **base strategy** (`best`, `balanced` (default), `cheapest`, `fixed:<model>`) composed with **filters** (`local_only`, `family:<name>`, `providers = [...]`), so "only Claude, cheapest" is expressible; `local_only` and `family:<name>` keep their names from [04 §4](../architecture/04-priya-ai.md) and mean "balanced + filter" when used alone. Per-strategy parameters in settings: `λ_cost`, `λ_lat`, `q_min`.
- [ ] `StrategyResolver`: layers shipped default → `/etc` → user → project `.priya.toml [ai]` → conversation override → call override; overrides carry `scope` and optional `expires_at` ("for today" = local midnight). Result `EffectiveStrategy` records which layer set each part (used by explanations).
- [ ] Privacy classes: request `PrivacyClass::{Normal, Private}` and catalog `privacy = local | cloud`; `[ai.privacy] rules = [{ paths = ["~/Documents/**"], class = "private" }, { providers_denied = ["xai"] }]`; `private` role and `Private` class → local only unless a rule allows a named provider.
- [ ] `ModelUseGate` trait (`check(principal, request_facts, model) -> Allow | Deny { rule, user_message }`) with a settings-backed implementation now; Phase 3.4 adds the Cedar `UseModel` implementation behind the same trait and the same tests. Each deny emits `policy.decision {action: "UseModel", source: "ai.privacy"}`.
- [ ] API and CLI: `v1.ai.use {strategy?, family?, model?, local_only?, scope: conversation|today|project|global, reset?}` → validated settings patch → `setting.changed`; `priya ai use balanced|best|cheapest|local|family <name>|model <id> [--for today|--here|--always]`, `priya ai use --reset`, `priya ai status` (effective strategy and where it came from). Update [02 §4](../architecture/02-core.md) `v1.ai.*` list in the same PR.
- [ ] `fixed:<model>` unavailable (disabled, unhealthy, privacy-denied) → plain error with the reason and a one-step alternative ("Opus is unreachable. Use Balanced for now?"), never a silent switch.

**Tests**
- Unit: strategy composition table (every base × filter), resolver layering with expiry at midnight (fake clock, timezone change), privacy rule globbing (`~` expansion, symlinks resolved), `review` family-diversity rule, `fixed:` failure messages.
- Integration: `priya ai use family claude --for today` → `setting.changed` with `expires_at`; next day (fake clock) the override no longer applies and a `setting.changed` revert event is written; project `.priya.toml` applies only to calls with that cwd.
- Security: a request whose attachments are under a private path is denied for every cloud model in the catalog (property test over random catalogs) — `private_paths_never_reach_cloud`.
- Regression: `strategy_composition_table`, `use_for_today_expires`, `private_paths_never_reach_cloud`, `fixed_model_unavailable_explains`.

**Done when**
- [ ] Every strategy and filter from [04 §4](../architecture/04-priya-ai.md) can be set by CLI and API, and `priya ai status` explains where the effective strategy came from.
- [ ] With a rule for `~/Documents/**`, a question about a file there is never sent to a cloud model (asserted in integration with request capture).

---

### 1.8 — Classifier
**Outcome:** every request is classified (category, difficulty 1–5, modalities, size, privacy class, latency need) cheaply and fast; short AI-control phrases typed in the bar ("use only Claude today", "cheapest mode", "use Opus for this") become typed `v1.ai.use` calls with a preview.
**Features:** AI-04, AI-05, AI-06, AI-10.
**Depends on:** 1.7.

**Tasks**
- [ ] `Classification { category, difficulty: 1..=5, modalities, est_input_tokens, est_output_tokens, privacy: PrivacyClass, latency: Interactive|Background, stakes: Normal|High, ai_control: Option<AiControl>, confidence, source: Caller|Heuristic|Model }`; callers may pass `RouteHints` that skip classification fields they already know (modules, proxy).
- [ ] `HeuristicClassifier` (pure, `priya-ai::classify::heuristics`): attachments → modalities; token estimate (provider-free estimator ±15 %); code signals (fences, file extensions, stack traces, diff markers) → `code_*`; log/error signals → `sysadmin`/`analysis`; length and question shape → `chat`/`summarize`/`extract`; explicit cues ("briefly", "think carefully", "step by step") → difficulty; Devanagari script and Hinglish markers → language tag (affects candidate skills later); attachment paths → privacy class via 1.7 rules.
- [ ] AI-control grammar: a small deterministic parser for strategy/family/model/scope phrases in English and Hinglish ("sirf Claude use karo aaj") → `AiControl` with confidence; ambiguous → falls to the model classifier.
- [ ] `ModelClassifier`: when heuristic confidence < `ai.classifier.model_threshold` (0.75), call role `fast` with a fixed system prompt (≤ 600 tokens, cache-friendly) and JSON-schema output; **recursion guard**: the call carries `RouteHints { category: classify, difficulty: 1, skip_classify: true }`, prefers local, and is exact-cached (1.13); timeout 800 ms → keep the heuristic result with difficulty raised by 1 (err on the stronger side).
- [ ] Module intent examples ([02 §5.2](../architecture/02-core.md)) included in the classifier prompt as a compact list so later module routing reuses this classifier.
- [ ] `intent.classified` event payload extended with the classification (no prompt text; prompt hash only).
- [ ] Bar handling of `ai_control`: shows a one-line preview card "Use only Claude models until tonight? · Do it · Cancel" → `v1.ai.use`; the setting change has an Undo toast.

**Tests**
- Unit: heuristic table tests (≥ 150 cases incl. Hindi/Hinglish), AI-control grammar (positive/negative/ambiguous), token estimator error bound on a corpus, recursion guard (classifier call never classified), timeout fallback raises difficulty.
- Integration: classifier with `FakeProvider` scripted JSON (valid, invalid JSON, timeout) → correct fallback; `intent.classified` contains no prompt text.
- Eval (nightly, from 1.17): category accuracy ≥ 85 %, difficulty within ±1 ≥ 90 %, AI-control intent precision ≥ 98 % (a false positive changes settings).
- Perf: heuristic path p95 < 2 ms; model path p95 < 700 ms with a local small model on the Zenbook.
- Regression: `heuristic_classifier_table`, `ai_control_grammar`, `classifier_recursion_guard`, `classifier_timeout_fallback`.

**Done when**
- [ ] Typing "use only Claude today" in the bar shows the preview, and confirming changes the effective strategy until midnight.
- [ ] Eval thresholds above met on the labelled set.

---

### 1.9 — Router
**Outcome:** each request goes to the best enabled model for the job — within one family (Haiku / Sonnet / Opus / Fable) and across families — under the effective strategy; cheap-first cascades with verification where a verifier exists; every decision is recorded and explained in plain language.
**Features:** AI-05, AI-06, AI-13, AI-15.
**Depends on:** 1.8.

**Tasks**
- [ ] `Router::route(&RouteRequest) -> RouteDecision { chosen, cascade: Vec<ModelId>, candidates: Vec<CandidateScore>, explored: bool, reason: ReasonCode, strategy: EffectiveStrategy }` in `priya-ai::router` (pure, builds on macOS).
- [ ] **Filter** stage with explicit `FilterReason`: `Disabled`, `Unhealthy`, `ModalityMissing`, `ContextTooSmall`, `ToolsUnsupported`, `PrivacyDenied { rule }`, `StrategyExcluded`, `BudgetInsufficient`, `RateLimited`, `Offline`. Candidates that fail are kept in the decision with their reason (explanations need them).
- [ ] **Score** stage: `score = Q̂ − λ_cost·Ĉn − λ_lat·L̂n` per [04 §5](../architecture/04-priya-ai.md). `Q̂` = posterior mean of Beta(α, β) with prior `α₀ = k·m`, `β₀ = k·(1−m)` where `m` = prior skill × difficulty adjustment and `k` = `ai.learning.prior_strength` (8), plus observed counts from 1.14. `Ĉ` = expected USD cost incl. cache-hit probability for this prefix (from 1.12) and expected output tokens; `Ĉn = log10(1 + Ĉ / c_ref)` with `c_ref = $0.001`; `L̂n` = expected seconds to first token (interactive) or total (background) / 10. Defaults: best λ_cost = 0, λ_lat = 0.02; balanced 0.15 / 0.05; cheapest = lexicographic (Q̂ ≥ `q_min` 0.70, then lowest cost). Values tuned by the eval suite (1.17) and stored in settings.
- [ ] **Explore** (bounded Thompson sampling): sample θ ~ Beta(α, β) instead of Q̂ only when all hold — base strategy is not `fixed`; `stakes = Normal`; caller didn't pass `explore = false`; ≤ 10 % of calls per category per day explored; explored choice's expected cost ≤ max(1.5 × greedy choice, greedy + $0.002); daily exploration spend ≤ `ai.router.explore_budget_usd_day` ($0.05) and ≤ 5 % of the daily budget; only candidates whose prior mean ≥ `q_min − 0.10`. RNG `ChaCha8Rng` seeded from the call id so every decision is replayable.
- [ ] **Cascade** (strategies `balanced`/`cheapest`, only when a verifier exists for the call): order candidates with Q̂ ≥ bar (`cheapest` 0.70, `balanced` 0.80) by cost; run the cheapest; verify; escalate at most 2 steps; total cascade cost bounded by the call budget. Phase 1 verifiers (`priya-ai::verify`): `JsonSchemaVerifier`, `TruncationVerifier` (`MaxTokens` stop, empty answer, refusal), `SelfConsistencyVerifier` (two cheap samples agree, for `classify`/`extract`), `ReviewRoleVerifier` (role `review`, different family, for `stakes = High`). Interactive streamed answers are not cascaded silently; instead the answer offers "Try a stronger model" (escalation on demand, recorded as an outcome).
- [ ] **Stickiness**: within a conversation (`conversation_id` from the bar thread, `priya ask --continue`, or proxy session) keep the same model while the provider cache is warm (5 min, or 1 h when long-TTL breakpoints were written) unless escalating, the user switches, or the model becomes unhealthy; switching drops provider-bound reasoning blocks from history.
- [ ] `Gateway::ask` wires classifier → router → (optimizer from 1.12) → adapter; `RouteRequest` built from role + classification + effective strategy + budget snapshot + health + rate-limit state.
- [ ] `model.called` event payload ([02 §1.1](../architecture/02-core.md)) extended: `call_id`, `conversation_id`, `caller` (`surface:bar`, `cli`, `module:<n>`, `agent:<n>`), role, classification summary, strategy + source layers, top 5 candidates with Q̂/θ/Ĉ/L̂/score and filter-reason counts, chosen, `explored`, `cascade_step`, tokens (in/out/cache read/cache write/reasoning), cost, latency (TTFT, total), `cache: hit|miss`, error kind. No prompt or answer text (hash only).
- [ ] Explainer (`priya-ai::explain`, deterministic templates through the translatable string catalogue, no model call): "I used Sonnet 5.5 because this looked like a medium code question. Haiku got 3 of the last 10 similar questions wrong here; Opus would have cost about 5× more ($0.04 instead of $0.008)." Costs in USD with two decimals. Covers every `ReasonCode`: best score, only candidate, pinned, family filter, local only, privacy rule, offline, budget, explored ("I tried a newer model to learn how well it does"), sticky, escalated, cache hit.
- [ ] `v1.ai.why {call_id | "last"}` → `{summary, facts, candidates}`; `priya ai why [<call-id>|last] [--details]`; `priya ask --why` prints the explanation after the answer.

**Tests**
- Unit (property tests with `proptest`): filters never pass a model that violates privacy, modality, context or strategy; `fixed` always returns the pinned model or an error; scores monotone in Q̂ and anti-monotone in cost; `cheapest` never picks a model with Q̂ < `q_min` if one ≥ `q_min` exists; exploration bounds (rate cap, cost cap, daily cap, never with `stakes = High`, never with `fixed`) hold over 10 000 simulated calls; same call id → same decision; cascade never exceeds 2 escalations or the call budget; stickiness kept while warm and dropped on unhealthy.
- Unit: explainer golden texts for every `ReasonCode` (English; Hindi strings later via translation).
- Integration: with only Anthropic enabled (FakeProvider modelling Haiku/Sonnet/Opus/Fable tiers and prices), the labelled mini-set routes simple chat → Haiku, code edit → Sonnet, hard design → Opus/Fable; with four families enabled, `cheapest`, `best`, `local_only`, `family:claude` select as specified; `model.called` events contain candidates and no prompt text.
- Eval: router adequacy and simulated cost on the full labelled set (1.17).
- Perf: `route()` p95 < 5 ms with 200 catalog models and full stats (excluding the classifier's model call).
- Regression: `route_within_family_claude_only`, `route_across_families_strategies`, `exploration_bounds`, `cascade_escalates_on_schema_failure`, `why_explains_every_reason_code`, `model_called_has_no_prompt_text`.

**Done when**
- [ ] With only Claude enabled, three different question types route to three different Claude models and `priya ai why` explains each choice in one or two plain sentences.
- [ ] With Claude + OpenAI + Gemini + a local model enabled, `cheapest`, `best`, `local_only` and `family:claude` behave as specified in [04 §4](../architecture/04-priya-ai.md) (integration test and owner check on the Zenbook).
- [ ] Exploration bound properties pass; decisions are replayable from the event log.

---

### 1.10 — Reliability and offline mode
**Outcome:** provider failures are absorbed (retry, skip, fall back to the next candidate) without the owner noticing; when the network goes away Priya AI switches to local models with a notice and switches back when it returns.
**Features:** AI-09.
**Depends on:** 1.9.

**Tasks**
- [ ] `CircuitBreaker` per (provider, base URL): Closed → Open after 5 consecutive failures or ≥ 50 % failures over the last 20 calls within 60 s; Open for 30 s doubling to 5 min; Half-open admits one probe call. Only retryable failures count (not `InvalidRequest`, `ContextTooLong`, `ContentFiltered`).
- [ ] Retry policy: retryable errors before the first streamed token → up to 2 retries with full jitter (base 500 ms, cap 8 s), honouring `retry-after` up to 20 s, otherwise fail over to the next router candidate (decision re-run with that model filtered, recorded as `cascade_step` with reason `failover`). After the first token: no silent retry — the answer shows "Connection lost · Try again".
- [ ] Explicit timeouts per call (connect 5 s; first token 20 s cloud / 60 s local cold start; idle between chunks 30 s; total by role: `fast` 30 s, `smart` 300 s, `coding` 600 s), all in settings.
- [ ] `RateLimitState` from response headers per provider/model → router filter `RateLimited` until reset; 429 storms open the breaker.
- [ ] `ConnectivityMonitor`: NetworkManager D-Bus `Connectivity` property (`full` / `limited` / `portal` / `none`) plus failure evidence (all cloud providers failing with `Network`) → `ai.connectivity_changed {online}`; offline → router filter `Offline` on cloud models; back online → probes then resume.
- [ ] Offline UX: `NoticeBanner` "You're offline — using Qwen3 Coder on this computer." / back online "Online again — using all your models."; with no local model: plain error "I'm offline and no local model is set up. Install Ollama and a model to keep working offline." with a link to the guide.
- [ ] `ai.provider.health_changed {provider, state, reason}` events; `priya doctor` section "AI providers": key present, reachable, breaker state, last error in plain language, local endpoints, connectivity.
- [ ] Cancellation everywhere: user Esc / `v1.task.cancel`-style cancel aborts the HTTP stream and releases budget reservations (1.11).

**UI**
- Bar states for this sub-phase: offline banner, back-online banner, failover (no banner; small "switched to GPT-5 because Claude was busy" line under the answer in Details), connection lost mid-answer with Try again, no local model error, long error text, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: Try again reachable with Tab and Enter; banners dismissable with Esc. Screen-reader names: "Offline, using local model", "Try again".
- 👤 Mockup approval (`ui/mockups/1.10/`) before building; gallery sign-off after.

**Human in the loop:** offline switching is automatic and always announced; the owner can pin "Local only" to stay offline-style even when online, and failovers are visible in the answer details and `priya ai why`.

**Tests**
- Unit: breaker state machine (table-driven, fake clock), jitter bounds, `retry-after` caps, timeout phase classification.
- Integration: FakeProvider scripted 529s → retries then failover to another family, answer completes, `model.called` shows `failover`; network namespace without connectivity (Linux CI) + fake NM D-Bus → offline notice, local model used, then recovery; cancel mid-stream frees the reservation.
- Chaos: provider closes connection at random chunk offsets (1 000 runs) — no panics, no hung streams, no leaked reservations.
- UI: banner and error states screenshots × theme × scale; a11y lint.
- Regression: `breaker_state_machine`, `failover_on_overloaded`, `offline_switches_to_local`, `offline_without_local_explains`, `cancel_releases_reservation`.

**Done when**
- [ ] Pulling the network on the Zenbook switches to a local model with a notice within 5 s of the next request, and back when the network returns.
- [ ] A forced provider outage (blocked host in `/etc/hosts` on a test user) never surfaces as an error if another candidate exists.

---

### 1.11 — Costs and budgets
**Outcome:** every call's cost is known to the cent, spend is tracked per model, family, role, caller and day, and budgets (per call, conversation, agent token, day, month) warn at a soft limit and stop cloud spend at a hard limit — local models keep working.
**Features:** AI-16, AI-22.
**Depends on:** 1.9.

**Tasks**
- [ ] `priya-ai::pricing`: `price(model, usage)` with input, output, cache read, cache write (short/long TTL multipliers from catalog), reasoning billed as output, long-context tiers; local = 0. OpenRouter's reported `usage.cost` overrides the estimate when present; difference logged.
- [ ] `costs_daily` projection (date in local timezone, provider, model, family, role, caller, tokens by kind, cost, calls, cache hits, saved amounts from 1.13) and `costs_by_conversation`.
- [ ] `BudgetGuard` with scopes `call`, `conversation`, `agent_token` (1.15), `day`, `month`, optional per-provider monthly cap; soft (default 80 %) and hard (100 %) thresholds; settings `[ai.budget] day_usd = 2.00, month_usd = 30.00, call_usd = 0.50` (defaults chosen with the owner at 1.16 review). Per-task budgets come with the task engine (Phase 3.10) through the same guard.
- [ ] Pre-call **reservation**: reserve the worst case (estimated input × price + `max_tokens` × output price) atomically before sending; reconcile with actual usage after; release on error/cancel; reservations are held in an in-memory ledger guarded for concurrency and rebuilt as empty on restart (only settled `model.called` costs are persisted and projected). If the worst case exceeds the remaining hard budget, first shrink `max_tokens` to fit, else filter the model (`BudgetInsufficient`) so the router picks a cheaper or local one.
- [ ] Events `ai.budget.warning {scope, used, limit}` and `ai.budget.exceeded {scope}`; notices via `NoticeBanner`; hard stop message: "Today's AI budget ($2.00) is used up. Local models still work. Raise today's limit?" with action → `v1.ai.budget.set {scope: day, usd, for_today: true}`.
- [ ] API/CLI: `v1.ai.costs {range, group_by}` and `priya ai costs [--today|--week|--month] [--by model|family|role|caller] [--json]`; `v1.ai.budget.get/set`, `priya ai budget [show|set day 2.00|set month 30]`.
- [ ] Answer footer data: each answer carries `{model, cost, cached}` so the bar can show "Sonnet 5.5 · $0.004".
- [ ] Show costs in the user's currency (default from locale, e.g. INR) with the conversion rate source and date; USD stays the internal unit

**UI**
- Bar: budget warning banner (soft), budget stop card (hard) with "Raise today's limit" and "Use local only", cost line in the answer footer. States: under budget, warning, exceeded, no cloud provider (no cost line), long amounts, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0. Costs use tabular figures and two decimals ([11 §6](../architecture/11-design-system-and-ux.md)).
- Keyboard path: banner actions with Tab/Enter. Screen-reader names: "Budget warning, 1 dollar 62 of 2 dollars used today".
- 👤 Mockup approval (`ui/mockups/1.11/`) before building; gallery sign-off after.

**Human in the loop:** the owner sets the limits; raising a limit is an explicit action (logged as `setting.changed`, undoable); hard limits never silently switch to a more expensive path, only to cheaper or local ones.

**Tests**
- Unit: price function golden table per provider (cache read/write, reasoning, tiers), reservation arithmetic, `max_tokens` shrink-to-fit, threshold crossing emits exactly one warning per scope per period, timezone/day rollover.
- Integration: 50 concurrent calls against a $0.10 day budget never settle above $0.10 + one call's estimation error; hard limit → cloud filtered, local still answers; projection rebuild equals incremental.
- Contract: estimated cost vs. provider-reported usage × catalog price within ±1 % on every usage cassette; OpenRouter actual cost parsed.
- UI: budget banner/card screenshots × theme × scale; a11y lint.
- Regression: `price_golden_table`, `budget_concurrency_never_overspends`, `hard_budget_falls_back_to_local`, `costs_projection_rebuild`, `budget_warning_once_per_period`.

**Done when**
- [ ] `priya ai costs --today --by model` on the Zenbook matches the providers' dashboards within 2 % for a day of normal use (owner spot check).
- [ ] The concurrency test proves hard budgets are never exceeded beyond the documented estimation bound.

---

### 1.12 — Token optimizer I (cache layout, provider caching, trimming, caps)
**Outcome:** requests are laid out so provider caches hit on follow-ups and repeated system prompts; long pasted logs and tool outputs are trimmed; outputs are capped per role — and the savings are measured.
**Features:** AI-11.
**Depends on:** 1.9.

**Tasks**
- [ ] `ContextBuilder` (`priya-ai::optimize::layout`) builds every `ModelRequest` from segments in fixed order: **stable** (system prompt, tool schemas, project memory later), **semi-stable** (conversation history, rolling summary), **volatile** (current date/time, new user turn, attachments). Canonical JSON (sorted keys) for tools; no timestamps or ids in stable segments.
- [ ] Provider cache controls: Anthropic breakpoints at the end of stable and semi-stable segments (long TTL for the stable segment when the role's expected reuse interval > 5 min, e.g. classifier and proxy system prompts), never more than 4; OpenAI `prompt_cache_key = blake3(conversation_id or caller+role)`; xAI conversation affinity if supported; Gemini relies on prefix stability.
- [ ] `PrefixCacheTracker`: (provider, model, prefix hash) → last write time and TTL; gives the router the cache-hit probability for `Ĉ` and reports observed hit rates; mismatches between predicted and reported cache reads are logged as "silent invalidator" diagnostics.
- [ ] Tool-schema compaction for Priya's own calls: strip `examples`, trim descriptions over 300 chars, inline/dedupe `$defs`, stable ordering. (Relevance-based tool selection needs the tool index, Phase 8.)
- [ ] `OutputTrimmer` for long text attachments and tool results in Priya's own calls: keep head 40 lines + tail 80 lines + every line matching the error pattern set (`error|fail|panic|denied|traceback|exception|warning`) with ±2 lines context, cap 4 000 tokens, insert "[… 1 234 lines omitted …]"; original kept as a blob so "show full log" works. Honours `history_edit = AppendOnly`: trimming applies only to new content, never to turns already sent.
- [ ] Output caps and effort per role (`fast` 512 tokens + "answer briefly" style line, `smart` 4 096, `coding` 8 192, `review` 2 048), overridable per call; `MaxTokens` stops are tracked so caps too low show up in stats.
- [ ] Savings ledger entries for provider caching (`cache_read × (input price − cache read price)` minus cache-write premiums) and trimming (removed tokens × input price), written into `model.called` (`saved: {technique: usd}`).

**Tests**
- Unit: segment ordering golden files; same conversation → byte-identical prefix across turns (property test); breakpoint selection by TTL rule; trimmer keeps all error lines and both ends (fixtures: pacman log, cargo build failure, journalctl dump); `AppendOnly` models never get earlier turns modified.
- Contract: cassette pairs (turn 1, turn 2) for Anthropic and OpenAI show `cache_read > 0` on turn 2.
- Integration: 10-turn conversation with FakeProvider modelling cache semantics → predicted hit rate within 10 % of reported; savings ledger matches hand-computed values.
- Live: two-call cache check on Anthropic and OpenAI (budget $0.05) — the second call reports cached tokens.
- Regression: `stable_prefix_byte_identical`, `cache_breakpoints_max_four`, `trimmer_keeps_errors`, `append_only_history_untouched`, `cache_savings_ledger`.

**Done when**
- [ ] On the Zenbook, the second and later turns of a conversation with a cloud model show cache reads > 50 % of input tokens (`priya ai costs --detail`).
- [ ] A 50 000-line pasted log is answered with ≤ 4 000 tokens of log in the request and the error lines intact.

---

### 1.13 — Token optimizer II (compaction, response cache, savings)
**Outcome:** long conversations stay within budget through compaction, repeated questions are answered from a local cache (exact or semantic, read-only answers only, invalidated when what they depend on changes), and `priya ai savings` reports tokens and money saved per technique.
**Features:** AI-11, AI-12, AI-19.
**Depends on:** 1.11, 1.12.

**Tasks**
- [ ] `Compactor`: trigger when history > 60 % of the chosen model's context or > `ai.compaction.max_history_tokens` (32 000); prefer provider server-side compaction / context management when `caps.server_compaction` (keeps `AppendOnly` providers valid and preserves their cache); otherwise summarise the oldest turns with role `fast` (or `private` when the conversation's privacy class is `Private`) into a rolling-summary block in the semi-stable segment, keep the last 6 turns verbatim, replace stale tool results with one-line stubs ("[tool output omitted: `journalctl -u nginx`, 2 KB]"). Client-side compaction on `AppendOnly` models starts a fresh request history (summary + recent turns, reasoning blocks dropped), recorded as a cache reset.
- [ ] `ResponseCache` in `priya.db` (`ai_response_cache`: key, kind `exact|semantic`, embedding, answer blob ref, model, quality floor, created, ttl, deps JSON, privacy class). **Exact** key = BLAKE3 of canonical (role, normalised messages, tools, response format) — used for deterministic calls (classification, titles, extraction at temperature 0). **Semantic** = cosine ≥ `ai.cache.semantic_threshold` (0.95) on `embed`-role vectors (local embedding model preferred; semantic cache disabled when no embed model is enabled), only for answer-only questions without attachments and without time-sensitive wording ("today", "now", "latest", "current").
- [ ] Eligibility and quality: a cached answer is served only if its model's Q̂ for the category ≥ the current strategy's floor; `fixed:` and "Try a stronger model" bypass the cache; `Private` entries never served to a different profile/user.
- [ ] Invalidation by dependencies: TTL (24 h default; 1 h for `sysadmin` answers about machine state); `setting.changed` for keys the answer read; file attachments' content hash; catalog version for questions about models; subscription to `changeset.recorded` (an extension point now — Phase 2 starts emitting it) evicting entries whose deps intersect changed paths; user "that's outdated" → evict + negative feedback (1.14).
- [ ] Cache hits still emit `model.called {cache: hit, cost: 0, saved}`; the bar answer footer shows "From an earlier answer · 2 h ago · Ask fresh".
- [ ] `SavingsLedger` per technique against a **no-optimizer baseline** (same routed model, no caching, no trimming, no compaction, no response cache, no cascade): provider caching, response cache (full avoided cost), trimming, compaction (tokens removed × input price on each later call), cascades (cost of the model the cascade avoided minus actual), output caps excluded (not estimable honestly). Stored in `costs_daily.saved_*`.
- [ ] `v1.ai.savings {range}` and `priya ai savings [--today|--week|--month] [--json]` — plain-language summary ("This week Priya saved about $3.10 (41 %): $1.90 from provider caching, $0.70 from repeated questions…").

**Tests**
- Unit: compaction trigger thresholds; summary placement keeps prefix stability for the rest of the history; time-sensitive phrase detector; cache key canonicalisation (whitespace, key order); dependency invalidation matrix (setting, file hash, catalog, changeset event); quality-floor gate.
- Integration: 40-turn conversation with FakeProvider → history stays under the threshold, answers still reference early facts (scripted check); repeated question → served from cache in < 30 ms; editing a referenced file → miss; `setting.changed` on a dependent key → miss; private entry not served to another user's priyad.
- Eval: semantic cache precision on a 100-pair paraphrase/near-miss set ≥ 98 % at the chosen threshold (false hits are worse than misses).
- Perf: exact lookup p95 < 5 ms; semantic lookup p95 < 30 ms with 10 000 entries (brute-force cosine on f32 vectors).
- Regression: `compaction_keeps_recent_turns`, `response_cache_exact_hit`, `semantic_cache_rejects_time_sensitive`, `cache_invalidated_by_dependency`, `savings_against_baseline`.

**Done when**
- [ ] `priya ai savings` shows caching and trimming savings on a repeated workload (scripted 30-question replay on the Zenbook) with numbers that match the ledger.
- [ ] No stale answer is served in the invalidation integration suite.

---

### 1.14 — Learning from outcomes
**Outcome:** Priya AI learns which model works best on this machine: verifier results, escalations, failovers and the owner's 👍/👎 update per-model statistics that the router uses; `priya ai stats` shows a plain-language league table.
**Features:** AI-14.
**Depends on:** 1.9.

**Tasks**
- [ ] `OutcomeSignal` enum and weights: `VerifierPass` (+1 success), `VerifierFail` (+1 failure), `Escalated` (+1 failure for the cheaper model), `UserThumbsUp` (+1 success), `UserThumbsDown` / "that was wrong" (+1 failure), `TriedStronger` (+0.5 failure), `Refusal` (excluded from quality, counted separately), plus reserved variants wired in later phases: `MergeAccepted`, `Discarded` (Phase 2/4), `RevertedWithin7d` (Phase 7). Signals without a call id are ignored.
- [ ] `ai.feedback {call_id, signal, comment_hash?}` and `ai.outcome {call_id, model, category, difficulty_bucket, signal, weight}` events; `v1.ai.feedback`; `priya ai rate <call-id|last> good|bad`; 👍/👎 buttons on answers in the bar (rendered in 1.16, API now).
- [ ] `model_stats` projection keyed by (model, category, difficulty bucket `easy` 1–2 / `medium` 3 / `hard` 4–5) — stores **observed** success/failure weights only (priors are added at read time so a new `priors_version` applies retroactively), cost EMA and latency EMAs (TTFT, total; α = 0.1), call counts, last used. Weekly discount γ = 0.95 on observed weights so silent provider-side model changes are re-learned. For proxied calls an additional (agent, model) dimension (ADR-007 quality tracking).
- [ ] Reset rules: new model id starts from priors (successors inherit priors, not observations); `priya ai stats --reset [<model>]` (`setting.changed`-style event so it is undoable by replay).
- [ ] `v1.ai.stats {category?}` and `priya ai stats [--category code_edit] [--json]`: league table per category with plain sentences ("For code questions, Sonnet 5.5 succeeded 46 of 50 times here; Haiku 5.5 31 of 40").
- [ ] Privacy: stats never leave the device; export only on explicit `priya ai stats --export file.json`.

**Tests**
- Unit: Beta update arithmetic, weekly discount, prior-at-read-time with priors version change, bucket mapping, successor reset.
- Integration: simulated outcome stream from `FakeProvider` where model A succeeds 90 % and model B 60 % on `code_edit` with equal priors → after 200 calls the router chooses A in ≥ 80 % of greedy decisions; projection rebuild equals incremental; reset restores priors.
- Eval: learning-convergence scenario in the eval suite (1.17) — regret curve stored with the baseline.
- Regression: `beta_update_math`, `stats_projection_rebuild`, `learning_prefers_better_model`, `stats_reset_to_priors`.

**Done when**
- [ ] 👎 on an answer from the CLI or API changes the model's stats and is visible in `priya ai stats`.
- [ ] The convergence scenario passes deterministically (seeded).

---

### 1.15 — Local proxy for agents
**Outcome:** any agent can use any enabled model through priyad on `127.0.0.1`: Claude Code via `ANTHROPIC_BASE_URL`, Codex via a Responses-API custom provider, OpenCode/Aider/Goose via Chat Completions — with the virtual model `priya-auto` letting Priya AI decide per request, and routing, caching, budgets, privacy and logging applied uniformly.
**Features:** AI-18.
**Depends on:** 1.5, 1.9, 1.10, 1.11.

**Tasks**
- [ ] `priya-ai::proxy` HTTP server inside priyad (`axum`/`hyper`), off by default; `ai.proxy.listen = "127.0.0.1:47800"` (loopback only — non-loopback addresses rejected by settings validation); `priya ai proxy enable|disable|status`.
- [ ] Endpoints: `POST /v1/messages` and `POST /v1/messages/count_tokens` (Anthropic format, SSE event names identical to Anthropic's), `POST /v1/responses` (OpenAI Responses, streaming events identical to OpenAI's; `previous_response_id` supported through a local response store kept `ai.proxy.response_store_ttl` = 1 h), `POST /v1/chat/completions` (streaming with `include_usage`), `GET /v1/models` (OpenAI list shape; Anthropic list shape when the `anthropic-version` header is present), `GET /health`.
- [ ] Virtual models: `priya-auto` (role from classification), `priya-auto:fast|smart|coding|review` (role pinned), plus every enabled catalog id and alias passed through (`anthropic/claude-sonnet-5-5` or bare `claude-sonnet-5-5`). Claude Code's small/background model variables map to `priya-auto:fast`.
- [ ] Format translation through the unified types: Anthropic `tool_use`/`tool_result` ↔ OpenAI function calls ↔ Responses items; images pass through; reasoning blocks returned only in the caller's own format and never forwarded to a different model; forced `tool_choice` requested against a model that rejects it → downgraded to `auto` + instruction and `strict` tools (recorded), or a 400 in the caller's format if the caller requires it.
- [ ] Errors mapped to each API's error shape (Anthropic `{type: "error", error: {type, message}}`; OpenAI `{error: {message, type, code}}`); budget exceeded → 429 with `retry-after` and a plain message; privacy-denied → 403 naming the rule; no candidate → 503.
- [ ] Per-agent tokens: `priya ai proxy token create <agent-name>` → random 256-bit token stored in the Secret Service, shown once; accepted as `x-api-key` or `Authorization: Bearer`; token → `caller = agent:<name>`, optional per-token strategy override and budget (`agent_token` scope from 1.11); `priya ai proxy token revoke`.
- [ ] Setup helpers: `priya ai proxy env claude-code` prints `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN` exports; `… env codex` prints a `config.toml` provider block (`base_url`, `wire_api = "responses"`, `env_key`); `… env opencode` prints its provider JSON. ADR-007 caveat printed for off-family use ("Claude Code with non-Claude models is not supported by Anthropic; quality may drop").
- [ ] Session stickiness for agents: session key from Anthropic `metadata.user_id`, OpenAI `prompt_cache_key`/`user`, or a hash of the first system+user message; cache layout preserved (the proxy never reorders an agent's prefix; it only adds breakpoints/`prompt_cache_key` when absent).
- [ ] Hardening: token required on every request; requests with a non-null `Origin` header or a `Host` other than the loopback address rejected (blocks browser CSRF and DNS rebinding); body limit 32 MiB; per-token concurrency limit (8); no response cache for proxied tool-using traffic.
- [ ] Every proxied call emits `model.called` with `caller = agent:<name>` and feeds (agent, model) stats.

**Human in the loop:** the proxy is opt-in per agent: the owner enables it, creates a token per agent and pastes the printed config. Each token can be revoked; `priya ai costs --by caller` shows what each agent spent.

**Tests**
- Unit: format translators round-trip (Anthropic ↔ unified ↔ Responses ↔ Chat) on golden request/response corpora incl. parallel tool calls and images; model-name resolution; error shape mapping table.
- Contract: proxy contract suite in all three API formats — the official Anthropic and OpenAI SDK request shapes (recorded from real clients: Claude Code, Codex, OpenCode traffic captured against a mock upstream) replayed against the proxy with `FakeProvider` upstream; streamed event sequences validated against each API's event grammar.
- Integration: Claude Code CLI (if installed on the runner; otherwise a recorded client replay) completes a scripted read-only Q&A turn with tool calls through `priya-auto`; Anthropic-format caller served by a local model through the compatible adapter; budget exhaustion → 429 in the right format.
- Security: no token → 401; wrong token → 401; `Origin: http://evil.example` → 403; bind to `0.0.0.0` rejected; tokens never in events or logs.
- Perf: proxy adds < 15 ms p95 to time-to-first-token and < 5 % CPU at 20 concurrent streams on the Zenbook.
- Live (manual, recorded in the PR): Claude Code via `ANTHROPIC_BASE_URL` and OpenCode via OpenAI-compatible config both answer a question with `priya-auto` (budget $0.20).
- Regression: `proxy_messages_stream_grammar`, `proxy_responses_stream_grammar`, `proxy_chat_stream_grammar`, `proxy_tool_translation_roundtrip`, `proxy_rejects_cross_origin`, `proxy_budget_429`.

**Done when**
- [ ] OpenCode (OpenAI-compatible) and Claude Code (`ANTHROPIC_BASE_URL`) both work against the proxy with `priya-auto` on the Zenbook, and their spend appears under their own caller names.
- [ ] Proxy security tests pass in CI.

---

### 1.16 — AI & Models settings, model picker, usage view
**Outcome:** the owner controls Priya AI without a terminal: a model chip in the bar with a picker (strategy, family, pin, scope) and "Why this model?", answer footers with model, cost and 👍/👎, a Settings window with the **AI & Models** page, and the **Usage & cost** view with budgets and savings.
**Features:** DS-07, DS-08, AI-04, AI-15.
**Depends on:** 1.5, 1.7, 1.9, 1.11, 1.13, 1.14.

**Tasks**
- [ ] PriyaUI components first (library + gallery): `ModelChip`, `StrategySegmented`, `FamilyFilterChips`, `ModelList` (virtualised, search), `WhySheet`, `AnswerFooter` (model, cost, cached badge, 👍/👎, "Try a stronger model"), `ProviderRow`, `BudgetMeter`, `UsageChart` (bar chart, tabular figures), `SavingsList`.
- [ ] Bar **model chip** ("Auto · Balanced"; after an answer "Sonnet 5.5"); click or `Ctrl+M` opens the **model picker**: strategy (Best / Balanced / Cheapest / Local only), family filter, pin a model (search), scope (This conversation / Today / Always), link "Why this model?". Applies via `v1.ai.use`; Undo toast.
- [ ] "Why this model?" sheet from the chip or the answer footer: `v1.ai.why` summary, and under Details a candidate table (model, expected quality, est. cost, why skipped).
- [ ] Answer footer wired to `v1.ai.feedback` and "Try a stronger model" (re-asks with escalation, recorded as `TriedStronger`).
- [ ] Minimal **Settings window** shell (Quickshell `FloatingWindow`, sidebar with only "AI & Models" and "Usage & cost" enabled; other pages arrive in Phases 3 and 5) with "Tell Priya what to change" input limited to AI-control phrases (1.8).
- [ ] **AI & Models page**: Providers (status, Connect via `ConnectProviderDialog`, Test, Disconnect), Local endpoints (discovered/added, trusted-local toggle, add endpoint), Models (enable toggles, local badge, price, context, availability, new badge), Default strategy, Roles (advanced, collapsed), Privacy ("Keep these folders on this computer" path list, provider deny list), Discovery (auto-enable successors, price source), Agents proxy (enable, tokens, copy setup), each change a validated `setting.changed` with Undo toast.
- [ ] **Usage & cost view**: Today / Week / Month tabs; total, by model / family / role / agent chart; savings by technique (`v1.ai.savings`); budgets with meters and edit (`v1.ai.budget.set`); "Export CSV".
- [ ] Notices list (from 1.6/1.10/1.11) in the AI page with history.
- [ ] Live updates via `v1.events.subscribe` (`model.called`, `ai.*`, `setting.changed`); QML type helpers regenerated from the API schemas.

**UI**
- Model chip and picker — states: auto, pinned, family filter on, local only, offline (cloud greyed with reason), no providers, long model names, Hindi text. Why sheet — loading, explanation, cache hit, explored, failover, error. Answer footer — streaming (cost pending), done, cached, rated up/down, local (no cost).
- AI & Models page — empty (no providers), loading, connected list, provider error, offline, invalid setting (inline error), locked by admin (`/etc` layer), long lists (200 models), Hindi text.
- Usage & cost — empty (no calls yet), loading, data, budget warning, budget exceeded, offline (local-only data), long numbers, Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; 150 % font scaling without clipping.
- Keyboard path: `Ctrl+M` opens the picker; arrows move between strategies; Enter applies; Esc closes; full Tab order through Settings; `Ctrl+F` searches models. Screen-reader names on every control ("Strategy: Balanced, selected", "Pin model Claude Opus 5.5", "Cost today 1 dollar 24"); charts have a text table alternative.
- 👤 Mockup approval (`ui/mockups/1.16/`: picker, why sheet, footer, AI page, usage view, light + dark) before building; gallery sign-off and recorded journey sign-off after.

**Human in the loop:** every routing control is in the owner's hands and undoable; "Why this model?" is one click from any answer; budgets and privacy rules are edited here and take effect on the next call; nothing changes provider connections without an explicit Connect/Disconnect.

**Tests**
- UI: Qt Quick component tests for every new component (keyboard, focus order, signals); screenshot baselines for every state × theme (light, dark, OLED-black, high-contrast) × scale (1.0, 2.0); a11y lint (`Accessible.name`/`role`), contrast check; reduced-motion variants.
- UI journeys (mock priyad): "open picker → Cheapest → ask → footer shows cheaper model → Why → explanation"; "Settings → connect provider → enable model → pin it → ask"; "Usage → raise today's budget → banner disappears".
- Integration: picker actions produce the same `setting.changed` events as the CLI (parity test, P7).
- Perf: picker opens < 100 ms; Usage view renders a month of data (10 000 calls) < 300 ms.
- Regression: `model_picker_states`, `why_sheet_states`, `ai_models_page_states`, `usage_view_states`, `picker_cli_parity`, `journey_switch_strategy_and_why`.

**Done when**
- [ ] Owner switches strategy, family and pinned model from the bar chip, sees "Why this model?" for three answers, and reads today's spend and savings in the Usage view — all keyboard-only once and mouse-only once.
- [ ] 👤 Gallery and journeys signed off.

---

### 1.17 — Router evaluation suite
**Outcome:** a labelled evaluation set (~300 prompts) measures classifier accuracy, routing adequacy, privacy safety and simulated cost every night against stored baselines; live smoke tests check every enabled provider within a hard budget; mutation testing guards the routing and money code.
**Features:** QA-12, QA-13, QA-14.
**Depends on:** 1.9, 1.14.

**Tasks**
- [ ] `eval/router/prompts.jsonl` (~300 items): `{id, text, attachments?, lang (en|hi|hinglish), category, difficulty, privacy, stakes, expected_min_tier, acceptable_families?, time_sensitive, ai_control?, source}`. Coverage: ≥ 25 per category across the nine categories, ≥ 30 Hindi/Hinglish, ≥ 20 private-path cases, ≥ 20 AI-control phrases (incl. near-misses), ≥ 20 with images, ≥ 15 very long inputs. Labels by Claude with owner spot-check of 10 %; disagreements resolved and recorded.
- [ ] Fixed simulation inputs: `eval/router/catalog-snapshot.toml` (pinned models and prices) and `eval/router/outcomes.toml` (success probability per tier × category × difficulty) so results don't drift with live prices.
- [ ] `cargo xtask eval router`: runs classifier (heuristics + recorded `fast`-model outputs from cassettes) and router per strategy; metrics — category accuracy, difficulty MAE, **adequacy** (chosen tier ≥ `expected_min_tier` and family acceptable), over-provisioning rate, **privacy violations (must be 0)**, AI-control precision/recall, simulated cost vs. baselines (always-frontier, always-small, random-enabled, oracle-cheapest-adequate); learning convergence and regret curve (1.14 scenario).
- [ ] Baselines: `eval/router/baselines/latest.json` + per-commit history; nightly job fails if adequacy drops > 2 pp, simulated cost rises > 10 %, or any privacy violation appears; results posted to the nightly dashboard issue; tuning of λ/`q_min` defaults recorded with the eval numbers that justified them.
- [ ] Live suite (`PRIYA_LIVE=1 cargo xtask test live`, nightly on `main`, CI secrets scoped to this job): per enabled provider — `list_models`, 16-token streamed call, one tool call, one image call where supported, cache check (two calls); local Ollama on the self-hosted runner; proxy round-trip in all three formats; hard total budget **$0.50** per run enforced by `BudgetGuard` (abort, not just warn); results tagged `suite: live`.
- [ ] Optional live eval subset (30 prompts, real calls, LLM-judge with a different family than the answering model) run before releases, budget $1.00, manual trigger only.
- [ ] Mutation testing: `cargo-mutants` weekly on `priya-ai::{router, pricing, budget, learning, classify::heuristics, verify}`; surviving mutants listed in the weekly report; threshold ≤ 10 % survivors for `router`, `pricing`, `budget`.

**Tests**
- Eval: the suite itself, tagged `suite: eval`, `covers: AI-05, AI-06, AI-08, AI-10, AI-13, AI-14, QA-12`.
- Live: tagged `suite: live`, `covers: AI-01, QA-13`.
- Mutation: tagged `suite: mutation`, `covers: QA-14`.
- Unit: metric computations (adequacy, MAE, regret) on toy data; budget abort in the live harness.
- Regression: `eval_router_no_privacy_violations` (runs on every merge using the fast heuristic-only path), `eval_router_against_baseline` (nightly).

**Done when**
- [ ] First baseline stored: adequacy ≥ 90 %, category accuracy ≥ 85 %, privacy violations 0, simulated cost ≤ 40 % of always-frontier at equal or better adequacy.
- [ ] Live job runs nightly under $0.50 and opens an issue on failure.
- [ ] Weekly mutation report generated and below thresholds.

---

### 1.18 — Phase gate
**Outcome:** Priya AI is complete for answer-only use on the owner's Zenbook; every Phase 1 acceptance test is in the permanent regression suite.
**Features:** all Phase 1 features.
**Depends on:** 1.1–1.17.

**Tasks**
- [ ] Run the full regression suite plus nightly eval, live, perf, security and mutation jobs on `main`; fix failures.
- [ ] Tag every Phase 1 acceptance test `regression`; `cargo xtask trace` shows each Phase 1 feature with passing tests.
- [ ] Update architecture docs to match the code: [04](../architecture/04-priya-ai.md) (trait additions, strategy composition, `HistoryEdit`, proxy port and hardening, verifiers, defaults for λ/`q_min`/exploration bounds), [02 §1.1 and §4](../architecture/02-core.md) (new `ai.*` events and `v1.ai.*` methods), [05 §2](../architecture/05-security.md) (`ModelUseGate` until Cedar lands), [11 §4](../architecture/11-design-system-and-ux.md) (final picker and usage states); ADRs for strategy composition and LAN-endpoint privacy default.
- [ ] Update `features.md` statuses for all Phase 1 IDs, `docs/human/05-the-phases.md` status, and the Priya AI section of `docs/human/04-everything-priyaos-will-do.md`.
- [ ] Owner demo (below) recorded.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green on `main` (unit, contract, integration, UI, security fast subset), plus nightly eval, live (≤ $0.50), perf and security; weekly mutation report below thresholds; quarantine list empty for `priya-ai`.
- [ ] Router eval baseline stored in `eval/router/baselines/latest.json` with adequacy ≥ 90 %, privacy violations 0, simulated cost ≤ 40 % of always-frontier.
- [ ] With only Claude enabled, different question types route to different Claude models and `priya ai why` explains each choice.
- [ ] With Claude + OpenAI + Gemini + local enabled, `cheapest`, `best`, `local_only` and `family:claude` behave as specified.
- [ ] Adding an Ollama model needs zero settings entries (discovery); a LAN LM Studio needs exactly one.
- [ ] Pulling the network cable switches to local models with a notice, and back.
- [ ] OpenCode (OpenAI-compatible) and Claude Code (`ANTHROPIC_BASE_URL`) both work against the local proxy with `priya-auto`.
- [ ] `priya ai savings` shows cache and trimming savings on a repeated workload; budgets stop cloud spend at the hard limit while local keeps working.
- [ ] Perf: route decision p95 < 5 ms; first token < 1.5 s with a local fast model; proxy overhead < 15 ms p95; idle priyad RSS < 80 MiB with the full catalog loaded.
- [ ] 👤 Owner demo on the Zenbook: three different questions with only Claude enabled (show the chip and "Why this model?"); enable a local model and switch to Cheapest from the picker; "use only Claude today" typed in the bar; pull the network; open Usage & cost and `priya ai stats`; run Claude Code through the proxy.
- [ ] Docs updated (architecture, ADRs, `features.md` statuses, `docs/human/04-…`, `docs/human/05-the-phases.md`).

## Out of scope

- Any change to the machine, and any tool execution against the system (read-only runs arrive with Priya Sandbox in Phase 2; agents act in Phase 4). Answers in Phase 1 come from the model and the user's attachments only.
- Choosing (agent, model) pairs and launching agents with their own model flags — ADR-007 lever 1 (ORC-04, Phase 4). Phase 1 delivers only lever 2, the proxy.
- Cedar policy engine and the `UseModel` Cedar schema (Phase 3.4) — Phase 1 uses the settings-backed `ModelUseGate`; task-scoped budgets and the kill switch (Phase 3.10); desktop notifications (UX-18, Phase 3).
- Provider batch APIs for background jobs (AI-20, Phase 13); Gemini explicit `cachedContents`.
- General natural-language settings (CORE-08, Phase 5) — only AI-control phrases are understood here; the complete bar chips, top-bar cost display and full Settings app (UX-17, UX-10, UX-11, Phase 5).
- Relevance-based tool selection and retrieval from memory/files (tool index and memory, Phase 8).
- Vision features, OCR fallback and screen capture (Phase 11) — image inputs are supported by the types and adapters only; voice/`transcribe` role (Phase 10).
- Bedrock, Vertex, Azure and other provider adapters; fine-tuning; electricity cost of local models; sharing learned statistics across machines or users.
