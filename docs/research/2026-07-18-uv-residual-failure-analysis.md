# uv 잔여 병목 정독 — M11 이후 실패 프로파일 (M12 스코핑 입력)

- 데이터: `.loco/eval/20260718T082449Z`(M11 개입 8K, ornith-1.0-9b, temp 0.1,
  seed 0..9) `run-update-vat-rate-*` 10런 중 실패 8런 전수 정독 + 축 1(인자
  누락 오형) 배치 횡단 센서스. 측정 0, 문서만.
- 선행: `docs/research/2026-07-17-m8-failure-analysis.md`(M8 실패 분류),
  `docs/experiments/2026-07-18-progress-grounding/report.md`(실험 2 판정 —
  지표 수준까지만 다룸; 본 노트가 그 밑의 서사층).
- 작성: 2026-07-18, 정독은 서브에이전트 3분담(무뮤테이션 3런 / max_turns
  3런 / 늦은 착수·거짓 finish 2런) 후 컨트롤러 종합.

## 1. 정답 지형 (재확인)

4지점, 표기 전부 상이(산포 함정 설계 의도대로 작동):

| 지점 | 파일 | 표기 | check 고정 |
|---|---|---|---|
| defaults | `inv-parse/src/defaults.rs:8` | `DEFAULT_VAT_PERCENT: u32 = 10` | check_vat_default (12) |
| pricing | `inv-core/src/rules/pricing.rs:4` | `amount_krw * 10 / 100` | check_vat_core (11_200) |
| invoice | `inv-report/src/invoice.rs:8` | `subtotal_krw * 110 / 100` | check_vat_report (112_000) |
| forecast | `inv-report/src/forecast.rs:8` | `net_krw as f64 * 1.10` | check_vat_report (224_000) |

판정 역학 두 가지가 실패 형태를 크게 규정했다:

- **cargo test는 fail-fast**: inv-core(check_vat_core)가 먼저 실패하면
  inv-report의 invoice·forecast 실패는 화면에 아예 안 나온다. B층 3런
  내내 inv-report 실패가 한 번도 노출되지 않았다("남은 건 core뿐" 착시).
- **pricing은 순수 산술식**: 이름·주석 앵커가 없어 8런의 어떤 grep
  어휘(`vat*`/`10%`/`부가세율`/`세율`/`0.10`)에도 안 걸렸고, 유일한 도달
  경로가 cargo 실패 신호였다. B층 완료 0/3.

## 2. 런별 분류 (실패 8런)

| 런 | outcome | 층 | 완료 지점 | 요지 |
|---|---|---|---|---|
| 0 | finished | A 착수 실패 | 0 | grep 1방에 4지점 전부 노출 → "프로젝트 구조 요약"으로 과제 재해석, 5턴 finish |
| 2 | repetition_stop | A 착수 실패 | 0 | "--vat 플래그 신설" 오해로 inv-cli만 탐색 → `args` 안 `"tool":"list_files"` 오형 5연속 정지 |
| 5 | finished | A 착수 실패 | 0 | grep 1방을 산출물로 간주, **전량 날조 summary**(존재하지 않는 상수·file:line·계수) 3턴 finish |
| 1 | max_turns | B 실행 손실 | defaults | S/R 4회(2회 defaults→write_file 우회 성공, 2회 pricing 미회복), S/R 오류를 "이미 수정됨"으로 오독 |
| 3 | max_turns | B 실행 손실 | defaults·invoice | **cargo 0회**, forecast 읽고도 방치, 보호 테스트 파일에 무익 편집 2회, 환각 코드 S/R 1회 |
| 4 | max_turns | B 실행 손실 | defaults·forecast | S/R 5회 중 트리거 도달 2연속만 회복, invoice S/R 2회 비연속 사각+래치 소진, pricing 미복귀 |
| 7 | max_turns | C 가설 고착(음성) | defaults | "세율=`0.10` float" 환각 고착 — no-match grep 6연발, 실물 `* 110 / 100`을 보고도 환각 상수로 편집, **cargo 0회** |
| 8 | finished | C 가설 고착(양성) | invoice | `cargo test -p inv-report check_vat_report` = **테스트명 필터 0매치·exit 0**을 4회(--nocapture 재시도 포함) "통과"로 오독, FINISH_NUDGE가 거짓 finish 처방 |

