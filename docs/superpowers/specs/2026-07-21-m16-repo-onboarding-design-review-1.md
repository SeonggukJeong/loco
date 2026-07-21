# M16 design review 1R

- Ready: **No**
- Date: 2026-07-21
- Reviewer: independent expert pass

## Summary

The M16 design correctly chooses B2 hard gate over B3/C/D/E, freezes budget against M15 confounds, refuses M15 0/51 as control, and puts thrifty guidance on the failure path rather than a permanent context dump. That spine is sound and matches `m16-candidates.md` intent. However, several contracts are still open in ways that will make implementers diverge on behavior that **affects finish discipline, control purity, and success-criterion arithmetic**. The highest-impact gaps are: (1) whether `update_repo_notes` is `is_mutating` and therefore re-arms `mutated_since_verify` / VERIFY_NUDGE; (2) SYSTEM pointer + template placement vs flag-off control; (3) ε / “통과 과제” dual wording; (4) finish latch priority vs VERIFY; (5) root-level ancestor hole and protected-path wording. No silent integrity break of the check/oracle kind was found for the frozen `tasks-real` sample, but the design is not yet implementable without guessing on the agent-loop contracts.

## Issues

### I1. `update_repo_notes` × `is_mutating` × VERIFY_NUDGE re-arm (unspecified, high balloon risk)

- Severity: **Important**
- Section: §3-3, §3-6, §4 (VERIFY/FINISH), integration with `src/agent/mod.rs`
- Description: Spec classifies the tool as “notes 쓰기”, excludes it from the **code** mut gate, and says eval `AutoApprover` 통과 — but never pins `Tool::is_mutating()`. Today any successful dispatch with `is_mutating()==true` sets `mutated_since_verify = true` (`agent/mod.rs` ~642–644), independent of tool name. `finish_nudge` / `status.record_mutation` correctly special-case only `edit_file`|`write_file`, but VERIFY_NUDGE does not.
- Why it matters: If notes is mutating (the natural reading of “승인 게이트 통과”), the intended happy path **code mut → cargo test → update_repo_notes (stale clear) → finish** re-arms VERIFY after a clean test: finish hits VERIFY_NUDGE, needs another verification turn, then finish again. On a track already dominated by MaxTurns/Timeout (M15: 27+18/51), that is a large balloon and confounds “notes device vs finish-discipline tax.” If notes is non-mutating, interactive REPL never confirms writes and AutoApprover is never consulted — also fine, but then §3-3’s approval wording is misleading.
- Suggestion: Pin one contract in the spec table:
  1. `is_mutating() = true` (approval + AutoApprover), **and**
  2. `mutated_since_verify` / status mutation / FINISH_NUDGE events **whitelist** `edit_file`|`write_file` only (do not use bare `is_mutating()` for verification state), **and**
  3. unit test: notes update after a green `run_command` does **not** re-arm VERIFY_NUDGE; code edit still does.
  Explicitly state dirty/stale is orthogonal to verification.

### I2. Control purity: SYSTEM pointer and thrifty templates not flag-scoped

- Severity: **Important**
- Section: §0 #5, §3-4, §3-8
- Description: Two tensions:
  1. §0 says thrifty templates live in “툴 description + 거부 body”; §3-4 says SYSTEM is only a 2–3 sentence pointer and full templates appear **only on failure**. Tool `doc()` strings are injected into SYSTEM every turn via `system_prompt(tool_docs, …)`. Full templates in `doc()` are therefore **not** thrifty — they are permanent SYSTEM bloat, contradicting §3-4’s token strategy.
  2. §3-8 disables tool registration, mut gate, and stale on `repo_notes=false`, but does **not** say the SYSTEM pointer is suppressed. Control could still say “Fill notes before editing code” with no tool — a sham instruction confound, not a clean M15-like control.
- Why it matters: Control/treatment confounds invalidate the flag A/B. Permanent template-in-doc bloat also undermines the “small-model thrifty” claim that justifies hierarchical notes over C/D injection.
- Suggestion: Fix a single matrix:

  | Surface | flag on | flag off |
  |---|---|---|
  | SYSTEM pointer (2–3 sentences) | yes | **no** |
  | `update_repo_notes` in registry + schema enum | yes | **no** |
  | tool `doc()` | short signature only (≤~2 lines); **no full templates** | n/a |
  | full root/module templates | schema fail + mut-gate reject bodies only | n/a |
  | mut gate / NOTES_STALE | on | off |

  Amend §0 #5 to match §3-4 (templates = reject bodies, not tool description dumps).

### I3. ε and “통과 과제 수” dual success criterion is under-defined

