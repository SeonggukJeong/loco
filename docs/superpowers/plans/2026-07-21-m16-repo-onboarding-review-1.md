# M16 plan review 1R

- Ready: **No**
- Date: 2026-07-21
- Reviewer: independent expert pass (plan)

## Summary

The plan tracks the 개정-2 design’s PR shape (pure notes → tool+flag → agent gate/stale → optional grounding → metrics → docs → verify, GPU deferred) and freezes the right marker strings, VERIFY whitelist snippet, finish order, and root-file vectors. It is **not Ready** for blind SDD: after Task 3, `Config::default().repo_notes = true` plus `Registry::guided(true)` will mut-gate every existing agent/eval scripted mutation test, and the plan never tells the implementer to opt those tests out. Production `guided()` call-site inventory is incomplete (`ui/repl.rs`), `Agent` is not told to store `repo_notes`, eval “force false” is underspecified for temp-dir unit tests and EffectiveConfig snapshotting, and `notes_bytes_max` recording into session extras is not wired in an implementation step. No Critical *design* mismatch on markers/finish order/whitelist intent — the blockers are SDD-execution holes that can ship a weakened gate or a broken main suite.

## Spec coverage matrix (design § → plan task, or GAP)

| Design contract | Plan task | Status |
|---|---|---|
| §3-1 layout + mapping vectors + normalize reject | T1 | Covered |
| §3-2 schema + soft-reject + caps + extra `##` | T1 | Covered |
| §3-3 `update_repo_notes`, markers, `is_mutating`, no list tool | T2 | Covered |
| §3-3 VERIFY whitelist (edit\|write only) | T3 | Covered (snippet matches real ~642 site) |
| §3-3 forbid edit/write into `.loco/notes/**` | T3 | Covered |
| §3-4 thrifty: templates reject-only; short `doc()`; SYSTEM pointer flag-scoped | T1–T3 | Covered (pointer step thin) |
| §3-4 control matrix (flag off = no tool / no pointer / no gate) | T2–T3 | Covered in intent |
| §3-5 certified set (start-scan + tool only; not bare disk) | T3 (+ T2 state helpers) | Covered |
| §3-5 root-file special case + mut-gate prefix | T3 tests | Covered |
| §3-5 gate every time (not once-latch) | T3 | **Weak** — implied, not stated |
| §3-6 dirty exact-key clear + NOTES_STALE body + finish VERIFY→STALE | T3 | Covered (A/B tests named) |
| §3-7 optional `[repo_notes]` grounding | T4 optional | Covered |
| §3-8 `repo_notes` default true; eval synthetic false; EffectiveConfig | T2 | **Partial** — force policy incomplete |
| §4 protected / H7: do **not** list notes | Global + checklist | Covered (no code) |
| §5-3 MARKS strings + `notes_bytes_max` + selftest | T5 | Markers OK; **bytes wiring GAP** |
| §5-3 `notes_offtool` (optional label) | — | GAP (optional; Minor) |
| §5-3 mechanism-alive / ε / DQ | — | Correctly out of code plan (GPU/pre-reg) |
| §9 PR order | T1–T7 | Matches collapsed PR plan |
| GPU control/treatment | T7 stop | Explicitly deferred — OK |

## Issues

### C1. Existing mutation tests will hit mut-gate after T3; plan never remediates

