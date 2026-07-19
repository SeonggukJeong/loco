# M13 배치 1 결과 — llama.cpp 앵커 (T6)

- 사전등록: `docs/experiments/2026-07-19-llamacpp-anchor/pre-registration.md`
  (승인 커밋 `7d76a25`, 승인된 내용은 그 부모 `63bbfd7`)
- 스탬프: `.loco/eval/20260719T082030Z`
- 대상 커밋: `7d76a25` (T1~T5 완료 시점, 코드 변경은 `814d87c`까지)
- 수행: 2026-07-19 08:20:30Z ~ **09:01:06.7Z** (벽시계 2436.7초 = 40.6분,
  런 가중 평균 67.1초/런). 종료 시각은 `started_at + duration_secs`로 계산한
  값이다 — 배치 종료 후 서버 로그를 `$STAMP`로 복사한 시점(09:02:19Z)이
  아니다

**이 문서의 모든 수치는 `report.json`을 직접 대조해 적은 것이다** — 러너
표준출력 요약을 옮기지 않았다(M12 교훈).

## 1. 배치 조건 (실측 등록)

- `llama-server --version`: `version: 9960 (a935fbffe)`,
  `built with AppleClang 21.0.0.21000099 for Darwin arm64`
- 기동: `scripts/serve.sh` (`LOCO_CTX=8192`, 기본 포트 8080, alias `ornith`),
  로그 `.loco/serve-T6.log` → 배치 산출물로 편입 `$STAMP/server-startup.log`
- 모델: ornith-1.0-9b Q4_K_M
- 명령: `cargo run -- eval tasks --repeats 3 --seed 0` — **디버그 빌드**
  (로그에 `Finished \`dev\` profile [unoptimized + debuginfo]` 확인,
  사전등록 §2-3의 `--release` 금지 준수)
- `.loco/config.toml` (사전등록 §2-1과 일치):
  ```toml
  context_tokens = 8192
  max_output_tokens = 4096
  command_timeout_secs = 60
  base_url = "http://localhost:8080/v1"
  ```
  대조 배치의 `effective_config`를 직접 조회해 앞의 세 값이 같음을 확인했다.
  `base_url`이 유일한 의도된 조건 변경이다.

### 배치 전 스모크 (사전등록 §2-4, 8항 전건 통과)

| # | 항목 | 결과 |
|---|---|---|
| 1 | `eval tasks --verify` / `eval tasks-large --verify` | 12/12 · 3/3 |
| 2 | json_schema 요청 1건 | **HTTP 200** |
| 3 | 기동 로그 `n_ctx_slot` | 8192 (`n_slots = 1`) |
| 4 | `/v1/models` `data[0].id` | `ornith` |
| 5 | `.loco/config.toml` 등록 조건 일치 | 일치 |
| 6 | `${TMPDIR}/.cargo` 트립와이어 | clear |
| 7 | 측정 중 `cargo build`/`test` 병행 | 없음 |
| 8 | 데몬화 래퍼 적용 | 적용 |

기동 전 `rm -f .loco/serve-T6.log`를 실행했다 — 사전등록 §2-2가 막으려는
"사후 재기동 로그로 검사 통과" 우회에서, 기존 파일이 남아 있으면 `>`
리다이렉션이 birth time을 보존해 우회가 성립할 수 있기 때문이다.

## 2. 기계 검사 3종 (사전등록 §3-2 — 전건 통과해야 앵커로 인정)

| 순위 | 검사 | 결과 |
|---|---|---|
| 주 | 전 런 `schema_fallback == false` | **발동 런 없음** (필드 누락 런도 없음) |
| 보조 | `parse_fail_first(총) == 0` | **0** |
| 환경 | `n_ctx_slot == 8192` | **8192** |

환경 검사의 로그 선후 관계(사전등록 §2-2): 로그 birth `1784449158`,
`$STAMP` epoch `1784449230` — 로그 시작이 배치보다 **72초 앞섬**. `%m`(mtime)이
아니라 `%B`(birth)로 비교했다.

**3종 전건 통과 → 이 배치를 앵커로 인정한다.**

## 3. 판정

```
앵커 35/36 (엄격 34)   대조 33/36 (엄격 32)   차이 +2 (엄격 +2)
```

- 대조: `20260718T222824Z` (M12 회귀 게이트 배치)
- **결함 하한 (§3-1)**: 35 ≥ 27 — 통과. 하한에 걸리지 않았으므로 판정으로 진행
- **동등성 판정 (§3-3)**: |+2| ≤ 4 → **동등 성립**
- **거짓 성공 finish**: 0

### 안정 집합 (분류 방아쇠 — 판정 항이 아님)

적용 목록(사전등록 §3-3과 동일, 드리프트 없음):
`add-function, chain-edits, count-usages, create-module, find-definition, fix-off-by-one`

**위반 없음** (6개 전부 3/3). 따라서 정독 방아쇠는 발동하지 않았고,
§3-3의 보류 갈래에 들어가지 않았다.

