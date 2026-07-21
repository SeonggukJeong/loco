# M16 SDD 인수인계 (다음 세션용)

**작성:** 2026-07-21 · **상태:** 설계 Ready · 플랜 Ready · **구현 0%** · SDD 착수 대기  
**브랜치 권고:** `m16/repo-onboarding` (main에서 분기; **main에 직접 구현 금지** — SDD 규율)

---

## 0. 한 줄

M15 cold-start 바닥(0/51) 위에 **계층 `.loco/notes/` + certified mut-gate + stale-finish** 를 올리고, flag on/off 재측정으로 들어 올림을 잰다. **코드는 아직 없음.** 다음 세션은 **subagent-driven-development**로 플랜 T1부터.

---

## 1. 읽기 순서 (다음 세션 시작 시)

1. **이 파일** (인수인계)
2. 스펙 (개정 2, Ready: Yes):  
   `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md`
3. 구현 플랜:  
   `docs/superpowers/plans/2026-07-21-m16-repo-onboarding.md`
4. (필요 시) 리뷰 이력:  
   `…-design-review-1.md` (I1–I8) · `…-design-review-2.md` (Ready Yes)
5. 배경: `docs/m16-candidates.md` · `docs/baselines.md` M15 ·  
   `docs/experiments/2026-07-20-m15-real-repo-baseline/report.md`
6. SDD 스킬: superpowers `subagent-driven-development`  
   - 진행 원장: `.superpowers/sdd/progress.md` (gitignored — 아래 §5 시드)  
   - `git clean -fdx` 시 원장 소실 → **이 커밋된 handoff + git log**로 복구

---

## 2. Git / 브랜치 상태 (세션 종료 시점)

| 항목 | 값 |
|---|---|
| 브랜치 | **main** (M15 병합 완료, remote 동기였음 — 이후 docs 3커밋 local ahead 가능) |
| M15 | `232e551` merge · `m15/real-repo-track` **삭제** · origin main 푸시 완료 |
| M16 docs on main | `7f525bd` 스펙 초안 → `1bfd525` 개정2+리뷰 → `b19b2cd` 플랜 |
| 구현 커밋 | **없음** |
| push | 세션 종료 시 확인: `git status -sb` — origin 대비 ahead면 push 여부는 사용자 판단 |

**SDD 시작 절차 (권고):**

```bash
git checkout main && git pull   # 필요 시
git checkout -b m16/repo-onboarding
# then: superpowers subagent-driven-development
# plan: docs/superpowers/plans/2026-07-21-m16-repo-onboarding.md
```

---

## 3. 스펙 핵심 계약 (구현자·리뷰어가 어기면 안 되는 것)

| # | 계약 |
|---|---|
| 1 | 저장: `.loco/notes/_root.md` + 디렉 층 · 디스크 SSOT · 세션 간 재사용 |
| 2 | 스키마: root summary+routes · dir role+entrypoints/notes · soft-reject fence/≥40줄 · 캡 1200/800 |
| 3 | 툴: `update_repo_notes` · `is_mutating=true` · 성공 접두 `repo notes updated:` |
| 4 | **VERIFY whitelist:** `mutated_since_verify`는 **`edit_file`\|`write_file`만** (notes로 재무장 금지) |
| 5 | mut 게이트: certified set · root + (조상 ≥1 **또는** 루트 파일 특례) · 접두 `repo notes mut gate:` |
| 6 | finish 순서: VERIFY → **NOTES_STALE** (`repo notes stale:`) → accept · once-latch |
| 7 | thrifty: 템플릿은 **거부 body만** · tool doc 짧음 · flag off 시 SYSTEM 포인터 **없음** |
| 8 | config `repo_notes` default **true** · `tasks/`·`tasks-large` eval **false** · silent no-op 없음 |
| 9 | `.loco/notes`를 **protected에 넣지 않음** |
| 10 | 1차 성공: `task_mean_pass ≥ 1/17` · DQ: 전패/전승 ≥13 (N=17) · M15 스탬프 control 금지 |
| 11 | **신규 크레이트 금지** · GPU 배치(T plan 밖)는 PROTOCOL 사전등록 후 |

