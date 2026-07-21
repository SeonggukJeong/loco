# M16 계층 레포 notes 온보딩 하네스 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement hierarchical `.loco/notes/` onboarding with schema validation, certified mut-gate, NOTES_STALE finish latch, config flag, and exp_metrics — per `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md` (개정 2, Ready: Yes).

**Architecture:** Pure notes module → tool + config-gated registry → agent certified gate / dirty / finish order → optional grounding → metrics. Disk is SSOT; VERIFY mutation whitelist stays `edit_file`|`write_file` only.

**Tech Stack:** Rust edition 2024, existing loco crates only (no new deps), `scripts/exp_metrics.py` stdlib.

**Plan revision:** 개정 2 — plan review **2R Ready: Yes** (1R C1·I1–I6 + 2R Minor).  
리뷰: `…-review-1.md` · `…-review-2.md`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md` (개정 2) — **source of truth**
- No new crates without user approval
- User-facing CLI messages Korean; model-facing English
- `deny_unknown_fields` on config partials
- Marker strings must match Rust constants character-for-character in `exp_metrics.py` + `--selftest`
- `cargo test` · `cargo clippy --all-targets -- -D warnings` green after every task
- Default `repo_notes=true` for **product** REPL/config; **all legacy tests** use `repo_notes: false` + `Registry::guided(false)` unless the test is *about* notes
- Eval: only basename `tasks-real` may keep config `repo_notes`; **every other** eval `tasks_dir` (including tempdirs, `tasks`, `tasks-large`) forces `repo_notes = false` before Agent construction **and** that same flag is what `EffectiveConfig` snapshots
- Do **not** add `.loco/notes` to task `protected`
- Templates full text only on reject bodies; tool `doc()` ≤ ~2 lines
- **Never implement on `main`** — Task 0 creates `m16/repo-onboarding`
- **Read-before-edit (implementer):** before patching any existing source file, Read it in the same task

### Marker / extra strings (verbatim)

| Use | String |
|---|---|
| schema fail | `repo notes schema:` |
| mut gate reject | `repo notes mut gate:` |
| stale finish | `repo notes stale:` |
| update ok | `repo notes updated:` |
| STALE full first line template | `repo notes stale: you edited code but did not update notes for: {keys}. Call update_repo_notes on each listed key, then finish.` |
| transcript extra kind | `notes_bytes_max` (usize, max over certified note file lengths after scan/write) |

---

## File map

| Path | Role |
|---|---|
| `src/notes/mod.rs` | module root |
| `src/notes/schema.rs` | parse/validate root & dir notes |
| `src/notes/path.rs` | notes key normalize + ancestor keys + dirty key |
| `src/notes/templates.rs` | thrifty template constants |
| `src/notes/state.rs` | `NotesState` + start-scan certify |
| `src/tools/update_repo_notes.rs` | tool impl |
| `src/tools/mod.rs` | `guided(repo_notes: bool)` |
| `src/config.rs` | `repo_notes: bool` |
| `src/agent/mod.rs` | `Agent.repo_notes`, gate, finish, VERIFY whitelist |
| `src/agent/prompt.rs` | `system_prompt(..., repo_notes: bool)` |
| `src/ui/repl.rs` | `Registry::guided(config.repo_notes)` |
| `src/main.rs` | same |
| `src/eval/mod.rs` | force-false policy + guided(cfg) + EffectiveConfig |
| `src/eval/report.rs` | `repo_notes` on EffectiveConfig |
| `src/session.rs` | optional grounding (T4); `record_extra` consumer of bytes |
| `src/lib.rs` | `pub mod notes` |
| `scripts/exp_metrics.py` | MARKS + COLS + selftest |
| `CLAUDE.md` | flag policy |

### `Registry::guided(bool)` call-site inventory (exhaustive as of plan 1R)

Must be zero-arg-free after T2 (`rg 'Registry::guided\\('` → all `guided(true|false|expr)`):

| File | Notes |
|---|---|
| `src/main.rs` | production one-shot |
| `src/ui/repl.rs` | interactive REPL |
| `src/eval/mod.rs` | eval agent |
| `src/tools/mod.rs` | unit tests |
| `src/agent/mod.rs` | `make_guided_agent` + many tests |
| `src/agent/repetition.rs` | test helper |

---

### Task 0: Feature branch

- [ ] **Step 1: Create branch (do not work on main)**

```bash
git checkout main
git pull   # if remote has newer
git checkout -b m16/repo-onboarding
git status -sb
```

Expected: on `m16/repo-onboarding`, clean or only intentional WIP.

- [ ] **Step 2: No commit required** (or empty commit only if team requires branch push)

---

### Task 1: Notes schema + path mapping (pure)

**Files:**
- Create: `src/notes/mod.rs`, `src/notes/schema.rs`, `src/notes/path.rs`, `src/notes/templates.rs`
- Modify: `src/lib.rs` — `pub mod notes;`

**Interfaces:**
- Produces:
  - `notes::path::{normalize_key, ancestor_keys, dirty_key, notes_fs_path, is_under_notes_dir}`
  - `notes::schema::{validate_root, validate_dir, SchemaError}` / or unified `validate(key, text)`
  - `notes::templates::{ROOT_TEMPLATE, DIR_TEMPLATE}`
  - `ROOT_MAX_BYTES=1200`, `DIR_MAX_BYTES=800`, soft-reject: fence ≥1 OR non-blank lines ≥40

- [ ] **Step 1: Failing tests — full §3-1 vectors**

| code path | ancestors (구체→상위) | dirty |
|---|---|---|
| `Cargo.toml` | `[]` | `_root` |
| `build.rs` | `[]` | `_root` |
| `src/main.rs` | `["src"]` | `src` |
| `src/exec/job.rs` | `["src/exec","src"]` | `src/exec` |
| `crates/core/app.rs` | `["crates/core","crates"]` | `crates/core` |

Also: reject `.`/`..`/NUL/escape outside notes root; normalize `//`, `./`, `\`→`/`; strip trailing `.md` on keys; **`root` alone is not `_root`** (only `_root` is root key).

