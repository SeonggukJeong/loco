# M8 실패 분류·M9 요구사항 후보 (Task 13)

측정 데이터: `docs/baselines.md`의 "M8 측정 조건·ornith 실측 사양표" 절 3배치
(각 3과제 × 3반복, 시드 0) — gemma-4-e4b@8K(`.loco/eval/20260716T163308Z`),
ornith-1.0-9b@8K(`.loco/eval/20260716T164620Z`), ornith-1.0-9b@32K
(`.loco/eval/20260716T171133Z`). 함정 대장·정답 파일 집합은
`tasks-large/README.md`(fixture 밖, 모델 비노출).

## 0. 방법

각 런의 `report.json`에서 outcome(직렬화 실명)·passed·turns·duration을 추출하고,
`run-<task>-<repeat>.jsonl` 트랜스크립트(`kind`: system/user/assistant/tool_result,
assistant의 `content`는 `{"thought","action":{"tool","args"}}` JSON 문자열)를
jq/python으로 턴 단위 요약(툴+인자+결과, ~500자 절삭)해 만든 뒤 읽었다. 정량
지표(검증 타임아웃 계수, `search and replace are identical` 자기-버그 횟수,
BadArgs 횟수, 첫 정답파일 도달 턴, 도구 호출 믹스)는 jq/python 스크립트로
전수 계산했고, 함정 발동 여부는 각 런의 툴 호출·사고(thought)·결과를 직접
읽어 판단했다(정량 스크립트가 "그 파일이 열렸는가"까지는 교차검증하지만
"그로 인해 오판했는가"는 트랜스크립트 정독으로만 판단 가능 — §3에서 근거
품질을 신뢰도별로 구분해 표기한다).

검증 타임아웃(`grep -c "command timed out"`)은 27런 전부 0 — `command_timeout_secs`
240s 상향(M8 §5) 이후 콜드 빌드발 타임아웃 오염은 이 배치에서 재발하지 않았다.
하네스 중단(오버플로 400 등으로 인한 report.json 미생성)도 0건 — 27런 전부
report.json에 항목이 존재한다.

## 1. 런별 분류표 (27행)

범례: 출=outcome(F=finished/M=max_turns/R=repetition_stop), 통=passed,
엄=passed_strict(=passed∧finished), S/R=`search and replace are identical`
자기-버그 발생 횟수, BadArgs=missing field/invalid type 오류 횟수.

### 1-1. gemma-4-e4b @ 8K (`20260716T163308Z`)

| 과제 | 반복 | 출 | 통 | 엄 | 턴 | 초 | S/R | BadArgs | 발동 함정 | 첫 정답파일 도달 |
|---|---|---|---|---|---|---|---|---|---|---|
| find-definition-large | 0 | F | ✗ | ✗ | 4 | 16.6 | 0 | 0 | 없음 (도구 실행 결함) | T1 grep 일격 (파일 직접 열람 없음) |
| find-definition-large | 1 | F | ✓ | ✓ | 5 | 24.5 | 0 | 0 | 없음 | T1 grep 일격 |
| find-definition-large | 2 | F | ✓ | ✓ | 6 | 24.1 | 0 | 0 | 없음 | T1 grep 일격 |
| fix-monthly-total | 0 | F | ✗ | ✗ | 18 | 85.5 | 0 | 0 | #9(FIXME, T12) | monthly.rs 미열람(grep만) |
| fix-monthly-total | 1 | F | ✗ | ✗ | 8 | 36.4 | 0 | 0 | 없음(관측) | monthly.rs 미열람(grep만) |
| fix-monthly-total | 2 | M | ✗ | ✗ | 25 | 102.6 | 0 | 0 | #8(추정, T6→T7) | monthly.rs@T6 (편집 없음) |
| update-vat-rate | 0 | F | ✓ | ✓ | 24 | 116.9 | 0 | 0 | #7C(T19 실패→T20 회복) | defaults.rs@T3, pricing.rs@T10, invoice.rs@T17, forecast.rs@T19 |
| update-vat-rate | 1 | F | ✓ | ✓ | 24 | 107.3 | 0 | 0 | 없음 | defaults.rs@T3, pricing.rs@T8, invoice.rs@T14, forecast.rs@T20 |
| update-vat-rate | 2 | M | ✗ | ✗ | 25 | 210.6 | 0 | 3 | 자기유발 리버트(비함정) | defaults.rs@T3, pricing.rs@T7, invoice.rs@T16, forecast.rs 왕복 |

### 1-2. ornith-1.0-9b @ 8K (`20260716T164620Z`)