- **Severity:** Critical  
- **Section/Task:** T2 Step 5–6 · T3 Step 1–5 · Global Constraints  
- **Description:** Product default is `repo_notes=true` (`Config::default` after T2). Agent tests build with `Config { max_turns, ..Default::default() }` and `Registry::guided()` via `make_guided_agent` (`src/agent/mod.rs` ~960–962) and many direct call sites; eval integration tests use `Config::default()` with a **tempdir** as `tasks_dir` (`src/eval/mod.rs` `pass_flow_syncs_protected_before_check` etc.). Those scripts call `write_file`/`edit_file` with empty `.loco/notes` and no `update_repo_notes`. After T3’s certified gate, every such edit is rejected (`repo notes mut gate:`) → finishes never accept / pass rates collapse → mass red suite.  
- **Why:** A subagent following T3 alone has no written step to set `repo_notes=false` + `guided(false)` on **legacy** tests (only new A/B notes tests are described). Recovery paths that weaken the gate, default the flag to false in product, or skip gate in test builds violate the design.  
- **Suggestion:** Explicit T2/T3 steps:
  1. Change `make_guided_agent` (and all non-notes agent tests) to `Config { repo_notes: false, .. }` and `Registry::guided(false)`.
  2. Same for eval unit tests that run a real agent against scripted mutations: pass `Config { repo_notes: false, ..Default::default() }`.
  3. Only notes-scenario tests use `true` + pre-cert or scripted `update_repo_notes`.
  4. Optionally: eval force-false for **any** tree whose basename is not `tasks-real` (see I2), so tempdir harness tests cannot silently run with default true.

### I1. Incomplete production `Registry::guided` call-site inventory

- **Severity:** Important  
- **Section/Task:** T2 Step 5  
- **Description:** Plan lists `main.rs` and `eval/mod.rs` only. Live code also calls `Registry::guided()` in **`src/ui/repl.rs:94`** (interactive REPL), plus `src/agent/mod.rs` (many tests), `src/agent/repetition.rs`, `src/tools/mod.rs` tests. After signature change to `guided(repo_notes: bool)`, omitting `repl.rs` fails `cargo clippy --all-targets` / binary build even if unit tests in tools/agent are fixed.  
- **Why:** SDD agents often edit only listed paths; clippy eventually catches it, but the plan’s “replace all call sites” list is factually wrong today.  
- **Suggestion:** Replace step text with an exhaustive inventory: `main.rs`, `ui/repl.rs`, `eval/mod.rs`, `tools/mod.rs` tests, `agent/mod.rs` (+ helpers), `agent/repetition.rs`. Require `rg 'Registry::guided\\('` clean of zero-arg form before commit.

### I2. Eval `repo_notes=false` force policy is under-specified (path + EffectiveConfig)

- **Severity:** Important  
- **Section/Task:** T2 Step 6  
- **Description:**
  1. “if tasks_dir is tasks/ or tasks-large/” does not define matching (basename vs path contains; absolute `…/loco/tasks`; trailing slash; `tasks-real` sibling).
  2. Eval unit tests use **anonymous tempdirs** as `tasks_dir` — basename force never fires → default true remains (feeds C1).
  3. `EffectiveConfig` is built once at batch end from the top-level `config` (`eval/mod.rs` ~142–150); `run_once` only clones for H1/H2 overrides. If force is applied only inside `run_once` and not on the config snapshotted into `report.json`, EffectiveConfig lies about the arm.  
- **Why:** Design §3-8 / §5-5 require synthetic regression false and an honest EffectiveConfig snapshot for experiment arms.  
- **Suggestion:** Pin algorithm, e.g. `match tasks_dir.file_name(): Some("tasks-real") => leave cfg; _ => cfg.repo_notes = false` **or** force false for `tasks`|`tasks-large` basenames **and** document that all other eval callers must pass false in Config (and do so in-repo tests). Apply force **once** on the config object used for both `Agent::new` / `guided(...)` and `EffectiveConfig { repo_notes: config.repo_notes, ... }`. Keep optional stderr warning when true on non-tasks-real (design R2-4).

### I3. Agent does not store `repo_notes`; gate/SYSTEM/finish have no flag source in the plan

