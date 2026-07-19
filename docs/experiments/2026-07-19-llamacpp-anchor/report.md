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

> **T7 각주 (`verify_*` 비교가능성)** — 위 `verify_total` 행의 짧은 메모를
> 아래에서 풀어 쓴다. 둘은 같은 결론을 가리키며 서로 모순되지 않는다.
>
> `verify_total`/`verify_zero`/`verify_allpass`/`verify_failed`는 M13 T7("상태선
> 무뮤테이션 접지 — 수선 A·B") 이후 구조적으로 상향된다. T7 이전에는 검증
> 줄이 뮤테이션 분기에서만 렌더됐다(무뮤테이션 분기는 조기 반환이라 검증
> 줄이 아예 없었다). T7부터는 무뮤테이션 케이던스 노트에도 실린다. M12
> 실험 리포트 §4의 관측 지표 ②와 직접 비교하지 말 것.
>
> 이 앵커 배치(T6, 대상 커밋 `7d76a25`)와 대조 배치(M12 회귀 게이트,
> `20260718T222824Z`)는 **둘 다 T7 이전**이므로 위 표의 0 vs 2 자체는 이
> 각주의 영향을 받지 않는다 — 영향을 받는 것은 **T8 이후** 배치와의 비교다.
> T12 Step 2가 이 각주를 `docs/baselines.md`의 M13 절로 승계한다(T7 시점에는
> 그 절이 아직 없다 — 순서: T6 → T7 → T8 → … → T12).
>
> ⚠️ **알려진 커버리지 공백**(M13 T7이 만든 것이 아니며 T7에서 고치지
> 않았다 — 기록만 한다): 케이던스 노트를 억제하는 `!stop` 가드는 코드에
> **두 곳** 있다 — 디스패치 경로(`src/agent/mod.rs` 507 근방)와 거부 경로
> (게이트 거부, `src/agent/mod.rs` 408 근방). T7이 고친/좁힌 회귀 테스트
> (`repetition_stop_still_fires_with_status_note_active`)는 **디스패치
> 경로만** 고정한다. 거부 경로의 `!stop`을 통째로 제거해도 전체 스위트가
> 초록불이다(T7 구현 중 실측 재확인: `368 passed; 0 failed`, clippy 무경고).
> 두 가드가 모두 테스트로 핀 되어 있다고 오해하지 말 것 — 거부 경로는
> M13 범위 밖의 기지 공백으로 남는다.

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

---

# M13 배치 2 결과 — 회귀 게이트 (T8)

- 스탬프: `.loco/eval/20260719T093254Z`
- 대상 커밋: `30c5615` (T7 — 상태선 무뮤테이션 접지 수선 A·B)
- 수행: 2026-07-19 09:32:54Z ~ **10:12:48.1Z** (벽시계 2394.1초 = 39.9분,
  런 가중 평균 65.9초/런). 종료 시각은 `started_at + duration_secs`로 계산

수치는 모두 `report.json`을 직접 대조해 적었다.

## 6. 배치 전 — 조건 동일성과 T7 diff 정독

- `.loco/config.toml`: 앵커 배치의 `effective_config`와 4개 값 전부 일치
  (`context_tokens 8192`, `max_output_tokens 4096`, `command_timeout_secs 60`,
  `base_url http://localhost:8080/v1`) — 앵커와 **같은 조건**임이 요구사항이다
- `llama-server --version`: `9960 (a935fbffe)` — 앵커와 동일
- 스모크 8항 전건 통과 (`--verify` 12/12·3/3, json_schema **200**,
  `n_ctx_slot 8192`, `id ornith`, config 일치, 트립와이어 clear, 병행 빌드 없음,
  데몬화 적용). 기동 전 `rm -f .loco/serve-T8.log`

**T7 diff 사람 정독** (사전등록 §1 — 파일 범위 제약은 내용을 보증하지 않으므로
필수 절차). 확인자: 컨트롤러 세션, 확인 시각: 2026-07-19 09:30Z 경, 배치 시작 전.

- `git diff --stat 814d87c..30c5615 -- src/ scripts/serve.sh tasks/` →
  `src/agent/mod.rs`(21), `src/agent/status_note.rs`(60)만.
  `scripts/serve.sh`·`tasks/`는 변경 없음
- **프로덕션 코드 변경은 정확히 두 곳**: `ZERO_MUT_CADENCE`를
  `[5,10,15,20]` → `[3,5,7,10,15,20]`(수선 A), `render()`의 무뮤테이션 분기에
  `verification_line()` 삽입(수선 B)
- `src/agent/mod.rs`의 변경 21줄은 **전부 `mod tests` 안**(hunk 시작이 2111·2115·
  2191·2205이고 `#[cfg(test)]`는 651줄) — 프로덕션 동작 변경 없음
- 따라서 이 게이트 결과가 귀속할 수 있는 T7 변경은 위 두 곳으로 좁게 고정된다

## 7. 기계 검사 3종

| 순위 | 검사 | 결과 |
|---|---|---|
| 주 | 전 런 `schema_fallback == false` | **발동 런 없음** (필드 누락 0) |
| 보조 | `parse_fail_first(총) == 0` | **0** |
| 환경 | `n_ctx_slot == 8192` | **8192** |

로그 선후: birth `1784453521`, `$STAMP` epoch `1784453574` — **53초 앞섬**
(`%B` 사용). **3종 전건 통과.**

## 8. 게이트 판정

```
게이트 35/36 (엄격 35)   앵커 35/36 (엄격 34)   차이 +0 (엄격 +1)
```

- 기준(사전등록 §4-1): 게이트 총합 ≥ 앵커 − 4 = **31** → 35 ≥ 31 **통과**
- **안정 집합 위반 없음** (6개 전부 3/3, 적용 목록은 사전등록과 동일·드리프트 없음)
  → 분류 방아쇠 미발동, §4-1의 보류 갈래에 들어가지 않았다
- 재측정 조항은 발동하지 않았다 (미달이 아니므로). T7 되돌림 없음
- 거짓 성공 finish: 0

### 과제별 (게이트 vs 앵커)

| 과제 | 게이트 | 엄격 | 앵커 |
|---|---|---|---|
| add-function | 3/3 | 3 | 3/3 |
| chain-edits | 3/3 | 3 | 3/3 |
| count-usages | 3/3 | 3 | 3/3 |
| create-module | 3/3 | 3 | 3/3 |
| edit-crlf-file | **2/3** | 2 | 3/3 |
| find-definition | 3/3 | 3 | 3/3 |
| fix-compile-error | 3/3 | 3 | 3/3 |
| fix-failing-test | 3/3 | 3 | 3/3 |
| fix-off-by-one | 3/3 | 3 | 3/3 |
| implement-from-doc | 3/3 | 3 | 3/3 |
| multiline-string-edit | 3/3 | 3 | 3/3 |
| rename-function | 3/3 | 3 | **2/3** |

총합 ±0은 상쇄다(`edit-crlf-file` −1, `rename-function` +1). 두 과제 모두
안정 집합 밖이므로 방아쇠 대상이 아니다. 이 상쇄를 T7의 효과로 읽지 않는다 —
n=3의 과제 단위 이동은 이 설계로 구별할 수 없다.

**비정상 종료**: `edit-crlf-file-1` (`outcome=timeout`, `passed=false`, 10턴).
게이트의 유일한 실패 런이다. 앵커의 유일한 실패는 `rename-function-0`
(RepetitionStop)이었으므로 실패의 성격도 바뀌었으나, 각 1건이라 방향을
주장하지 않는다.

## 9. 수선 A·B 실측 관측 (판정 아님)

**수선 B는 실제로 배선됐다.** 무뮤테이션 노트에 검증 줄이 실린 건수:
**게이트 14건 vs 앵커 0건**(앵커는 T7 이전이므로 구조적으로 0).

검증 줄이 무엇을 접지했는지의 분포:

| 렌더된 검증 줄 | 건수 |
|---|---|
| `last command gave no exit code` (규칙 5) | 8 |
| `last command exited 101` (규칙 5) | 3 |
| **`last cargo test: 1 failed (max_of_list)`** (규칙 2) | **3** |

마지막 행이 이 수선의 존재 이유다 — 브리프가 동기로 든 `fix-failing-test-1`
시나리오, 즉 **모델이 곧 환각으로 "고치려 드는" 그 테스트 이름이 뮤테이션
0회 상태의 turn 3에 모델 앞에 놓이는 것**이 실제로 발생했다. 다만 이것은
장치가 발동했다는 관측이지 그 결과 모델이 더 잘했다는 증거가 아니다 —
이 배치는 그 인과를 판정할 설계가 아니다.

**수선 A**도 발동했다. 무뮤테이션 노트의 턴 분포: `turns: 3` 11건,
`turns: 5` 3건. T7 이전이라면 11건 전부 렌더되지 않았을 것이다.

### 관측 지표 (게이트 vs 앵커)

| 지표 | 게이트 | 앵커 | 비고 |
|---|---|---|---|
| `sr_error` | 31 | 41 | |
| `sr_correction` | 7 | 14 | |
| `recovered` | 28/31 | 37/41 | 90.3% vs 90.2% |
| `finish_missing` | 1 | 1 | |
| `status_note` | 78 | 69 | 수선 A 조밀화 반영 |
| `verify_total` | 3 | 0 | **구조적 상향 — §4 각주대로 비교 불가** |
| `args_tool_key` | 27 | 25 | |
| `cargo_after_mut` | 35/36 | 35/36 | |

`verify_total` 3 vs 0은 T7 수선 B가 만든 **구조적** 차이이며, 이 두 수를
성능 비교로 읽으면 안 된다(§4의 T7 각주).

## 10. 게이트 결론

- **게이트 통과** — T7의 상태선 수선은 소형 레포 트랙에서 회귀를 만들지
  않았다(35/36, 앵커 대비 ±0, 안정 집합 위반 없음)
- 수선 A·B 모두 프로덕션에서 발동함이 트랜스크립트로 실증됐고, 그중
  가장 값진 형태(실패 테스트 이름 접지)가 3건 발생했다
- T7 커밋 `30c5615`은 되돌리지 않는다. 파일럿(T11)은 이 코드로 진행한다
