# M16 계층 레포 notes 온보딩 하네스 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement hierarchical `.loco/notes/` onboarding with schema validation, certified mut-gate, NOTES_STALE finish latch, config flag, and exp_metrics — per `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md` (개정 2, Ready: Yes).

**Architecture:** Pure notes module (schema/path/templates) → tool + config-gated registry → agent certified gate / dirty / finish order → optional grounding → metrics. Disk is SSOT; VERIFY mutation whitelist stays `edit_file`|`write_file` only.

**Tech Stack:** Rust edition 2024, existing loco crates only (no new deps), `scripts/exp_metrics.py` stdlib.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md` (개정 2) — **source of truth**
- No new crates without user approval
- User-facing CLI messages Korean; model-facing English
- `deny_unknown_fields` on config partials
- Marker strings must match Rust constants character-for-character in `exp_metrics.py` + `--selftest`
- `cargo test` · `cargo clippy --all-targets -- -D warnings` green after every task
- Default `repo_notes=true` for REPL; **eval `tasks/` / `tasks-large` must use false** (document + harness defaults below)
- Do **not** add `.loco/notes` to task `protected`
- Templates full text only on reject bodies; tool `doc()` ≤ ~2 lines

---

## File map

| Path | Role |
|---|---|
| `src/notes/mod.rs` | module root |
| `src/notes/schema.rs` | parse/validate root & dir notes |
| `src/notes/path.rs` | notes key normalize + ancestor keys + dirty key |
| `src/notes/templates.rs` | thrifty template constants |
| `src/notes/state.rs` | `NotesState`: certified, dirty, stale_nudged, bytes tracking |
| `src/tools/update_repo_notes.rs` | tool impl |
| `src/tools/mod.rs` | `guided(repo_notes: bool)` |
| `src/config.rs` | `repo_notes: bool` |
| `src/agent/mod.rs` | SYSTEM pointer, gate, finish order, VERIFY whitelist |
| `src/agent/prompt.rs` (or wherever `system_prompt` lives) | optional pointer injection |
| `src/session.rs` | optional `[repo_notes]` strip (Task optional) |
| `src/eval/mod.rs` | default false for non-tasks-real; EffectiveConfig |
| `src/eval/report.rs` | `repo_notes` on EffectiveConfig |
| `src/lib.rs` | `pub mod notes` |
| `scripts/exp_metrics.py` | MARKS + COLS |
| `CLAUDE.md` | commands + flag policy |

---

### Task 1: Notes schema + path mapping (pure)

**Files:**
- Create: `src/notes/mod.rs`, `src/notes/schema.rs`, `src/notes/path.rs`, `src/notes/templates.rs`
- Modify: `src/lib.rs` — `pub mod notes;`

**Interfaces:**
- Produces:
  - `notes::path::{NotesKey, normalize_key, ancestor_keys, dirty_key, is_notes_tool_path}`
  - `notes::schema::{validate_root, validate_dir, SchemaError}`
  - `notes::templates::{ROOT_TEMPLATE, DIR_TEMPLATE}`
  - Constants: `ROOT_MAX_BYTES=1200`, `DIR_MAX_BYTES=800`, soft-reject lines≥40 / fence

- [ ] **Step 1: Add module stubs and failing tests for path vectors**

In `src/notes/path.rs` (tests first in same file `#[cfg(test)]`):

```rust
// Expected from spec §3-1:
// Cargo.toml → ancestors empty, dirty = "_root"
// src/main.rs → ancestors ["src"], dirty "src"
// src/exec/job.rs → ["src/exec", "src"], dirty "src/exec"
// crates/core/app.rs → ["crates/core", "crates"], dirty "crates/core"
```

Implement `normalize_key`, `ancestor_keys(code_path: &str) -> Vec<String>`, `dirty_key(code_path: &str) -> String` per spec (root file → dirty `_root`).

- [ ] **Step 2: Run tests — expect fail then implement until pass**

```bash
cargo test -q notes::
```

Expected: FAIL then PASS for mapping vectors + reject `..` / escape.

- [ ] **Step 3: Schema tests then implement**

Cases:
- valid root with summary 1–3 lines + routes ≥1 bullet `- x → y`
- reject empty routes, summary 0 lines, summary 4+ lines
- reject >1200 bytes
- reject fence or ≥40 non-blank lines
- valid dir with role + entrypoints OR notes bullets
- extra `## do_not` allowed if size OK

- [ ] **Step 4: Templates constants**

```rust
pub const ROOT_TEMPLATE: &str = r##"## summary
...
"##;
pub const DIR_TEMPLATE: &str = r##"## role
...
"##;
```

Copy from spec §3-4 exactly enough for gate bodies.

- [ ] **Step 5: `cargo test -q notes::` + `cargo clippy --all-targets -- -D warnings`**

- [ ] **Step 6: Commit**

```bash
git add src/notes src/lib.rs
git commit -m "feat(notes): schema parser, path mapping, thrifty templates (M16 T1)"
```

