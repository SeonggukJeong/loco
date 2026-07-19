# 실험 리포트: 정직한 하네스 — 회귀 게이트

- 사전등록: `pre-registration.md`(상태: 승인됨, 2026-07-19 — 사용자가 승인 판정을
  독립 리뷰어에게 명시 위임, 리뷰어 1R NOT APPROVED → 수정 `34d0321` → 2R
  **APPROVED**)
- 대상 커밋(사전등록 고정): `b87dca3`(T9 완료 시점) — 배치 시점까지
  `src/ tasks/ tasks-large/ Cargo.toml Cargo.lock scripts/` diff는 배치 1
  직전·직후·배치 2 직후 3회 재확인 전 구간 0줄(문서 전용 유지 확인)
- 작성: 러너(무인 수행, `.superpowers/sdd/task-11-report.md`) 기계적 사실 기록 +
  컨트롤러 판정 확정(본 문서) — 아래 수치는 러너 원자료를 `report.json`·
  `run-*.jsonl`·`scripts/exp_metrics.py` 재실행으로 전건 재검증한 결과이며,
  §4에 러너 초안의 오류 1건을 정정한다(정정 경위는 해당 절)

## 성격 (실험이 아니라 회귀 게이트)

M12는 새 개입의 효과를 입증하는 실험이 아니라, 신규 장치(검증 실질 접지·
오형 개입 일반화·S/R 도달률)가 기존 성능을 깨지 않았는지 확인하는
**회귀 게이트**다(스펙 §0-3 축소 결정·§5). uv(update-vat-rate) 통과율 개선은
M12의 목표가 아니다.

## 1. 배치 ↔ 커밋 ↔ 스탬프

| 배치 | 성격 | 조건 | 명령 | 스탬프 | `git rev-parse HEAD` | interrupted |
|---|---|---|---|---|---|---|
| 1 | 게이트 | v2(ctx 8192[로드 8192]·out 4096·timeout 60·temp 0.1) | `cargo run -- eval tasks --repeats 3 --seed 0` | `20260718T222824Z` | `2ce0f310c19c6b77f0959aab1eee99ec0ad4fe63` | false |
| 2 | 관찰(게이트 아님) | M11 조건(ctx 8192[로드 12288]·out 4096·timeout 240·temp 0.1) | `cargo run -- eval tasks-large --filter update-vat-rate --filter fix-monthly-total --repeats 10 --seed 0` | `20260718T231049Z` | `2ce0f310c19c6b77f0959aab1eee99ec0ad4fe63` | false |

두 배치 모두 HEAD 불변(측정 중 커밋 없음). `report.json.effective_config`가
사전등록 조건과 양쪽 모두 일치(배치 1: ctx 8192/out 4096/timeout 60/temp 0.1,
배치 2: ctx 8192/out 4096/timeout 240/temp 0.1). `lms ps`/`curl
/api/v0/models` 재확인: 배치 1 시작 시 ornith-1.0-9b 단독 로드 ctx 8192,
배치 2 전 재로드 후 ornith-1.0-9b 단독 로드 ctx 12288 — 조건과 일치.

