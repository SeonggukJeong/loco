# M10 0단계 — S/R 루프 법의학 (Task 1)

목적: `docs/superpowers/specs/2026-07-17-m10-experiment-infra-stubborn-loops-design.md`
§3이 요구하는 "측정 비용 0" 법의학 — M9 1·2단에서 `search and replace are
identical`(이하 S/R) 오류가 발생한 런 전수(15런)를 실제로 정독해, §4(차단
임계 3회 누적)·§5(섭동 온도 0.7)의 세부값을 확정하거나 조정한다. 전제(반복이
문자 단위 복사라는 가설)가 뒤집히면 스펙 개정으로 회귀해야 하므로(§3), 그
판정도 함께 기록한다.

배치 라벨(`docs/baselines.md` "M9 행동 지표 비교"와 동일): A=`20260717T015330Z`
(1단 gemma, S/R 0건 — 대상 외), B=`20260717T020632Z`(1단 ornith@8K),
C=`20260717T022652Z`(1단 ornith@32K), D=`20260717T031126Z`(2단 gemma),
E=`20260717T032507Z`(2단 ornith@8K), F=`20260717T034527Z`(2단 ornith@32K).

## 0. 방법

각 `.loco/eval/<배치>/run-<과제>-<반복>.jsonl`에서 `kind != assistant`인 이벤트의
`content`에 나타나는 `"search and replace are identical"` 개수를 세어(브리프 Step 1
스크립트) 대상 런을 식별했고, 브리프 Step 2 덤프(턴 번호 + 액션/결과 요약)로
각 런을 정독했다. `edit_file` 호출만 따로 뽑아 `search == replace` 여부와
직전/이전 호출과의 문자열 동일성을 자동 비교하는 보조 스크립트(1회성, 저장하지
않음)로 교차검증했다. 파일 크기(ⓒ)는 `wc -l`로 `tasks-large/` 픽스처 원본을
직접 측정했다(에이전트가 읽은 sandbox 상대 경로가 그대로 fixture 트리 경로와
일치).

## 1. Step 1 결과 — 대상 15런 확인

브리프의 스크립트를 그대로 실행한 결과, 기대치(B:2·C:4·D:3·E:2·F:4=15, A:0)와
**정확히 일치**했다:

```
20260717T020632Z run-fix-monthly-total-0.jsonl SR=7
20260717T020632Z run-fix-monthly-total-2.jsonl SR=2
20260717T022652Z run-fix-monthly-total-0.jsonl SR=1
20260717T022652Z run-fix-monthly-total-2.jsonl SR=1
20260717T022652Z run-update-vat-rate-0.jsonl SR=5
20260717T022652Z run-update-vat-rate-1.jsonl SR=3
20260717T031126Z run-fix-monthly-total-1.jsonl SR=2
20260717T031126Z run-fix-monthly-total-2.jsonl SR=1
20260717T031126Z run-update-vat-rate-0.jsonl SR=1
20260717T032507Z run-fix-monthly-total-0.jsonl SR=6
20260717T032507Z run-fix-monthly-total-2.jsonl SR=1
20260717T034527Z run-fix-monthly-total-0.jsonl SR=3
20260717T034527Z run-fix-monthly-total-2.jsonl SR=1
20260717T034527Z run-update-vat-rate-0.jsonl SR=5
20260717T034527Z run-update-vat-rate-1.jsonl SR=3
```

편차 없음 — 강제로 맞추지 않았다. 오류 수 합(1단 19·2단 23)도 `docs/baselines.md`
"M9 2단" 표의 기존 집계와 일치(교차검증 통과).

## 2. 런별 표 (15런)

범례: SR=`search and replace are identical` 발생 횟수, 복사=반복 호출이 직전
또는 이전 동일 호출과 문자 단위로 동일한가, 갈아타기=`write_file`로의 전환
성공/실패, 결과=`report.json`의 `passed`/`outcome`.