---

### Task 2: Config flag + tool + registry wiring

**Files:**
- Create: `src/tools/update_repo_notes.rs`
- Create: `src/notes/state.rs` (certified scan helpers used by tool/agent)
- Modify: `src/config.rs`, `src/tools/mod.rs`, `src/main.rs`, `src/eval/mod.rs`, `src/eval/report.rs`
- Test: config load + tool unit tests + registry count

**Interfaces:**
- `Config.repo_notes: bool` default **true**
- `PartialConfig.repo_notes: Option<bool>`
- `Registry::guided(repo_notes: bool)` — if true append `UpdateRepoNotes`
- Tool name `"update_repo_notes"`
- Markers:
  - success prefix: `repo notes updated:`
  - schema fail: `repo notes schema:`
- `is_mutating() -> true`
- `preview`: short “write notes {key} ({n} bytes)”
- Forbid: n/a for this tool; agent forbids edit/write into notes (Task 3)

- [ ] **Step 1: Failing test — unknown key rejected by deny_unknown if typo; known key loads**

```rust
// config: repo_notes = false in TOML partial applies
```

- [ ] **Step 2: Implement config field + Default true + apply()**

- [ ] **Step 3: Failing tool tests**

```rust
// write _root valid content → file at root/.loco/notes/_root.md
// body starts with "repo notes updated:"
// invalid schema → Err containing "repo notes schema:" and ROOT_TEMPLATE
// path escape rejected
```

- [ ] **Step 4: Implement `UpdateRepoNotes` tool**

```rust
pub struct UpdateRepoNotes;
// name: update_repo_notes
// doc: "update_repo_notes(path, content): Replace hierarchical repo notes for key `path` (_root or dir). Keep entries short."
// is_mutating: true
```

Args: `{ "path": string, "content": string }` via serde.

- [ ] **Step 5: `Registry::guided(repo_notes: bool)`**

Replace all `Registry::guided()` call sites:
- `main.rs`: `Registry::guided(config.repo_notes)`
- `eval/mod.rs`: use cfg (see Step 6)
- tests: `Registry::guided(true)` or `false` as needed

Update test `guided_registry_has_all_six_tools` → seven when true / six when false.

- [ ] **Step 6: Eval default policy**

In eval run setup, after loading cfg:

```rust
// Spec §3-8 / §5-5: synthetic trees must run with repo_notes=false unless explicitly overridden.
// Recommended: if tasks_dir is tasks/ or tasks-large/, force cfg.repo_notes = false
//               if tasks-real/, leave config (experiment sets true/false per arm via .loco/config.toml)
```

Also log stderr one-liner if `repo_notes && !tasks_real` (optional R2-4).

- [ ] **Step 7: `EffectiveConfig` add `repo_notes: bool` + report.json test**

- [ ] **Step 8: cargo test + clippy**

- [ ] **Step 9: Commit**

```bash
git commit -m "feat(notes): update_repo_notes tool + repo_notes config + registry (M16 T2)"
```

---

### Task 3: Agent — certified gate, dirty, finish order, VERIFY whitelist

**Files:**
- Modify: `src/agent/mod.rs` (primary)
- Possibly: `src/agent/prompt.rs` for SYSTEM pointer when `config.repo_notes`
- Use: `src/notes/state.rs`

**Interfaces:**
- `NotesState` in `run()` scope:
  - `certified: BTreeSet<String>`
  - `dirty: BTreeSet<String>`
  - `stale_nudged: bool`
  - `bytes_max: usize`
- On run start if `repo_notes`: scan `.loco/notes`, validate, fill certified + bytes
- SYSTEM: append 2–3 sentence pointer **only if** `repo_notes` (spec §3-4)
- Before code `edit_file`/`write_file` dispatch (and **before** approval if possible):
  - if path under `.loco/notes` → error guide to update_repo_notes
  - else require gate §3-5
- On success `update_repo_notes`: cert insert, dirty remove exact key, update bytes
- On success code edit/write: dirty insert `dirty_key(path)`
- **VERIFY whitelist change** at ~642:

```rust
// BEFORE (bad for notes):
} else if self.registry.get(&turn.action.tool).is_some_and(|t| t.is_mutating()) {
    mutated_since_verify = true;
}
// AFTER:
} else if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
    mutated_since_verify = true;
    unreleased_due_to_pipe = false;
}
// status.record_mutation already edit|write only — keep
```

- Finish order when summary present:

```text
1) if mutated_since_verify && !verify_nudged → VERIFY_* (existing)
2) else if repo_notes && !dirty.is_empty() && !stale_nudged → NOTES_STALE
3) else Finished
```

`NOTES_STALE_NUDGE` constant (exact):

```text
repo notes stale: you edited code but did not update notes for: {keys}. Call update_repo_notes on each listed key, then finish.
```

Mut-gate reject prefix: `repo notes mut gate:`

