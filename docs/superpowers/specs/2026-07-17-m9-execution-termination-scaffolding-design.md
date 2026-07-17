# M9 — 실행·종료 스캐폴딩 설계 (edit_file 자기-버그 + finish 규율 + 2단 측정)

승인일: 2026-07-17. 브레인스토밍으로 확정, 스펙 전문 리뷰 1R 반영(Critical 0·
Important 3·Minor 6·Nit 3 — 전건, 코드 실측 대조 리뷰; 2R 진행 중).
직전 상태: M8 완료(main `86882f8`,
리워드 후속 커밋 포함), tasks-large 기준선(리워드 전 픽스처) gemma-4-e4b 44.4% ·
ornith-1.0-9b 55.6%@8K · 88.9%@32K(엄격 44.4%), 소형 세트 기준선 gemma 72.2% ·
ornith 94.4% (`docs/baselines.md`). 스코핑 입력: `docs/m9-candidates.md` +
`docs/research/2026-07-17-m8-failure-analysis.md`(이하 "실패분석").

## 1. 배경 — 실패 데이터가 정한 범위

실패분석 §4의 우선순위(빈도 × 재현성 × 백로그 정합성)를 그대로 따른다:

1. **`edit_file` 자기-버그(search==replace 동일 텍스트)** — 27런 중 9런·총 37회,
   전부 ornith. 8K에서는 실패의 직접 원인(7연속 실패 → 반복정지), 32K에서는
   대체 도구로 갈아타는 데 턴 예산을 소진해 엄격 통과율을 묶는 원인.
   **컨텍스트 크기와 무관하게 재발** — 컨텍스트 축이 아니라 스캐폴딩 축의 개입
   대상. M8 스펙 §8 백로그에 없던 신규 최우선 항목.
2. **finish 종료 규율** — `finish({})`(필수 `summary` 누락) 9회 반복(8K, 5연속으로
   반복정지 1건), 32K에서는 강박적 재검증으로 형태가 바뀌며 턴 규모가 오히려
   커짐(9→19턴). `finish` 전용 교정이 없는 것이 원인.
3. 환각 교정(실패분석 P2)은 근거 2런으로 재현성이 낮아 제외. repo-map 계열은
   9/9 런이 T1 grep 일격으로 함정을 무력화한 실측에 따라 **강등 유지**. 오버플로
   내성 4건은 이번 배치 발생 0건으로 백로그 유지.

**이 설계의 조준점은 "도달 이후"의 실행·종료 신뢰성이다.** M8 실측에서 대형
저장소 고유 실패(파일 미발견·재수출 오인·갓파일 스크롤)는 관측되지 않았다.
실패의 다수 — 특히 ornith 엄격 실패의 지배적 축(S/R 37회·finish 실패) — 는
정답 파일 도달 *이후* 단계에서 발생했고, 미도달 런들(ornith 환각 2런, gemma
진단 회피)도 원인이 항해가 아니었다 — 이들은 아래 한계와 §2 비목표로 별도
처분한다. 두 한계를 명시한다:

- **gemma의 실패 축(진단 회피·과신)은 이번 범위가 아니다.** 본 설계의 교정은
  전부 "시도 후 실패" 시점에 발동하므로, 시도 자체를 안 하는 gemma fix-monthly
  0/3은 M9으로 개선되지 않을 가능성이 높다.
- **교정 주입의 효과는 가설이다.** 같은 계열의 개입(3연속 스트릭 교정, finish
  인자 예시 에코)이 발동하고도 루프를 못 끊은 런이 실측에 있다. "더 이르고(2연속)
  더 구체적인 처방이면 끊긴다"는 검증 대상이며, 그래서 측정을 행동 지표 중심
  2단으로 설계한다 — 효과가 없으면 그것도 명확한 데이터로 남긴다.

## 2. 목표·비목표·성공 기준

**목표**:
1. `edit_file` S/R 자기-버그 스캐폴딩(도구 층 + 에이전트 층) — §3
2. finish 종료 규율 스캐폴딩 2종 — §4
3. 2단 측정 — 리워드된 픽스처 재베이스라인(스캐폴딩 전) → 스캐폴딩 후 재측정,
   리워드 효과와 스캐폴딩 효과를 분리 — §5