- **Severity:** Important  
- **Section/Task:** T3 Interfaces · Step 2–4  
- **Description:** `Agent::new` currently snapshots temperature / max_turns / context_tokens etc. from `Config` but **not** a free-form config handle (`src/agent/mod.rs` ~155–182). Plan says “on run start if `repo_notes`” and “SYSTEM pointer only if `repo_notes`” without a step to add `repo_notes: bool` (or equivalent) on `Agent` and set it in `new`. Registry presence of `update_repo_notes` is a brittle proxy (and breaks if tests pass mismatched registry/config). `initial_history` → `prompt::system_prompt(docs, root)` has no flag parameter today (`src/agent/prompt.rs:11`).  
- **Why:** Blind implementer may hardcode gate always-on, always inject SYSTEM (control contamination), or only check registry inconsistently with §3-4 matrix.  
- **Suggestion:** T3 Step 2 checklist: (1) `Agent.repo_notes: bool` from `config.repo_notes`; (2) `system_prompt(..., repo_notes: bool)` or append pointer in `initial_history`; (3) mut-gate / NOTES_STALE / start-scan only when `self.repo_notes`; (4) unit test: flag false → no pointer substring, no gate on edit.

### I4. Mut-gate insertion point left as “if possible”

- **Severity:** Important  
- **Section/Task:** T3 Interfaces (“before approval if possible”)  
- **Description:** Design §3-5: reject **before** approval/preview when practical. Real loop order is Action event → `preview` → `approver.approve` → `dispatch` (`agent/mod.rs` ~512–597). Soft wording lets SDD put the gate after AutoApprover/TtyApprover, causing interactive users to confirm a write that is then rejected for missing notes.  
- **Why:** UX + wasted turns; also gate denials should still feed a tool_result body with `NOTES_MUT_GATE_MARK` + template without calling `dispatch`.  
- **Suggestion:** Hard requirement: for `edit_file`|`write_file`, after args.tool salvage and notes-path ban, **before** `gate_preview`, if `repo_notes && !gate_ok` → push tool result error (same shape as other rejections), count turn, `continue` — no preview, no approve, no dispatch. State “every failure, no once-latch.”

### I5. `notes_bytes_max` session/transcript wiring missing from implementation tasks

- **Severity:** Important  
- **Section/Task:** T3 Interfaces (`bytes_max`) · T5  
- **Description:** Design §5-3 (개정 2): required column; max over keys of file len after last cert write; source preferred as session transcript **extra** on success `update_repo_notes` and **start-scan**. Plan T5 says “parse from transcript extras if present else max from success lines” but T2/T3 never step `session.record_extra("notes_bytes_max", …)` (or agreed kind/name) after scan/update. Flag-off `"-"` is stated for metrics only.  
- **Why:** Metrics PR lands with a column that is always `"-"` or heuristic-only; mechanism sizing unusable without a second PR.  
- **Suggestion:** T3 step: maintain `NotesState.bytes_max`; on start-scan and each successful notes write, `session.record_extra` with a fixed kind (document the exact kind string next to MARKS). T5: prefer that extra; fallback to parsing `repo notes updated:` lines; selftest both paths.

### I6. Feature branch creation absent from the plan body

- **Severity:** Important (SDD safety)  
- **Section/Task:** Global / Task 1 commit · (handoff-only today)  
- **Description:** Commits are instructed from T1 onward with no `git checkout -b m16/repo-onboarding`. Branch creation lives only in `2026-07-21-m16-sdd-handoff.md`, which a plan-only subagent may never open. Default `repo_notes=true` mid-stack on **main** would poison local `eval tasks/` until T2 force lands correctly.  
- **Why:** Plan self-describes as the SDD task list; handoff is optional context.  
- **Suggestion:** Task 0 / T1 Step 0: create branch from main; never implement on main. Restate eval false force as a hard gate before any default-true commit is considered mergeable.

### M1. Gate “every time” / certified mid-run residual not restated

- **Severity:** Minor  
- **Section/Task:** T3  
- **Description:** Design once-latch only for NOTES_STALE / VERIFY; mut-gate is every attempt; mid-run shell overwrite keeps cert (1차 residual). Plan omits these as explicit bullets.  
- **Suggestion:** One line under T3 Interfaces.