- [ ] **Step 1: Unit/integration tests (scripted agent) for scenarios A/B from spec §3-6**

Test A: mut without verify + dirty → finish → VERIFY text; finish → STALE text; finish → Finished  
Test B: mut + run_command ok + dirty → finish → STALE only  
Test: notes update after green test does **not** set mutated_since_verify  
Test: gate blocks edit without certified root  
Test: root-only `Cargo.toml` edit allowed when only `_root` certified  
Test: `src/x.rs` needs certified `src` (or deeper) + `_root`

- [ ] **Step 2: Implement NotesState + wire into Agent::run**

- [ ] **Step 3: Implement gate + finish + whitelist**

- [ ] **Step 4: SYSTEM pointer gated**

- [ ] **Step 5: cargo test (focus agent + notes) + full suite + clippy**

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(agent): notes certified mut-gate, stale finish, VERIFY whitelist (M16 T3)"
```

---

### Task 4 (optional product): `[repo_notes]` grounding strip

**Files:**
- Modify: `src/session.rs`, maybe agent after notes update

**Out of effect claim** — skip if time; if implemented:

- Marker `[repo_notes] ` + keep-latest strip like status
- Inject only on update success / gate reject (short)
- Tests for strip + pack survival of task message

- [ ] **Step 1–N:** only if shipping grounding  
- [ ] **Commit:** `feat(session): optional repo_notes grounding strip (M16 T4)`

---

### Task 5: exp_metrics notes columns

**Files:**
- Modify: `scripts/exp_metrics.py`

**MARKS** (exact prefixes):

```python
"notes_schema_reject": "repo notes schema:",
"notes_mut_gate": "repo notes mut gate:",
"notes_stale_finish": "repo notes stale:",
"notes_updates": "repo notes updated:",
```

**COLS:** add after existing token cols (or at end before nav):  
`notes_bytes_max` — parse from transcript extras if present else max from success lines; flag-off runs `-`.

**selftest:** fixture transcript with each marker → process() asserts counts; `notes_bytes_max` numeric.

- [ ] **Step 1: Extend MARKS/COLS + selftest (fail first if asserted)**

```bash
python3 scripts/exp_metrics.py --selftest
```

- [ ] **Step 2: Implement counting in run_metrics/process**

- [ ] **Step 3: selftest ok**

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(metrics): notes markers and notes_bytes_max (M16 T5)"
```

---

### Task 6: Docs — CLAUDE.md + experiment stub

**Files:**
- Modify: `CLAUDE.md` (or `Claude.md` — match repo)
- Create: `docs/experiments/2026-07-21-m16-repo-onboarding/README.md` (stub pointing to pre-registration TODO)
- Optional: update `docs/m16-candidates.md` status → “spec Ready; plan exists”

Document:
- `repo_notes` config default true
- eval tasks/tasks-large **must** false
- control/treatment measurement protocol pointer to spec §5
- new tool name

- [ ] **Step 1: Edit docs**

- [ ] **Step 2: Commit**

```bash
git commit -m "docs(m16): CLAUDE flag policy + experiment stub (M16 T6)"
```

---

### Task 7: Verify gates + handoff (no GPU)

**Not** the GPU batch (needs pre-registration approval).

- [ ] **Step 1:**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify
cargo run -- eval tasks-large --verify
# tasks-real if fixtures present:
cargo run -- eval tasks-real --verify
python3 scripts/exp_metrics.py --selftest
```

Expected: all green; registry 6 tools when false.

- [ ] **Step 2: Commit any fixups**

- [ ] **Step 3: Stop** — GPU control/treatment requires PROTOCOL pre-registration (separate session). Do not run 51×2 without approval.

---

## Spec coverage checklist (self-review)

| Spec area | Task |
|---|---|
| schema + soft-reject + caps | T1 |
| path/dirty/root-file | T1, T3 |
| templates reject-only | T1–T3 |
| tool + is_mutating + markers | T2 |
| config flag + guided(bool) | T2 |
| eval false policy + EffectiveConfig | T2 |
| certified set + gate | T3 |
| VERIFY whitelist | T3 |
| finish VERIFY→STALE | T3 |
| SYSTEM pointer flag-scoped | T3 |
| protected not listing notes | T3 docs (no code change if never listed) |
| grounding optional | T4 |
| exp_metrics | T5 |
| CLAUDE / experiment docs | T6 |
| verify gates | T7 |
| GPU batch | **out of this plan** (pre-reg) |

## Placeholder scan

No TBD steps; GPU explicitly deferred.

## Type/name consistency

- Tool: `update_repo_notes`
- Config: `repo_notes: bool`
- Marks: `repo notes schema:`, `repo notes mut gate:`, `repo notes stale:`, `repo notes updated:`
- Dirty clear: exact key only

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-21-m16-repo-onboarding.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task + review between tasks  
2. **Inline Execution** — this session with executing-plans checkpoints  

Which approach?
