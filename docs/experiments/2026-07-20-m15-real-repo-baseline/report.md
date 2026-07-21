# M15 베이스라인 배치 결과 — `tasks-real` cold start

| | |
|---|---|
| 사전등록 | `pre-registration.md` (승인 `66a3c7e`, 본문 고정 `0b89dcb`, 개정 A 로드 37632 · `1d10e09`) |
| 성격 | **효과 실험 아님** · 대조 없음 · 기술 통계 + 실격 판정 |
| 수행 | 2026-07-21 · 브랜치 `m15/real-repo-track` · 러너 HEAD `1d10e09` |
| 한 줄 | **0/51** 통과 · 전패 **17/17** · 실격(전패≥13) **예** · M16 대조 **비인용** |

외부 요약: 같은 디렉터리 `README.md` · 레포 루트 `README.md` "프로젝트 상태".  
수치 원천: 각 스탬프 `report.json` + `python3 scripts/exp_metrics.py --pool` → `metrics/pooled.txt`.

---

## 1. 배치 조건 (실측)

| 항목 | 값 |
|---|---|
| 모델 | ornith-1.0-9b Q4_K_M · alias `ornith` |
| 전역 config | `context_tokens=8192` · `max_output_tokens=4096` · `base_url=http://localhost:8080/v1` |
| 실효 운용점 | **32768** (`TaskSpec` · H9 `effective_context_tokens` 전 런 확인) |
| 서버 로드 | **`n_ctx_slot=37632`** (`LOCO_CTX=37632`, 개정 A) |
| max_turns / timeout_secs | 25 / 600 (과제 `task.toml`) |
| 표본 | N=17 · `--repeats 3` · `--seed 0` · 총 **51런** |
| 프롬프트 | 이슈 본문 only (온보딩·경로 힌트 없음) |

서버 로그: `metrics/serve-37601.log` (`n_ctx_slot = 37632`).  
프리플라이트: `metrics/preflight.txt` · 세 트리 `--verify` · selftest · json_schema 200.

---

## 2. 배치 ↔ 스탬프

| 하위 | 스탬프 | 과제 수 | 통과 | 엄격 | ff | duration | avg s/런 |
|---|---|---|---|---|---|---|---|
| B1 | `20260721T043543Z` | 4×3=12 | 0 | 0 | 0 | 5282s (~88m) | 435 |
| B2 | `20260721T060346Z` | 3×3=9 | 0 | 0 | 0 | 4693s (~78m) | 515 |
| B3 | `20260721T072200Z` | 4×3=12 | 0 | 0 | **1** | 4026s (~67m) | 329 |
| B4 | `20260721T082907Z` | 3×3=9 | 0 | 0 | 0 | 3885s (~65m) | 425 |
| B5 | `20260721T093354Z` | 3×3=9 | 0 | 0 | 0 | 4508s (~75m) | 496 |
| **합** | | **51** | **0** | **0** | **1** | **~6.22h** | — |

`stamps.txt`: `batch_all_done 2026-07-21T104902Z`. 하위 배치 전원 rc=0.  
`schema_fallback_count` 전 배치 **0**.

---

## 3. 사전등록 판정 (기계 적용)

| 규칙 | 적용 | 결과 |
|---|---|---|
| 효과 승자 | 해당 없음 | — |
| 실격: 전패 과제 ≥13 (N=17) | 전패 **17** | **실격 — 예** |
| 실격: 전승 과제 ≥13 | 전승 **0** | 해당 없음 |
| §9-A5 처분 | 실격 시 | **베이스라인 확보 실패** · M16 대조 **인용하지 않음** · M15 인프라 병합은 막지 않음 |
| 소표본 (PROTOCOL 3) | 통과율 n=17 과제 | 비율 보고 가능하나 점추정 0 |
| 재측정 | 정상 완주 후 숫자 불만 | **0회** (사전 공약 준수) |

**판정 (컨트롤러 초안, 사람 리뷰 대상):**  
이번 배치는 **cold-start 실레포 바닥 좌표를 0/51로 고정**했다.  
“개선 여지가 남은 양의 베이스라인”으로서는 **실격**.  
측정 체계·표본·실패 분해는 **유효한 M15 산출**이다.

---

## 4. 과제별 (3런 outcome)

| 과제 | pass | outcomes |
|---|---|---|
| delta-1089-whole-file-commit | 0/3 | timeout, timeout, max_turns |
| fd-1873-path-sep | 0/3 | timeout×3 |
| fd-404-min-exact-depth | 0/3 | repetition_stop, max_turns×2 |
| fd-535-prune | 0/3 | max_turns, timeout, max_turns |
| fd-615-hidden-dot-pattern | 0/3 | max_turns, timeout×2 |
| fd-675-number-parse-error | 0/3 | max_turns×2, timeout |
| fd-898-strip-cwd-exec | 0/3 | max_turns, timeout, max_turns |
| rg-1138-no-ignore-dot | 0/3 | repetition_stop×3 |
| rg-1159-exit-status | 0/3 | max_turns×2, timeout |
| rg-1176-fixed-strings-file | 0/3 | max_turns×3 |
| rg-1293-glob-case-insensitive | 0/3 | max_turns, timeout, **finished** |
| rg-1390-no-context-sep | 0/3 | max_turns×2, timeout |
| rg-1420-no-ignore-exclude | 0/3 | max_turns, timeout, max_turns |
| rg-1466-no-ignore-files | 0/3 | repetition_stop, max_turns×2 |
| rg-1868-passthru-context | 0/3 | max_turns×2, timeout |
| rg-568-leading-hyphen | 0/3 | timeout×2, max_turns |
| rg-740-passthru | 0/3 | max_turns×2, timeout |