### M2. `notes_offtool` heuristic omitted

- **Severity:** Minor  
- **Section/Task:** T5 / design §5-3  
- **Description:** Optional analysis label (disk schema-OK ∧ `notes_updates==0`). Not required for Ready if documented as deferred.  
- **Suggestion:** T5 optional step or explicit “deferred post-M16.”

### M3. Path normalize vectors incomplete in test sketch

- **Severity:** Minor  
- **Section/Task:** T1 Step 1  
- **Description:** Spec also requires `build.rs` root-only, reject `.`/`..`/escape/NUL, `//` and `./` normalize, `\`→`/`, strip trailing `.md`, reject bare `root` as alias (`_root` only). Plan sketch lists four happy paths + “reject `..` / escape” only.  
- **Suggestion:** Expand table to full §3-1 vector set.

### M4. T4 grounding optional but strip pattern underspecified

- **Severity:** Nit  
- **Section/Task:** T4  
- **Description:** Status strip is specialized (`remove_status_note` + 9-space CONT_INDENT). Generic “like status” may copy the wrong helper. Acceptable while T4 is skippable / non-claim.  
- **Suggestion:** If shipping T4, either generalize strip or duplicate the suffix-only contract with marker `[repo_notes] `.

### M5. Placeholder “agent/prompt.rs (or wherever)”

- **Severity:** Nit  
- **Section/Task:** File map  
- **Description:** Prompt lives at `src/agent/prompt.rs` — known. Drop the hedge.

## Non-issues / OK

- **VERIFY whitelist change** at ~642: today the `is_mutating()` arm is only reached for non-`run_command` mutators (`run_command` is handled in the preceding branch). Adding `update_repo_notes` with `is_mutating=true` **would** re-arm VERIFY; plan’s edit\|write match is correct and behavior-equivalent for the current six-tool set. `status.record_mutation` / FINISH_NUDGE `MutationOk` already whitelist edit\|write — leave them.
- **Marker strings** match design §5-3 / STALE body from 개정 2 character-for-character.
- **Finish order** VERIFY → STALE → accept and tests A/B match design §3-6 / R2-6.
- **Templates reject-only** and short `doc()` are stated; control SYSTEM pointer off when flag false is stated.
- **No new crates**; ε/DQ/GPU correctly outside implementation tasks; pre-reg called out at T7 stop.
- **Protected / H7:** explicit non-listing is correct; no extra exception code needed.
- **schema tool enum** derives from `registry.names()` (`schema_tool_names`) — registering the tool is enough; no separate enum task required.
- **T1 before T2 before T3** ordering is sound; tool without gate for an intermediate commit is acceptable on a feature branch if eval force-false is correct.
- **Optional T4** out of effect claim matches design PR4.
- **`notes_offtool` as optional** — not a Ready blocker if mechanism-alive still uses the four MARKS.

## Verdict

**Ready: No** — Critical **1** (C1), Important **6** (I1–I6), Minor **3**, Nit **2**.

### Minimum to Ready Yes

1. **C1:** Mandate legacy agent + eval scripted tests use `repo_notes=false` / `guided(false)`; only notes tests enable the feature.  
2. **I1:** Full `guided(bool)` call-site list including `ui/repl.rs`.  
3. **I2:** Precise eval force policy + apply to EffectiveConfig snapshot; cover tempdir unit tests.  
4. **I3:** `Agent.repo_notes` + SYSTEM pointer gated on that field.  
5. **I4:** Mut-gate **before** preview/approve; no once-latch.  
6. **I5:** `record_extra` (or fixed encoding) for `notes_bytes_max` in T3, consumed in T5.  
7. **I6:** Branch-create step in the plan itself.

After those land in the plan text, re-review should be a short pass; no design reopen required.