(통과 2런: uv-6 max_turns 통과 — defaults grep 식별자 경로, uv-9 finished.)

## 3. 실패 3층 구조

### A층 — 첫 뮤테이션 착수 실패 (0·2·5, 3/8)

세 런 모두 첫 수 `grep "10\b"`으로 **정답 4지점을 1턴 만에 전부 시야에
넣고도** 편집 단계에 도달하지 못했다. 특정 표기를 놓친 게 아니라, 17KB·
50매치 절단의 고소음 결과 직후 과제 표상이 붕괴하고 각자의 제네릭
행동(레포 설명 / 검색 결과 보고 / CLI 코드 탐색)으로 표류했다. M9/M10이
겨냥한 S/R·검증 루프보다 상류의 축이며, `first_mut_turn`이 정의되지 않는
런들이다.

분기: 표류가 "답을 말하는 질문" 모드에 착지하면 finish가 자연 종점(0·5,
3~5턴 종료 — 상태선 cadence 5턴·FINISH_ARGS_CORRECTION 2연속 요건 모두
미달, **뮤테이션 0 finish를 막는 장치는 설계상 없음**: VERIFY_NUDGE는
뮤테이션 후 전용), "더 탐색" 모드에 착지하면 오형 루프로 반복정지(2 —
유일하게 하네스가 잡아서 끝낸 런).

### B층 — 실행 손실 (1·3·4, 3/8)

발견은 대체로 성공(defaults 3/3 완료·노출은 거의 전 지점). 침몰 원인은
S/R 실행 실패와 실패 지점 미복귀다. 3런 합산 75턴 부검:

| 용도 | 턴 | 비율 |
|---|---|---|
| 신규 탐색 | 28 | 37% |
| 오류(S/R 12·0매치 2·BadArgs 4) | 18 | 24% |
| 오류 후 재확인 read | 5 | 7% |
| 신규 정보 없는 중복 재독 | 11 | 15% |
| 성공 뮤테이션(지점 명중 5·보호 테스트 2·주석 1·비대상 1) | 9 | 12% |
| cargo 검증 | 4 | 5% |

오류+뒷수습+중복 재독 = **34턴(45%)**. 표기별 최종 성적: `= 10` 3/3,
`1.10` 1/3, `* 110 / 100` 1/3, `* 10 / 100` 0/3.

**S/R 트리거 도달률이 병목**: 2연속 트리거(SR_CORRECTION+온도 섭동)에
도달한 스트릭은 **2/2 즉시 회복**. 그러나 12건 중 8건이 사각 — 사이에
read/cargo가 끼면 스트릭 리셋(런 1 pricing ×2, 런 4 invoice ×2), 래치는
런당 1회라 두 번째 파일부터 처방 부재(런 4). 사각 8건의 회복은 1건(런 1
defaults, 자발적 write_file)뿐. 32K 배치의 sr recovered 22/48(대조
36/45) 열화도 같은 축으로 추정된다(본 노트 범위 밖, 미정독).

