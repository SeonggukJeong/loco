# M16 plan review 2R

- Ready: **Yes**
- Date: 2026-07-21
- Reviewer: independent expert pass (plan re-review after 개정 1)

## Summary

Plan 개정 1 closes every 1R Critical/Important SDD blocker with concrete steps, code pins, and inventory — not hand-waves. Task 0 branch, legacy test opt-out (`make_guided_agent` + dual `repo_notes: false` / `guided(false)`), exhaustive `guided(bool)` call sites (including `ui/repl.rs`), basename `tasks-real`-only eval force with EffectiveConfig honesty, `Agent.repo_notes` + flag-scoped SYSTEM, mut-gate **before** preview/approve, and `record_extra("notes_bytes_max")` are all present in the plan body. Residual notes are operational polish; none block correct blind SDD or design fidelity. **Ready: Yes.**

## 1R disposition

| ID | 1R topic | Status | Where latched in 개정 1 |
|---|---|---|---|
| **C1** | Legacy mutation tests vs default `true` + mut-gate | **Addressed** | Global Constraints (legacy tests false); T2 Step 5 (`make_guided_agent` + all non-notes mutators: `repo_notes: false` + `guided(false)`); T3 Step 1 “Legacy still green”; eval force covers tempdir harness paths |
| **I1** | Incomplete `guided()` call sites | **Addressed** | File map + exhaustive inventory (`main`, `ui/repl`, `eval`, `tools` tests, `agent/mod`, `repetition`); T2 Step 5 + `rg 'Registry::guided\(\)'` zero-arg gate; T7 re-check |
| **I2** | Eval force path + EffectiveConfig | **Addressed** | Global + T2 `apply_eval_repo_notes_policy`: only basename `tasks-real` leaves flag; else force false; same cfg for Agent/registry and `EffectiveConfig`; unit tests tempdir vs `tasks-real` |
| **I3** | `Agent.repo_notes` / SYSTEM source | **Addressed** | File map; T3 Interfaces: field from `config.repo_notes` (not registry-only); `system_prompt(..., repo_notes)`; flag-false matrix; test no pointer + edit without notes |
| **I4** | Mut-gate before approval | **Addressed** | T3 hard order: salvage → notes-path ban → mut-gate tool_result continue → else preview/approve/dispatch; every time / no once-latch; Approver-not-required test |
| **I5** | `notes_bytes_max` wiring | **Addressed** | Global extra kind table; T3 success path `session.record_extra` kind `notes_bytes_max`; T5 prefer extra then success lines; selftest |
| **I6** | Feature branch | **Addressed** | Global “Never implement on main”; **Task 0** `m16/repo-onboarding`; handoff points at T0 |

### 1R Minors / Nits

| ID | Status |
|---|---|
| M1 gate every-time / cert residual | **Addressed** (T3 Interfaces bullets) |
| M2 `notes_offtool` | **Addressed** (T5 deferred, explicit) |
| M3 full §3-1 vectors | **Addressed** (T1 Step 1 table + normalize rules) |
| M4 T4 strip discipline | **Addressed** (suffix-only / don’t reuse status blindly) |
| M5 prompt path hedge | **Addressed** (`src/agent/prompt.rs` named) |

## Spot-check (plan text actually contains…)

| Check | Present? |
|---|---|
| Task 0 branch `m16/repo-onboarding` | Yes |
| `make_guided_agent` → `repo_notes: false` + `guided(false)` | Yes (T2 Step 5) |
| Eval basename policy (`tasks-real` only exempt) | Yes (`apply_eval_repo_notes_policy` pin) |
| EffectiveConfig from same post-policy cfg | Yes (T2 Step 6 + unit tests) |
| `Agent.repo_notes` + `system_prompt(..., repo_notes)` | Yes (T3) |
| Gate before preview/approve/dispatch | Yes (T3 hard order) |
| `record_extra` kind `notes_bytes_max` | Yes (Global table + T3) |
| `ui/repl.rs` in guided inventory | Yes |

## Issues (remaining only)

### R2-M1. Start-scan may update `bytes_max` without `record_extra`

- **Severity:** Minor  
- **Section/Task:** T3 Interfaces (start-scan vs success bullet)  
- **Description:** Design §5-3 wants extras on **start-scan and** successful cert write. Plan records extra explicitly on success notes tool; start-scan only “update `bytes_max`.” Cold-start eval (empty scan) is unaffected. Product multi-session reuse with no mid-run notes write could leave transcript without `notes_bytes_max` until first tool update.  
- **Suggestion:** One clause: after start-scan, if any key certified, `session.record_extra("notes_bytes_max", …)` once (or always write `0` / max). Not an SDD blocker for tasks-real cold start.

### R2-M2. Dual false pairing only enforced by prose + full suite

- **Severity:** Minor (residual of C1)  
- **Section/Task:** T2 Step 5  
- **Description:** Zero-arg `guided()` is machine-gated by `rg`; mismatched `Config::default()` (`repo_notes: true`) + `guided(false)` is not. After T3 that combo dead-ends (gate on, no notes tool). Plan requires both false for legacy mutators; T3 full `cargo test` catches misses.  
- **Suggestion:** Prefer routing all guided agent tests through `make_guided_agent` (already false-paired); avoid ad-hoc `Agent::new` + `Default::default()` for mutators. Optional comment in helper.

### R2-N1. `run_eval` takes `&Config` today — implementer must clone

- **Severity:** Nit  
- **Section/Task:** T2 Step 6  
- **Description:** Policy is `fn(..., cfg: &mut Config)`. Real `run_eval` signature uses shared `&Config`; correct impl is `let mut cfg = config.clone(); apply(...);` then Agent + EffectiveConfig from `cfg`. Obvious from types; no plan bug.  
- **Suggestion:** Optional one-liner in Step 6.

## Non-issues / OK

- VERIFY whitelist still pinned to `edit_file`|`write_file`; notes `is_mutating` true without re-arming VERIFY.
- Finish VERIFY → NOTES_STALE → accept; STALE body and four MARKS match design character-for-character.
- Templates reject-only; control SYSTEM pointer off when flag false.
- Protected / H7: still “do not list notes.”
- T4 optional / out of effect claim; GPU deferred with PROTOCOL stop.
- T1→T2→T3 order: suite green after T2 (no gate yet), gate lands T3 with legacy already false — no mid-stack main-eval poison if Task 0 branch held.
- Marker table unifies Rust/Python contract for SDD.

## Spec coverage (delta only)

No new design gaps vs 1R matrix. `notes_offtool` remains deferred by plan (design optional). ε/DQ/GPU still correctly outside implementation tasks.

## Verdict

**Ready: Yes** — Critical **0**, Important **0**, Minor **2**, Nit **1**.

1R C1 and I1–I6 are fully addressed in plan text with implementable pins. Proceed to SDD on `m16/repo-onboarding` from Task 0; no further plan revision required before coding. Optional polish (start-scan `record_extra`, helper-only guided tests) can land during T3 without a 3R plan cycle.