마커 문자열 (exp_metrics 문자 일치):

- `repo notes schema:`
- `repo notes mut gate:`
- `repo notes stale:`
- `repo notes updated:`

---

## 4. 플랜 태스크 맵

| Task | 내용 | 완료? |
|---|---|---|
| T1 | `src/notes/` schema + path + templates | **pending** |
| T2 | tool + `repo_notes` config + `Registry::guided(bool)` + EffectiveConfig | **pending** |
| T3 | agent certified gate · dirty · finish order · VERIFY whitelist | **pending** |
| T4 | optional `[repo_notes]` grounding (효과 주장 밖) | **pending** (skip 가능) |
| T5 | `exp_metrics.py` notes columns + selftest | **pending** |
| T6 | CLAUDE.md + experiment stub | **pending** |
| T7 | cargo test/clippy/verify 게이트 · **GPU 없음** | **pending** |

GPU control/treatment 51×2는 플랜 밖 · 사전등록 세션.

---

## 5. SDD 원장 시드 (`.superpowers/sdd/progress.md`에 동일 내용 append)

다음을 progress.md **맨 위 또는 맨 아래 M16 섹션**으로 둔다. Task complete 줄이 없으면 전부 pending.

```markdown
# M16 준비 (2026-07-21, SDD 인수인계)
스펙: docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md (개정 2, 2R Ready=Yes)
플랜: docs/superpowers/plans/2026-07-21-m16-repo-onboarding.md
handoff: docs/superpowers/plans/2026-07-21-m16-sdd-handoff.md
리뷰: …-design-review-1.md (I1–I8) · …-design-review-2.md (Ready Yes)
base (main at plan commit): b19b2cd (confirm with git log)
실행: subagent-driven-development · 브랜치 m16/repo-onboarding (Task 1에서 생성 권고)
M15: main 232e551 병합 완료 · 베이스라인 0/51 실격 · 대조 비인용
주의:
  - VERIFY whitelist edit_file|write_file only
  - templates reject-body only; control SYSTEM pointer off
  - certified set; root-file gate special case
  - tasks/ eval repo_notes=false
  - no new crates; no GPU batch in T1–T7
Task 1: pending
Task 2: pending
Task 3: pending
Task 4: pending (optional)
Task 5: pending
Task 6: pending
Task 7: pending
```

---

## 6. 다음 세션 시작 프롬프트 (복붙용)

```text
M16 SDD 이어가. 인수인계: docs/superpowers/plans/2026-07-21-m16-sdd-handoff.md
스펙 Ready 개정2, 플랜 T1–T7. superpowers:subagent-driven-development 로
브랜치 m16/repo-onboarding 만들고 Task 1부터. progress.md 원장 확인 후
완료 태스크 재디스패치 금지. GPU 배치는 하지 말 것.
```

---

## 7. 하지 말 것

- main에 바로 구현 커밋 (브랜치 먼저)
- M15 0/51 스탬프를 control로 인용
- notes를 `is_mutating` 이유로 VERIFY 재무장
- tool doc에 thrifty 템플릿 전문 상주
- 신규 crate
- 사전등록 없이 tasks-real GPU 배치
- `git clean -fdx` 후 원장만 믿고 재구현 (git log + 이 handoff 대조)

---

## 8. 세션에서 끝난 일 (2026-07-21)

- M15 closeout · main merge · m15 브랜치 삭제 · origin push
- M16 brainstorm (계층 notes · root+조상 게이트 · 스키마+stale · flag 재측정)
- 전문가 리뷰 1R → 개정1 → 2R Ready Yes → 개정2 Minor
- 구현 플랜 T1–T7 작성 · 커밋
- **구현 코드 0**
