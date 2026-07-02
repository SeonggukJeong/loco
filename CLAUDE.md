# loco — air-gapped small-model coding CLI (Rust)

Rust CLI driving a local LLM via any OpenAI-compatible API (LM Studio default `http://localhost:1234/v1`).
Spec: `docs/superpowers/specs/2026-07-02-loco-design.md`. Plans: `docs/superpowers/plans/`.
M1-M2 done (streaming /chat + read-tool agent REPL); M3 (mutating tools + confirmation gate) is next.

## Commands
- `cargo test` — full suite (fast, <1s)
- `cargo clippy --all-targets -- -D warnings` — required gate per task; `--all-targets` matters (also lints test code)
- `cargo run` — agent REPL (default input runs the tool loop; /chat for plain streaming chat); `cargo run -- -p "question"` — one-shot agent; summary to stdout, progress to stderr, exit codes 0/1/2
- Server-down smoke: temp dir with `.loco/config.toml` pointing `base_url` at `http://127.0.0.1:1/v1` (port 1 is reliably closed; ephemeral ports may be occupied)

## Hard constraints (from spec)
- Edition 2024. Dependency list is fixed by the spec — ask the user before adding any crate
- reqwest stays `default-features = false, features = ["json", "stream", "rustls-no-provider"]`; TLS crypto provider is rustls+ring (direct dep, user-approved) — no OpenSSL and no aws-lc-sys in the graph (Windows offline builds need no cmake/NASM). `main()` installs the ring provider at startup
- Network calls only to the configured endpoint; the HTTP client uses `.no_proxy()` (corporate proxies must not capture localhost LLM traffic)
- User-facing CLI messages in Korean; identifiers and SYSTEM_PROMPT in English
- Errors: `thiserror` in `llm` module, `anyhow` at app level
- Conventional commits (subjects may be Korean)

## Architecture
- lib + thin bin: modules declared in `src/lib.rs` (`config`, `llm`, `ui`); `src/main.rs` is wiring only
- config: layered TOML — global config_dir then `./.loco/config.toml`, later file wins, `deny_unknown_fields` rejects typos
- llm: hand-rolled SSE line parser (`llm/sse.rs`); chat retry = 3 total attempts with 200/400ms backoff, 5xx retried, 4xx immediate; `list_models` deliberately has NO retry (startup check, fail fast — spec §9 exception)
- REPL history: index 0 is the system prompt (`/clear` truncates to 1); failed or empty streams pop the user turn so it can be retyped
- tools: `Tool` trait + `Registry::read_only()` (read_file/list_files/grep); `path::confine` rejects absolute/drive/UNC/`..`/symlink-escape paths and accepts `\` separators; model-facing tool output/errors are English
- agent: one JSON `{thought, action}` per turn forced via `response_format: json_schema` (shallow schema); tool results wrapped in `<tool_result>` user messages (no `role:"tool"`); per-turn parse retry x3, `finish_reason: length` gets a "shorter" correction, blind 400 fallback ladder (drop json_schema → inline system prompt); finish is handled by the loop, not the registry
- REPL keeps separate agent and /chat histories; Ctrl+C cancels an in-flight run (history rolls back to the pre-request snapshot)

## Notes
- `.superpowers/` is git-ignored scratch (subagent-driven-development progress ledger lives there); `git clean -fdx` destroys it
- No git remote — local-only repo
- Live smoke needs LM Studio running with a model loaded; `model = ""` in config auto-selects the server's first model
