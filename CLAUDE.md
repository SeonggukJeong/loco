# loco — air-gapped small-model coding CLI (Rust)

Rust CLI driving a local LLM via any OpenAI-compatible API (LM Studio default `http://localhost:1234/v1`).
Spec: `docs/superpowers/specs/2026-07-02-loco-design.md`. Plans: `docs/superpowers/plans/`.
M1 (streaming chat REPL) merged to main; M2 (read-tool agent) is next.

## Commands
- `cargo test` — full suite (fast, <1s)
- `cargo clippy --all-targets -- -D warnings` — required gate per task; `--all-targets` matters (also lints test code)
- `cargo run` — REPL; `cargo run -- -p "question"` — one-shot streaming mode
- Server-down smoke: temp dir with `.loco/config.toml` pointing `base_url` at `http://127.0.0.1:1/v1` (port 1 is reliably closed; ephemeral ports may be occupied)

## Hard constraints (from spec)
- Edition 2024. Dependency list is fixed by the spec — ask the user before adding any crate
- reqwest stays `default-features = false, features = ["json", "stream", "rustls"]` — no OpenSSL in the graph (reqwest 0.13 renamed `rustls-tls` → `rustls`)
- Network calls only to the configured endpoint; the HTTP client uses `.no_proxy()` (corporate proxies must not capture localhost LLM traffic)
- User-facing CLI messages in Korean; identifiers and SYSTEM_PROMPT in English
- Errors: `thiserror` in `llm` module, `anyhow` at app level
- Conventional commits (subjects may be Korean)

## Architecture
- lib + thin bin: modules declared in `src/lib.rs` (`config`, `llm`, `ui`); `src/main.rs` is wiring only
- config: layered TOML — global config_dir then `./.loco/config.toml`, later file wins, `deny_unknown_fields` rejects typos
- llm: hand-rolled SSE line parser (`llm/sse.rs`); chat retry = 3 total attempts with 200/400ms backoff, 5xx retried, 4xx immediate; `list_models` deliberately has NO retry (startup check, fail fast — spec §9 exception)
- REPL history: index 0 is the system prompt (`/clear` truncates to 1); failed or empty streams pop the user turn so it can be retyped

## Notes
- `.superpowers/` is git-ignored scratch (subagent-driven-development progress ledger lives there); `git clean -fdx` destroys it
- No git remote — local-only repo
- Live smoke needs LM Studio running with a model loaded; `model = ""` in config auto-selects the server's first model