| 배치 | 런(과제-반복) | SR | 복사 패턴 | 갈아타기 | 결과 | 근거(이벤트 인덱스) |
|---|---|---|---|---|---|---|
| B | fix-monthly-total-0 | 7 | 단일 발생(4, 한 줄) → idx6 편집으로 형식상 통과(부호는 미수정) → 두 줄 블록으로 문자단위 복사 4연속(12=14=19=23) → 변주(25) → 회귀(27=23) | 없음(edit_file만 반복) | ✗/repetition_stop | `run-fix-monthly-total-0.jsonl` [4,12,14,19,23,25,27] |
| B | fix-monthly-total-2 | 2 | 단일 발생(4) 후 실수정 성공, **성공 후 재시도 회귀**(14=4, 6번 성공 이후) | 없음(edit_file로 결국 우회 성공) | ✓/finished | `run-fix-monthly-total-2.jsonl` [4,6,14] |
| C | fix-monthly-total-0 | 1 | 단일 발생(4), 다음 호출(6)에서 실수정 — 그러나 수정 내용이 부호가 아니라 들여쓰기만 바뀜(부호 미수정) | 미해당 | ✗/repetition_stop(원인은 이후 read_file 5연속 반복, S/R과 무관) | `run-fix-monthly-total-0.jsonl` [4,6,8,10,12,15,17] |
| C | fix-monthly-total-2 | 1 | 단일 발생(4), 다음 호출(6)에서 실수정(부호까지 정정) 성공 | 미해당 | ✓/finished | `run-fix-monthly-total-2.jsonl` [4,6] |
| C | update-vat-rate-0 | 5 | defaults.rs 2회(22, 24 — 각각 개별 no-op, 24가 22보다 앞에 문서 주석 한 줄을 더한 변주, 다음 시도(26)에서 바로 실수정 성공) + pricing.rs 문자단위 복사 3회(28=34=36, 비연속 — 다른 시도가 끼어듦) | 없음(edit_file 실수정으로 우회) | ✗/max_turns | `run-update-vat-rate-0.jsonl` [22,24,28,34,36] |
| C | update-vat-rate-1 | 3 | invoice.rs 2연속 복사(38=40) 후 실수정 성공(42) → **성공 후 재시도 회귀**(46=38/40, 이미 고친 파일에 옛 텍스트로 재시도) | 없음 | ✗/max_turns | `run-update-vat-rate-1.jsonl` [38,40,42,46] |
| D | fix-monthly-total-1(gemma) | 2 | monthly.rs 문자단위 복사 2회(44=50, 비연속 — 중간에 존재하지 않는 코드를 지어낸 시도 3회가 끼어듦) — **파일당 최대 2회, 임계(3) 미도달** | 없음(결국 실수정 성공) | ✗/max_turns(finish 못 함) | `run-fix-monthly-total-1.jsonl` [41,44,47,50,53,56] |
| D | fix-monthly-total-2(gemma) | 1 | 단일 발생(20), 다음 호출(23)에서 실수정 성공 | 미해당 | ✓/finished | `run-fix-monthly-total-2.jsonl` [20,23] |
| D | update-vat-rate-0(gemma) | 1 | 단일 발생(48), 다음 호출(50)에서 실수정(괄호 제거) | 미해당 | ✗/max_turns | `run-update-vat-rate-0.jsonl` [48,50] |
| E | fix-monthly-total-0 | 6 | 문자단위 복사 3연속(4=6=9) → 변주(12, 2번째 케이스 추가) → **회귀**(14=9) → 복사 반복(16=14) | 없음 | ✗/repetition_stop | `run-fix-monthly-total-0.jsonl` [4,6,9,12,14,16] (baselines.md 기존 서술과 일치) |
| E | fix-monthly-total-2 | 1 | 단일 발생(4), 다음 호출(6)에서 실수정 성공 | 미해당 | ✓/finished | `run-fix-monthly-total-2.jsonl` [4,6] |
| F | fix-monthly-total-0 | 3 | 문자단위 복사 3연속(4=6=9) | 없음(edit_file만 3회 반복 후 정지) | ✗/repetition_stop | `run-fix-monthly-total-0.jsonl` [4,6,9] |
| F | fix-monthly-total-2 | 1 | 단일 발생(4), 다음 호출(6)에서 실수정 성공 | 미해당 | ✓/finished | `run-fix-monthly-total-2.jsonl` [4,6] |
| F | update-vat-rate-0 | 5 | defaults.rs 문자단위 복사 4연속(22=26=32=35) → SR_CORRECTION 문구 수용해 write_file 전환(38, **성공**) → pricing.rs 단일 발생(40) → write_file 전환(44, **성공**) | **성공** 2/2(defaults.rs 41행, pricing.rs 118행 — 온전한 내용, 절삭 없음) | ✓/max_turns(수정은 맞았으나 검증·finish에 턴 소진) | `run-update-vat-rate-0.jsonl` [22,26,32,35,37,38,40,44,46,48,52,53] |
| F | update-vat-rate-1 | 3 | invoice.rs 문자단위 복사 3회(38=42=44 — 38·42 사이에 read_file 1회가 끼었으나 인자는 완전히 동일) → SR_CORRECTION 수용해 write_file 시도(47) | **실패** 3연속 — 원인은 크기·절삭이 아니라 **`content` 필드 누락**(BadArgs, "missing field `content`")이 3회 반복되며 그대로 런 종료(max_turns) | ✗/max_turns | `run-update-vat-rate-1.jsonl` [38,40,42,44,46,47,48,49,50,51,52,53] |