- Severity: **Important**
- Section: §2-2, §5-4
- Description: Success is “과제 수준 통과 평균 ≥ ε = 1/17 **또는** 통과 과제 수 ≥ 1”. Under task-level mean of per-task pass rates with ×3, mean ≥ 1/17 ⇔ **≥ 3 total passes / 51**. The second disjunct depends on what “통과 과제” means:
  - any task with ≥1/3 passes → **one** lucky run meets success;
  - any task with 3/3 → much stronger;
  - M15 “전패/전승 과제” language = all-three same outcome.
  Spec also defers disqualification band restatement to pre-registration without writing the N=17 absolute (≥13 all-fail / all-pass) that M15 already froze.
- Why it matters: Implementers or pre-registration authors can honestly pick different ε interpretations; post-hoc choice is exactly what PROTOCOL forbids. The OR makes the **weaker** limb the real bar.
- Suggestion: In §2-2 pin, before any GPU time:
  - `task_mean_pass = mean_i (passed_count_i / repeats)` with repeats=3;
  - ε: `task_mean_pass ≥ 1/17` **or** (pick one and delete the other limb, or define) `|{i : passed_count_i ≥ 1}| ≥ 1` as an **explicitly weaker mechanism floor** and name it as such;
  - disqualification: copy M15 absolute: all-fail ≥ 13 or all-pass ≥ 13 for N=17 (formula `0.98·√N` + absolute), and state whether disqualification applies to each arm independently.
  Prefer a **single** primary criterion for “최소 들어 올림” and demote the other to secondary reporting.

### I4. Finish priority: NOTES_STALE vs VERIFY_NUDGE not ordered

- Severity: **Important**
- Section: §3-6
- Description: Spec says document NOTES_STALE “with” VERIFY_NUDGE and fix order in code comments+tests later. Current finish branch only knows VERIFY (summary present ∧ `mutated_since_verify` ∧ !verify_nudged). When both dirty notes and unverified code mut apply, first-finish behavior (which latch fires, how many forced extra turns) is undefined.
- Why it matters: `notes_stale_finish` counts, turn tax, and interaction with I1 change with order. Two implementers can ship different once-latch sequences (VERIFY then STALE vs STALE then VERIFY vs merged message) and still claim compliance.
- Suggestion: Publish an ordered table now, e.g. on summary-bearing `finish`:
  1. RepetitionStop (pre-existing)
  2. VERIFY_NUDGE / VERIFY_NUDGE_PIPE (code verification; once)
  3. NOTES_STALE_NUDGE (once)
  4. accept finish  
  Rationale: verification is load-bearing for `passed`; notes hygiene is secondary. Add a multi-latch integration test that asserts the sequence and that both can fire on consecutive finish attempts without infinite reject.

### I5. Eval / REPL flag wiring for `tasks/` vs `tasks-real` is not a single contract

- Severity: **Important**
- Section: §3-8, §5-5, §9 PR2/PR5
- Description: Product default “true 권고”; `tasks/`·`tasks-large` get “게이트 no-op **또는** false”. Those are different behaviors (tool visible vs absent; schema enum size; SYSTEM text). Eval today always `Registry::guided()` with no config knob (`eval/mod.rs` ~190). PR2 wants flag-off 미등록 but PR5 adds the config key later — workable, but the **eval policy** (how control/treatment and synthetic regression set the bit) is missing.
- Why it matters: Accidental `repo_notes=true` on `tasks/` spot breaks M14/M15 comparability. Accidental false on treatment voids the arm. “no-op or false” invites divergence.
- Suggestion: Pick one:
  - **Recommended:** config default `repo_notes = true` for REPL; eval harness sets from arm config only (runner writes `.loco/config.toml`); synthetic regression batches **must** use `repo_notes=false` (documented in CLAUDE.md + pre-registration). No silent no-op mode.
  - Thread `Config` into `Registry::guided(&cfg)` or `Agent::new` tool set construction in the same PR that adds the tool.
  - Snapshot `repo_notes` on `EffectiveConfig` (batch-level is enough if arms are separate stamps).

### I6. Root-level edit paths have zero directory ancestors → permanent mut-gate fail

- Severity: **Important** (product; frozen `tasks-real` solutions are under `src/`/`crates/` so sample impact is low)
- Section: §3-1, §3-5
- Description: Gate = `_root` OK ∧ **≥1 directory-ancestor notes** OK. For `Cargo.toml`, `README.md`, `build.rs` at repo root, the ancestor list is empty → condition 2 can never pass even with a perfect `_root.md`. Dirty-key rule (“most specific ancestor”) is also empty → stale may never arm for root-only edits.
- Why it matters: Product footgun; any future root-touching task is unsolvable under treatment without a special case. Spec claims “root + ancestor” as the compromise but does not define the root-file case.
- Suggestion: Pin: if the edit path’s parent is the project root (no directory notes key), **`_root` alone satisfies the ancestor clause** (or treat `_root` as a member of the ancestor set for gate+dirty). Unit vectors: `Cargo.toml`, `src/main.rs`, `crates/core/app.rs`, `src/exec/job.rs`.