4. 문서 갱신 — baselines.md(재베이스라인 절 + 행동 지표 추출 레시피), README,
   CLAUDE.md

**비목표 (명시)**:
- 환각 교정(존재하지 않는 코드를 `search`에 인용하는 케이스 전용 개입) — 근거 2런
- repo-map·검색 강화·트리 개인화 — 강등 유지, 승격 조건은 §7
- 오버플로 내성 4건(M8 §8 이월분) — 발생 0건, 백로그 유지
- 턴 예산 힌트("예산이 얼마 안 남았다, finish하라") — 미완성 조기 finish를 유발해
  거짓 성공 finish 카운트를 악화시킬 위험이 엄격 지표와 상충
- `run_command`로 답 파일을 쓰면 VERIFY_NUDGE가 안 걸리는 사각지대(gemma
  find-definition r0) — 기록만, run_command 뮤테이션 분류는 별도 설계 필요
- 기존 `tasks/`·`tasks-large/` 픽스처 변경 일절 없음(리워드는 이미 완료된 상태)

**성공 기준**:
1. 게이트: `cargo test` + `cargo clippy --all-targets -- -D warnings` +
   `eval tasks --verify` 12/12 + `eval tasks-large --verify` 3/3
2. 행동 지표(주): 1단에서 관측된 실패 루프가 2단에서 소멸·감소 —
   ① S/R 스트릭발 반복정지 0건, S/R 오류 발생 런의 "2회 이내 회복률"(S/R 오류 후
   2번째 시도 안에 해당 지점 수정 완료) 1단 대비 상승
   ② finish 인자누락발 반복정지 0건, "검증 완료 후 finish 도달률" 상승
   (1단에서 해당 루프가 아예 발생하지 않으면 그 지표는 "비악화 + 단위 테스트
   게이트"로 대체 판정. 발생 런이 적으면 — 배치당 3런 미만 — 비율 대신 발생 런
   전수를 나열하고 방향성으로 판정한다: 소표본에서 비율은 노이즈에 좌우된다)
3. 통과율(보조): tasks-large 엄격 통과율 1단 대비 비악화(개선 기대), tasks/
   스포트(ornith@8K) 기준선 94.4%(34/36) 대비 하락 1런 이내(33/36 이상 —
   회귀 하한 게이트, 상회는 제한 없음)
4. 신규 모델-대면 텍스트 전부 영문(기존 관례: identifiers/SYSTEM_PROMPT/교정문
   영문, 사용자 CLI 메시지 한국어)

## 3. 개입 1 — edit_file S/R 자기-버그 (2층)

**관측 패턴**(실패분석 §3): 사고(thought)는 항상 올바른 수정 의도를 서술하는데,
`replace` 필드 생성 시점에 수정 전 텍스트를 그대로 채운다. 현재도 거부되지만
("search and replace are identical - no change would be made") 오류문이 원인
("무엇이 잘못됐는가")만 말하고 처방("무엇을 해야 하는가")이 없다.

**3-1. 도구 층** (`tools/edit_file.rs`): 오류문에 처방 한 문장을 추가한다 —

> `search and replace are identical - no change would be made. Put the code as
> it is NOW in `search`, and the code AFTER your change in `replace`.`

1회차부터 발동, 무상태. 기존 스트릭 판정 키(첫 문장 = 첫 `.`까지)가 안정적으로
유지되도록 추가 문장은 별도 문장으로 붙인다(첫 문장 자체는 불변).

**3-2. 에이전트 층** (`agent/repetition.rs`): S/R 오류 **2연속** 시 전용 교정을
1회 주입한다(신규 상수 `SR_CORRECTION`, 별도 래치) —

> `Your `replace` is identical to `search`. Write the MODIFIED code in
> `replace`. If you cannot produce a different `replace`, rewrite the whole
> file with write_file, applying the fix.`

- 판정: `tool == "edit_file"` ∧ 오류 첫 문장이 S/R 키와 정확 일치, 2연속.
- 기존 일반 교정(3연속 `EDIT_STRATEGY_CORRECTION`)과의 관계: **S/R 키 스트릭은
  전용 교정이 전담**하고 일반 오류-스트릭 교정 대상에서 제외한다(같은 스트릭에
  교정 2건이 겹쳐 주입되는 노이즈 방지). 다른 오류 스트릭에 대한 일반 교정은
  불변. 단, 인자까지 동일한 S/R 반복에는 윈도 기반 순환 교정(3회째
  `REPEAT_CORRECTION`)이 별도로 얹힌다 — 이는 의도된 에스컬레이션이다(2회째
  전용 교정 → 3회째 순환 교정 → 5회째 반복정지).