부수 관측: ① S/R 오류를 "이미 수정돼 있다"로 오독(런 1 — 오류문에 "파일은
변경되지 않았다"가 명시돼 있지 않음) ② 환각 코드가 S/R 동일성 검사에 먼저
걸려 "S/R 오류"로 위장(런 3 — 매치 존재 검사가 뒤라 존재하지 않는 코드임이
안 드러남) ③ 보호 경로(tests/) 편집에 성공 응답이 나가 4턴 낭비(런 3).

### C층 — 가설 고착의 양면 (7·8, 2/8)

두 런 모두 작업 범위를 외부 준거(check = 무필터 `cargo test`)가 아니라
모델 내부 가설로 정의했고, 증거 피드백의 부호가 얼굴을 갈랐다:

- **런 7 (거짓 음성)**: "세율=`0.10` float 상수" 표기 가설 → no-match
  grep 6연발로 탐색 무한 연장(첫 뮤테이션이 도구결과 19번째), `reservation`
  부분문자열 홍수(16.6KB) 직후 과제 표류, max_turns. **cargo 0회.**
- **런 8 (거짓 양성)**: `cargo test --package inv-report check_vat_report`
  — cargo는 `check_vat_report`를 **테스트명 필터**로 해석, 0개 실행·exit 0.
  이 공허한 초록불을 4회(--nocapture 재시도 포함, 스펙 리뷰 1R 실측 계수)
  전부 "통과"로 읽고 종결. 유일한 실질 검증(exit
  101, invoice+forecast FAILED)은 직후 출력토큰 절단으로 forecast 분이
  유실. **FINISH_NUDGE가 0-테스트 exit 0으로 무장돼 거짓 finish를 직접
  처방**했고, 상태선 "verification: last command exited 0"도 거짓 초록을
  강화했다 — M11 장치 2종이 이 런에선 역효과.

두 런 모두 과제문이 지시한 무필터 `cargo test`를 한 번도 실행하지 않았고,
그 한 번이 남은 지점 전부를 기대값과 함께 열거해 줬을 것이다. 수렴점:
**검증의 실질(몇 개 돌았고 몇 개 실패했고 무엇이 실패했는지)을 exit code
대신 접지**하는 것 — 런 7에는 착수·검증 처방으로, 런 8에는 거짓 초록불
차단으로 작용하는 동일 메커니즘.

## 4. M11 장치 감사 (uv 8런 기준)

| 장치 | 판정 |
|---|---|
| 상태선 — 사실 접지 | 정확(오신념과 모순되는 증거 제시 사례 포함: 런 1 t20) |
| 상태선 — 행동 유도 | 확증 1건(런 8 @10 직후 첫 cargo), 무시 다수(런 3: "verification: none" 7회 반복에도 cargo 0회), **역효과 1건**(런 8 @20 "exited 0"이 공허한 초록 강화) |
| 상태선 — cadence 사각 | A층 조기 finish 2런은 5턴 미만 종료로 한 번도 미발화 |
| FINISH_NUDGE | **역효과 1건**(런 8 — 0-테스트 exit 0으로 무장, 거짓 finish 처방). 무장 조건이 "exit 0"뿐인 것이 원인 |
| VERIFY_NUDGE | A층에 설계상 미적용(뮤테이션 0 finish는 관할 밖) |
| SR_CORRECTION+섭동 | 도달 시 2/2 회복 — 유효. 도달률 4/12가 병목(비연속 리셋+런당 래치) |
| REPEAT_CORRECTION | 오형·재독 루프에 무력(주입 후에도 동일 호출 지속, 3례) |
| salvage | `args` 안 `"tool"` 키를 액션 레벨로 올리는 역방향 규칙 부재 → 런 2가 5연속 오형으로 정지 |

## 5. 축 1 연계 — 인자 누락 오형 센서스 (배치 횡단)

오형 형상(전 배치 공통, 완결 JSON — 잘림 아님):

```json
{"action": {"args": {"path": "src/lib.rs", "tool": "write_file"}, "tool": "write_file"},
 "thought": "Rewrite the file with the fix."}
```

페이로드 인자(content/search/pattern/command/path)를 생성하지 않고 `tool`
이름을 args 안에 중복 복사. write_file 한정이 아니라 **6도구 전반의 계열
버그**다(M10 F-uv1 원형은 grep `pattern` 누락 — 동일 형상). BadArgs
에코("You sent keys: [path, tool]")를 받고도 동일 호출을 그대로 재복사
(최대 5연속 → 반복정지). **S/R 교정("전체 파일을 write_file로 다시
써라")이 이 오형으로의 깔때기로 작동한 사례 확인**(092740Z
fix-failing-test-0: 교정 직후 첫 write_file부터 content 없이 발사).
SR_PERTURB는 S/R 오류 전용이라 BadArgs 스트릭엔 디코딩층 개입이 없다.

배치별 발생률(전 툴콜 대비, 괄호는 오형 발생 런 수):

| 조건 | 대조(M10 코드) | 개입(M11 코드) |
|---|---|---|
| 8K uv+fm | 152633Z: 4/311 = 1.3% (4런) | 082449Z: 10/298 = 3.4% (5런) |
| tasks/ 스포트 | 215729Z: 3/313 = 1.0% (3런) | 092740Z: 26/342 = 7.6% (10런) · 115152Z: 12/339 = 3.5% (7런) |
| 32K | 164905Z: 10/292 = 3.4% (7런) | 101234Z: 29/322 = 9.0% (9런) |

- 개입 배치가 일관 상향(2.5~8배). 단 **인과 미확정**: ① 상태선 직후 턴의
  오형률은 일반 턴과 차이 없음(국소 인접 효과 부재 — 082449Z 3.3% vs
  3.4%, 092740Z 6.5% vs 7.9%, 101234Z 6.0% vs 10.1%) ② 발생이 런당
  5연속 버스트라 배치 합계가 소수 런에 지배됨(분산 민감) ③ 개입군이
  뮤테이션 단계에 더 많이 도달하는 분포 이동 효과 후보. 상태선의 문맥
  수준 효과 가능성은 배제 못 하나 관찰 데이터로는 분리 불가.
- 직접 사망 기여: 092740Z 실패 6런 중 3런(fix-failing-test-0·multiline-1
  content 각 5연속, rename-2 command 5+path 3), 115152Z multiline-1
  재실패, 082449Z fm-3(command 5연속), 101234Z uv-4(content 5연속).

## 6. ④ grep-first 맵 판정

**부활 근거 없음.** 8런 전수에서 발견 실패가 주인인 런은 없다 — A층은
4지점 노출 후 착수 실패, B층은 노출 후 실행 실패, C층은 가설 고착. 유일한
구조적 발견 갭(pricing 순수 산술식)은 지도가 아니라 검증 신호(cargo 실패
테스트명)로 도달하는 것이 정도다. M8 판정(트리 의존 탐색 0/27) 유지.

## 7. M12 개입 후보 (우선순위 제안)

1. **검증 실질 접지** — run_command 결과에서 cargo test 출력(`running N
   tests`/`N passed; N failed`/`N filtered out`)을 파싱해: ① "0 tests
   ran + 필터 0매치"에 1줄 무효화 노트(M11 파이프 노트와 같은 자리) ②
   상태선 verification 필드에 exit code 대신 실질(실패 테스트명 포함) ③
   FINISH_NUDGE 무장 조건을 exit 0 → "≥1 passed"로 강화. C층 양면·B층
   실패 지점 복귀·M11 장치 역효과 2건을 한 메커니즘으로 커버. 하네스가
   이미 아는 정보의 접지(소형모델 네이티브 원칙 합치).
2. **오형(BadArgs) 개입 일반화** — ① 동일 오류 스트릭 온도 섭동(M10 승자
   메커니즘을 S/R 전용에서 일반화) ② salvage 역방향 규칙(args 안 `tool`
   키 → 액션 레벨) ③ 구법 제약 디코딩(LM Studio `response_format`) —
   구조적 근절이나 실험 면적 큼(탈출구 설계·스트리밍 상호작용), 별도 암
   또는 후속 검토.
3. **뮤테이션 0 finish 게이트** — mutations==0 ∧ run_command==0인 첫
   summary-finish 1회 반려+접지 노트. A층 2런 직격. 단 읽기-전용 과제
   (find-definition·count-usages)는 뮤테이션 0 finish가 정답이라 반려
   비용·오유도 리스크 설계 필요(핵심 설계 쟁점).
4. **S/R 트리거 도달률** — 파일별 누적 카운터(비연속 허용)·래치 파일별화.
   B층 사각 8건 + 32K sr recovered 열화 겨냥. M10 arm-block의 "파일별
   누적" 계수를 차단이 아니라 교정·섭동 발화에 재사용하는 구도.
5. **소품** — S/R 오류문에 "The file was NOT modified" 명시(런 1 오독),
   edit_file 검사 순서 교체(매치 존재 → S/R 동일성; 런 3 환각 위장),
   grep 계수 헤더·부분문자열 홍수 힌트(런 5 날조 계수·런 7 표류).

## 8. 원자료

- 정독 대상: `.loco/eval/20260718T082449Z/run-update-vat-rate-{0,1,2,3,4,5,7,8}.jsonl`
- 오형 센서스 스탬프: §5 표의 7종 (대조 3·개입 4)
- 오형 추출·센서스 스크립트: 세션 스크래치(1회용, 리포 밖) — 방법론:
  missing-field tool_result의 직전 assistant 턴 추출 / 툴콜 총수 대비
  발생률 + 직전 결과 상태선 유무 교차표