## 3. ⓐ 반복 호출의 문자 단위 복사 여부 — 메아리 가설 확인

**15런 전부에서, S/R 오류가 2회 이상 겹치는 9개 런 전원이 문자 단위 복사를
보였다.** 6개 런(C fm0·C fm2·D fm2·D uv0·E fm2·F fm2)은 S/R 오류가 1회만
발생하고 바로 다음 시도에서 자기 교정에 성공해 "반복"이라 부를 대상 자체가
없다.

복사가 확인된 패턴은 세 가지로 나뉜다:

1. **순수 복사 반복** — F fm0(4=6=9, 완전히 연속)·F uv0 defaults.rs
   (22=26=32=35, 사이사이 `read_file` 재확인이 끼었을 뿐)·F uv1
   invoice.rs(38=42=44, 사이에 `read_file` 1회): 직전 `edit_file` 호출과
   `search`·`replace`가 바이트 단위로 동일 — 다른 도구를 끼워 넣어도
   다음 `edit_file` 시도는 그대로 복사.
2. **변주(범위 확장) — 회귀 여부는 갈림** — `docs/baselines.md`가 이미
   기록한 E fm0(4=6=9 → 12에서 두 번째 케이스를 추가한 변주 → 14=9로 회귀
   → 16=14)가 **B fm0에서도 재현됐다**(12=14=19=23 → 25에서 앞뒤 문맥을
   늘린 변주 → 27=23으로 회귀). C uv0 defaults.rs(idx22→24, 문서 주석
   한 줄을 추가한 변주)도 같은 "범위만 넓히는 자기복사" 유형이지만, 이
   경우는 회귀 없이 바로 다음 시도(26)에서 진짜 수정으로 넘어갔다. 세
   경우 모두 변주 시도 자체가 여전히 `search==replace`를 벗어나지
   못했다(진짜 교정이 아니라 범위만 넓어진 자기복사) — 복사 어트랙터
   가설과 정합.
3. **성공 후 재시도 회귀** — B fm2(idx4 → idx6 실수정 성공 → idx14가 idx4와
   동일 인자로 재발), C uv1(idx38/40 → idx42 실수정 성공 → idx46이 idx38/40과
   동일한 "고치기 전" 텍스트로 재발). 이미 고쳐진 파일에 대해 모델이 옛
   `search`/`replace`를 다시 그대로 제출한다 — 상태를 "잊고" 이전 시도를
   문자 그대로 복사하는 또 다른 형태의 복사 어트랙터.

D fm1(gemma)은 존재하지 않는 코드("BUGGY"/"FIXED" 주석이 붙은 완전히 지어낸
`monthly_total` 함수)를 반복 제시하는 **환각형 실패**와, 실제 코드 블록을
`search`=`replace`로 그대로 제출하는 **복사형 실패**가 교대로 나타났다(41→44
→47→50→53). 환각 시도들은 서로 문자 그대로 동일하고, 복사 시도(44,50)도
서로 문자 그대로 동일 — "매 시도가 진짜 새로운 시도"인 경우는 관측되지 않았다.

**결론(에스컬레이션 트리거 판정)**: 15런 어디에서도 "반복 호출이 직전 출력의
진짜 새로운 변주(문자 단위 복사가 아님)"인 사례는 없었다. 유일한 "변주"
시도(B fm0 idx25, E fm0 idx12)조차 그 자체가 여전히 `search==replace`인
무의미한 확장이었고 즉시 원래 인자로 회귀했다. **전제는 뒤집히지 않았다 —
스펙 개정·재리뷰 불필요.**

