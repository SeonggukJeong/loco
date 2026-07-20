# M14 회귀 게이트 배치 결과 — 정직한 검증 신호 II

- 사전등록: `docs/experiments/2026-07-20-honest-verification-ii/pre-registration.md`
  (승인 커밋 `ff21372` — 상태 행 갱신 커밋이 승인의 성립 근거, 전언 승인 불가)
- 스탬프: `.loco/eval/20260720T090943Z`
- 대상 커밋: `a45e268` (Task 1~11 완료 시점, 브랜치 `m14/honest-verification-ii`)
- 대조: `.loco/eval/20260719T093254Z` (커밋 `30c5615`, M13 T8 게이트 배치 —
  **M13 앵커 `20260719T082030Z`가 아니다.** 사전등록 근거를 그대로 승계: 이
  배치가 M14 직전 상태와 같은 커밋이고, 앵커를 쓰면 T7 한 태스크 분량의
  코드 변경이 M14의 변화인 것처럼 섞여 들어간다)
- 수행: 2026-07-20 09:09:43Z ~ 09:55:46.9Z (벽시계 2763.9초 = 46.1분, 런
  가중 평균 76.1초/런 — `report.json` `duration_secs`/`avg_duration_secs`.
  종료 시각은 `started_at + duration_secs`로 계산한 값)

**이 문서의 모든 수치는 `report.json`과 `python3 scripts/exp_metrics.py`
1차 산출물을 직접 대조해 적은 것이다** — 러너 요약을 그대로 옮기지 않았다
(M12에서 서사가 뒤집힌 전례, CLAUDE.md
[[loco-measurement-report-cross-check]]). 재현 명령과 결과는 각 절에 병기한다.

법의학 분석은 `.superpowers/sdd/m14-forensics.md`(독립 에이전트, 읽기 전용
분석 — 배치 재수행 없음)의 결과를 인용한다. 이 리포트는 그 확신도 표기를
그대로 보존한다 — 완화하거나 강화하지 않는다. **팔당 n=1, 반복 없음.**
아래 모든 "차이"는 단일 배치 대 단일 배치이며, 어느 항목도 통계적 유의성을
주장하지 않는다.

## 1. 배치 조건 (실측 등록)

```
$ python3 -c "import json; d=json.load(open('.loco/eval/20260720T090943Z/report.json')); print(d['effective_config'])"
{'base_url': 'http://localhost:8080/v1', 'temperature': 0.1, 'context_tokens': 8192,
 'max_output_tokens': 4096, 'max_turns': 25, 'command_timeout_secs': 60, 'loco_version': '0.1.0'}
```

대조 배치(`20260719T093254Z`)의 `effective_config`도 동일 조회로 확인했고
7개 필드 전부 일치한다. 명령은 두 배치 모두 `cargo run -- eval tasks
--repeats 3 --seed 0`. `git diff a45e268..HEAD -- src/`는 이 리포트 작성
시점에도 비어 있다(대상 커밋 이후 문서 전용 커밋만 쌓였음을 재확인).

## 2. 게이트 판정

| | M14 게이트 | 대조 |
|---|---|---|
| 스탬프 | `20260720T090943Z` | `20260719T093254Z` |
| 커밋 | `a45e268` | `30c5615` |
| 통과 | **36/36** | 35/36 |
| 엄격 | **35/36** | 35/36 |
| 거짓 성공 finish | 0 | 0 |
| `schema_fallback_count` | 0 | (필드 없음 — B-3 이전 배치) |
| `duration_secs` | 2763.9 | 2394.1 |
| `avg_duration_secs` | 76.1 | 65.9 |

- **임계값**: 사전등록 §임계값 근거에 따라 **≥33/36**(M12 방식, M13
  "앵커−4"=31 불채택). **36/36으로 최초 측정에서 충족** — 사전 공약된
  1회 재측정은 사용하지 않았다.
- 유일한 비엄격 런: `find-definition` repeat 2 (`passed=true`,
  `outcome=timeout`). §6-5에서 법의학적으로 다룬다.
- **거짓 성공 finish 0건** — 게이트를 넘는 데 "결과 없이 finish"가
  기여하지 않았다.

