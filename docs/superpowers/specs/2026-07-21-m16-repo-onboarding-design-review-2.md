# M16 design review 2R

- Ready: **Yes**
- Date: 2026-07-21
- Reviewer: independent expert pass (re-review after 개정 1)

## Summary

개정 1 closes the eight Important gaps from 1R with explicit contracts rather than “구현 시 정함.” `is_mutating=true` plus a VERIFY/status/FINISH_NUDGE whitelist, the flag×surface matrix, single primary ε, finish order VERIFY→STALE, eval `false`/no silent no-op, root-file gate special case, protected negatives, and the certified-set gate are all latched in the body and Key Decisions. Residual issues are operational footnotes and implementer polish (marker body text, `notes_bytes_max` encoding, start-of-run cert vs mid-run shell corruption). None block a single correct implementation or a valid control/treatment measurement if pre-registration copies §2-2/§5 verbatim.

## 1R Important disposition (I1–I8)

| ID | Status | Where latched |
|---|---|---|
| **I1** VERIFY re-arm | **Addressed** | §3-3 `is_mutating=true` + whitelist; §3-6 orthogonality; §7 #4; unit-test obligation |
| **I2** control SYSTEM / templates | **Addressed** | §0 #5; §3-4 flag matrix (pointer off / short doc / templates reject-only) |
| **I3** ε dual OR | **Addressed** | §2-2 single primary `task_mean_pass ≥ 1/17`; `tasks_with_any_pass` 2차 only; DQ ≥13 absolute, arm-independent |
| **I4** finish order | **Addressed** | §3-6 ordered table VERIFY → NOTES_STALE → accept + integration-test sketch |
| **I5** eval flag policy | **Addressed** | §3-8 no silent no-op; synthetic **must** `false`; EffectiveConfig snapshot; tool+flag in PR2 |
| **I6** root-file ancestors | **Addressed** | §0 #4; §3-1 vectors; §3-5 root-only special case; dirty=`_root` |
| **I7** protected / H7 | **Addressed** | §4 explicit “do not list notes”; H7 auto-exclude; no special-case code |
| **I8** off-tool gate / metrics | **Addressed** | §3-5 certified set (start scan + tool only); gate ≠ bare disk reparse; `notes_offtool`; §5-3 mechanism-alive |

1R Minors/Nits I9–I14 are also absorbed (soft-reject fixed, marker strings drafted, exact-key dirty documented, PR collapse, mapping vectors, extra `##` allowed). Not re-opened.

## Issues (remaining only)

### R2-1. `NOTES_STALE_NUDGE` body text still undrafted

- Severity: **Minor**
- Section: §3-6, §5-3
- Description: Match contract is “전문 첫 줄 = 상수,” but unlike mut-gate (`repo notes mut gate:`) and update OK (`repo notes updated:`), the English STALE body is not written. Implementers can diverge on wording while sharing a constant name.
- Why it matters: `exp_metrics` substring discipline; low if PR3 freezes a string before PR5.
- Suggestion: One-line draft in §3-6, e.g.  
  `NOTES_STALE_NUDGE = "repo notes stale: you edited code but did not update notes for: … Update via update_repo_notes, then finish."`  
  and match `repo notes stale:` in MARKS (same style as other notes markers).

### R2-2. `notes_bytes_max` source encoding unspecified

- Severity: **Minor**
- Section: §5-3
- Description: Column is required, but whether the value is a transcript `record_extra`, a tool-result suffix, or report-only is open. Flag-off `"-"` is clear.
- Why it matters: PR5 vs agent loop ownership; not a validity threat if treatment always has some path.
- Suggestion: Prefer `session.record_extra("notes_bytes_total", n)` (or max) updated on successful `update_repo_notes` / start scan; exp_metrics reads extras. Document one sentence in §5-3.

### R2-3. Certified membership not revalidated after start

- Severity: **Minor**
- Section: §3-5
- Description: After start-of-run certification (or tool success), mid-run `run_command` overwrite of a certified path leaves the key certified without re-parse. Eval cold start + option A make this rare; product multi-session is the residual.
- Why it matters: Documented shell residual; does not reopen I8’s measurement hole (shell still cannot *gain* cert mid-run without the tool).
- Suggestion: Keep as residual risk in §6; optional later “re-parse on gate if mtime changed” is out of M16 1차.

### R2-4. Synthetic `repo_notes=false` is policy, not harness-enforced

- Severity: **Minor**
- Section: §3-8, §5-5
- Description: Correct single contract (must false; no no-op), but nothing stops `cargo run -- eval tasks/` under default `true` after PR2 lands. Comparability break would be operator error.
- Why it matters: Real footgun for local spot checks once default is true.
- Suggestion (optional, still Ready without it): eval logs a one-line stderr notice when `repo_notes=true` on non-`tasks-real` trees, or CLAUDE.md + pre-registration alone as already required.

### R2-5. ε can hold while arm remains all-fail-DQ

- Severity: **Nit**
- Section: §2-2
- Description: Three scattered 1/3 passes ⇒ `task_mean_pass = 1/17` and `all_fail` can still be ≥13. Spec already separates ε vs DQ; no contradiction.
- Suggestion: One clause in pre-registration: “primary lift (ε) and DQ labels are reported independently; ε alone does not clear DQ.”

### R2-6. Integration-test sketch allows two paths after first VERIFY

- Severity: **Nit**
- Section: §3-6
- Description: “2차 STALE(또는 검증 해제 후 STALE)” matches once-latch VERIFY semantics (second finish skips VERIFY). Fine; pin the no-test-between case in the test name so implementers do not require a cargo test between the two finishes.
- Suggestion: Test A: mut+dirty+unverified → finish → VERIFY → finish → STALE → finish → accept. Test B: mut → test (clear verify) + dirty → finish → STALE only.

## Non-issues / explicitly OK

- B2 + budget-fixed both arms + no M15 stamp control remain sound.
- Whitelist change is behavior-equivalent for today’s two mutating tools and correctly excludes notes from VERIFY re-arm.
- Control with tool absent + no SYSTEM pointer is a clean sham-free baseline for the same binary.
- Certified start scan preserves product session reuse without letting mid-run shell writes unlock the gate.
- Root-file special case matches frozen `tasks-real` layout and fixes the product hole.
- Single ε limb removes 1R’s success-criterion fishing path.
- PR collapse (tool+flag+EffectiveConfig together) removes the 1R registration/config chicken-egg.
- Mechanism-alive definition is operational for cold-start treatment (updates or reject traffic).
- Integrity: no harness oracle seeding; analysis-only overlap label; notes outside protected/H7.

## Verdict

**Ready: Yes** — Critical **0**, Important **0**, Minor **4**, Nit **2**.

1R I1–I8 are fully addressed. Remaining items are documentation/encoding polish and operator footguns; they do not block implementability or experiment validity. Proceed to user spec approval → writing-plans → pre-registration (copy §2-2 / §5-1–5-3 / mechanism-alive / DQ absolutes verbatim).