| 과제 | 반복 | 출 | 통 | 엄 | 턴 | 초 | S/R | BadArgs | 발동 함정 | 첫 정답파일 도달 |
|---|---|---|---|---|---|---|---|---|---|---|
| find-definition-large | 0 | R | ✓ | ✗ | 9 | 59.3 | 0 | 0 | 없음(#11 회피) | T1 grep 일격 |
| find-definition-large | 1 | F | ✓ | ✓ | 10 | 66.7 | 0 | 0 | 없음(#11 회피, 명시 검토) | T1 grep 일격 |
| find-definition-large | 2 | F | ✓ | ✓ | 17 | 121.0 | 0 | 0 | 없음(#11 회피) | T1 grep 일격 |
| fix-monthly-total | 0 | R | ✗ | ✗ | 13 | 102.9 | 7 | 0 | 없음(진단은 T1 즉시 정확) | monthly.rs@T1 |
| fix-monthly-total | 1 | F | ✓ | ✓ | 8 | 72.2 | 0 | 0 | #2/#9 노출·저항(T4) | monthly.rs@T2 |
| fix-monthly-total | 2 | F | ✓ | ✓ | 5 | 50.7 | 1 | 0 | 없음(진단은 T1 즉시 정확) | monthly.rs@T1 |
| update-vat-rate | 0 | M | ✗ | ✗ | 25 | 302.4 | 7 | 0 | 없음(자기유발 파일 손상 T13) | pricing.rs@T4(T16까지 미완), defaults.rs@T20(T23 완료), invoice/forecast 미도달 |
| update-vat-rate | 1 | M | ✗ | ✗ | 25 | 349.9 | 0 | 1 | 없음(탐색 마비+환각) | 4개 파일 중 실제 edit 성공 0건(T23부터 환각 검색블록) |
| update-vat-rate | 2 | M | ✗ | ✗ | 25 | 286.5 | 3 | 7 | 없음(과제 범위 환각) | 4개 파일 전부 미도달(inv-cli에 없는 `--vat` 플래그 기능을 25턴 내내 구현) |

### 1-3. ornith-1.0-9b @ 32K (`20260716T171133Z`)

| 과제 | 반복 | 출 | 통 | 엄 | 턴 | 초 | S/R | BadArgs | 발동 함정 | 첫 정답파일 도달 |
|---|---|---|---|---|---|---|---|---|---|---|
| find-definition-large | 0 | F | ✓ | ✓ | 6 | 171.6 | 0 | 0 | 없음(#11 회피) | T1 grep 일격 |
| find-definition-large | 1 | R | ✓ | ✗ | 19 | 232.0 | 0 | 0 | 없음(#11 회피) | T1 grep 일격 |
| find-definition-large | 2 | R | ✓ | ✗ | 15 | 129.6 | 0 | 0 | 없음(#11 회피) | T1 grep 일격 |
| fix-monthly-total | 0 | R | ✗ | ✗ | 8 | 64.1 | 1 | 0 | 없음(진단은 T1 즉시 정확) | monthly.rs@T1 |
| fix-monthly-total | 1 | F | ✓ | ✓ | 7 | 60.2 | 0 | 0 | #2/#9 노출·저항(T4) | monthly.rs@T2 |
| fix-monthly-total | 2 | F | ✓ | ✓ | 6 | 66.0 | 1 | 0 | 없음(진단은 T1 즉시 정확) | monthly.rs@T1 |
| update-vat-rate | 0 | F | ✓ | ✓ | 23 | 544.3 | 7 | 0 | 없음(edit_file 자기버그, write_file 전량 우회) | pricing.rs@T2(완료T9,write_file), forecast.rs@T3(완료T12,write_file), defaults.rs@T13(완료T15,write_file), invoice.rs@T8(완료T17,write_file) — 4곳 전부 T17까지 완료, T23 finish |
| update-vat-rate | 1 | M | ✓(관대) | ✗ | 25 | 311.7 | 6 | 2 | 없음(edit_file 자기버그, sed 최종 우회) | defaults.rs@T3(완료T21,sed), pricing.rs@T8, invoice.rs@T13, forecast.rs@T23(완료T25,sed — 턴 소진과 동시) |
| update-vat-rate | 2 | M | ✓(관대) | ✗ | 25 | 380.5 | 4 | 2 | 없음(edit_file 자기버그, write_file+python3 혼합 우회) | defaults.rs@T3(완료T13,write_file), pricing.rs@T15(완료T19,python3), forecast.rs@T20(python3), invoice.rs@T21(python3) — T24 cargo test 전체통과 확인, T25는 불필요한 재확인 grep으로 턴 소진 |

## 2. 함정 발동 집계 (정량 교차검증 포함)

- **#9 (거짓 FIXME, totals.rs)**: 트랜스크립트에 FIXME 텍스트가 실제로 노출된 런은
  fix-monthly-total 9런 중 3런(gemma 8K r0, ornith 8K r1, ornith 32K r1) —
  `grep -c "FIXME: 반품"` on tool_result 전수 확인.
- **#8 (거짓 주석, monthly.rs doc)**: monthly.rs의 `calc_total_v2` 위 거짓 주석이
  tool_result에 노출된 런은 fix-monthly-total 9런 중 8런(gemma r1만 예외 —
  grep 스니펫이 주석 앞에서 끊겨 실제로 한 번도 노출되지 않음, 정량 확인).
  노출됐다고 전부 "발동"은 아니다 — 노출 후 진단이 왜곡된 것으로 보이는 경우만
  발동으로 세면: gemma r2(T6 정독 직후 T7에서 "코드가 완전하고 견고해 보인다"고
  명시적으로 오판, 시점이 정확히 일치 — **추정 근거, thought 필드가 주석 문구를
  직접 인용하지는 않음**) 1건이 유일하게 확실한 후보.
- **#2 (v1/v2 오인 유도)**: totals.rs를 `read_file`/`grep`로 실제로 연 런은
  27런 중 4런(gemma 8K fix-monthly r0[5회 재독], ornith 8K fix-monthly r1,
  ornith 8K update-vat r1, ornith 32K fix-monthly r1) — 이 중 **전부 저항**
  (잘못된 파일을 고치려 시도한 런 0건). v1이 "오래돼 보이지만 실제로는
  무관"이라는 함정의 설계 의도와 달리, 열어본 모델은 전원 정확히 무시하거나
  (gemma r0는 열었지만 결국 편집 자체를 안 함) 명시적으로 분리해 추론했다
  (ornith 8K r1: "v1은 별개 함수, 영향 없음"; ornith 32K r1도 동일 패턴 —
  monthly.rs를 고치기 직전에 totals.rs를 "비교 대조용"으로 열어보고 그대로
  monthly.rs만 고쳤다).
- **#7 (세율 다지점 산개, notation 차이)**: update-vat-rate 9런 중 gemma 8K
  2런(r0 실패→회복, r2는 파일 서명을 잘못 복사해 실패 — 순수 #7 notation
  놀람이라기보다 부주의)만 `edit_file`의 "search block not found" closest-match
  힌트로 즉시 회복하는 형태로 #7과 실제로 조우했다. **ornith은 8K·32K 6런
  전부 #7 자체보다 훨씬 이전 단계(도구 실행 결함·환각·edit_file 자기-버그)에서
  좌초해 #7C(forecast.rs f64 표기)까지 도달조차 못했다**(32K도 예외 없음 —
  §1-3의 S/R 열 참고, 실패 원인이 전부 "search==replace 동일" 자기-버그이지
  "search block not found" 표기 놀람이 아니다) — #7은 "도달하면 스캐폴딩이
  구제하는" 함정이지 "도달을 막는" 함정이 아님이 이번 배치에서 재확인됨.
- **#1/#3/#4/#5/#6/#10/#11(재수출)**: 27런 전수에서 **실질적으로 발동 0건**.
  - #1(csv.rs 죽은 v0 코드): 트랜스크립트의 "csv.rs" 매치는 전부 시스템 프롬프트
    트리 목록에서 온 것(정량 확인) — 어떤 런도 실제로 이 파일을 열거나 그
    죽은 코드 블록을 읽지 않았다. 모델들이 "부가세율"/"DEFAULT_VAT_PERCENT"/
    "apply_tax" 같은 구체적 토큰으로 검색해 "vat" 광역 검색을 피했기 때문으로
    보인다(추정).
  - #11(재수출 사슬): find-definition-large **9런 전부**(gemma 3 + ornith 8K 3
    + ornith 32K 3) `grep "fn restock_threshold"`(gemma는 `restock_threshold`
    광역 검색이지만 grep 결과에 `pub fn`과 `pub use`가 함께 잡혀 구분 가능)
    또는 `grep "fn X"`(ornith, `pub use` 라인은 애초에 매치조차 안 됨 — 질의
    정밀도로 함정을 구조적으로 회피)로 단번에 정답에 도달, 재수출 지점을
    정의로 오인한 런 0건. 32K로 컨텍스트를 늘려도 이 패턴(T1 grep 일격)은
    그대로였다 — 컨텍스트 크기와 무관한, 순전히 질의 방식의 승리다.
  - #3/#5/#6(상주 함정)은 애초에 이 3과제와 결선되지 않은 것으로 설계됐고
    (플랜 §"과제↔함정 결선" 각주), 실제로도 어느 런의 grep 결과에도 우연히
    섞여 혼란을 유발한 흔적을 찾지 못했다.
  - #10(rules/mod.rs 갓파일)은 find-definition-large 과제의 설계 의도이지만,
    9런 전부 `grep "fn restock_threshold"` 한 방으로 파일을 특정해 "스크롤해서
    찾아야 하는" 어려움 자체를 겪지 않았다 — grep 도구가 이 함정을 완전히
    무력화한다(§4에서 M9 함의로 재론).

## 3. 과제×모델 실패 서사

### find-definition-large — 함정(#10/#11)은 grep 한 방에 무력화, 진짜 병목은 딴 곳

**9개 런(gemma 3 + ornith 8K 3 + ornith 32K 3) 전부** **T1의 단일 grep 호출**로
`inv-core/src/rules/mod.rs`를 정확히 특정했다. gemma는 `restock_threshold`(식별자만)
검색, ornith은 `fn restock_threshold`(함수 정의 패턴) 검색 — 두 질의 스타일 모두
결과에서 `pub fn`(정의)과 `pub use`(재수출)가 동시에 보이거나(gemma) `pub use` 라인
자체가 매치되지 않아(ornith) 재수출 오인의 여지가 없었다. **함정 #10(700줄 갓파일)과
#11(재수출 사슬)은 이 9런에서 단 한 번도 실제 난이도로 작용하지 않았다** — grep이
갓파일 스크롤 문제와 재수출 오인 문제를 동시에 무력화하며, 32K로 컨텍스트를 늘려도
이 그림은 바뀌지 않았다(9런 모두 T1 grep, 예외 없음).

실패는 전부 진단 이후 단계에서 발생했고, 8K→32K로 갈수록 오히려 **길어진다**:
gemma r0는 `run_command`로 `echo`를 리다이렉션 없이 실행해(`{"command": "echo"}`)
answer.txt를 아예 쓰지 못했고, 검증 없이 finish했다(§4에서 재론 — `write_file`이
아니라 `run_command echo`로 답을 쓰면 VERIFY_NUDGE가 걸리지 않는다). ornith 8K는
3런 모두 정답을 즉시 찾았지만 `finish({})`(필수 `summary` 필드 누락)를 반복
호출해(9회 누적: 5+0+4) r0는 5회째에 반복정지(RepetitionStop)까지 도달했다 —
과제 자체는 이미 T4에 풀려 있었는데도. **ornith 32K는 이 종료 실패가 더 악화된다**:
r1(19턴)·r2(15턴) 둘 다 답을 T1에 확정하고 `cargo test`/`answer.txt` 검증까지
여러 번 통과 확인한 뒤에도 `cat answer.txt`나 `grep -rn "fn restock_threshold"`를
7~9회씩 재반복하는 **강박적 재검증 루프**에 빠졌다 — r2는 15턴 내내 단 한 번도
`finish`를 시도하지 않았다(도구 믹스 참고, finish 호출 0건). 컨텍스트가 늘어난
만큼 "이미 끝난 일을 다시 확인할" 여유 턴도 늘어난 셈이라, 종료 실패가 8K보다
32K에서 턴 수 기준으로 더 크게(9→17→19턴, 최댓값 기준) 나타난다.

### fix-monthly-total — 모델별 실패 축이 완전히 다르다 (헤드라인)

**gemma는 진단 자체에 실패한다.** 3런 전부 `LineKind::Sale => acc - line.amount_krw,`를
고치는 `edit_file` 호출을 **단 한 번도 하지 않았다**(9번 항의 도구 믹스 참고).
r0/r1은 애초에 monthly.rs 전체를 읽지도 않고(grep 스니펫만으로 판단), r2는 T6에서
버그 줄을 정확히 읽고도 T7에서 "코드가 완전하고 견고해 보인다"고 판단한 뒤
`cargo test --no-run-all`(존재하지 않는 플래그)↔`cargo test --no-run`↔`cargo test`(101)
3단 순환을 6바퀴 반복하며 25턴을 소진했다.

**ornith은 진단은 항상 즉시·정확하지만, `edit_file` 기계적 실행에서 무너진다.**
3런 전부 T1(또는 T1의 grep 뒤 T2)에 monthly.rs를 열자마자 사고(thought)에서 정확한
버그를 명시했다("판매 라인을 -로 처리하고 있다. 판매는 더해져야 한다" 등). 그런데
그 직후 `edit_file` 호출에서 **`search`와 `replace`에 동일한(수정 전) 텍스트를
그대로 넣는 자기-버그**("search and replace are identical" 오류)가 r0에서 7회,
r2에서 1회 발생한다. r2는 2번째 시도에서 진짜로 다른(수정된) replace를 생성해
빠져나오지만(5턴 만에 완료 — 27런 중 최단), r0는 7번 연속 같은 실수를 반복하다
반복정지로 끝난다(끝내 Sale 분기의 부호를 단 한 번도 실제로 바꾸지 못함). r1은
이 자기-버그 없이 첫 시도에 성공(8턴) — 3런의 유일한 분기점이 "같은 정답을
알고 있는 상태에서 `edit_file` 인자를 몇 번 만에 올바르게 채우는가"였다.

이 대조는 `docs/baselines.md`의 "과제별 강약 역전"(gemma fix-monthly 0/3 vs
ornith update-vat 0/3) 관찰을 한 단계 더 파고든다: fix-monthly-total 단독으로도
같은 역전이 이미 나타나며, 원인이 **완전히 다른 두 축**(gemma=진단 회피/과신,
ornith=진단은 맞지만 실행 기계 오류)이라는 것이 이번 정독으로 새로 확인됐다.

**32K에서도 ornith의 이 자기-버그는 그대로 재현된다.** 32K r0는 8K r0와 거의
동일한 패턴(첫 `edit_file` 시도가 곧바로 "search and replace are identical"로
거부)을 보이는데, 이번엔 반복 탐지가 `edit_file` 재시도가 아니라 **같은 8줄
창을 5회 재조회하는 `read_file`** 쪽에서 먼저 걸려 8턴만에 반복정지로 끝난다
(8K는 13턴). r2는 8K와 마찬가지로 1회 실패 후 즉시 회복해 6턴에 성공한다.
**S/R 버그 자체의 발생률과 회복 실패율은 8K와 32K 사이에 유의미한 차이가
없다** — 컨텍스트 크기가 이 실행 버그의 근본 원인이 아니라는 근거다(§4에서
M9 함의로 재론).

### update-vat-rate — 8K에서 ornith 0/3, 원인은 함정이 아니라 자기유발 오류

gemma 8K는 2/3(트라이 사이트 A/B/D를 그대로 밟고, C(forecast.rs)의 표기 차이(#7C)에서
1런은 걸렸다 회복, 1런은 안 걸림). 3번째 런(r2)은 4곳을 전부 정확히 고친 뒤
마지막 실패 테스트 원인을 오판해 **invoice.rs를 12%에서 10%로 되돌리는** 자기파괴적
수정을 하고(그 과정에서 일본어 문자열이 섞인 이상 주석까지 생성 — 생성 결함),
25턴 안에 복구하지 못했다. 이 실패는 함정 카탈로그의 어느 항목과도 무관한
**턴 예산 압박 하의 자기 번복**이다.

ornith 8K는 3/3 전부 실패(max_turns, 평균 25턴·313s)하며, 이 배치 전체에서 가장
심각한 실패 군집이다. 세 런의 실패 원인이 모두 다르고, 셋 다 11종 함정 카탈로그
어느 것으로도 설명되지 않는다:
- r0: `edit_file` 자기-버그(search==replace) 7회 + **실제 파일 손상 1건**
  (T13: `search`가 시그니처 앞부분만 포함해 원문 뒤쪽 절반이 replace 뒤에 그대로
  붙어버리는 중복 서명 생성 — T14에서 "파일이 손상됐다"고 스스로 인지하고
  T16에 복구). 25턴 동안 pricing.rs·defaults.rs 2곳만 고치고 invoice.rs·
  forecast.rs는 아예 열지도 못했다.
- r1: **탐색 마비**. T1~T22(25턴 중 22턴!)를 오직 `read_file`/`grep`로만 소진
  (report.rs를 3번 완전히 동일한 내용으로 재독, "vat_percent|VAT_PERCENT" 동일
  grep 2회 반복 등 — 새 정보 없이 도돌이). **첫 `edit_file` 시도가 T23**에야
  나오는데, 존재하지 않는 코드(`let vat_amount = total_amount - net_krw;`)를
  `search`에 인용하는 **환각**이라 실패한다. T24-25도 같은 패턴 반복. 25턴 동안
  성공한 edit이 0건, 4개 정답 파일 중 실제로 write를 시도한 파일도 0개.
- r2: **과제 범위 환각**. 부가세 계산 로직을 고치는 대신, 존재하지 않는
  `--vat` CLI 플래그 기능을 처음부터 끝까지(T4-T25) 구현하려 시도한다
  (`Command::Report`에 `vat: Option<String>` 필드 추가, `parse_args` 배선,
  `extract_flag_value` 헬퍼 신설). 이 과정에서 **protected 경로인
  `inv-cli/tests/cli_basic.rs`를 편집**하기까지 한다(과제 3개 공통
  protected 목록에 `inv-cli/tests`가 명시돼 있어 샌드박스 종료 시 무효화될
  작업 — 헛수고 확인). 25턴 내내 pricing.rs·invoice.rs·forecast.rs·
  defaults.rs 중 단 하나도 열지 않았다.

세 런 모두 "함정에 걸렸다"기보다 **프롬프트가 요구한 작업 자체에서 이탈**했다는
공통점이 있다 — r0는 도구 실행 정확도, r1은 행동으로 수렴하지 못하는 과잉 탐색,
r2는 과제 자체의 오인식. 8K 컨텍스트에서 셋 다 25턴을 다 쓰고도 회복하지 못했다.

### update-vat-rate 32K — "구제"의 실체: 도구 갈아타기 여유, 종료 여유는 아님

baselines.md는 이미 32K에서 update-vat-rate가 0/3(8K)→3/3(관대, 32K)로 뒤집히지만
엄격은 44.4%로 그대로라고 기록했다. 이번 정독으로 그 기제가 턴 단위로 드러난다:
**32K 3런 모두 8K와 똑같은 edit_file "search==replace 동일" 자기-버그를 4지점
전부에서 다시 겪는다**(S/R 카운트 r0=7·r1=6·r2=4, §1-3) — 컨텍스트가 이 버그를
없애지는 않는다. 다른 점은 실패한 `edit_file` 뒤에 **다른 도구로 갈아탈 턴 여유가
있었다**는 것이다:

- r0(엄격 통과, 이 배치 유일): pricing.rs·forecast.rs·defaults.rs·invoice.rs
  네 곳 모두 `edit_file`이 막히자 **`write_file`로 파일 전체를 재작성**해
  우회했다(T9·T12·T15·T17). 4곳을 T17까지 다 고치고, T18-22에서 검증한 뒤
  T23에 깔끔히 finish — 23턴 만에 엄격 통과.
- r1(관대만): `edit_file` 실패 → `write_file`(필수 `content` 필드 누락으로
  또 실패) → **`sed -i`**(macOS BSD sed 문법 오류로 1차 실패, `-i ''`로 재시도해
  성공)로 이어지는 3단 에스컬레이션을 파일마다 반복한다. 네 번째이자 마지막
  지점(forecast.rs)의 `sed` 수정이 **정확히 T25(턴 예산의 마지막 한 칸)**였다
  — 고치자마자 턴이 소진돼 `finish`를 호출할 기회 자체가 없었다. 하네스의
  `check`(모델과 무관하게 항상 실행)가 그 직후 `cargo test`를 돌려 통과를
  확인했을 뿐이다.
- r2(관대만): `edit_file` 실패 → `sed`(macOS 문법으로 실패) → **인라인
  `python3 -c` 스크립트**로 파일을 직접 다시 쓰는 방식까지 도달해 4지점을
  전부 고쳤다. T24에 `cargo test` 전체 통과를 스스로 확인했지만, **T25(마지막
  턴)를 `finish`가 아니라 불필요한 확인용 `grep`에 썼다** — 이미 확인된
  사실을 한 번 더 확인하려다 종료 기회를 놓쳤다.

세 런을 나란히 보면 **"32K가 여는 것은 대체 도구를 시도할 턴 여유"**이지
"깔끔하게 마무리할 여유"가 아님이 뚜렷하다 — r1·r2는 정답을 다 만들고
검증까지 마쳤는데도 마지막 한 턴을 finish가 아닌 다른 곳(sed 실행 자체,
불필요한 재확인)에 썼다. 컨텍스트 확장과 종료 규율이 서로 독립적인 축이라는
baselines.md의 결론이, 이 세 런에서는 "같은 25턴 예산을 무엇에 쓰는가"라는
구체적 턴 단위 증거로 뒷받침된다.

## 4. M9 요구사항 후보

**관측 한계 명시(스펙 §5 준수)**: 회복된 컨텍스트 오버플로 횟수와 `pack()` 축약
발동 빈도는 이 하네스에서 수집 불가능하다(eval이 Notice 이벤트를 버리고, `pack`의
제자리 변형은 트랜스크립트에 흔적을 남기지 않는다). 아래 우선순위는 **관측
가능한 증거**(턴 수, 도구 호출, outcome, 검증 타임아웃 0건, 하네스 중단 0건)에만
근거하며, "컨텍스트 압박"을 해석축으로 사용하지 않는다. 32K 민감도 배치가 8K
대비 무엇을 구제하고 무엇을 못 구제했는지는 §5(baselines.md 기존 관찰)를 그대로
인용할 뿐, 그 기제(축약이 덜 일어나서/프롬프트가 안 잘려서 등)는 이 노트에서
추정하지 않는다.

우선순위는 **빈도 × 재현성 × 스펙 §8 백로그 정합성**으로 매겼다.

1. **`edit_file` 자기-버그(search==replace 동일 텍스트) 검출·차단 — 최우선
   신규 후보, 스펙 §8에 없는 항목**. 27런 중 9런(전부 ornith — 8K fix-monthly
   r0×7·r2×1, 8K update-vat r0×7·r2×3, 32K fix-monthly r0×1·r2×1, 32K
   update-vat r0×7·r1×6·r2×4)에서 총 37회 발생. **gemma 9런(이 배치에서
   gemma는 8K만 측정 — 전부)에서는 단 한 번도 발생하지 않았다** —
   진단은 맞는데(`thought`가 항상 올바른 의도를 서술) `replace`
   필드 생성 시점에 수정 전 텍스트를 그대로 채우는 ornith 특유의 실행
   결함으로 보인다. 이미 정답을 알고 있는 상태에서 반복 실패해 턴을
   낭비하거나(최악 8K fix-monthly r0는 7연속 실패 후 반복정지), 파일 손상
   (8K update-vat r0 T13, 시그니처 중복 생성)으로 번지거나, 32K에서는
   `write_file`/`sed`/`python3`로 갈아타는 데 턴 예산 대부분을 써
   **정답은 만들지만 finish할 턴이 안 남는**(update-vat 32K r1·r2, §3)
   결과로 이어진다 — 8K에서는 실패의 직접 원인, 32K에서는 엄격 통과율이
   낮게 묶이는 원인이라는 점에서 이 배치 전체를 관통하는 단일 최대
   병목이다. loco 쪽에서 `edit_file` 호출 시점에 `search==replace`를
   더 일찍(현재도 거부는 하지만) 더 명확한 힌트("당신이 지금 무엇을 바꾸려
   했는지 다시 명시하라" 류)로 감지·교정 유도하면 저비용으로 막을 수 있는
   실패 축이다. 스펙 §8 백로그의 어느 항목과도 겹치지 않는 새 발견이며,
   **컨텍스트 크기와 무관하게 재발**하므로(§3 "32K에서도 ornith의 이
   자기-버그는 그대로 재현된다") 32K 확장 같은 컨텍스트 축 개선으로는
   해소되지 않는다 — 스캐폴딩(에러 메시지·전략 교정 임계값) 쪽 개입이
   필요하다는 근거이기도 하다.
2. **과제 범위/코드 존재 환각 방지 — 검색 강화(스펙 §8 "검색 강화")와 직결**.
   ornith update-vat r1(존재하지 않는 코드를 `search`에 인용, 22턴 탐색 후
   3턴 헛손질)과 r2(존재하지 않는 CLI 기능을 25턴 내내 구현)가 근거. 둘 다
   "읽은 적 없는 내용을 사실로 취급"하는 패턴 — grep/read_file 결과에 없는
   내용을 `edit_file`의 `search`에 넣으면 스펙상 항상 실패하지만(현재도
   "closest match" 힌트가 뜨긴 함), 그 실패가 반복돼도 "가짜 코드 계속 인용"
   또는 "엉뚱한 기능 계속 구현"으로 수렴하지 않고 전략을 못 바꾼다. M5의
   전략 교정(같은 오류 3연속 시 1회 주입)이 "edit_file/write_file → write_file로
   재작성" 힌트만 주는데, "search 텍스트가 존재하지 않는 코드다" 케이스는
   다른 교정(예: "먼저 read_file로 그 파일을 열어 실제 내용을 확인하라")이
   필요해 보인다 — 다만 27런 중 2런뿐이라 재현성은 낮음, §8의 기존 "검색
   강화" 항목 확장으로 묶어 제안.
3. **repo-map/트리 개인화 — 이번 배치에서는 우선순위가 낮아짐(스펙 §8
   "repo-map 도구"에 대한 하향 수정 제안)**. find-definition-large **9런
   전부**(gemma 3 + ornith 8K 3 + ornith 32K 3, §2·§3) grep 한 방으로
   함정 #10(갓파일)·#11(재수출)을 완전히 무력화했다 — depth-3 트리 절삭이나
   재수출 오인은 **이 3과제의 실측 실패에 전혀 기여하지 않았다**
   (update-vat-rate의 pricing.rs도 트리에서 안 보이지만, 실패한 런 중 어느
   런도 "pricing.rs를 못 찾아서" 실패한 게 아니라 이미 grep으로 도달한 뒤
   다른 이유로 무너졌다 — §1-1·§1-2·§1-3 첫 정답파일 도달 턴 참고).
   `docs/research/2026-07-16-aider-repo-map.md`가
   제안한 "최근 접근/언급 파일 강제 포함" 개인화는 이번 3과제 실패 데이터가
   요구하지 않는다 — grep이 이미 이 문제를 해결하고 있다. repo-map 계열은
   백로그 유지하되, **이번 배치는 그 우선순위를 끌어올릴 근거를 주지
   않았다**는 점을 명시한다(스펙 §8 백로그 자체를 취소하자는 게 아니라,
   "실패 데이터가 정한다"는 §8의 조건을 이번엔 만족 못 했다는 뜻).
4. **종료 규율(finish 인자 누락·강박적 재검증) — 스펙 §8에 없는 별도 축,
   32K가 오히려 악화시킨다(baselines.md 기존 관찰을 턴 단위로 재확인)**.
   ornith find-definition-large 8K 3런에서 `finish({})`(필수 `summary`
   누락) 9회 발생, 그중 5회 연속이 반복정지까지 도달(이미 풀린 과제에서).
   **32K에서는 형태가 오형식 `finish` 반복에서 "강박적 재검증"으로 옮겨가며
   턴 수 자체가 늘어난다**(§3) — r1은 19턴, r2는 15턴 내내 이미 확인된
   정답을 `cat`/`grep`으로 반복 재확인하다 반복정지(r2는 finish 시도가
   0건). `VERIFY_NUDGE`는 "편집 후 검증 없이 finish"만 잡고 "finish 인자
   자체가 불완전"하거나 "검증을 이미 마쳤는데도 또 검증하는" 경우는 안
   잡는다 — `Registry::dispatch`의 BadArgs 에코가 매 회 동일한 오류를
   반복해서 보여주는데도 모델이 같은 실수를 반복한다는 뜻이다. baselines.md가
   이미 32K 민감도에서 "컨텍스트가 탐색 병목은 풀어도 종료 규율은 별개"라고
   기록했는데, 이번 정독은 한 걸음 더 나아가 **"별개"를 넘어 "32K가 종료
   실패의 턴 규모를 오히려 키운다"**(9→19턴)는 것을 보여준다 — 컨텍스트
   확장이 아니라 **`finish` 전용 스캐폴딩**(예: 같은 필드-누락 오류가
   2회 연속이면 세 번째 시도 전에 정확한 인자 예시를 한 번 더 주입하거나,
   `run_command`/`cat`으로 이미 확정된 답을 재확인하는 패턴이 감지되면
   "정답은 이미 확인됐다, finish를 호출하라"는 직접 힌트를 주입)이 필요해
   보인다 — 이는 M5의 일반 전략 교정과 별개로 `finish` 툴에 특화된 교정을
   신설하는 문제다.
5. **컨텍스트 오버플로 내성 4건(스펙 §8 이월)** — 이번 3배치에서 오버플로로
   인한 하네스 중단은 **0건**(27런 전부 report.json 존재, §0). 승격 근거
   ("M8 측정에서 오버플로 중단이 실제 발생하면 그 데이터가 우선순위 승격
   근거") 자체가 이번엔 발생하지 않았으므로, 이 축은 백로그에 그대로 둔다
   — 위 §5 관측 한계 문단대로 "압박은 있었지만 안 보였을 뿐"이라는 해석은
   하지 않는다(증거가 없다는 뜻이지, 없었다는 뜻이 아니다).
6. **repo-map 3단계(tree-sitter/syn) 도입** — 스펙 §8·aider 노트 §4의
   백로그 그대로. 위 3번 항목과 같은 이유로 이번 배치는 승격 근거를 주지
   않았다.

## 5. 관측 한계 (본 노트 자체)

1. 함정 발동 판정은 27런 전수를 직접 정독했지만(§0), "노출됨"(정량 스크립트로
   확인 가능)과 "그로 인해 오판함"(정성적 판단)을 구분했다 — §3에서 "추정"이라
   표시한 항목은 thought 필드가 함정 텍스트를 직접 인용하지 않아 인과를
   100% 확정할 수 없다.
2. `search and replace are identical` 카운트는 문자열 그대로 카운트한 정량치라
   신뢰도가 높지만, "파일 손상"(update-vat r0 T13류)은 트랜스크립트 정독으로만
   식별 가능해 이 노트가 언급한 것 외에 놓친 사례가 있을 수 있다.
3. 첫 정답파일 도달 턴은 `read_file`/`edit_file`/`write_file`의 `path` 인자가
   정답 파일과 정확히 일치하는 첫 턴만 잡는다 — grep 결과에 정답 파일 내용이
   스니펫으로 노출된 시점(더 이른 턴일 수 있음)은 포함하지 않는다(§3에서
   "직접 열람"과 "grep 스니펫만"을 구분해 서술한 이유).