`schema_fallback_count`는 M14의 B-3(신규 집계)이 처음 채우는 필드다.
대조 배치는 B-3 이전 코드로 돌았으므로 그 필드 자체가 `report.json`에
없다 — 대조에서 "0"이 아니라 "필드 부재"로 정확히 구분해 적는다.

## 3. 과제별

| 과제 | M14 | 엄격 | 대조 | 엄격 |
|---|---|---|---|---|
| add-function | 3/3 | 3 | 3/3 | 3 |
| chain-edits | 3/3 | 3 | 3/3 | 3 |
| count-usages | 3/3 | 3 | 3/3 | 3 |
| create-module | 3/3 | 3 | 3/3 | 3 |
| edit-crlf-file | 3/3 | 3 | **2/3** | 2 |
| find-definition | 3/3 | **2** | 3/3 | 3 |
| fix-compile-error | 3/3 | 3 | 3/3 | 3 |
| fix-failing-test | 3/3 | 3 | 3/3 | 3 |
| fix-off-by-one | 3/3 | 3 | 3/3 | 3 |
| implement-from-doc | 3/3 | 3 | 3/3 | 3 |
| multiline-string-edit | 3/3 | 3 | 3/3 | 3 |
| rename-function | 3/3 | 3 | 3/3 | 3 |

M14가 대조 대비 통과에서 앞서는 유일한 지점은 `edit-crlf-file`
(대조 2/3 → M14 3/3)이며, `find-definition`은 엄격에서만 1건 뒤진다
(둘 다 §6-5에서 다루듯 기존 `finish_reason: length` 사각지대이지 신규
장치 귀속이 아니다). **n=3 과제 단위 이동을 M14의 효과로 읽지 않는다** —
이 리포트는 판정을 36/36 대 임계값 하나로만 내린다.

## 4. 관측 항목 — 사전등록 declared 4항목에 대한 답 (판정 아님)

사전등록 "관측 항목" 절이 선언한 순서 그대로 답한다. **이 절의 어떤
수치도 게이트 판정에 반영되지 않는다** — 36/36 ≥ 33/36 하나가 판정의
전부다.

### 4-1. A-1 파이프 해제 차단 발동 횟수

`pipe_unreleased` 열 합계: **M14 0, 대조 0.** `verify_nudge_pipe` 열도
**양쪽 0.**

오발동 여부 전수 확인: 발동 런이 0건이므로 대조할 런 자체가 없다.

**근본 원인 — 파이프 있는 `run_command` 호출 자체가 0건이다.** 두
배치의 모든 `run-*.jsonl`에서 `kind=="assistant"`인 레코드의 `content`를
JSON으로 파싱해 `action.tool=="run_command"`인 호출을 전수 추출했다
(재현 스크립트는 `.superpowers/sdd/m14-forensics.md` §Shift 2와 동일 절차,
이 리포트 작성자가 독립적으로 재실행해 일치 확인):

```
M14:  run_command 호출 54건. 고유 명령: '', cargo build 2>&1,
      cargo build 2>&1 && cargo test 2>&1, cargo test,
      cargo test --lib shapes::perimeter, cargo test 2>&1,
      cat answer.txt, cat src/lib.rs, xxd data/greeting.txt
대조: run_command 호출 58건. 고유 명령: cargo build 2>&1,
      cargo build 2>&1 && cargo test 2>&1, cargo test, cargo test 2>&1,
      cat answer.txt, cat src/lib.rs, grep -rn "fn area" src/,
      od -c data/greeting.txt, xxd data/greeting.txt
```

`|` 문자를 포함한 명령: **두 배치 모두 0건.** `&&`/`2>&1`은 파이프가
아니고 `has_unquoted_pipe`가 false를 반환하는 것이 정의상 옳다.

**"고장"이 아님을 소스에서 별도로 확인했다** (법의학 문서 §Shift 2 인용):
배선은 `src/agent/mod.rs:575-596`(`is_piped` 판정 → `record_command_result`
→ `unreleased_due_to_pipe`), `src/agent/mod.rs:387, 508, 634`
(`VERIFY_NUDGE_PIPE`/`FINISH_NUDGE` 파이프 변형 선택), `src/agent/status_note.rs:157,
164-165`(규칙 4 무효화 + `"via pipe"` 한정자)에 존재하고, 전용 단위
테스트 5개(`src/agent/mod.rs:2230, 2245, 2265`; `src/agent/status_note.rs:455,
472`; `src/tools/run_command.rs:190, 220`)가 잡고 있다.

