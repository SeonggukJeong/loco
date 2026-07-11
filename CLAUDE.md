# loco — air-gapped small-model coding CLI (Rust)

Rust CLI driving a local LLM via any OpenAI-compatible API (LM Studio default `http://localhost:1234/v1`).
Spec: `docs/superpowers/specs/2026-07-02-loco-design.md`. Plans: `docs/superpowers/plans/`.
M1-M4 done (guided agent + eval harness implemented); baseline pass-rate measurement is next (Task 14+).

## Commands
- `cargo test` — full suite (fast, <1s)
- `cargo clippy --all-targets -- -D warnings` — required gate per task; `--all-targets` matters (also lints test code)
- `cargo run` — agent REPL (default input runs the tool loop; /chat for plain streaming chat); `cargo run -- -p "question"` — one-shot agent; summary to stdout, progress to stderr, exit codes 0/1/2
- `cargo run -- --auto` (or `-p ... --auto`) — auto-approve the confirmation gate (still blocked by `auto_deny_patterns`); without it, mutating actions prompt `적용할까요? [y/N]` interactively, or are denied outright in `-p` non-interactive mode
- `cargo run -- eval <tasks-dir> [--repeats N] [--seed N] [--timeout-scale F]` — eval harness (e.g. `cargo run -- eval tasks/ --repeats 3`); report table to stdout + `./.loco/eval/<stamp>/report.json` (+ per-run transcripts `run-<task>-<repeat>.jsonl`); exit 0 on normal completion regardless of pass rate, 1 on harness errors (server down, bad task defs) or a Ctrl+C partial report
- Session transcripts: one JSONL file per run at `./.loco/sessions/*.jsonl` (best-effort; `.loco/.gitignore` keeps them out of commits)
- Tunable config (defaults): `max_turns`=25, `max_output_tokens`=2048, `context_tokens`=8192, `command_timeout_secs`=60, `auto_deny_patterns`; unknown keys rejected (`deny_unknown_fields`)
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
- tools: `Registry::guided()` adds write_file/edit_file/run_command to the M2 three (6-tool set; `finish` stays loop-handled, not in the registry); `path::confine_for_write` is the write variant of `confine` — target need not exist, but the deepest *existing* ancestor is canonicalized and root-checked (blocks escape via a symlinked ancestor). `edit_file`'s `search` match is a 3-stage ladder (exact → ignore-trailing-whitespace → indent-shift), erroring on 0 or ≥2 matches; write_file/edit_file preserve the file's dominant EOL style (CRLF in, CRLF out, `tools/eol.rs`). `run_command` runs `sh -c`/`cmd /C` from project root in its own process group so timeout/cancel kills the whole tree (`kill -9 -<pgid>` / `taskkill /T /F`); UTF-8 with CP949 lossy fallback for Korean Windows consoles; output middle-truncated past 8000 bytes — the exec mechanics live in `tools/exec.rs`, shared with eval's `check` execution
- agent: confirmation gate lives inside the turn loop via the `Approver` trait (`TtyApprover`/`AutoApprover`/`NonInteractiveApprover`) — fires only for `is_mutating()` tools with a successful `preview()`; denial feeds back to the model as a tool result. Repetition detection on the last `(tool, args)` key: 3rd identical repeat injects one correction message, 5th returns `AgentOutcome::RepetitionStop` (exit 2). Tool dispatch runs via `spawn_blocking` (synchronous tools don't block the async runtime) with a shared `cancel: AtomicBool` the REPL flips on Ctrl+C
- session: `Session` (src/session.rs) owns chat history + the JSONL transcript together — every push records both. `pack()` is the spec §6 budget: elide oldest tool-result bodies first, then drop oldest user+assistant pairs atomically (system prompt and final message preserved), mutating stored history in place (transcript keeps unabridged originals). Context-overflow 400 (heuristic: body contains "context") packs at a shrinking budget and retries, up to 2 shrinks per `run()` call before propagating — takes priority over the blind 400 fallback ladder, which doesn't fire on overflow
- `--auto`: `AutoApprover` approves everything except `run_command` matching `auto_deny_patterns` (cross-platform default list — `rm -rf`, `sudo`, `git push`, etc., configurable in TOML), blocked only in auto mode; interactive mode shows a warning line instead
- `eval` (`src/eval/`): in-process harness — per task×repeat: fixture → temp sandbox → `Agent::run` with `AutoApprover` + seed(base_seed+repeat) → protected paths re-synced from the fixture (deletes agent-added files — anti reward-hack) → the `check` command's exit code decides pass; `check` always runs, regardless of outcome. An LLM error aborts the whole harness; Ctrl+C writes a partial report (`interrupted: true`, exit 1). Timeouts: `agent::bounded::run_bounded` sets `ToolCtx.cancel` then waits `cancel_grace` so `run_command` can kill its process group before giving up (no orphans) — the same helper wires `-p`'s Ctrl+C handling (exit 2). `AgentOutcome::Cancelled` is a loop-top cancel check, keeping the grace window fast
- `tasks/`: 12 zero-dependency cargo-crate fixtures (one dir each — `task.toml` + `fixture/`); all currently use `check = "cargo test"`, `protected = ["tests", "Cargo.toml"]`; `tasks/.gitattributes` pins the CRLF fixture byte-exact

## Notes
- `.superpowers/` is git-ignored scratch (subagent-driven-development progress ledger lives there); `git clean -fdx` destroys it
- No git remote — local-only repo
- Live smoke needs LM Studio running with a model loaded; `model = ""` in config auto-selects the server's first model
- `TtyApprover` is intentionally synchronous-blocking on `readline()`, not polled/async — an async stdin reader left running during the prompt would steal the Ctrl+C keystroke meant for the REPL's `select!`, orphaning the reader task
- Small local models (e.g. gemma-4-e4b) can loop on `finish_reason: length` for long outputs (summaries, etc.) until `max_turns` → exit 2 (spec §3 v1 blind spot, bounded, not a hang). Mitigate: raise `max_output_tokens` in `./.loco/config.toml` (git-ignored via `.loco/.gitignore` = `*`, so local-only) or load a larger model
- eval's integration tests (in `src/eval/mod.rs`, `src/eval/sandbox.rs`) are `#[cfg(unix)]`-gated (checks run via `sh -c`); the `tasks/` fixture crates are not cargo workspace members, so root `cargo build`/`test`/`clippy` ignore them
