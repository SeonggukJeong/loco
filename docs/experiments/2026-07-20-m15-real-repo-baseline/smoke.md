# M15 배치 전 스모크 — r_obs 측정과 서버 로드 분기 (§4-1-1)

측정 시각: 2026-07-21. 브랜치 `m15/real-repo-track`.

## 1. 조건

| 항목 | 값 |
|---|---|
| 관측용 서버 로드 | **`n_ctx_slot = 40960`** (배치 조건 아님 — 관측 전용) |
| 스모크 운용점 | `context_tokens=32768`, `max_output_tokens=4096` (사본 내 `.loco/config.toml`만) |
| 전역 `.loco/config.toml` | **미변경** (`context_tokens=8192` 유지) |
| 모델 | ornith-1.0-9b Q4_K_M |
| T1 동결 | `마진=1024`, 커밋 `d583ff8` (`thresholds.md`) |
| T20 `n_ctx_train` | **262144** (GGUF 직독) |

서버 로그: `smoke-server.log` — `n_ctx_slot = 40960`.

## 2. 시도 전부 (탈출구 순서)

| # | 과제 | max_turns | r_obs | max_prompt | pack_fired | 종료 | 비고 |
|---|---|---|---|---|---|---|---|
| 1 | fd-1873-path-sep | 40 | 1.1033 | 7769 | 0 | RepetitionStop | 소형 레포, 예산 미달 |
| 2 | fd-1873-path-sep | 60 | 1.0899 | 8411 | 0 | (중단/단축) | 탈출구 1 |
| 3 | rg-1868-passthru-context | 50 | 1.1954 | 26249 | 0 | max_turns | 탈출구 3 과제 교체; est=24061 (예산 93%) |
| 4 | rg-1868-passthru-context | 80 | 1.1978 | 16684 | 0 | finished | 조기 finish |
| **5** | **rg-1868-passthru-context** | **100** | **1.2587** | **32011** | **9** | exit 2 | **채택** — pack 도달 |

탈출구: 1(같은 과제 max_turns↑) → 2/재시도 → 3(과제 교체). 교체 전 fd `r_obs`와 교체 후 rg `r_obs`를 모두 기록했고, **채택값은 최대 `r_obs=1.2587`**(보수 — `L_req`가 커짐).

## 3. 채택 세션 (원자료)

- 과제: **`rg-1868-passthru-context`** (표본 동결 N=17에 포함, 규약 6 탈락 없음)
- 세션 JSONL: `docs/experiments/2026-07-20-m15-real-repo-baseline/smoke/attempt5-rg-1868-max100.jsonl`
- `--session` 원 출력:

```
r_obs=1.2587 max_prompt=32011 max_est=25760 first_turn_prompt_tokens=2111 pack_fired=9 budget_ratio_max=1.2405 overflow_shrink=0 overflow_giveup=0
# estimator inline_system=False {'slope_per_est_token': 0.9751198940792256, 'intercept_per_message': 49.71073128713918, 'n': 63}
```

### §5-5 `prompt_tokens` 의미

첫 턴 `prompt_tokens=2111` — **정의상 캐시 미적중**(세션 시작). 이후 턴의 `prompt_tokens`는 서버 캐시 적중 여부에 좌우될 수 있으나, `r_obs`는 턴별 `prompt_tokens/estimate_tokens`의 **최댓값**이라 오버플로 결정에 직접 쓰인다.

### pack 도달

`pack_fired=9` (≥1). 추정 `estimate_tokens`가 예산 25,804를 넘기며 elide 기록됨(세션 내 `kind=pack` 9건).  
참고: 실제 `prompt_tokens`는 추정 편향(`r_obs≈1.26`) 때문에 예산 초과가 더 일찍 나타날 수 있음 — pack 트리거는 **추정** 기준.

## 4. L_req · 분기

산식 (T1 동결):

> **`L_req` = ⌈(32768 − 4096) · 0.9 · r_obs + 4096 + 1024⌉**

```
r_obs = 1.2587
L_req = ceil(28672 × 0.9 × 1.2587 + 5120) = ceil(37600.3) = 37601
n_ctx_train = 262144 ≥ 32768  → 분기 3 직행 조건 아님
```

| # | 조건 | 판정 |
|---|---|---|
| 1 | `L_req ≤ 32768` | 아니오 (37601 > 32768) |
| 2 | `32768 < L_req ≤ n_ctx_train` | **예** (37601 ≤ 262144) |
| 3 | `L_req > n_ctx_train` | 아니오 |

### 채택 분기 = **2**

- **확정 서버 로드 ctx = `L_req` = 37601** (4③ 동결값 등호 — `≥` 아님, 단일 값)
- 배치 시 `LOCO_CTX=37601 scripts/serve.sh` (또는 등가 핀)
- `TaskSpec.context_tokens`는 계속 **32768** (운용점). 로드만 올린다.

### 사후 슬랙 (마진과 구분)

```
inner = (ctx−mo)·0.9·r_obs + mo = 36576.5
posthoc_slack = n_ctx_slot_obs − inner = 40960 − 36576.5 = 4383.5
```

`마진=1024`는 산식 **입력항**. 슬랙은 관측 로드 대비 **사후 기록**이며 동일 개념이 아니다.

## 5. 픽스처 무오염 · verify

```
find tasks-real -maxdepth 3 -name target   → 0
find tasks-real -maxdepth 3 -name .loco    → 0
cargo run -- eval tasks-real --verify     → 검증 17/17 통과
```

스모크는 전부 `mktemp` 사본에서 수행. 서버는 스모크 종료 후 정지.

## 6. T23 입력 요약

| 항목 | 값 |
|---|---|
| `r_obs` | **1.2587** |
| 첫 턴 `prompt_tokens` | 2111 (캐시 미스) |
| `pack_fired` | 9 |
| `L_req` | **37601** |
| 분기 | **2** |
| 확정 로드 | **37601** |
| `n_ctx_train` | 262144 |
| 표본 N | 17 (T21 재실사) |