**판정: 트리거 조건 미발생. "불필요"도 "고장"도 아니다.** 이 배치는
장치가 실제 상황에서 올바르게 발화한다는 증거를 제공하지 않는다 — 그
증거는 이 배치의 몫이 아니다(사전등록 "성격" 절). 선례는 M13의
`ARGS_TOOL_SWITCH_NOTE`가 20세션에서 0이었던 것과 동형이다: 12개
`tasks/` 과제가 파이프를 유도하지 않을 뿐이며, 파이프 마스킹은 원래
M13 파일럿의 실사용에서 관측된 실패 양상이다. 사전등록 자체가
**"효과는 `tasks/`로 실증되지 않는다"**고 미리 인정하고 들어간 사실과
정합한다.

**이 배치에서 신뢰 범위를 가진 유일한 M14 신규 장치는 A-3(모델 대면
diff)이다** — §4-3.

### 4-2. FINISH_NUDGE 발동 횟수

`finish_nudge_total`(=`finish_nudge`+`finish_nudge_pipe`, 상호배타) 합계:
**M14 0, 대조 0.**

**이 항목은 §3-3-3 회귀를 배치에서도 "볼 수 있게" 하려는 관측이었으나,
결과는 "악화되지 않았다"가 아니라 "측정되지 않았다"다.** 근거:

무장 조건("뮤테이션 성공 후 `exit code: 0` run_command")은 **양 배치
모두 36/36 런에서 충족**됐다(전 런 재생, 법의학 문서 §Shift 2). 그러나
무장 이후 다음 assistant 턴 수 분포:

| 무장 후 assistant 턴 수 | 대조 런 수 | M14 런 수 |
|---|---|---|
| 1 | 28 | 27 |
| 2 | 1 | 4 |
| 3 | 2 | 1 |
| 4 | 3 | 1 |
| 5 | 1 | 1 |
| 6 | 0 | 1 |
| 7 | 1 | 1 |

FINISH_NUDGE는 K=4 창 안에 탐색 턴 4개 + 반복 호출 1회 이상을 요구한다.
27~28개 런이 무장 직후 바로 `finish`를 호출해 창 자체가 형성되지
않는다. 꼬리가 가장 긴 런 각 1개(양 배치의 `run-edit-crlf-file-1`)를
정독 확인한 결과 둘 다 K=4 조건 미충족으로 **미발화가 정답**이었다
(M14: 뮤테이션이 탐색 턴 안에 끼어들어 무장을 해제; 대조: 반복 호출은
있으나 탐색 턴이 K=4에 못 미침).

**대조군도 0이므로 이 지표는 이 배치에서 판별력이 없다** — 기준선
자체가 0이면 그 지표로 회귀의 존재도 부재도 말할 수 없다. §3-3-3의
회귀 여부는 이 스포트 배치가 아니라 §7 기준 6(A-1 풍선효과 가드 단위
테스트 — 서로 다른 두 파이프 명령 교대 5회 → FINISH_NUDGE 발동)이
이미 잡았고, 그 가드는 배치 레벨과 무관하게 유효하다. **이 배치가
§3-3-3에 대해 제공하는 것은 "악화되지 않았다"가 아니라 "측정되지
않았다"이며, 그렇게 기록한다.**

풍선효과 관련 나머지 관측(finish 누락 스트릭 등)은 §4-4에서 함께 다룬다.

### 4-3. A-3 diff 첨부 횟수 · 절단 횟수

**첨부 횟수** — `model_diff` 열 합계: **M14 56, 대조 0**(대조는 A-3 이전
코드). 56건의 내역은 마커 `" lines, +"` 보유 결과 레코드를 전수
분류해 **`edit_file` 성공 렌더 44건 + `write_file` 성공 렌더 12건**임을
확인했다(법의학 문서, 이 리포트 작성자가 재확인). **"diff 렌더 56회"가
맞고 "edit_file 56회"는 아니다** — `edit_file` 호출 수와 혼동하지
않는다.

