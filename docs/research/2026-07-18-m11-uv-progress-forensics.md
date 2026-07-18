# M11 0단계 — uv 진행 상태 실패 법의학

목적: `docs/superpowers/specs/2026-07-18-m11-progress-state-grounding-design.md`
(커밋 `1ddcb69`) §3이 요구하는 "측정 비용 0" 법의학. 브레인스토밍 중 수행한
미니 판독(§1 반전 2)의 3덩어리 분류(조기 finish / 탐색 루프 / 편집기 턴
소진 / 성공)를 공식 스크립트로 재도출해 확인하고, §2 기준 2의 대조 실측
수치(①=3/10, ②=3/5, 풍선 가드=2런)를 확정하거나 정정한다. 전제(3덩어리
분류)가 뒤집히면 스펙 개정으로 회귀해야 하므로(§3), 그 판정도 함께
기록한다.

## 0. 데이터·좌표계

로컬(git-ignored) `.loco/eval/` 배치 4종을 쓰되 역할상 3종으로 나뉜다:

1. **8K 초기 반복 배치 2종** — `20260717T125544Z`·`20260717T140556Z`. §3의
   ⓓ(파이프 위장 전수)에만 사용, ⓐ·ⓑ 분류 대상 아님(이후 배치로 대체됨).
2. **8K 최종/승자 배치** — `20260717T152633Z`(M10 승자 암③ = 현 main,
   uv+fm 각 10런). §1 표의 원본이자 ⓐ·ⓑ의 핵심 분류 대상.
3. **32K 검증 배치** — `20260717T164905Z`(uv+fm 각 10런, `context_tokens:
   32768`, 그 외 effective_config는 152633Z와 동일 — `temperature: 0.1`,
   `max_turns: 25`, `command_timeout_secs: 240`). ⓑ의 컨텍스트-크기 일반화
   확인 대상.

모델은 4개 배치 전부 `ornith-1.0-9b`, `--repeats 10 --seed 0`. **판정
기준(승패)은 각 태스크 픽스처의 `check = cargo test`가 유일한 리워드
신호이며(LLM judge 없음), 아래 표의 `outcome`/`passed`는 `report.json`을
그대로 인용한다** — 리워드 픽스처(태스크 정의) 자체의 변경은 이 노트에서
없다.

**좌표계 정의(핀)**: 이 노트와 `exp_metrics`의 "턴"은 **tool_result 이벤트
1-기준 순번**이다 — 에이전트 `turns` 카운터(length 턴 등 포함)와 다른
좌표계이므로 스펙 §1 표의 턴 수와 직접 비교하지 않는다.

## 1. 방법

브리프(`.superpowers/sdd/task-1-brief.md`) Step 1의 스크립트를 스크래치패드
(`/private/tmp/.../scratchpad/classify.py`, 리포에 넣지 않음)에 그대로
전사해 실행했다. `run-*.jsonl`의 `kind=="tool_result"` 이벤트만 훑어
`edit_file`/`write_file` 성공 디스패치(`mut_ok`), `cargo`가 포함된
`run_command`의 파이프 여부(`cargo_bare`/`cargo_piped`), 도구 시퀀스
(`seq`)를 뽑고 `report.json`의 `outcome`/`passed`와 병합한다.

## 2. ⓐ 152633Z uv 10런 분류표 — 스펙 §1 표 재검증

Step 2 실행: `python3 classify.py .loco/eval/20260717T152633Z
.loco/eval/20260717T164905Z`

원본 스크립트 출력(uv 부분만 발췌, tab 구분):

```
run	outcome	passed	turns	mut_ok	cargo_bare	cargo_piped
run-update-vat-rate-0	finished	False	4	0	0	0
run-update-vat-rate-1	max_turns	False	25	0	0	0
run-update-vat-rate-2	max_turns	False	24	0	0	0
run-update-vat-rate-3	max_turns	False	25	4	0	2
run-update-vat-rate-4	max_turns	False	25	6	0	1
run-update-vat-rate-5	finished	False	1	0	0	0
run-update-vat-rate-6	max_turns	False	25	5	0	0
run-update-vat-rate-7	max_turns	False	25	0	0	0
run-update-vat-rate-8	max_turns	False	25	5	0	0
run-update-vat-rate-9	finished	True	15	4	2	0
```