**finished 1건** (`rg-1293` r2): 코드 수정 없이 플래그 설명 산문으로 `finish` — 거짓 성공 finish(check 실패). 테스트 오판이 아님.

---

## 5. outcome 분포 (51런)

| outcome | n | 비고 |
|---|---|---|
| max_turns | **27** | 주력 |
| timeout | **18** | 600s · 턴 적어도 발생 |
| repetition_stop | 5 | rg-1138 전량 포함 |
| finished | 1 | check 실패 |

대부분 **채점 전 탈락** (턴·시간·루프).

---

## 6. pool 지표 (`metrics/pooled.txt`)

```
# pooled over 5 stamp dir(s), 51 runs
# pass_rate tasks=17 mean=0.0000 ci95=[0.0000,0.0000] resamples=10000 seed=0
# disqualification N=17 all_pass=0 all_fail=17 band=4.04 disqualified=True
# nav_hit[fail] tasks=17 excluded=0 mean=0.7451 ci95=[0.5686,0.9020]
# fix_hit[fail] tasks=17 excluded=0 mean=0.2157 ci95=[0.0980,0.3529]
# tokens est_ratio_max=1.7874 max_prompt=28682 pack_turns=22 overflow_shrink=0 overflow_giveup=0
# a3_diff attached=29 truncated=3 truncation_rate=0.1034
```

| 지표 | 값 | 해석 |
|---|---|---|
| 통과 평균 | **0** · CI [0,0] | 바닥 |
| nav_hit (실패 층) | **~0.75** | 오라클 근처를 **읽는** 런이 다수 |
| fix_hit (실패 층) | **~0.22** | **고치는** 비율은 낮음 |
| pack_turns | 22 | 대형(rg)에서 elide 발동 |
| est_ratio_max | 1.79 | 스모크 r_obs 1.26과 동일 정의(최댓값); 런 간 상한 |
| overflow | 0 shrink / 0 giveup | 로드 37632  sufficed for 요청 경로 |

통과 층은 전 과제 empty → nav/fix pass-층 **제외 17** (분모 0 회피 규칙 정상).

---

## 7. 과제·채점 건전성

| 검사 | 결과 |
|---|---|
| 배치 전 `eval tasks-real --verify` | **17/17** |
| spot 재검증 (fd-1873, rg-1138) | 변별 ✓ · 해결 ✓ |
| 해석 | solution 있으면 통과 가능 · **cold start로 못 풂** |

---

## 8. 비교가능성 · 한계

1. **`tasks/` · M13 파일럿 · M14와 통과율 나란히 비교 금지** (채점·프롬프트·규모·트랙).  
2. 단일 모델·단일 양자화·단일 운용점.  
3. 이슈 only = **의도적 가혹 조건** (제품 온보딩 루프 아님).  
4. pool nav/fix은 touch∩oracle 휴리스틱 · `exp_metrics` 정의 따름.  
5. 분기 2: `n_ctx_slot(37632) ≠ context_tokens(32768)` — 리포트에 병기.  
6. 실격 대역은 정규근사 휴리스틱 · 부트스트랩 CI와 수치 일치 불필요 (사전등록).

---

## 9. 얻은 것 / 다음

| 얻은 것 | 내용 |
|---|---|
| 바닥 좌표 | 실레포 cold start **0/51** |
| 실패 지도 | MaxTurns/Timeout · nav≫fix · zero-mut 다수 |
| 실격 | 양의 베이스라인 **확보 실패** 라벨 |
| 인프라 | 조달·leak·5분할·H9·로드 등호 완주 |
| pack | 대형 발동 · 통과 구원 아님 |
| 다음 방향 | 온보딩 하네스 → `docs/m16-candidates.md` |

| 다음 | |
|---|---|
| 문서 | 본 report + `docs/baselines.md` M15 (본 커밋) |
| 설계 | **다음 세션 M16 스펙** (탐색→notes→작업) |
| 병합 | 인프라 Ready vs 베이스라인 실격 **구분** (사전등록 A5) |

---

## 10. 재현 명령 (참고)

```bash
# 조건: LOCO_CTX=37632, config 8192/4096, ornith
# 하위 배치 예 (B1)
cargo run -- eval tasks-real --repeats 3 --seed 0 \
  --filter fd-1873-path-sep --filter fd-404-min-exact-depth \
  --filter fd-535-prune --filter fd-615-hidden-dot-pattern

python3 scripts/exp_metrics.py --pool \
  .loco/eval/20260721T043543Z \
  .loco/eval/20260721T060346Z \
  .loco/eval/20260721T072200Z \
  .loco/eval/20260721T082907Z \
  .loco/eval/20260721T093354Z
```

스탬프 디렉터리는 git-ignored (`.loco/eval/`).