**절단 횟수** — 전용 열 없음(사전등록이 미리 명시한 사각지대,
`exp_metrics.py`의 `MARKS`에 미포함). 대체 절차대로 `run-*.jsonl`
전체에서 리터럴 `"[diff truncated]"`(`src/tools/diff.rs:67`)를 grep:

```
$ grep -l "\[diff truncated\]" .loco/eval/20260720T090943Z/run-*.jsonl
run-implement-from-doc-0.jsonl
run-implement-from-doc-1.jsonl
run-implement-from-doc-2.jsonl
$ grep -o "\[diff truncated\]" .loco/eval/20260720T090943Z/run-*.jsonl | wc -l
       3
$ grep -o "\[diff truncated\]" .loco/eval/20260719T093254Z/run-*.jsonl | wc -l
       0
```

**절단 3건, 전부 `implement-from-doc`의 3개 반복 각 1건씩.** 이 과제는
새 파일을 처음부터 작성하는 형태가 아니라 기존 파일을 상당 분량
고치는 형태라 A-3의 예산(스펙 §3-5-1)을 규칙적으로 넘긴 것으로 보이나,
n=1(과제 1개, 반복 3회뿐)이라 이 배치만으로 "이 과제 유형이 항상
절단된다"고 일반화하지 않는다 — 관측만 한다. 대조는 A-3 이전이라
구조적으로 0.

### 4-4. 풍선효과 감시

- **finish 누락 스트릭** (`finish_missing`/`finish_missing_maxrun`):
  M14 **0**건, 대조 **1**건(`run-count-usages-1`, maxrun 1). M14가 늘리지
  않았다 — 오히려 대조에 있던 유일한 발생이 M14에서는 재현되지 않았다
  (n=1, 방향을 주장하지 않는다).
- **거짓 성공 finish**: `report.json` `false_finish_count` 양 배치 **0**.
- **`stop_cause` 분포**: 양 배치 모두 36런 전부 `-`(=RepetitionStop 아님).
  `outcome` 분포도 동일하게 finished 35 + timeout 1. **`repetition_stop`
  런이 아예 없으므로** 이 watch 항목 자체가 이 배치에서 발생하지 않았다.
- **동일 파이프 명령 재실행으로 인한 반복 정지**: 위와 같은 이유로
  해당 없음 — `repetition_stop` 런이 0건이라 대조할 대상이 없다.
- **`REPEAT_CORRECTION` 직후 finish가 파이프 VERIFY_NUDGE로 거부된
  횟수**: `verify_nudge_pipe`가 양 배치 모두 0이므로 정의상 **0**이다
  (파이프 VERIFY_NUDGE 자체가 한 번도 발화하지 않았다 — §4-1과 같은
  근본 원인).

풍선효과 4항목 전부 "악화 없음"으로 읽을 수 있는 형태로 나왔으나,
그 중 3항목(`stop_cause`, 파이프 재실행 정지, REPEAT_CORRECTION 직후
파이프 거부)은 §4-1과 같은 이유로 **트리거 조건 자체가 없어 판별력이
없다.** 유일하게 실질적으로 비교 가능한 것은 finish 누락 스트릭이며,
그마저 n=1 대 0의 단일 사건이다.

## 5. 행동 지표 — 전체 요약 (판정 아님)

`python3 scripts/exp_metrics.py <stamp>`의 `# summary` 줄을 두 배치 각각에
대해 직접 실행해 그대로 옮긴 값이다(재현 완료, 발주 표와 전건 일치).

| 지표 | 대조 | M14 |
|---|---|---|
| `sr_error` | 31 | 35 |
| `sr_correction` | 7 | 11 |
| `recovered`(2트라이) | 28/31 | 28/35 |
| `repeat_corr` | 5 | 6 |
| `finish_missing` | 1 | 0 |
| `status_note` | 78 | 81 |
| `verify_total` | 3 | 4 |
| `verify_allpass` | 0 | 0 |
| `args_tool_key` | 27 | 28 |
| `args_tool_switch` | 1 | 2 |
| `length_retry` | 7 | 9 |
| `status_no_summary` | 0 | 4 |
| `model_diff` | 0 | 56 |
| `cargo_after_mut` | 35/36 | 34/36 |
| `pipe_note`/`verify_nudge_pipe`/`finish_nudge_pipe`/`status_pipe_qual`/`finish_nudge` | 0 | 0 |
| `[diff truncated]`(grep 직접 카운트, 추출기 미추적) | 0 | 3 |