해석 표(귀속 = 성공 뮤테이션 기준, 스펙 §3 회귀 조건):

| 런 | outcome | passed | 턴(이벤트) | mut_ok | cargo(bare/piped) | 귀속 | 비고 |
|---|---|---|---|---|---|---|---|
| 0 | finished | False | 4 | 0 | 0/0 | 조기 finish | `gr rd rd ls` — 4턴 만에 finish, 뮤테이션 0회 |
| 5 | finished | False | 1 | 0 | 0/0 | 조기 finish | `gr` 단 1턴 만에 finish — 가장 극단적 사례 |
| 1 | max_turns | False | 25 | 0 | 0/0 | 탐색 루프 | read/grep/list_files만 25턴, 편집 시도 0회 |
| 2 | max_turns | False | 24 | 0 | 0/0 | 탐색 루프 | grep 1회 후 read 23연속, 편집 시도 0회 |
| 7 | max_turns | False | 25 | 0 | 0/0 | 탐색 루프 | `edit! edit!` 실패 2회(위치 9,10) 이후 read로 회귀 — 스펙 서술과 일치 |
| 3 | max_turns | False | 25 | 4 | 0/2 | 편집기 턴 소진 | `cargo test 2>&1 \| tail -50`(파이프, ⓓ의 run-3형) 2회 — exit code가 tail의 0으로 위장 |
| 4 | max_turns | False | 25 | 6 | 0/1 | 편집기 턴 소진 | `cargo check -p inv-report \| tail -30` 1회(파이프) |
| 6 | max_turns | False | 25 | 5 | 0/0 | 편집기 턴 소진 | cargo 명령 **전무**(비-cargo `run_command` 1회만) — 검증 시도 자체 없음 |
| 8 | max_turns | False | 25 | 5 | 0/0 | 편집기 턴 소진 | cargo 명령 전무 |
| 9 | finished | True | 15 | 4 | 2/0 | 성공 | `TEST`(bare) 2회, exit code 0 실측 후 finish |

**결과: 스펙 §1 표의 4덩어리 귀속(조기 finish 0·5 / 탐색 루프 1·2·7 /
편집기 턴 소진 3·4·6·8 / 성공 9)이 공식 스크립트 재도출과 정확히 일치했다
— 편차 없음.** `report.json`의 `update-vat-rate.false_finish_count == 2`
(런 0·5)로 교차검증도 일치.

## 3. ⓑ fm 10런 + 32K 20런 동일 분류 — 일반화 여부

### 3-1. 152633Z fm 10런

전 런(10/10)이 `mut_ok >= 1`, `passed == True` — **조기 finish·탐색
루프에 해당하는 런이 하나도 없다.** 8런은 `finished`, 2런(6·8)은
`max_turns`이지만 둘 다 뮤테이션이 이미 성공한 상태다.

런 6·8을 직접 정독하면(트랜스크립트 원문): 두 런 모두 `cargo test
--package inv-report 2>&1 | tail -30` 결과가 실제로 `test ... ok`
2건과 `exit code: 0`을 **왜곡 없이** 보여준다(tail 30줄 안에 요약이
전부 들어가 파이프 위장이 발동하지 않은 경우) — 그런데도 모델은 finish를
호출하지 않고 이미 읽은 파일(`monthly.rs`/`totals.rs`/`report.rs` 등)을
반복해서 다시 읽다가 `max_turns`에 도달한다. 이는 §1의 "편집기 턴
소진"(전파 미완으로 grep을 새로 시도)과도, "탐색 루프"(뮤테이션 0회)와도
다른 **네 번째 패턴 — 검증 후 미종결 배회**다. uv에는 나타나지 않고(uv는
9번 런만 성공했고 그 즉시 finish했다), fm에서만 2/10로 관측됐다.