- 주입 방식은 M5 관례를 따른다: 해당 tool_result 본문에 병합. 반복 윈도의 결과
  해시는 병합 **전** 원문 기준이므로(M5 기존 교정과 동일 경로) 교정 병합이 윈도
  계수를 리셋하지 않는다 — 신규 교정도 같은 경로를 유지해야 한다.
- 임계 2연속의 근거: 도구 층 강화문(3-1)이 1회차 처방이므로, 그것마저 무시한
  2회차가 "문구가 아니라 전략 개입이 필요한" 시점이다. 기존 3연속을 기다리면
  8턴 윈도의 반복정지(5회)까지 여유가 2턴뿐이다.

## 4. 개입 2 — finish 종료 규율 (2종)

**4-1. 인자누락 스트릭 교정** (`agent/mod.rs`): summary 없는 finish **2연속** 시
1회 주입(별도 래치, `FINISH_ARGS_CORRECTION`). 기존 `FINISH_ERR` 에코는 인자
예시(`{"tool": "finish", "args": {"summary": ...}}`)를 이미 담고 있는데도 5연속
반복을 못 막았다(실패분석 §3 find-definition). 차별점은 **모델이 실제로 내보내야
하는 전체 턴 형태**를 제시하는 것 —

> `Your finish call is missing `summary`. Respond with exactly this shape:
> {"thought": "...", "action": {"tool": "finish", "args": {"summary": "<your
> final answer>"}}}. Do not call finish with empty args again.`

카운트는 "summary 없는 finish"의 연속 횟수 — **디스패치된** 다른 액션이나 유효
finish가 오면 리셋하고, 액션 없는 턴(파싱 실패 재시도·length-cut)은 증가도
리셋도 없이 유지한다. 기존 FINISH_ERR 에코·반복 계수 편입(M5 §7.3)은 불변 —
이 교정은 그 위에 얹히는 2연속 시점의 1회 추가 개입이다.

**4-2. 검증완료 후 finish 유도** (`agent/mod.rs`, `FINISH_NUDGE`): 목표 패턴은
"이미 확인한 사실을 문자 그대로 재확인하는 루프"(32K find-definition r1·r2의
`cat`/`grep` 7~9회 재반복)다. 런당 상태 `mutated`·`armed`·`idle_count`·래치로
이벤트 구동 상태기계를 정의한다. 여기서 "뮤테이션"은 **edit_file/write_file의
성공 디스패치**를 뜻한다(registry의 `is_mutating()` 분류와 다름 — run_command는
이 정의에서 뮤테이션이 아니고, sed/python3 우회 쓰기는 아래 run_command
이벤트로만 취급된다).

| 이벤트 (디스패치된 액션 기준) | 전이 |
|---|---|
| edit_file/write_file 성공 | `mutated=true`; `armed=false`; `idle_count=0` |
| edit_file/write_file 실패 시도(오류·게이트 거부 포함) | `armed=false`; `idle_count=0` — 수정 의도가 있으면 재검증 국면이 아니다(S/R 루프 중 finish 유도 방지). 이후 검증 성공이 오면 재무장 가능 |
| run_command Ok ∧ 본문 첫 줄 `exit code: 0` | `mutated`이고 비무장이면 `armed=true; idle_count=0`; 이미 `armed`면 `idle_count += 1`(재검증도 카운트 — 매 검증마다 리셋하면 run_command 재검증 루프에 영원히 발동하지 않는다) |
| run_command 그 외(비0 종료코드·타임아웃·취소·Err) | `armed=false`; `idle_count=0` — 실패한 검증 뒤 finish 유도는 역효과 |
| read_file/grep/list_files | `armed`면 `idle_count += 1` |
| finish 시도(유효·무효 무관) | `idle_count=0` — 종료 의도가 이미 있음; 무효 finish 교정은 4-1 전담 |
| 액션 없는 턴(파싱 실패 재시도·length-cut) | 상태·카운터 불변(증가도 리셋도 없음) |