**⚠ `verify_total`/`verify_allpass`를 이 표에서 M14 전후로 직접 비교하지
말 것** — §7의 비교가능성 각주를 볼 것. 이 배치에서는 두 값 모두 0에
가까워(대조 0/3, M14 0/4) 각주가 우려하는 "폴백이 하락으로 오독되는"
상황이 애초에 발생하지 않는다(폴백 자체가 §4-1처럼 파이프 명령이
0건이라 발동할 조건이 없었다) — **각주가 불필요해진 것이 아니라, 이
배치에서는 각주가 걱정하는 오독 경로가 우연히 막혀 있을 뿐이다.**
다음 배치에서 파이프 사용이 관측되면 다시 살아나는 위험이다.

## 6. 법의학 — 관측된 이동의 귀속

전 항목 **팔당 n=1, 반복 없음, 사후 분석**이다. `.superpowers/sdd/m14-forensics.md`
의 확신도 표기를 그대로 옮긴다.

### 6-1. `sr_error` 31 → 35, 회복 정체 — **M14 비귀속** (확신도 높음, 직접 반증)

우려했던 가설(Task 6이 `edit_file`의 ±3줄 컨텍스트를 diff로 바꿨고, 이
레포는 모델이 결과 본문을 다음 `search`로 복사하는 현상을 문서화하고
있다)은 **반증되었다.**

**증거 1 — 증분은 단일 실행에 집중, 양방향 churn.** 실행별 델타(이
리포트 작성자가 `exp_metrics.py` 원본에서 재수확인):

| 실행 | 대조 | M14 | Δ |
|---|---|---|---|
| `chain-edits-0` | 3 | 5 | +2 |
| `chain-edits-1` | 4 | 2 | −2 |
| `chain-edits-2` | 4 | 3 | −1 |
| `fix-compile-error-0` | 1 | 2 | +1 |
| `fix-failing-test-0` | 0 | 2 | +2 |
| `rename-function-0` | 1 | 0 | −1 |
| `rename-function-1` | 1 | 0 | −1 |
| `rename-function-2` | 0 | 4 | **+4** |
| 순 차분 | 14 | 18 | **+4** |

8개 실행이 양방향(+9/−5)으로 움직였다. 단조 증가가 아니다.

**증거 2 — 회복 실패 증분은 전부 실행 1개에서 나온다.**
`sr_recovered < sr_recovery_denom`인 실행: 대조는 `add-function-2`(1/2),
`fix-off-by-one-1`(1/2), `fix-off-by-one-2`(1/2) 3건. M14는 **동일 3건이
동일 수치로 유지**되고 `rename-function-2`(0/4) 1건이 추가될 뿐이다
(이 리포트 작성자가 재확인, 위와 일치). 추가된 4건은 서로 다른 4개
사건이 아니라 **한 실행 안에서 동일 호출이 4회 반복된 것** — 실효
표본은 n=1 사건이다.

**증거 3 — diff 복사 가설의 직접 반증.** M14 `run-rename-function-2`의
실패 호출(rec 21/24/28/32, 4회 동일):

```json
{"path": "src/receipt.rs",
 "search":  "    format!(\"total: {}\", total_price(items))",
 "replace": "    format!(\"total: {}\", total_price(items))"}
```

이 실행에서 rec 21 이전에 렌더된 diff는 rec 15 단 하나(`use crate::cart::total_price;`
→ `use crate::cart::price_total;`)뿐이고, `format!` 문자열은 그 diff
본문 어디에도 없다. 실제 출처는 rec 19의 `cargo test` 출력에 담긴
rustc 진단(`help: a function with a similar name exists` 아래 `-`/`+`
줄)이며, **이 rustc 출력은 두 배치에서 동일**하다 — loco가 만든 텍스트가
아니다. 모델은 rustc의 `-` 줄을 `search`·`replace` 양쪽에 복사했다
(`+` 줄을 `replace`에 넣는 데 실패).