### 3-2. 164905Z(32K) uv 10런

```
run	outcome	passed	turns	mut_ok	cargo_bare	cargo_piped
run-update-vat-rate-0	max_turns	False	25	4	0	0
run-update-vat-rate-1	max_turns	False	25	2	1	0
run-update-vat-rate-2	finished	True	13	5	1	0
run-update-vat-rate-3	max_turns	False	24	6	1	0
run-update-vat-rate-4	max_turns	False	25	8	0	0
run-update-vat-rate-5	timeout	True	23	5	0	1
run-update-vat-rate-6	max_turns	False	25	6	0	0
run-update-vat-rate-7	max_turns	False	25	8	2	0
run-update-vat-rate-8	timeout	False	16	3	0	0
run-update-vat-rate-9	finished	True	17	4	0	2
```

10/10 런이 `mut_ok >= 1` — **조기 finish·탐색 루프 런이 32K에서는
하나도 없다**(8K에서는 5/10). "편집기 턴 소진"은 6런(0·1·3·4·6·7)으로
강하게 재현된다. 성공은 2런(2·9). 그리고 8K에는 없던 **`timeout`
outcome이 2런(5·8) 등장** — `command_timeout_secs`는 8K·32K 동일(240초)
이므로 config 차이가 아니라 32K 컨텍스트에서의 실제 장시간 실행(런-9의
`cargo test --workspace`류 전체 워크스페이스 빌드+테스트 등)이 벽시계
타임아웃에 걸린 것으로 보인다 — outcome 4종째로, §1의 원 분류표(finished/
max_turns 2종만 포함)가 다루지 않는 범주다. 런 5는 `passed=True`(사실상
성공에 준하나 outcome이 다름), 런 8은 `passed=False`.

### 3-3. 164905Z(32K) fm 10런

10/10 `finished`, `passed=True`, 전 런 `mut_ok >= 1` — 3-1에서 관측한
"검증 후 미종결 배회"조차 32K에서는 사라졌다(fm은 8K·32K 모두 실패 사례가
사실상 없는 쉬운 과제).

### 3-4. 일반화 판정

3덩어리(조기 finish / 탐색 루프 / 편집기 턴 소진) 중 **"편집기 턴
소진"만 과제·컨텍스트를 넘어 강하게 일반화된다**(152633Z uv 4런 →
164905Z uv 6런). "조기 finish"·"탐색 루프"(뮤테이션 0회 계열)는
**152633Z uv에만 집중돼 있고**, 같은 배치의 fm(0/10)에도 32K uv(0/10)
에도 나타나지 않는다 — 이 두 덩어리는 "8K + 다지점 전파형 과제"라는
좁은 조건의 산물일 가능성이 높다. 반대로 fm은 uv 분류표에 없는
"검증 후 미종결 배회"(8K 한정, 2/10)를, 32K uv는 "timeout"이라는 4번째
outcome(2/10)을 각각 새로 보여준다.

**이 결과는 §1의 152633Z uv 분류 자체를 뒤집지 않는다**(2절의 재검증이
정확히 일치) — ⓑ는 스펙이 명시한 대로 "상태선 내용·발동 조건의 최종
확정 근거"를 위한 탐색이며, 그 결과 §4의 발동 조건(뮤테이션 0회 +
turns 5/10/15/20 → "files edited: none yet")이 겨냥하는 덩어리가
**8K·다지점 전파 과제에 편중돼 있어, 32K나 단일지점 과제(fm)에서는
발동 대상 런 자체가 드물 수 있다**는 점을 Task 8 사전등록의 기대 효과
크기 보정 입력으로 기록해 둔다.

## 4. ⓒ 픽스처 표기 산포 — "재검색 푸터 무산" 판정

`cd tasks-large/update-vat-rate && diff fixture/<f> solution/<f>` (4지점):