### I7. protected / H7 wording is ambiguous and easy to implement backwards

- Severity: **Important**
- Section: §4
- Description: “`.loco/notes`를 protected로 넣어 지우지 말 것” can be read as (A) put notes **into** the protected list so they are not wiped, or (B) do **not** put them in protected. Actual `sync_protected` **restores from fixture**; fixture has no notes → listing notes as protected **deletes** them before check. H7 counts protected diffs only for listed paths — exclusion is automatic if notes are not listed.
- Why it matters: Wrong reading changes check-time sandbox state and H7 footprints; reviewers will thrash.
- Suggestion: Replace with explicit negatives: “Do **not** add `.loco/notes` to task `protected` or the implicit protected set. H7 therefore ignores notes writes; no special H7 exception code. `sync_protected` must not delete or restore notes (achieved by not listing them). Notes must not affect `passed`.”

### I8. Shell / non-tool writes can satisfy the mut gate while metrics claim “device idle”

- Severity: **Important**
- Section: §3-3 (disk re-validate), §5-3 mechanism gate
- Description: Gate accepts disk files re-parsed as schema-OK, not only `update_repo_notes` successes. Dirty clear, however, is tool-only. Under `AutoApprover`, `run_command` can `mkdir`+`printf` schema-OK notes and pass the code mut gate without ever calling the tool → `notes_updates=0`, `notes_mut_gate=0`, `notes_schema_reject=0` while the gate was effectively used. §2-2 “all mechanism counters 0 → batch interpretation hold” becomes false-negative for device activity and false-positive for “idle.”
- Why it matters: Mechanism observability is part of the success gate; off-tool satisfaction breaks it.
- Suggestion: Prefer one of:
  1. **Strict (recommended for eval honesty):** mut gate only accepts keys that were written by a successful `update_repo_notes` in this run (in-memory “certified” set), still allowing **session reuse** by certifying pre-existing disk files once at run start after schema parse; block `edit_file`/`write_file` on `.loco/notes/**` (already option A). Residual shell risk: document as un-closeable with `run_command`, or add a post-run metrics heuristic (notes files exist but `notes_updates==0` → `notes_offtool` label).
  2. Or drop “all zero → hold” and require `notes_updates>0` **or** certified disk presence as the mechanism-alive signal.

### I9. Soft-reject is “optional 권고” but O3 defaults to include — latch the 1차 scope

- Severity: **Minor**
- Section: §3-2 soft-reject, §8 O3
- Description: Schema table marks soft-reject optional; O3 default is include. Without a hard 1차 yes/no, control/treatment code can differ across PRs mid-milestone.
- Why it matters: Soft-reject changes reject rates and template spam; comparability within M16.
- Suggestion: 1차 = include fence≥1 OR non-blank lines≥40 as hard schema failure for both root and dir layers; pin constants; selftest vectors for borderline 39/40 lines.

### I10. Marker strings for new metrics not pinned as Rust constants contract

- Severity: **Minor**
- Section: §5-3
- Description: Names are given (`notes_schema_reject`, …) but not the **exact English substrings** that `exp_metrics.MARKS` must mirror (project discipline: verbatim constants, selftest). `notes_updates` especially needs a stable success body fragment not collidable with errors. `notes_bytes_max` is optional — thrifty claims then lack a numeric observer.
- Why it matters: Metrics drift / silent zero columns after renames (historical loco pain).
- Suggestion: In design or plan, list constants, e.g. `NOTES_SCHEMA_REJECT_MARK`, `NOTES_MUT_GATE_MARK`, `NOTES_STALE_NUDGE`, `NOTES_UPDATE_OK_PREFIX` (“repo notes updated:”), require PR6 selftest fixtures; make `notes_bytes_max` required for treatment reporting (can be “-” when flag off).

### I11. Dirty key = most-specific (possibly non-existent) vs gate = any ancestor OK