**증거 4 — 배치 전수 스캔: diff 마커를 포함한 `search`는 0건.** 두
배치의 모든 `edit_file` 실패(M14 45건, 대조 37건)에서 직전 `search`
문자열이 `-`/`+`/`@@`로 시작하는 줄을 갖는지 검사: **M14 0/45, 대조
0/37.** 또한 그 실패 시점까지 렌더된 모든 결과 본문에 `search` 문자열이
그대로 들어 있는지("결과 본문 복사" 패턴) 검사: **M14 0/35, 대조
1/31**(`run-multiline-string-edit-1` rec 8) — **가설이 예측하는 방향의
정반대**(현상이 diff 장치가 없는 대조에서 1회 관측되고 M14에서는
0회다).

**증거 5 — 결정적: 동일 실패가 대조군에도 바이트 단위로 존재한다.**
대조 `run-rename-function-1.jsonl` rec 15는 M14 `run-rename-function-2`
rec 21과 경로·검색문자열·들여쓰기·`search==replace` 축퇴까지
**완전히 동일**하다. **M14의 diff 장치가 존재하지 않는 팔에서 같은
실패가 발생했다.** 두 실행의 차이는 실패 발생이 아니라 회복 경로다 —
대조는 `read_file` 2회로 실제 텍스트를 확인해 2트라이 회복, M14는
동일 호출이 4회 반복된 뒤 SR_CORRECTION·REPEAT_CORRECTION이 주입되고
`write_file`로 전환해(SR_CORRECTION의 처방문을 정확히 따랐다) 성공했다
(`passed=true, outcome=finished`). 두 배치 모두 `rename-function` 3회
반복 중 정확히 1회 이 축퇴 실패를 겪었다 — 발생률은 동일하고 반복
깊이만 다르다.

**함의**: `sr_correction` 7→11 증가도 같은 사건의 종속 결과다 —
`rename-function-2` 한 실행이 반복 4회에 걸쳐 교정을 여러 번 유발했다.
독립 신호로 읽지 않는다. M14가 교정을 무력화했다는 해석은 트랜스크립트와
배치된다 — 개입은 실제로 작동해 실행을 통과시켰다.

**반증 가능성에 대한 정직한 진술**: 검증된 것은 "실패 `search` 텍스트가
loco diff 본문에서 유래하지 않았다"와 "동일 실패가 대조군에 존재한다"
둘이다. **검증되지 않은 것**은 "diff 형식 노출이 모델을 축퇴
`search==replace` 쪽으로 일반적으로 편향시킨다"는 **약한 프라이밍
가설**이다 — 직접 복사가 아니어도 형식 노출이 확률을 바꿀 수 있고,
이 가설은 **n=1·반복 없음에서 원리적으로 판정 불가**다. 증거 5(대조군에
동일 실패 존재)와 증거 1(양방향 churn)은 이 가설을 지지하지 **않지만**,
반증하지도 않는다. 판정하려면 diff on/off 2팔 × 반복 다수의 사전등록
실험이 필요하다 — 이 배치의 설계로는 할 수 없다.

### 6-2. 파이프 계열 장치 전건 0

§4-1에서 이미 답했다 — 트리거 조건 미발생, 확신도 높음.

### 6-3. `args_tool_switch` 1 → 2 — M13 "미관측" 상태 갱신

발화 위치: 두 배치 공통 `run-fix-compile-error-1` 1회 + M14 추가
`run-rename-function-2` 1회. M14 추가분의 실체(`run-rename-function-2.jsonl`
rec 35 → rec 37):

```json
{"action": {"args": {"path": "src/receipt.rs", "tool": "write_file"}, "tool": "run_command"}}
```

`action.tool`은 `run_command`인데 `args.tool`이 `write_file`을 지목 —
규칙이 `write_file`로 재디스패치하고 `ARGS_TOOL_SWITCH_NOTE`를 붙였다.
결과는 `missing field content` 오류였으나(모델이 `content`를 안
보냄), **규칙 자체는 명세대로 정확히 동작**했고 이 전환은 §6-1의 S/R
루프 탈출 경로의 일부다.