- [ ] **Step 2: `cargo test -q notes::` fail → implement path → pass**

- [ ] **Step 3: Schema tests then implement**

- valid root: summary 1–3 lines + routes ≥1 `- path → role`
- reject: empty routes, summary 0, summary ≥4, >1200 bytes, fence, ≥40 non-blank lines
- valid dir: role + (entrypoints OR notes bullets), ≤800 bytes
- extra `## do_not` allowed if size/soft-reject OK

- [ ] **Step 4: Templates** — copy thrifty bodies from spec §3-4 (full text for reject injection)

- [ ] **Step 5:** `cargo test -q notes::` + `cargo clippy --all-targets -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/notes src/lib.rs
git commit -m "feat(notes): schema parser, path mapping, thrifty templates (M16 T1)"
```

---

### Task 2: Config + tool + registry + **legacy tests opt-out** + eval force

**Files:**
- Create: `src/tools/update_repo_notes.rs`, `src/notes/state.rs` (scan/cert helpers OK here or T3)
- Modify: `src/config.rs`, `src/tools/mod.rs`, `src/main.rs`, `src/ui/repl.rs`, `src/eval/mod.rs`, `src/eval/report.rs`
- Modify: **all** `Registry::guided()` sites listed above; **`make_guided_agent` → false**

**Interfaces:**
- `Config.repo_notes: bool` default **`true`**
- `Registry::guided(repo_notes: bool)`
- Tool `update_repo_notes`; success `repo notes updated:`; schema err `repo notes schema:`; `is_mutating() == true`
- Eval force (apply **once** on the `Config` used for Agent **and** `EffectiveConfig`):

```rust
// Pin: basename of tasks_dir
// Some("tasks-real") => do not force (experiment arm uses .loco/config.toml)
// _ => cfg.repo_notes = false
// (covers tasks, tasks-large, tempfile names, anything else)
fn apply_eval_repo_notes_policy(tasks_dir: &Path, cfg: &mut Config) {
    let is_real = tasks_dir.file_name().and_then(|s| s.to_str()) == Some("tasks-real");
    if !is_real {
        cfg.repo_notes = false;
    }
}
```