## 4. ⓑ write_file 갈아타기 성공/실패

- **성공 사례 — F uv0**: SR_CORRECTION의 "rewrite the whole file with
  write_file" 문구를 수용해 `inv-parse/src/defaults.rs`(41행)·
  `inv-core/src/rules/pricing.rs`(118행 결과물)를 `write_file`로 전체
  재작성 — 두 번 모두 내용이 온전했고(절삭 없음), 이어서 `invoice.rs`(171행)·
  `forecast.rs`(154행)도 예방적으로 같은 방식으로 갈아탔다. 이후 컴파일
  오류(타입 불일치)도 `edit_file`로 정상 수정(idx52-53, S/R 아님). 이 런은
  결과적으로 `passed=True`지만 `outcome=max_turns` — write_file 자체는
  실패하지 않았고, 탐색·복사 루프에 턴을 많이 쓴 탓에 검증·finish할 턴이
  바닥났다.
- **실패 사례 — F uv1**: 같은 SR_CORRECTION 문구를 수용해 `write_file`로
  전환을 **시도**했으나(idx47) `{"path": "inv-report/src/invoice.rs", "tool":
  "write_file"}` — **`content` 필드 자체가 누락**된 호출을 3회 연속 반복
  (idx47,49,51 모두 동일 BadArgs: `missing field \`content\``). 실패
  원인은 스펙 §10이 우려한 "8K에 파일 전체가 안 들어감(길이 잘림)"이
  **아니라**, 갈아타는 시점에 모델이 새 도구의 필수 인자(`content`)를 아예
  채우지 못하는 **호출 형식 결함**이었다. 같은-오류 스트릭 교정(idx53, "The
  same error keeps occurring... rewrite it completely with write_file")이
  주입됐지만 트랜스크립트가 거기서 끝나 있어(`outcome=max_turns`) 회복
  여부는 확인되지 않는다 — `report.json`은 `passed=False`.

두 사례를 종합하면: **갈아타기 자체(내용 생성)는 문제가 아니다 — 실제
전환에 성공하면 크기가 원인인 실패는 관측되지 않았다.** 다만 갈아타기
"시도"가 도구 호출 형식 오류로 실패할 수 있다는 새 리스크가 확인됐다(§10
백로그에 추가할 사항이나, 이번 태스크의 범위는 세부값 확정이므로 판정
절에서만 언급한다).

## 5. ⓒ 대상 파일 크기 — 8K에서 전체 재작성 가능성

```
$ wc -l tasks-large/fix-monthly-total/fixture/inv-report/src/monthly.rs
     190 tasks-large/fix-monthly-total/fixture/inv-report/src/monthly.rs

$ wc -l tasks-large/update-vat-rate/fixture/inv-parse/src/defaults.rs \
        tasks-large/update-vat-rate/fixture/inv-core/src/rules/pricing.rs \
        tasks-large/update-vat-rate/fixture/inv-report/src/invoice.rs \
        tasks-large/update-vat-rate/fixture/inv-report/src/forecast.rs
      41 .../inv-parse/src/defaults.rs
     117 .../inv-core/src/rules/pricing.rs
     171 .../inv-report/src/invoice.rs
     154 .../inv-report/src/forecast.rs
     483 total
```

가장 큰 표적 파일도 190행(monthly.rs)·171행(invoice.rs)이며, F uv0가 이
중 4개 파일 전부를 `write_file`로 절삭 없이 성공적으로 재작성한 실측
사례다(§4). **8K 컨텍스트에서 표적 파일 전체 재작성이 안 될 위험(스펙 §10
리스크)은 이번 15런 + 3런 대조군에서 실증되지 않았다** — 관측된 유일한
write_file 실패(F uv1)는 크기·절삭이 아니라 인자 누락이었다.

## 6. ⓓ 리뷰 1R 선행 판독의 근거 수록 (B/E uv1, F fd1)

스펙 §1·§6이 재검증 루프 개입을 강등한 근거가 된 두 판독을 이 법의학
노트에도 근거 트랜스크립트와 함께 남긴다(§3 요구, 백로그 §6의 입력).

- **B/E uv1 — 뮤테이션 0회의 미완성 탐색 루프**: 두 런 모두 S/R 오류가
  전혀 없어(위 15런에 미포함) `--filter`로 별도 확인했다.
  `run-update-vat-rate-1.jsonl`(B: `.loco/eval/20260717T020632Z/`, E:
  `.loco/eval/20260717T032507Z/`) 정독 결과 두 런 모두 이벤트 38개
  전체에 걸쳐 `edit_file`/`write_file` 호출이 **단 한 번도 없다** — 코드
  탐색(`read_file`/`grep`/`run_command grep|cat`)만 반복하다 마지막
  5턴이 `cat inv-report/tests/check_vat_report.rs`(exit 0) 동일 반복이고
  (idx24/26/30/32/35/37, REPEAT_CORRECTION이 idx34에서 1회 주입됐지만
  무시), `report.json`은 두 런 모두 `passed=False, outcome=repetition_stop,
  turns=18`. `finish_nudge`는 뮤테이션 없이는 무장하지 않으므로(M9
  스캐폴딩 설계), "정지 대신 finish 유도" 개입은 애초에 발동 조건이
  성립하지 않았을 런이다 — 반복정지가 정당한 종결이라는 리뷰 1R 판정과
  일치.
- **F fd1 — write_file 반복 정지로 엄격 손실**: `run-find-definition-large-1.jsonl`
  (`.loco/eval/20260717T034527Z/`) 정독 결과, 정답(`inv-core/src/rules/mod.rs`)은
  idx3에서 이미 찾았고 idx6 `write_file`로 `answer.txt` 작성 + idx8-9
  `cargo test` exit 0까지 조기 완료했으나, idx10 `finish {}`(summary
  누락)가 오류를 반환한 뒤 모델이 **동일한 `write_file(path=answer.txt,
  content="inv-core/src/rules/mod.rs\n")` 호출을 9회(idx6,18,24,31,37,41,
  45,49,51) 반복**하며 그 사이사이 빈 `finish {}`도 반복 — 5회째 동일
  히트에서 반복정지. `report.json`: `passed=True, outcome=repetition_stop,
  turns=25`(엄격 기준으로는 손실). 반복 히트가 `write_file`이라 S/R 개입
  대상이 아니고, 재현 표본이 이 1런뿐이라 스펙 §6의 처분(개입 보류, 데이터
  축적 대기)이 이 근거로 뒷받침된다.

## 7. 판정 — 개입 세부값 확정

### 7-1. 차단 임계 — **3회 누적 유지**

15런 전수에 반례가 없다:

- 실제로 임계(3)에 도달한 런(B fm0·E fm0·F fm0·C uv0 pricing.rs·F uv0
  defaults.rs·F uv1 invoice.rs)은 전부 문자 단위 복사가 확인된 완고
  케이스였고, 3회째 누적 시점은 이미 도구 처방(1회차)·SR_CORRECTION(2회차)
  두 텍스트 교정이 무력화된 뒤였다 — 스펙 §4의 근거("3회째 강제 전환")와
  정확히 부합.
- **파일당 최대 2회에 그쳐 임계 미도달한 D fm1**(monthly.rs 문자단위
  복사 2회, 환각 시도가 사이사이 끼어 비연속)은 스펙 §2 비목표가 이미
  "gemma는 발동 임계 미도달"이라 기록한 것과 이 노트가 독립적으로
  재확인한 결과다 — 오탐(과도 차단) 위험이 낮다는 근거.
- **누적(비연속) 설계의 정당성**도 실증됐다 — C uv0(pricing.rs 3회가
  idx28·34·36으로 비연속), F uv0(defaults.rs 4회가 idx22·26·32·35로
  비연속)처럼 "다른 시도가 끼어드는" 완고 케이스가 실제로 존재해, 연속
  스트릭만 보는 개입은 이들을 놓쳤을 것이다.
- 차단이 발동했을 상황(F uv0 defaults.rs)에서 실제로 write_file 전환이
  성공(§4)했으므로, 강제 전환이 실제로 유효할 수 있다는 근거도 있다.
  다만 F uv1(invoice.rs, 정확히 3회째에서 조직적으로 write_file 전환을
  시도했으나 인자 누락으로 실패)은 차단 임계의 값 자체보다 **전환 신뢰성**의
  한계를 보여준다 — 임계값 조정으로 해결될 문제가 아니라 §10 리스크
  목록에 남을 사항이다.

값을 낮출 근거(2회)는 없다 — 2회째에서 이미 멈출 만큼 뚜렷한 완고 신호를
보인 런이 없고(단일 발생 6런은 2회째 없이 자가교정), 값을 높일 근거(4회
이상)도 없다(B fm0가 7회까지 방치됐을 때 아무 이득이 없었다 — 25·27번째
시도까지 그대로 복사만 반복). **3회 유지.**

### 7-2. 섭동 온도 — **0.7 유지 (단, 이번 법의학으로는 값 자체를 검증할 수 없음)**

이 15런은 전부 기본 temperature(0.1, config 기본값)로 수행됐다 — 이
법의학 노트에는 "다른 온도에서 어떻게 됐는가"를 보여줄 데이터가 **존재하지
않는다**. 따라서 0단계가 답할 수 있는 것은 §5가 겨냥하는 **전제**
("저온 샘플링의 복사 어트랙터" 가설, H2)이지 **값**(0.7이 맞는 숫자인지)이
아니다.

- 전제 확인: §3(ⓐ)에서 확인했듯 반복은 예외 없이 문자 단위 복사이고,
  텍스트 교정 3층(도구 처방→SR_CORRECTION→REPEAT_CORRECTION)이 전부
  무력화되는 사례(B fm0·E fm0·F fm0)가 실재한다 — 디코딩 층 개입을
  실험할 근거는 충분하다.
  트리거 시점(S/R 2연속)도 적절해 보인다 — E fm0·F fm0의 3연속 복사는
  2회째(idx6/6)에서 이미 스트릭 2에 도달해 섭동이 조기에 걸렸을
  타이밍이고, B fm0의 재발(idx12→14) 역시 2회째(idx14)에서 스트릭이
  다시 채워진다(idx6의 성공적 편집이 스트릭을 리셋했다가 idx12에서
  재축적 — §5 원복 핀이 의도한 그대로 동작할 지점).
- 값(0.7) 자체는 이번 법의학으로 확정도 반증도 할 수 없다 — 이는 실제
  arm③ 측정(추후 태스크)이 답할 질문이다. 0단계의 역할은 "실험할
  가치가 있는 가설인가"까지이지, "0.7이 정답 숫자인가"가 아니다. 값을
  바꿀 근거가 없으므로 스펙 기본값을 그대로 유지하고, 실험 1의 결과로
  재검토한다.

## 8. 자체 검토

- 표의 모든 행은 실제 `.loco/eval/<스탬프>/run-<과제>-<반복>.jsonl`
  이벤트 인덱스를 인용했다(§2 표 "근거" 열) — 지어낸 수치 없음. SR 발생
  횟수는 브리프 스크립트의 원출력(§1)과 표 값이 1:1 대응하고, 1단/2단
  오류 합(19/23)도 `docs/baselines.md` 기존 집계와 일치시켜 교차검증했다.
- ⓒ의 `wc -l` 값은 이 세션에서 직접 실행한 결과이며, monthly.rs=190행은
  스펙 §3이 이미 "확인됨"이라 적은 값과 일치한다.
- ⓓ의 B/E uv1·F fd1 판독은 지시된 그대로 별도 정독해 재확인했다(15런에는
  포함되지 않는 대조군 3런).
- 판정 절(§7)의 두 결론(차단 임계 유지, 섭동 온도 유지-단 값 미검증)은
  모두 §2-§6의 표·근거에서 직접 도출했으며, 반증 사례(에스컬레이션
  트리거)는 없었다.
- 남는 불확실성: (1) F uv1의 write_file 인자-누락 실패가 재현 가능한
  패턴인지 단일 사례인지는 이 표본(1건)만으로는 알 수 없다 — §10 백로그
  항목으로 남긴다. (2) C fm0의 repetition_stop은 S/R 오류(idx4-5) 이후
  전혀 다른 원인(부호를 고치지 못한 채 들여쓰기만 바뀐 "성공" 이후의
  read_file 반복)이었다 — 이 런을 "S/R 루프가 실패를 유발했다"는 근거로
  쓰지 않도록 표에 명시했다.