**의의**: M13이 20세션에서 0회로 "미관측"이라 기록한 규칙이 이 법의학이
분석한 2개 배치 — 대조 `20260719T093254Z`(M13 T8 게이트, 1회)와 이 배치
`20260720T090943Z`(2회) — 에서 총 3회 실발화했다. `20260718T222824Z`
(M12 회귀 게이트 배치 — 이 법의학의 분석 대상이 아니었다)에서도 별도로
1회 발화가 있어, `python3 scripts/exp_metrics.py`로 세 배치를 모두
재확인하면 총 4회다. "불필요"가 아니었음이 뒷받침된다 — CLAUDE.md의
`ARGS_TOOL_SWITCH_NOTE` "미관측" 서술은 이 배치로 갱신 대상이다. 다만
n이 작아 발화율은 추정하지 않는다.

### 6-4. `length_retry`(7→9)·`status_no_summary`(0→4)·`cargo_after_mut`(35→34)

**`length_retry`**: 실행별로 증가 3·감소 2·동일 3의 **양방향 churn**
(`count-usages-1` 0→1, `edit-crlf-file-1` 2→1, `find-definition-0` 1→2,
`find-definition-1` 1→1, `find-definition-2` 1→2, `fix-compile-error-0`
1→1, `fix-compile-error-2` 1→0, `fix-off-by-one-2` 0→1). CLAUDE.md가
이미 기록한 기존 사각지대(소형 모델의 `finish_reason: length` 루프)이며
두 팔 모두에 존재한다. M14 귀속 근거 없음(중간 확신도, n=1).

**`status_no_summary`**(M14 신규 한정자, 대조는 한정자 자체가 없어 0이
자명): 발화 4건 전수 확인 — `fix-compile-error-0` rec 12
(`cargo build 2>&1`, exit 101), `-1` rec 11(`cargo test 2>&1`, exit 101),
`-1` rec 19(`cargo test 2>&1`, exit 101), `-2` rec 12(`cargo build 2>&1`,
exit 101). **4건 모두 컴파일 실패라 libtest 요약 줄이 애초에 출력되지
않은 경우**다. 한정자는 참인 사실만 진술했다 — 오탐 0건. 마일스톤
논지("하네스가 모르는 것을 단언하지 않는다")대로 동작(확신도 높음).

**`cargo_after_mut`**(35/36→34/36): 차분은 `find-definition` 3개
실행 안에만 있다 — repeat 0 `1→0`, repeat 1 `0→1`, repeat 2 `1→0`.
순 −1이고 양방향. `find-definition`은 뮤테이션이 `answer.txt` 쓰기라
`cargo test`가 자연스러운 검증이 아닌 과제이며, 다른 11개 과제에서는
차분이 0. 체계적 회귀로 읽을 근거 없음(중간 확신도, n=1).

### 6-5. 유일한 비엄격 실행 — `find-definition` repeat 2 (`outcome=timeout`, `passed=true`)

원인은 벽시계 타임아웃. `report.json` per-run `duration_secs` =
**305.00초**(배치 내 최장). 대조도 정확히 1건의 타임아웃을 가진다 —
`edit-crlf-file` repeat 1, **302.59초**, 그쪽은 `passed=false`. 각 팔에
동일 한계선의 타임아웃이 1건씩 있고, M14 쪽은 이미 정답을 기록한
뒤였던 덕에 통과했다.

기전: rec 3~7에서 `grep` → `write_file answer.txt` 성공, **여기서
이미 `passed=true`가 확정**됐다. 이후 rec 9에서 `finish_reason: length`
(모델이 동일 JSON 객체를 반복 생성하다 잘림), 재요청 후 rec 16에서
**2회째** length 절단, rec 19에서 타임아웃.

**M14 장치 연루 여부 — 검증 결과 없음(확신도 높음, 대조군 동일 절단 +
컨텍스트 바이트 동일 + 배선 소스 확인)**:

1. 대조의 동일 과제·동일 반복도 **같은 지점(rec 7)에서 length 절단**을
   겪었다 — 대조는 `(empty)` 본문으로, M14는 반복 JSON 블롭으로 잘렸을
   뿐 동일 실패 양상이고 생성량만 다르다. 순수 샘플링 분산이다.
2. 절단 시점까지 두 팔의 tool_result는 **바이트 동일**하다 — 신규
   파일 쓰기라 A-3(diff 렌더)이 개입하지 않았고, `[status]` 블록 문안도
   동일하다. 모델이 본 컨텍스트에 M14발 차이가 없다.