```
inv-parse/src/defaults.rs:
< pub const DEFAULT_VAT_PERCENT: u32 = 10;
> pub const DEFAULT_VAT_PERCENT: u32 = 12;

inv-report/src/forecast.rs:
< pub fn forecast_projection(net_krw: i64) -> i64 { (net_krw as f64 * 1.10) as i64 }
> pub fn forecast_projection(net_krw: i64) -> i64 { (net_krw as f64 * 1.12) as i64 }

inv-report/src/invoice.rs:
< pub fn invoice_total(subtotal_krw: i64) -> i64 { subtotal_krw * 110 / 100 }
> pub fn invoice_total(subtotal_krw: i64) -> i64 { subtotal_krw * 112 / 100 }

inv-core/src/rules/pricing.rs:
< pub fn apply_tax(amount_krw: i64) -> i64 { amount_krw + amount_krw * 10 / 100 }
> pub fn apply_tax(amount_krw: i64) -> i64 { amount_krw + amount_krw * 12 / 100 }
```

4지점 표기가 정확히 `10` / `1.10` / `110 / 100` / `10 / 100`으로
산포한다(스펙 §1 반전 1의 서술과 일치). 4개 옛 텍스트(치환 전 `search`
줄) 중 문자 그대로 겹치는 것이 없다 — `defaults.rs`의 `10`은 상수 선언문
전체 줄, `pricing.rs`의 `10 / 100`은 표현식 일부지만 두 줄 자체가 서로
다른 syntax(`u32 = 10;` vs `* 10 / 100`)라 "옛 텍스트 전체 줄"을 검색어로
쓰는 재검색 푸터는 애초에 매치 대상이 되지 않는다. **판정 확정**: 브레인
스토밍 초안의 "편집 성공 시 옛 텍스트 전역 재검색" 메커니즘은 이 픽스처
에서 발동 자체가 불가능하다 — §1 반전 1 그대로, 후속 마일스톤이 같은
안을 재발명하지 않도록 여기 기록해 둔다.

## 5. ⓓ 파이프 위장 전수

브리프 Step 3의 스캔(4개 배치, `cargo`가 포함된 `run_command`
tool_result 중 `cargo` 이후 부분에 따옴표 밖 `|`가 있는 경우 — naive
`|` 포함 판정)을 그대로 실행:

| 배치 | 이벤트 수 | 관련 런(파일명) |
|---|---|---|
| 125544Z | 11 | fm-2, fm-5, fm-6(×4), fm-7(×2), fm-8(×2) |
| 140556Z | 9 | fm-0(×2), fm-5(×2), fm-6, fm-8(×4) |
| 152633Z | 8 | fm-4, fm-5, fm-6(×2), fm-8, **uv-3(×2)**, **uv-4** |
| 164905Z | 8 | fm-3(×2), fm-5, fm-6, fm-8, **uv-5**, **uv-9(×2)** |
| **합계** | **36** | **21개 고유 런**(uv 4런·fm 17런) |

브리프가 명시한 기대 사례 — **152633Z uv run-3의 `cargo test 2>&1 |
tail -50` 포함, 2회** — 정확히 확인됐다. uv에 국한하면 4런·6이벤트
(152633Z run-3×2·run-4×1, 164905Z run-5×1·run-9×2)이고, 나머지 30건은
전부 fm — "run-3형 파이프 위장"은 uv보다 오히려 **fm 쪽에서 훨씬 흔하다**
(`cargo test --package inv-report 2>&1 | tail -30`이 fm의 관용구에
가깝다). 다만 fm의 경우 대부분 tail 30줄 안에 전체 테스트 요약이 들어가
실제로는 위장이 발동하지 않는다(3절 참고) — **위장이 실제로 판정을
왜곡한 사례는 uv run-3(exit code 0으로 위장, 실제 상태는 미상 —
런 자체는 mut_ok=4로 편집기 턴 소진 귀속, 최종 실패)에 한정된다.**