**발동 조건**: `armed` ∧ `idle_count ≥ 4` ∧ 카운트된 최근 4턴 중 **1턴 이상이
반복 호출**(반복 윈도에 동일 (도구|인자) 키가 이미 존재하는 호출) ∧ 미래치 →
1회 주입 후 런당 래치.

> `You already ran a successful verification. If the task is complete, call
> finish with a summary now; do not re-verify what you have already confirmed.`

- 반복-호출 조건의 근거: 무장 신호(exit-0 run_command)에는 `cat`/`grep`류도
  포함되므로, 이 조건이 없으면 "1지점 수정 → 부수적 검증 → 다음 지점 *신규*
  탐색 4턴"의 작업 한중간에 발동해 런당 1회 래치를 소진하고 정작 말미의 강박
  재검증 국면에 개입이 남지 않는다. 관측된 재검증 루프는 전부 문자 그대로의
  반복이었고(실패분석 §3) 신규 탐색은 반복이 아니므로, 이 신호가 두 국면을
  가른다. 판별은 기존 반복 윈도(8턴, (도구|인자) 키)를 재사용한다.
- 종료코드 판별: run_command Ok 본문 첫 줄의 `exit code: {code}` 문자열 파싱.
  타임아웃·취소 본문에는 이 줄이 없어 "성공 검증"에서 자연 배제된다. VERIFY_NUDGE
  의 해제 기준("종료코드 무관 Ok")은 기존대로 두며 이 상태기계와 별개다.
- 조건부 문구("If the task is complete")로 조기 finish 압박을 완화하고, 부작용은
  2단 측정의 거짓 성공 finish 카운트로 감시한다.

**주입 상한**: 신규 교정 3종(SR/FINISH_ARGS/FINISH_NUDGE)은 각각 런당 1회
래치 — 기존 래치형 주입 3종(순환 교정·오류 스트릭 교정·VERIFY_NUDGE 반려)과
합쳐 런당 최대 6회, 각 1~3문장. 8K 컨텍스트 압박에 유의미한 추가가 아니다.

## 5. 측정 계획 — 2단

**공통 조건**: baselines.md "M8 측정 조건" 절과 동일(`max_output_tokens = 4096`,
`command_timeout_secs = 240`, `--repeats 3 --seed 0`; 8K 배치 `context_tokens =
8192`/로드 12288, 32K 배치 `context_tokens = 32768`/로드 49152). **리워드된
픽스처**(58aab75 이후) 사용 — M8 수치와의 직접 비교는 하지 않고 참고만 한다.
측정 중 빌드/테스트 병행 금지. 각 단계 시작 전 두 tasks 트리 `--verify` 통과
확인. 모델 전환은 측정 주체가 배치 사이에 직접 수행한다: 이전 모델 언로드 →
대상 모델 로드(컨텍스트 길이 확인) → `curl localhost:1234/api/v0/models`로
로드 상태 검증 후 배치 시작(`model = ""` 자동 선택은 로드된 첫 모델을 잡으므로
언로드가 필수).

**1단 — 재베이스라인(스캐폴딩 없음, 구현 착수 전 수행)**: tasks-large 3배치 —
gemma-4-e4b@8K, ornith-1.0-9b@8K, ornith-1.0-9b@32K. M8 대비 차이 = 리워드
효과(누출 제거). 결과는 baselines.md에 "M9 재베이스라인" 절로 기록하고, 이것이
이후 비교의 기준선이 된다.

**2단 — 스캐폴딩 후**: 같은 3배치 + 회귀 확인용 tasks/ 스포트 1배치(ornith@8K,
12과제 × 3반복). 1단 대비 차이 = 스캐폴딩 효과. **스포트 배치만은 v2 기준선
측정 조건을 따른다**(`command_timeout_secs` 기본 60s, 로드 ctx 8192 — v2
94.4%가 그 조건에서 측정됐으므로, 공통 조건(240s·로드 12288)을 쓰면 회귀
게이트의 기준선과 조건이 어긋난다).