Optional: if `cfg.repo_notes && !is_real` after explicit override attempt — not needed if force always wins for non-real.

- [ ] **Step 1: Config load test** — `repo_notes = false` in TOML applies; unknown key still denied

- [ ] **Step 2: Implement config field**

- [ ] **Step 3–4: Tool tests + implement `UpdateRepoNotes`**

- [ ] **Step 5: Change `guided` signature + fix every call site**

```text
main.rs / ui/repl.rs: Registry::guided(config.repo_notes)
eval: after apply_eval_repo_notes_policy, Registry::guided(cfg.repo_notes)
make_guided_agent: Config { repo_notes: false, max_turns, ..Default::default() }, Registry::guided(false)
All other agent/repetition/tools tests that mutate without notes: guided(false) + repo_notes: false
```

Gate: `rg 'Registry::guided\(\)'` finds **zero** zero-arg calls.

- [ ] **Step 6: Eval policy + EffectiveConfig**

- Implement policy on a **clone**: `let mut cfg = config.clone(); apply_eval_repo_notes_policy(tasks_dir, &mut cfg);` then Agent + registry + EffectiveConfig all use `cfg` (today `run_eval` takes `&Config`)  
- `EffectiveConfig { ..., repo_notes: cfg.repo_notes }` from **same** post-policy cfg
- Unit test: tempdir tasks_dir → EffectiveConfig.repo_notes == false even if Default is true  
- Unit test: path ending in `tasks-real` does not force (can set true via config and see snapshot true)

- [ ] **Step 7: tools test** — guided(true) has 7 names including `update_repo_notes`; guided(false) has 6 and not that name

- [ ] **Step 8: full `cargo test` + clippy** — suite green **before** T3 gate (mutations still unrestricted because gate not wired)

- [ ] **Step 9: Commit**

```bash
git commit -m "feat(notes): update_repo_notes tool + repo_notes config + registry (M16 T2)"
```

---

### Task 3: Agent — `repo_notes` field, certified gate, dirty, finish, VERIFY whitelist, bytes extra

**Files:**
- Modify: `src/agent/mod.rs`, `src/agent/prompt.rs`
- Use: `src/notes/state.rs`, session `record_extra` if available

**Interfaces:**
- `Agent.repo_notes: bool` set in `Agent::new` from `config.repo_notes` (not inferred only from registry)
- `prompt::system_prompt(tool_docs, root, repo_notes: bool)` — 2–3 sentence pointer **only if** `repo_notes`
- `NotesState` in `run()` when `self.repo_notes`:
  - start-scan `.loco/notes` → certified + update `bytes_max` → **always** `session.record_extra("notes_bytes_max", bytes_max)` once after scan (0 if empty)
  - on each success notes tool: cert, dirty.remove(exact key), recompute bytes_max, **`session.record_extra("notes_bytes_max", …)` again**
  - on success code edit/write: dirty.insert(dirty_key)
- Prefer **all** guided mutator unit tests via `make_guided_agent` (false-paired). Avoid ad-hoc `Agent::new(..., Config::default(), guided(false))` mismatches.
- **Mut-gate (hard order):** after args.tool salvage, for `edit_file`|`write_file`:
  1. if path under `.loco/notes/**` → tool_result error → continue (**no** preview/approve/dispatch)
  2. if `self.repo_notes && !gate_ok(certified)` → tool_result starting with `repo notes mut gate:` + template → continue (**no** preview/approve/dispatch)
  3. else existing preview → approve → dispatch  
  Gate failures are **every time** (not once-latch). Mid-run shell overwrite of certified files does not revoke cert (1차 residual).
- **VERIFY whitelist** replace bare `is_mutating()` arm with `edit_file`|`write_file` only (snippet as before)
- Finish: VERIFY → NOTES_STALE (only if `self.repo_notes`) → accept; STALE body exact prefix `repo notes stale:`
- Flag false: no SYSTEM pointer, no start-scan, no mut-gate, no STALE

- [ ] **Step 1: Tests first**