인용된 36건 전부를 눈으로 확인한 결과 **따옴표 안 `\|`(grep 패턴 등)로
인한 오탐은 0건**이었다(전 36건이 `2>&1 | tail`/`2>&1 | head` 형태의
실제 파이프). 스캔 자체가 이번 표본에서는 오탐 없이 정확했다.

## 6. 사전등록 입력 — §2 기준 2 수치 확정

스펙 §2 기준 2가 서술한 대조 실측(152633Z uv, cargo 기준) 3개 수치를
2절의 재도출 결과로 검산한다.

- **① 탐색 루프 런 비율** = 성공 뮤테이션 0회 ∧ `max_turns` 런 수 / 10
  = 런 1·2·7 = **3/10** — 스펙 서술(3/10)과 **정확히 일치, 정정 없음**.
- **② 검증 실행률** = (뮤테이션 런 중 cargo 기준 검증 실행 런) /
  (뮤테이션 런 전체). 뮤테이션 런(`mut_ok >= 1`) = 3·4·6·8·9(5런). 이 중
  cargo 명령(`cargo test`/`cargo check`류, bare 또는 piped 불문 —
  "run_command ≥1회"가 지표 정의)이 **한 번이라도 실행된** 런은 3(파이프
  ×2)·4(파이프 ×1)·9(bare ×2) = **3런**, 6·8은 cargo 명령이 아예 없음
  (run-6·8 모두 `cargo_bare=0, cargo_piped=0`). → **3/5** — 스펙
  서술(3/5, 런 3·4·9)과 **정확히 일치, 정정 없음**. run-6이 "grep을
  run_command로 돌려 VERIFY_NUDGE 기준으로는 검증됐다고 오집계될 뻔한
  런"이라는 스펙의 각주도 실측 확인(run-6 seq에 `cargo` 없는 `cmd` 토큰
  1개 존재 — 비-cargo run_command).
- **풍선 가드(뮤테이션 0회 `finished`)** = 런 0·5 = **2런** —
  `report.json`의 `false_finish_count == 2`와도 일치. 스펙 서술(2런)과
  **정확히 일치, 정정 없음**.

**세 수치 모두 정정 없이 확정**: ①=3/10, ②=3/5, 풍선 가드=2런.

## 7. 전제 반전 여부 판정

2절의 재검증이 스펙 §1 표(152633Z uv 10런의 4덩어리 귀속)와 **완전히
일치**했다 — 탐색 루프로 분류된 런(1·2·7) 중 어느 것도 성공 뮤테이션을
포함하지 않았고(런 7의 실패 `edit` 시도 2회는 §3 회귀 조건이 명시한
"실패 시도는 분류 불변" 예외에 해당), 편집기 턴 소진 런(3·4·6·8) 전부
`mut_ok >= 1`이면서 미완주(전부 `passed=False`)였다. §2 기준 2의 세
수치도 정정 없이 확정됐다(6절).

3절(ⓑ)의 일반화 조사는 "조기 finish"·"탐색 루프" 두 덩어리가
152633Z uv(8K·다지점 전파 과제)에 편중돼 있고 fm이나 32K uv로는 잘
전이되지 않는다는 것을 보였으나, 이는 §3의 반전 조건("탐색 루프 런에
성공 뮤테이션이 있었다")에 해당하지 않는다 — 152633Z uv 자체의 분류는
한 런도 흔들리지 않았다. 새로 관측된 "검증 후 미종결 배회"(fm, 8K)와
"timeout" outcome(uv, 32K)은 §1 분류표의 범위 밖에 있는 **추가** 관측이지
기존 분류의 **반박**이 아니다.

**결론: 전제는 뒤집히지 않았다 — 스펙 개정·재리뷰 불필요. M11 플랜을
그대로 진행한다.** 3절의 일반화 범위 관측(조기 finish/탐색 루프의
8K·다지점 전파 편중, 32K에서의 소멸)은 Task 8 사전등록의 기대 효과
크기 보정 입력으로 남긴다.