**행동 지표 추출**: 코드 계측 없음 — 교정 주입은 tool_result로 트랜스크립트에
남으므로 `run-*.jsonl`에서 jq/python으로 추출한다. 지표: ① 런별 S/R 오류 횟수와
"오류 후 2회 이내 회복" 여부 ② SR_CORRECTION 주입 후 다음 뮤테이션 성공까지 턴
수 ③ finish 인자누락 연속 길이 분포 ④ 마지막 검증 성공 → finish 시도 간 턴 간격
⑤ 반복정지 원인 분류(S/R발/finish발/기타) ⑥ 거짓 성공 finish 카운트(report.json
기존 필드). 추출 레시피는 baselines.md에 기록한다. **report.json 스키마 변경
없음.**

## 6. 테스트

- `agent/repetition.rs` 단위: S/R 2연속 발동·1회 래치·비연속 리셋·S/R 스트릭의
  일반 교정 배제·다른 오류 스트릭의 일반 교정 불변
- `agent/mod.rs` Scripted 루프: ① finish({}) 2연속 → FINISH_ARGS_CORRECTION
  1회 주입 후 유효 finish로 회복 ② 뮤테이션→검증 성공(exit 0)→반복 호출을
  포함한 비뮤테이션 4연속 → FINISH_NUDGE 주입 ③ 검증 성공 후 edit_file 시도가
  카운터·무장을 리셋(이후 검증 성공으로 재무장) ④ 종료코드 비0·타임아웃 검증은
  무장하지 않음 ⑤ 반복 호출이 없는 신규 탐색 4연속은 발동하지 않음(반복-호출
  조건) ⑥ 무효 finish 시도가 4-2 카운터를 리셋
- `tools/edit_file.rs`: 강화된 오류문의 첫 문장 불변성(스트릭 키 안정성)

## 7. 리스크와 백로그

**리스크**:
- 교정 문구가 루프를 못 끊을 위험(같은 계열 개입이 실패한 실측 전례) — 행동
  지표로 효과를 판정하고, 무효로 판명되면 M10에서 다른 개입(예: S/R 3회째에
  해당 edit_file 호출을 write_file 재작성 안내로 강제 전환)을 검토
- FINISH_NUDGE 오탐(미완성 상태에서 조기 finish 유도) — 조건부 문구 + 런당 1회
  래치 + 뮤테이션 시도 리셋 + 반복-호출 발동 조건으로 완화, 거짓 성공 finish
  카운트로 감시. 잔여 미탐 여지: 재검증이 매번 미묘하게 다른 명령이면 반복-호출
  신호가 잡히지 않는다(관측 데이터에서는 전부 문자 그대로의 반복이었음) —
  행동 지표 ④·⑥으로 감시하고 발생 시 M10에서 신호를 재설계
- 재베이스라인 수치가 M8과 크게 다를 수 있음(리워드 효과) — 그 경우에도 2단
  비교는 1단 기준이므로 판정에는 영향 없음, 서사만 조정

**백로그 (M9에서 하지 않음)**:
- **grep 내성 항해 과제 세트(M10 후보)** — find-definition이 9/9 런 T1 grep
  일격에 뚫린 것은 현 과제 세트가 항해 난이도를 못 재고 있다는 뜻. "정확한
  식별자를 모르는 상태에서 시작하는" 과제가 실패 데이터를 만들 때에만 repo-map
  계열을 승격한다(M8 §8 "실패 데이터가 정한다" 원칙 유지)
- gemma 진단 회피·과신 축(시도 전 단계의 개입 — 프롬프트/과제 제시 설계)
- 환각 교정(read_file 유도형 전략 교정) — P2, 재현 데이터 축적 대기
- 오버플로 내성 4건, CARGO_HOME 격리·cargo 해석 고정, 트랙 B 품질, 16K 배포
  프로파일 — M7/M8 백로그 그대로
- run_command 뮤테이션 분류(VERIFY_NUDGE 사각지대 해소)
- 턴 예산 힌트 — 엄격 지표와의 상충 우려가 해소되는 설계가 나올 때까지 보류
- `docs/m9-candidates.md`의 컨텍스트 관리 2안(서브에이전트 컨텍스트 임대,
  도구 목록 온디맨드 로딩) — 채택 신호("항해 중 컨텍스트 소진이 지배적 실패
  유형")가 이번 배치에서 발생하지 않아(오버플로 중단 0건) 백로그 유지
