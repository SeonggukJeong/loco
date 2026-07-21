# M16 SDD 인수인계 (다음 세션용)

**작성:** 2026-07-21 · **갱신:** 2026-07-21 (T1–T6 상륙 후)  
**상태:** 설계 Ready (2R) · 플랜 Ready (2R Yes) · **구현 T1–T6 완료** · T7 게이트 대기  
**브랜치:** `m16/repo-onboarding` (**main에 직접 구현 금지**)

---

## 0. 한 줄

M15 cold-start 바닥(0/51 DQ) 위에 **계층 `.loco/notes/` + certified mut-gate + stale-finish** 를 올렸다.  
다음: **T7** cargo/clippy/verify/selftest 게이트(GPU 없음) → 사전등록 → control/treatment 재측정.

---

## 1. 읽기 순서 (다음 세션 시작 시)

1. **이 파일** (인수인계)
2. 스펙 (개정 2, Ready: Yes):  
   `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md` (**§5** 측정)
3. 구현 플랜:  
   `docs/superpowers/plans/2026-07-21-m16-repo-onboarding.md`
4. 실험 stub: `docs/experiments/2026-07-21-m16-repo-onboarding/README.md` (pre-reg TODO)
5. 배경: `docs/m16-candidates.md` · M15 report (0/51 DQ · control 비인용)
6. SDD 원장: `.superpowers/sdd/progress.md` (gitignored)

---

## 2. Git / 브랜치 상태

| 항목 | 값 |
|---|---|
| 브랜치 | **`m16/repo-onboarding`** |
| HEAD (T5) | `9dc4d46` feat(metrics)… — T6 커밋이 이 위에 쌓임 |
| T1 | `2d9a6a9` schema/path/templates |
| T2 | `3c64ca9` tool + config + registry |
| T3 | `18f4589` agent gate / stale / VERIFY whitelist |
| T4 | `f4a5dfc` grounding **deferred** (효과 주장 밖) |
| T5 | `9dc4d46` exp_metrics notes MARKS + `notes_bytes_max` |
| T6 | `docs(m16): CLAUDE flag policy + experiment stub` (본 커밋) |
| GPU | **미실행** · 사전등록 전 |

```bash
git checkout m16/repo-onboarding
# T7 gates from plan Task 7
```

---

## 3. 스펙 핵심 계약 (어기면 안 되는 것)

| # | 계약 |
|---|---|
| 1 | 저장: `.loco/notes/_root.md` + 디렉 층 · 디스크 SSOT |
| 2 | 툴: `update_repo_notes` · `is_mutating=true` · 성공 접두 `repo notes updated:` |
| 3 | **VERIFY whitelist:** `mutated_since_verify`는 **`edit_file`\|`write_file`만** |
| 4 | mut 게이트: certified set · 접두 `repo notes mut gate:` |
| 5 | finish: VERIFY → **NOTES_STALE** (`repo notes stale:`) → accept · once-latch |
| 6 | config `repo_notes` default **true** · eval non-`tasks-real` **false 강제** · silent no-op 없음 |
| 7 | `.loco/notes`를 **protected에 넣지 않음** |
| 8 | 1차 성공: `task_mean_pass ≥ 1/17` · DQ: 전패/전승 ≥13 · **M15 스탬프 control 금지** |
| 9 | **신규 크레이트 금지** · GPU는 PROTOCOL 사전등록 후 |

마커 (exp_metrics 문자 일치): `repo notes schema:` · `repo notes mut gate:` · `repo notes stale:` · `repo notes updated:`

---

## 4. 플랜 태스크 맵

| Task | 내용 | 완료? |
|---|---|---|
| T0 | 브랜치 `m16/repo-onboarding` | **done** |
| T1 | `src/notes/` schema + path + templates | **done** `2d9a6a9` |
| T2 | tool + `repo_notes` + `Registry::guided(bool)` + EffectiveConfig | **done** `3c64ca9` |
| T3 | agent certified gate · dirty · finish · VERIFY whitelist | **done** `18f4589` |
| T4 | optional `[repo_notes]` grounding | **deferred** `f4a5dfc` |
| T5 | `exp_metrics.py` notes columns + selftest | **done** `9dc4d46` |
| T6 | CLAUDE.md + experiment stub | **done** (본 커밋) |
| T7 | cargo test/clippy/verify 게이트 · **GPU 없음** | **pending** |

GPU control/treatment 51×2는 플랜 밖 · 사전등록 세션.

---

## 5. Flag 정책 (CLAUDE.md와 동일)

- Product default: `repo_notes = true`
- Eval: `apply_eval_repo_notes_policy` — basename `tasks-real`만 config 유지; 그 외 false
- Tool: `update_repo_notes`
- 측정: design **§5**

---

## 6. 다음 세션 (T7)

```text
M16 branch m16/repo-onboarding. Run Task 7 gates only (no GPU):
  cargo test
  cargo clippy --all-targets -- -D warnings
  cargo run -- eval tasks --verify
  cargo run -- eval tasks-large --verify
  cargo run -- eval tasks-real --verify   # if fixtures present
  python3 scripts/exp_metrics.py --selftest
  rg 'Registry::guided\(\)' src/   # zero zero-arg calls
Commit if needed; do not invent baseline numbers.
```