### 과제별

| 과제 | 앵커 | 엄격 | 대조 |
|---|---|---|---|
| add-function | 3/3 | 3 | 3/3 |
| chain-edits | 3/3 | 3 | 3/3 |
| count-usages | 3/3 | 3 | 3/3 |
| create-module | 3/3 | 3 | 3/3 |
| edit-crlf-file | 3/3 | 3 | 3/3 |
| find-definition | 3/3 | 3 | 3/3 |
| fix-compile-error | 3/3 | 3 | 3/3 |
| fix-failing-test | 3/3 | 3 | **2/3** |
| fix-off-by-one | 3/3 | 3 | 3/3 |
| implement-from-doc | 3/3 | 3 | 3/3 |
| multiline-string-edit | 3/3 | **2** | **1/3** |
| rename-function | **2/3** | 2 | **3/3** |

총합 +2는 세 과제의 상쇄 결과다(`multiline-string-edit` +2,
`fix-failing-test` +1, `rename-function` −1). 과제 단위로는 서로 반대
방향의 이동이 섞여 있으므로, 총합 +2를 "llama.cpp가 더 낫다"로 읽지 않는다 —
사전등록의 동등성 판정은 스택 전환이 **깨뜨리지 않았음**을 확인하는 것이지
개선을 주장하는 장치가 아니다.

### 비정상 종료 런

- `multiline-string-edit-2`: `outcome=timeout`이나 `passed=true` (8턴, 305.0초).
  판정 `check`는 통과했고 엄격 집계에서만 빠졌다
- `rename-function-0`: `outcome=repetition_stop`, `passed=false` (12턴).
  이 배치의 유일한 실패 런

## 4. 관측 항목 (판정 아님)

| 지표 | 앵커 | 대조 | 비고 |
|---|---|---|---|
| 빈-content 턴 (`"(empty)"`) | 6 | 3 | **대리 지표** — 아래 한계 참조 |
| `sr_error` | 41 | 34 | |
| `sr_correction` | 14 | 8 | |
| `recovered` | 37/41 | 28/34 | 90.2% vs 82.4% |
| `repeat_corr` | 6 | 10 | |
| `finish_missing` | 1 | 7 | |
| `finish_nudge` | 0 | 2 | |
| `status_note` | 69 | 78 | |
| `verify_total` | 0 | 2 | 두 배치 모두 T7 이전이다 — 이 0 vs 2는 T7과 무관하다. 다만 **T8 게이트 배치와는 비교하지 말 것**(T7 수선 B가 무뮤테이션 노트에도 검증 줄을 실어 구조적으로 상향시킨다 — 사전등록 §4-2 비교불가 각주) |
| `args_tool_key` | 25 | 35 | |
| `cargo_after_mut` | 35/36 | 30/35 | |

**빈-content 턴은 대리 지표다.** 스펙 §3-2의 1순위 관측 대상은
`finish_reason == "length"` 턴 수인데 `finish_reason`이 트랜스크립트에
영속되지 않아 직접 셀 수 없다. 사각지대: 일부 content가 남은 채 잘린 절단은
정상 턴과 구별되지 않는다. 다만 §3-2 실측상 사고 토큰이 예산을 소진하면
content가 부분이 아니라 **완전히 비어** 오므로 지배적 경우는 포착된다.

배치 전 json_schema 스모크 응답에서 이 현상을 직접 관측했다:
`finish_reason: "length"`, `content: ""`(완전히 빈 문자열), 예산은
`reasoning_content`가 소비. 스모크는 `max_tokens=64`라 당연한 결과이므로
이것이 배치 조건(`max_output_tokens=4096`)에서의 발생률을 뜻하지는 않는다 —
현상의 **형태**를 확인한 것이다.

앵커 6건 vs 대조 3건은 절대 수가 작아(36런 중) 방향을 주장할 근거가 되지
않는다. 판정에 쓰지 않으며, M14 관측 대상으로 남긴다.

### 문서 드리프트 발견

`CLAUDE.md`는 `args_tool_key`/`args_tool_switch`에 대해 "0 across every batch
so far, since all predate the code"라고 적고 있으나, 대조 배치
`20260718T222824Z`를 현재 추출기로 재실행하면 `args_tool_key=35`,
`args_tool_switch=1`이다. 대조 배치가 T9 코드 이후에 돈 배치이므로 그
서술은 낡았다. **T12 문서화에서 정정할 것.**

## 5. 결론

- llama.cpp 스택 전환은 `tasks/` 12과제 소형 레포 트랙에서 **동등성을
  깨뜨리지 않았다** (35/36 vs 대조 33/36, ±4 이내)
- 기계 검사 3종 전건 통과 — C1형 조용한 전면 실패는 발생하지 않았다
- 이 배치가 **T8 회귀 게이트의 기준**이 된다: 게이트 총합이
  `35 − 4 = 31` 미만이면 미달
