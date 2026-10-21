# Ollama cassette

A real streamed answer, replayed by `crates/priya-ai/tests/cassettes.rs` through
`priya_testkit::http_script::ScriptedHttp`. Request headers are never recorded,
so no key is kept (Ollama needs none anyway).

- Server: Ollama 0.40.6 on the owner's Mac (macOS, Apple Silicon), `http://127.0.0.1:11434/v1`.
- Model: `qwen2.5:0.5b` (qwen2, 494M parameters, Q4_K_M).
- Question: "Say hello in five words.", `max_tokens = 16`.
- Recorded 2026-10-20 with `PRIYA_RECORD=ollama cargo test -p priya-ai --test cassettes -- --exact record`.

Record again when Ollama's streaming format changes, then check the replay tests
still pass. Gemini's cassette goes in `../gemini/` (recorded by the owner on the
Zenbook after `priya ai key set gemini`, with `PRIYA_RECORD=gemini`). It asks
`gemini-3.8-flash` with `reasoning_effort = "low"`, as PriyaOS does by
default, and a 512-token limit, since Gemini counts its thinking against it.