Legacy: `make_guided_agent` still green (false).  
New:
- A/B finish scenarios (spec) with `repo_notes: true` + scripted notes updates  
- notes update after green test does **not** set `mutated_since_verify`  
- gate blocks edit without cert; root-only `Cargo.toml` with only `_root`; `src/x.rs` needs `src`  
- `repo_notes: false` → edit without notes succeeds; system prompt lacks notes pointer substring  
- mut-gate path never requires Approver (use NonInteractive/Auto and assert no approve for gated reject — or count previews)

- [ ] **Step 2: `Agent.repo_notes` + prompt signature + initial_history**

- [ ] **Step 3: Gate before preview + finish + whitelist + NotesState + record_extra**

- [ ] **Step 4: full cargo test + clippy**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(agent): notes certified mut-gate, stale finish, VERIFY whitelist (M16 T3)"
```

---

### Task 4 (optional): `[repo_notes]` grounding

**Out of effect claim.** Skip unless shipping.

- Marker `[repo_notes] ` (note trailing space)  
- Keep-latest strip: **suffix after `</tool_result>` only**, same discipline as status (do **not** reuse status indent blindly — either generalize strip helper or duplicate with this marker)  
- Inject only on notes update success / mut-gate reject (short)

- [ ] Implement + tests or **explicit skip commit message** “T4 deferred”

---

### Task 5: exp_metrics

**Files:** `scripts/exp_metrics.py`

```python
"notes_schema_reject": "repo notes schema:",
"notes_mut_gate": "repo notes mut gate:",
"notes_stale_finish": "repo notes stale:",
"notes_updates": "repo notes updated:",
```

- COLS: `notes_bytes_max` — prefer transcript extra `notes_bytes_max`; else max parsed from success lines; flag-off / missing → `-`
- `notes_offtool`: **deferred** (optional post-M16); do not block T5
- selftest: all four MARKS + numeric `notes_bytes_max` from fixture with extra or success line

- [ ] Step 1–3: selftest fail → implement → pass  
- [ ] Commit: `feat(metrics): notes markers and notes_bytes_max (M16 T5)`

---

### Task 6: Docs

- `CLAUDE.md`: `repo_notes` default true; eval non-`tasks-real` forced false; tool name; pointer to spec §5  
- `docs/experiments/2026-07-21-m16-repo-onboarding/README.md` stub (pre-reg TODO)  
- Update handoff if branch base commit changes

- [ ] Commit: `docs(m16): CLAUDE flag policy + experiment stub (M16 T6)`

---

### Task 7: Verify gates (no GPU)

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify          # must run with repo_notes false via policy
cargo run -- eval tasks-large --verify
cargo run -- eval tasks-real --verify     # fixtures if present
python3 scripts/exp_metrics.py --selftest
rg 'Registry::guided\(\)' src/   # expect no zero-arg
```

- [ ] Fix if red  
- [ ] Stop — **no** 51×2 GPU without PROTOCOL pre-registration

---

## Spec coverage checklist

| Spec area | Task |
|---|---|
| schema + soft-reject + full path vectors | T1 |
| tool + markers + is_mutating | T2 |
| guided(bool) all sites + legacy false | T2 |
| eval force + EffectiveConfig honesty | T2 |
| Agent.repo_notes + SYSTEM flag matrix | T3 |
| certified gate before preview | T3 |
| VERIFY whitelist + finish order | T3 |
| notes_bytes_max record_extra | T3→T5 |
| optional grounding | T4 |
| exp_metrics | T5 |
| docs | T6 |
| verify | T7 |
| branch | T0 |
| GPU | out |

## Plan review history

| R | Result |
|---|---|
| 1R | Ready: No — C1 legacy tests, I1 call sites, I2 eval force, I3 Agent field, I4 gate before preview, I5 bytes extra, I6 branch |
| 개정 1 | 1R 전건 반영 |
| 2R | **Ready: Yes** — C0 · I0 · Minor 2 (start-scan extra · make_guided_agent pairing) |
| 개정 2 | 2R Minor + clone 정책 한 줄 |

---

## Execution handoff

After plan **2R Ready: Yes**, run SDD on branch `m16/repo-onboarding` from Task 0.

1. **Subagent-Driven (recommended)**  
2. **Inline Execution**