3. M14 트랜스크립트가 20레코드(대조 14레코드)인 것은 M14의 C-2가
   `finish_reason`/notice를 **트랜스크립트에만** 추가 기록하기 때문
   (`Session::record_extra`, `src/session.rs:116-118`은 `self.transcript.record(...)`만
   수행하고 `self.messages`를 건드리지 않는다 — 소스 확인). 모델이 보는
   컨텍스트는 오염되지 않는다.

**결론: 어떤 M14 장치도 연루되지 않았다.** 기존 `finish_reason: length`
루프 사각지대이며 두 팔 모두에 나타난다.

## 7. 비교가능성 각주 재확인 — `verify_allpass`·`verify_total`

`docs/baselines.md` "M14 — 비교가능성 각주" 절(스펙 §8-3 전문)이 이미
경고한 대로, **§3-4-2 규칙 4 → 규칙 5 폴백이 이 두 원지표의 모집단을
바꾼다.** 폴백은 파이프 + allpass 렌더를 규칙 5 문자열로 보내
`verify_total`/`verify_allpass` 어느 리터럴 패턴에도 걸리지 않게 한다 —
모델이 파이프를 쓰는 만큼 두 값이 함께 내려가고, 그 하락은 회귀가
아니라 폴백이 작동한 증거일 수 있다.

이 배치에서는 `verify_total` 3→4, `verify_allpass` 0→0(§5 표)이다.
**직접 비교하면 안 된다는 각주는 여전히 유효하지만, 이 배치에서는
그 각주가 우려하는 오독 상황 자체가 실질적으로 발생하지 않는다** —
§4-1·§6-2가 이미 확인했듯 파이프 있는 `run_command`가 두 배치 모두
0건이라 폴백이 발동할 조건(파이프 + allpass)이 애초에 없었다.
`verify_allpass`가 양쪽 다 0인 것은 폴백의 흔적이 아니라 "이 배치의
36개 런 중 규칙 4(all P passed, exit-code 교차검증까지 통과)가 렌더된
런이 하나도 없었다"는 별개의 사실이다. **각주가 불필요해진 것이
아니다** — 다음 배치에서 파이프 사용이 관측되면 이 폴백은 다시
`verify_allpass`를 끌어내릴 수 있고, 그때는 이 각주를 반드시 다시
적용해야 한다.

## 8. 결론

- **게이트 통과** — 36/36(≥33/36), 엄격 35/36, 거짓 성공 finish 0. 최초
  측정에서 임계값을 충족해 사전 공약된 재측정을 쓰지 않았다.
- **관측 항목 4종(사전등록) 중 신뢰 범위를 가진 것은 A-3 하나뿐이다** —
  diff 56회 첨부(edit_file 44 + write_file 12) 확인, 절단 3회(전부
  `implement-from-doc`). A-1·FINISH_NUDGE·풍선효과 watch 대부분은 트리거
  조건(파이프 명령, K=4 창 형성, `repetition_stop`)이 이 스포트 배치에서
  발생하지 않아 **관측 불가**였다 — "불필요"로 재해석하지 않는다.
- **`sr_error` 31→35 상승은 M14에 귀속되지 않는다.** diff 복사 가설은
  반증됐고(증거 3·4·5), 대안 설명(단일 실행 내 반복 + 노이즈)이 적극적
  증거를 갖는다. 약한 프라이밍 가설은 이 설계로 판정 불가능하다.
- **`args_tool_switch`가 M13의 "미관측"을 갱신했다** — 신규 발견이며
  CLAUDE.md 갱신 대상이다.
- **`verify_allpass`/`verify_total`의 M14 비교가능성 각주는 이 배치에서도
  유효하다** — 우연히 이 배치의 오독 위험이 실현되지 않았을 뿐이다.
- **§3-3-3(FINISH_NUDGE 회귀 감시)은 이 배치로 측정되지 않았다** —
  대조군도 0이라 판별력이 없다. 그 감시는 §7 기준 6의 단위 테스트가
  전담한다.
- **전 항목 팔당 n=1, 반복 없음, 사후 분석.** "M14 비귀속"은 제시된
  인과 경로가 반증됐고 대안 설명이 적극적 증거를 갖는다는 뜻이지,
  "M14가 해당 거동에 아무 영향이 없음이 입증됐다"는 뜻이 아니다 — 후자는
  이 설계로 입증 불가능하다.