- Severity: **Minor**
- Section: §3-5, §3-6
- Description: Gate may pass with only `src.md` while dirty inserts `src/exec` for an edit under `src/exec/job.rs`. Clearing dirty requires writing a **new** deeper notes file, not refreshing the ancestor that unlocked the gate. Once-latch then allows finish with abandoned deep dirty keys.
- Why it matters: Not wrong, but models will often update the wrong layer; `notes_stale_finish` may fire often without teaching the hierarchy. Document as intentional or dirty the shallowest existing OK ancestor instead.
- Suggestion: Spec sentence: “Dirty keys are the most specific directory key for the edit path (file need not exist). Clearing requires `update_repo_notes` on that exact key (or any equal-or-deeper key — pick one).” Prefer exact-key clear for simplicity; add one sentence that once-latch implies incomplete hierarchy updates are tolerated.

### I12. PR plan: tool registration depends on config before config PR; grounding optional vs mechanism

- Severity: **Minor**
- Section: §9
- Description: PR2 “flag off 미등록” needs the flag or a temporary always-on; PR5 adds the key. PR4 “optional” grounding is fine but `[repo_notes]` strip must not land without keep-latest tests (pack elision lesson). Eight PRs are realistic if each keeps `cargo test`/clippy green.
- Why it matters: Mid-stack broken main if PR2 always registers while default true and tasks eval runs without false.
- Suggestion: Merge flag+EffectiveConfig snapshot in the same PR as tool registration (collapse PR2+PR5 registration surface), or land tool behind `#[cfg]`/default false until eval arm config exists. Keep PR4 truly optional and out of the effect claim.

### I13. Path mapping edge vectors incomplete in prose

- Severity: **Nit**
- Section: §3-1
- Description: Normalization (`//`, `.`/`..`, escape) is stated; absolute paths, Windows separators, trailing slashes, `.md` suffix, and notes key `_root` vs `root` are not in the vector list (only promised as unit tests).
- Why it matters: Low; PR1 can fix if tests are mandatory.
- Suggestion: Add a table of (code path → gate keys → dirty key) including root file, one-level, nested, and `crates/core/...`.

### I14. `## do_not` in template but not in schema

- Severity: **Nit**
- Section: §3-2, §3-4
- Description: Template shows `## do_not`; schema neither requires nor rejects it. Harmless; models may invent sections.
- Suggestion: One line: extra `##` sections allowed if size/soft-reject pass.

## Non-issues / explicitly OK

- **B2 over B3/C/D/E** for M16 scope; rejecting Timeout/max_turns as a co-primary arm (§0 #7, §2-3) is correct experiment hygiene.
- **Refusing M15 0/51 stamp as control** and requiring flag-off remeasurement (§0 #6, §5-2) matches PROTOCOL and baselines.md disqualification.
- **Hierarchical disk SSOT** with per-layer byte caps and no per-turn full-tree inject is the right small-model posture; session-only notes would not transfer to real use.
- **Schema = format only** (no LLM-as-judge of note quality) keeps the design implementable without new crates.
- **Stale finish once-latch** mirrors VERIFY_NUDGE philosophy; infinite finish reject is correctly avoided.
- **Option A** forbidding direct `edit_file`/`write_file` on `.loco/notes/**` is the right default for a single schema path.
- **Oracle/prompt freeze** and analysis-only `notes_oracle_overlap` (not auto-DQ) preserve sample integrity; harness never seeds notes with solutions.
- **Frozen sample paths** under `src/` / `crates/` mean the root-ancestor hole (I6) does not by itself void the N=17 measurement, though it must still be specified.
- **`first_mut_turn` / nav_hit / fix_hit** remain meaningful: exp_metrics first mut is already edit/write-only; notes turns will not falsely set `first_mut_turn` if that stays true.
- **Budget fixed 25/600 both arms** is the honest way to measure device tax vs lift.
- **No new crates**; line-scan schema is feasible.
- **PR1 pure parser** before agent wiring is the right cut.
- **Mechanism columns + selftest obligation** match M10–M15 metrics discipline once I10 pins strings.
- **Non-goals** (RAG, issue rewrite, model swap, notes semantic scoring) are cleanly out of scope.

## Verdict

**Ready: No** — Critical = 0, but **Important = 8** (I1–I8) that block a correct single implementation and/or clean control/treatment measurement if left to inventiveness.

Minimum to flip Ready → Yes:

1. Pin `is_mutating` + VERIFY whitelist (I1).  
2. Flag-scope SYSTEM pointer; templates only on reject bodies; short tool docs (I2).  
3. Single unambiguous ε / 통과 과제 / DQ absolute for N=17 (I3).  
4. Ordered finish latch table (I4).  
5. One eval flag policy (no “no-op or false”) (I5).  
6. Root-file ancestor rule (I6).  
7. Protected wording fix (I7).  
8. Certified notes set vs off-tool gate + mechanism-alive definition (I8).

After those land in the design (even as short decision tables), a second review pass can be short.