대조 스탬프: 배치 1 = `20260718T115152Z`(M11 실험 2 스포트 재측정 배치,
33/36). 배치 2 = `20260718T082449Z` — **M11 실험 2 자신의 8K 개입 배치를
그대로 재사용**한 참조점(사전등록 명시: "M12는 이 축에 별도 대조군을 새로
만들지 않는다"). 이 스탬프는 M12 코드가 아니라 M11 시점 코드로 측정됐다 —
M12 배치와의 차이는 M12 신규 장치 하나만이 아니라 그 시점 이후의 모든
변경(M12 전체)을 포함한다.

## 2. 수행 메커니즘 (정직 기록)

macOS에는 `setsid` 커맨드가 없어(util-linux 전용) `python3 -c
"os.setsid(); os.execvp(...)"` 래퍼로 동등 효과(세션 분리, PPID=1 재부모화)를
`ps` 출력으로 확인 후 대체 수행했다 — 사전등록·브리프의 "setsid 데몬화"
문구를 문자 그대로는 만족하지 못했으나 조건·표본·시드·판정 규칙에는
영향이 없다. 최초 `setsid` 직접 호출 1회는 명령어 부재로 즉시 실패했으나
eval 프로세스가 시작되지 않은 상태에서 감지돼 GPU 시간 손실은 0이다.
`nohup ... & disown`으로 쉘에서 분리해 실행하고, 통지에 의존하지 않고
`report.json` 존재 여부와 `ps -p <pid>` 폴링(20~30초 간격)으로 완료를 직접
확인했다. 배치 2는 약 55분 소요(60분 상한 근접 — 데몬화가 실제로 필요했던
사례). 측정 중 `cargo build`/`test`는 실행하지 않았다(배치 사이에만 verify
게이트 재확인).

하네스 에러·LLM 에러·부분 리포트(`interrupted: true`) 없음 — 중단 규칙
미발동, 재수행 없음.

## 3. 배치 1 결과 — 게이트 판정 (≥33/36)

**33/36 통과(91.7%) — 게이트를 정확히 임계값에서 충족.** 엄격 32/36(88.9%),
거짓 성공 finish 1건, `avg_duration_secs` 68.07s/런(전체 2477.0s ≈ 41.3분).
대조(`115152Z`) = 33/36, 엄격 31/36, 거짓 성공 finish 0건.

| 과제 | 통과 | 엄격 |
|---|---|---|
| add-function | 3/3 | 3/3 |
| chain-edits | 3/3 | 3/3 |
| count-usages | 3/3 | 2/3 |
| create-module | 3/3 | 3/3 |
| edit-crlf-file | 3/3 | 3/3 |
| find-definition | 3/3 | 3/3 |
| fix-compile-error | 3/3 | 3/3 |
| fix-failing-test | 2/3 | 2/3 |
| fix-off-by-one | 3/3 | 3/3 |
| implement-from-doc | 3/3 | 3/3 |
| multiline-string-edit | 1/3 | 1/3 |
| rename-function | 3/3 | 3/3 |

실패 런 3건(전수, `report.json` 대조):

- `run-fix-failing-test-1` — outcome=finished, passed=False (거짓 성공 finish
  1건의 실체 — 귀속 분석은 §6)
- `run-multiline-string-edit-0` — outcome=repetition_stop, stop_cause=other
  (S/R 스트릭 아님)
- `run-multiline-string-edit-1` — outcome=repetition_stop, stop_cause=other
  (S/R 스트릭 아님)

outcome 분포: finished 33 / repetition_stop 3 (Timeout 0건 — CLI 출력의
"timeout×1"은 `--timeout-scale` 값 표시이지 발생 건수가 아니다).

**판정: 게이트 통과. 33/36은 사전등록 "≥33/36"을 만족하는 최솟값이다 —
재측정 없이 1회 측정으로 임계값에 정확히 걸렸다.** 사전등록 §종결 규칙에
따라 재측정은 게이트 미달 시에만 트리거되므로 본 건은 재측정 대상이
아니며, 실제로 재측정하지 않았다. 이 사실은 완화하지 않고 그대로
기록한다 — 통과 여유가 없다는 것 자체가 정직한 관측이다.

## 4. 배치 2 결과 — 관찰 지표 (게이트 아님)

**11/20 통과(55.0%)**, 엄격 9/20(45.0%), 거짓 성공 finish 3건,
`avg_duration_secs` 160.03s/런(전체 3278.9s ≈ 54.6분).

| 과제 | 통과 | 엄격 |
|---|---|---|
| fix-monthly-total | 9/10 | 9/10 |
| update-vat-rate | 2/10 | 0/10 |

outcome 분포: finished 12 / repetition_stop 2 / max_turns 6 (Timeout 0건).

### 정정: 러너 초안(`task-11-report.md`)의 대조 구성 오류

러너의 초안은 대조(`082449Z`)를 "uv 1/10·fm 10/10, 전체 11/20"으로
기록했으나, 이는 **틀린 값이다** — `082449Z`가 아니라 M10/M11이 재사용해온
더 오래된 스탬프 `20260717T152633Z`(fm 10/10 엄격8, uv 1/10, 전체 11/20)의
수치가 잘못 옮겨 적힌 것이다. `082449Z`의 `report.json`을 직접 대조하면:

| | `082449Z`(대조, 실측) | `231049Z`(배치 2) |
|---|---|---|
| 전체 | **10/20**(50.0%, 엄격 8/20, ff 3) | 11/20(55.0%, 엄격 9/20, ff 3) |
| fix-monthly-total | **8/10**(엄격 7, ff 0) | 9/10(엄격 9, ff 0) |
| update-vat-rate | 2/10(엄격 1, ff 3) | 2/10(엄격 0, ff 3) |

`python3 scripts/exp_metrics.py .loco/eval/20260718T231049Z
.loco/eval/20260718T082449Z`의 요약 행(`sr_recovered=23/30`)이 사전등록
§관찰지표의 명시값과 정확히 일치해 대조 스탬프 선정 자체는 맞았다 — 오류는
스탬프 선정이 아니라 그 스탬프의 **과제별 통과 구성**을 옮겨 적는
과정에서 발생했다. 정정된 그림은 러너 초안의 "총 통과 수 동일, 구성만
이동(uv 1→2, fm 10→9)"이 아니라 **"배치 2가 대조 대비 전체 +1런, fm +1런
(8→9, 엄격 7→9), uv는 통과 수 동률(2→2)이나 엄격은 1→0으로 하락"**이다.
관찰 지표이며 게이트 판정에는 영향이 없다(배치 2는 애초에 게이트가 아니다)
— 다만 이 마일스톤 자체가 하네스의 정직성을 다루므로, 문서 자신의 오류도
발견 즉시 정정해 기록한다.

### 사전등록 5개 관찰 지표 (기록 의무, 승패 아님)

| 지표 | 배치 1 (스포트) | 대조(115152Z) | 배치 2 (uv+fm@8K) | 대조(082449Z, 정정값) |
|---|---|---|---|---|
| ① `empty_test_note` 발동 수 | 0 | 0 | **2**(전부 `run-update-vat-rate-8`) | 0 |
| ② verification 실질 렌더 수(`verify_total`/`verify_allpass`/`verify_failed`) | 2 / 2 / 0 | 0/0/0 | 3 / 0 / 3 | 0/0/0 |
| ③ `sr_recovered`/분모 | 28/34 (82.4%) | 29/35 (82.9%) | 16/25 (64.0%) | 23/30 (76.7%) — 사전등록 명시값과 일치 |
| ④ 오형 스트릭발 반복정지(`stop_cause==sr`) | 0건 | 0건 | **1건**(`run-fix-monthly-total-3`, seed 3) | 0건 |
| ⑤ missing-field 오형률(런 본문 raw grep count) | 10건(9런에 분산) | 12건 | 5건(4런, 전부 update-vat-rate) | 10건 |

지표 ⑤는 `grep -o '"Error: invalid arguments: missing field' run-*.jsonl |
wc -l`로 재검증(4스탬프 전부 위 값과 일치).

## 5. 지표 해석 규율 교차 확인 (사전등록 5규율 — 기계적 적용)

1. **`sr_corr_total`=55 비교 금지**: 본 문서는 55를 어느 배치와도 비교하지
   않는다.
2. **`sr_corr_total` 증감 애매성**: 요약 행 자체에는 `sr_corr_total` 합계가
   집계되지 않는다(런별 컬럼). 배치 2에서 `sr_recovered` 비율이 대조 대비
   하락(76.7%→64.0%)하고 `stop_cause=sr` 반복정지가 0→1로 관측됐다 — 다만
   §6의 법의학이 보이듯 이 둘은 **같은 단일 런**(`fix-monthly-total-3`)에서
   나온 것이라 "루프를 일찍 끊었다"/"트리거 미달" 어느 해석의 근거로도 쓰지
   않는다(n=1).
3. **`sr_error` M12 경계 비교 금지**: 배치 1 `sr_error=34`·배치 2
   `sr_error=25`를 대조(115152Z=35, 082449Z=30)와 병기했으나 재분류 경계
   때문에 원자료 기록일 뿐 개입 효과로 서술하지 않는다.
4. **`args_tool_key`/`args_tool_switch` 첫 비영값 관측**: 배치 1에서
   `args_tool_key=35`·`args_tool_switch=1`, 배치 2에서 `args_tool_key=83`·
   `args_tool_switch=1` — 대조 두 스탬프는 구조적으로 0(pre-M12, 코드 경로
   부재)이므로 "과거 대비 증가"로 서술하지 않고 "실전 트랜스크립트에서도
   발동함을 보여주는 최초 관측"으로만 기록한다. 두 배치 합계 118건
   (35+83)을 전건 수동 대조한 결과 정상 형태(turn) 위 오탐 0건 —
   잘못 형성되지 않은 턴에 이 규칙이 잘못 발동한 사례는 없다.
5. **파이썬 재구현 세 컬럼(`sr_corr_total`·`perturb_turns_ext`·기존
   `perturb_turns`) 한계**: `agent/repetition.rs`의 Rust 상태기계를 독립
   재구현한 시뮬레이션이며 Rust 원본과의 동기화 자동검증이 없다. 배치 1의
   게이트 판정(33/36)은 `report.json.passed`에서 직접 나온 값이라 이
   한계의 영향을 받지 않는다.

## 6. 이상 징후 3건 — 법의학적 귀속 (정직 기록)

이 절이 본 문서의 핵심이다: 세 이상 징후 각각이 M12 신규 장치의 결과인지,
아니면 우연히 동시에 관측된 무관한 사실인지를 트랜스크립트 원자료로
가른다.

### 6-1. 배치 2의 신규 `stop_cause == sr` 반복정지 (`run-fix-monthly-total-3`)

**M12의 새 실패가 아니다.** 대조(`082449Z`)의 동일 seed 런도
`repetition_stop`·`passed=False`로 이미 실패하고 있었다 — `stop_cause`는
마지막 결과 바디의 순수 함수이므로 **바뀐 것은 라벨뿐**이다(대조에서는
`stop_cause=other`, 배치 2에서는 `stop_cause=sr` — 실패 자체는 두 배치
모두 존재).

궤적 자체는 M12가 갈랐다: 두 배치의 `run-fix-monthly-total-3.jsonl`을
이벤트 단위로 직접 대조하면 이벤트 0~4(system·user·turn1 read_file·그
결과·turn2 edit_file 시도)까지 **바이트 동일**하고, 첫 차이는 이벤트 5
(turn2 edit_file 실패의 tool_result 본문)에서 나타난다:

- 대조(082449Z): `"...Put the code as it is NOW in \`search\`, and the
  code AFTER your change in \`replace\`."`
- 배치 2(231049Z): 위 문장 + `" The file was NOT modified - it still
  contains your search text unchanged."`(T1 §4-2-1이 추가한 소품 문장)

이후 두 궤적은 갈라진다 — 대조는 같은 오류를 2회 더 반복한 뒤
REPEAT_CORRECTION을 거쳐 최종적으로는 다른 경로(missing-field 루프)로
5회째 `repetition_stop`(stop_cause=other)에 도달하고, 배치 2는 상태선이
개입(`[status] files edited: none yet | turns: 5 of 25 used`)한 뒤에도
같은 S/R을 계속 반복해 `stop_cause=sr`로 종결한다. **Axis-2(파일별
S/R 카운터·SR_CORRECTION) 장치는 두 배치에서 차등 발동하지 않았다** —
`sr_correction` 컬럼은 양쪽 다 1(단일 파일이라 파일별 래치가 M9의 런
래치와 동일하게 동작). 즉 이 런의 결말 차이는 확대된 트리거·파일별
카운터가 아니라, **소품 문장 하나가 만든 궤적 분기**에서 비롯됐다는 것이
트랜스크립트에서 직접 확인된다.

### 6-2. 배치 2의 `sr_recovered` 하락(76.7% → 64.0%)

**분산된 회귀가 아니라 같은 단일 런이다.** `fix-monthly-total-3`을 양쪽
배치에서 제외하면:

- 대조: sr_recovered 23/30 → **21/28 (75.0%)**(그 런의 기여분: 대조에서는
  2/2 전량 회복)
- 배치 2: sr_recovered 16/25 → **16/20 (80.0%)**(그 런의 기여분: 배치
  2에서는 0/5 전량 미회복)

제외 후에는 **배치 2가 대조보다 높다**(80.0% > 75.0%). 즉 76.7%→64.0%의
하락은 25개 표본 전반에 분산된 열위가 아니라, 한 런이 반복정지로
종결되며 그 런 안의 S/R 오류 5건이 전부 "미회복"으로 집계된 결과다.
분모 조작(재분류 경로로 옮겨간 건수) 효과도 아니다 — `sr_error` 재분류
경계(§5 규율 3)로 인해 이 쌍의 분모가 바뀐 사례는 이 런 자체를 제외하면
약 1건 수준으로, 하락폭의 대부분을 설명하지 않는다.

### 6-3. 배치 1의 거짓 성공 finish (`run-fix-failing-test-1`)

**M12 신규 장치 어느 것에도 귀속되지 않는다.** 트랜스크립트(11개 이벤트,
5턴)를 직접 확인:

1. turn1 `run_command`(cargo test, exit 101 — 실패)
2. turn2 `read_file`(tests/csv.rs)
3. turn3 `read_file`(src/lib.rs)
4. turn4 `edit_file` 시도 — `search==replace` 오류로 **실패**(성공 뮤테이션
   0건)
5. turn5 `finish`(요약 포함, passed=False로 판정 — 거짓 성공)

성공 뮤테이션이 한 번도 없었으므로 **VERIFY_NUDGE는 계약대로 침묵했다**
(그 장치는 뮤테이션 이후 미검증 상태에서만 발동하는데, 뮤테이션 자체가
없었다). **상태선도 렌더되지 않았다** — 무뮤테이션 케이던스는 5/10/15/20
턴에서 발동하는데, 이 런은 정확히 turn5에서 `finish`로 종결돼(turn5는
`run_command`가 아니라 `finish`가 소비했으므로 케이던스가 걸릴 다음
tool_result 자체가 없다) 케이던스 지점에 도달은 했지만 렌더할 툴 결과가
없이 턴 예산이 소진됐다 — 한 턴 차이로 놓쳤다. edit_file 실패 시 나온
S/R-동일 오류 자체는 실재하는 매치 위에서 난 것이라(환각 코드가 아님)
검사 순서 교체(T1)의 영향권 밖이고, **pre-M12 코드였어도 동일한 오류
문구가 나왔을 것**이다. 세 요소(VERIFY_NUDGE 무장 조건·상태선 케이던스·
S/R 오류 문구) 어느 것도 M12가 이 거짓 성공을 만들거나 놓치는 데
관여하지 않았다.

이 세 귀속은 전용 법의학 패스로 확립했고, 세 번째(§6-3)는 최종 리뷰어가
독립적으로 재현해 같은 결론에 도달했다.

## 7. 긍정 증거 — 기함이 실제로 잡은 거짓 초록불

`run-update-vat-rate-8`(배치 2)에서 `cargo test --package inv-report
check_vat_report`가 **exit 0**·**0 tests run**·**15 filtered out**으로
끝났다(테스트 이름 필터가 아무것도 매치하지 않은 채 "성공"으로 보인
경우). `empty_test_note`가 그 자리에서 발동했고("note: 0 tests ran (15
filtered out) - cargo test filters by test NAME, not file name; this exit
0 did not verify anything"), **모델은 바로 다음 턴에 필터 인자를 바꿨다**
(`--package inv-report check_vat_report` → `--package inv-report --
check_vat`). 이는 M12가 표적으로 삼은 정확한 거짓 초록불 패턴이 실전
트랜스크립트에서 발동해 모델의 다음 행동을 실제로 바꾼 사례다. 최종
리뷰어가 이 시나리오를 eval 하네스 밖의 실제 크레이트에 대해 살아있는
LM Studio로 재현해 별도로 확인했다.

## 8. 열린 사항 (해결되지 않은 채로 기록)

- **가설, 미검증(n=1)**: T1이 추가한 소품 문장("The file was NOT modified -
  it still contains your search text unchanged.")이 오히려 모델의 "내
  search가 맞다"는 오신념을 **확증**해 재제출을 강화할 가능성이 있다.
  §6-1의 궤적 분기가 이 문장 직후에서 시작되는 것이 계기이지만, 표본이
  이 런 하나뿐이고 그 런의 최종 결말(반복정지)이 문장 자체 때문인지 다른
  요인(파일 하나에 갇힌 탐색) 때문인지 분리할 수 없다. **가설로만
  기록한다 — 단정하지 않는다.** 향후 배치에서 유사 패턴을 관찰 대상으로
  유지할 것.
- **무뮤테이션 거짓 성공 격차(해결되지 않은 채 존재)**: 두 배치 합산
  거짓 성공 finish 4건 중 3건("finish with a confident summary after zero
  successful mutations" — 배치 1 `fix-failing-test-1`, 배치 2
  `update-vat-rate-0`·`update-vat-rate-5`, 각각 `first_mut_turn=0`)이
  이 패턴이다. 이는 실재하고, M12 이전부터 있었으며, M12가 건드리지
  않았다 — 스펙 §1·§8이 이미 이 게이트(A층 착수 실패 대응)를 M13
  후보로 명시적으로 이연했다. 상태선의 무뮤테이션 케이던스(턴 5/10/15/20)
  는 이 4건 중 2건(`fix-failing-test-1`, `update-vat-rate-0`)을 정확히
  한 턴 차이로 놓쳤다 — `finish`가 케이던스 트리거 지점(turn5)의 tool_result
  자체를 대체해버려 렌더할 기회가 없었다. 이 갭은 M13에서 다룰 문제이지
  M12가 자초한 문제가 아니다.

## 9. 판정

**배치 1(게이트): 통과. 33/36 ≥ 33/36 — 임계값에 정확히, 재측정 없이 1회
측정으로.** 사전등록 §종결 규칙("재측정도 <33/36이면 비병합·정지")은
발동 조건(게이트 미달) 자체가 성립하지 않아 적용 대상이 아니다. 세 이상
징후 중 M12 신규 장치에 귀속되는 실패는 0건(§6). **판정 규칙의 기계적
적용 결과: 병합 대상.**

**배치 2(관찰): 게이트 아님, 판정에 영향 없음.** 전체 통과 수는 대조 대비
+1(§4 정정 참고), fm은 개선, uv는 동률(엄격은 하락, 표본 n=2). 사전등록이
uv 개선을 목표로 등록하지 않았으므로 이 관찰은 병합 여부와 무관하다.

실제 병합 실행(Task 11 브리프 Step 5)은 이 문서 작성자의 권한 밖이며
컨트롤러가 이 문서 커밋 이후 수행한다.

## 10. 원자료 위치

- 배치 스탬프: `.loco/eval/20260718T222824Z`(배치 1), `.loco/eval/20260718T231049Z`(배치 2)
- 대조 스탬프: `.loco/eval/20260718T115152Z`(배치 1 대조), `.loco/eval/20260718T082449Z`(배치 2 대조)
- 지표 재현: `python3 scripts/exp_metrics.py .loco/eval/<배치 스탬프> .loco/eval/<대조 스탬프>`
- 러너 원자료 기록(§4 정정 대상 포함): `.superpowers/sdd/task-11-report.md`
- 사전등록: `pre-registration.md`(본 디렉토리)
