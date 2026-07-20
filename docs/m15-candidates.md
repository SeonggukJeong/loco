# M15 입력 후보 — 기성 에이전트 CLI 탐색 산출

**작성**: 2026-07-20 · **근거**: codex 0.144.1 / grok-build 0.2.106 대상 탐색 프로브
**상태**: M14 구현 중에 시작한 별도 탐색(M14 범위 밖). 결론 미도달 — §6에 중단 지점 기록.

## 0. 이 문서가 답한 질문 — 그리고 답하지 못한 질문

두 개의 다른 질문이 있었고, 이 문서는 **첫째만 답했다.**

**질문 A(답함)**: "codex/grok-build을 **가져다 써서**(설정·플래그·추출) 폐쇄망
소형모델용으로 쓸 수 있나?" → **아니오.** 근거는 §1·§2·§3.

**질문 B(미답)**: "grok-build을 **클론해서 코드를 고쳐** 우리 것으로 만들 수 있나?"
→ **모른다.** §5의 타당성 프로브는 6회 편집에서 중단됐고, 진짜 시험(크레이트 삭제
경로)은 시도하지 않았다. **A의 근거를 B의 근거로 전용하지 말 것** — 아래 §4-D·§5에
어떤 숫자가 어느 질문에만 유효한지 명시한다.

**질문 A에 대한 답: 아니오. 단 근거는 "기성품이 못 한다"가 아니다.**

이 구분이 이 문서에서 가장 중요하다. **32K에서 grok-build은 던진 과제를 전부 풀었다**
— 소형 과제(`fix-off-by-one`)와, M13이 지목한 경로 미지정 대형 레포 축
(`find-definition-large`, 83파일·5크레이트) **양쪽 다**. 같은 과제에 loco를 돌린
n=1 대응쌍도 통과다. **탐색 범위에서 loco의 능력 우위는 관측되지 않았다.**

거절 근거는 능력이 아니라 셋이다:
1. codex는 M13 스택(llama.cpp)에 **구조적으로 못 붙는다** (§1-1, 확정)
2. grok의 프롬프트 10,209토큰은 **grok 자신의 스위치로 줄지 않는다** (§2, 실측) —
   8K 운용이 불가능해지고, 32K에서도 예산의 31%를 시작 전에 쓴다
3. 포크 유지비 (§1-3, 확정)

여기에 문서화되지 않은 넷째가 있다: **측정 장치**(`eval`·`--verify`·`exp_metrics.py`·
사전등록 프로토콜·14마일스톤 베이스라인)는 전부 loco의 트랜스크립트 형식에 묶여 있다.
이식은 그것을 통째로 버린다. M13이 배운 것이 "측정이 약한 고리였다"임을 생각하면
가장 잃으면 안 되는 것을 잃는 거래다.

**⚠ 증거 강도 상한.** 아래 전부 **과제당 런 1회**의 탐색이다. 사전등록 없음, 반복 없음,
대조군 없음. **비율로 환산하거나 통과율로 인용하지 말 것.** 확정 사실(문서·소스·에러
메시지)과 단일 관측을 아래에서 라벨로 구분한다.

---

## 1. 확정 사실 — 문서·소스·에러 메시지로 반증 가능

### 1-1. codex는 llama.cpp에 붙지 않는다

`wire_api`의 유일한 합법값이 `"responses"`이고 `"chat"`은 **하드 에러**다
(2025-12-09 deprecate, 2026-02 제거, [discussion #7782]). llama.cpp는 Responses API를
구현하지 않았다 — 추적 이슈 [#19138]·[#14702] 둘 다 열린 상태.

**따라서 M13에서 표준으로 고정한 스택(`scripts/serve.sh`)이 통째로 못 쓰인다.**
LM Studio는 Responses API를 구현하므로 그 경유로만 가능하다.

### 1-2. codex는 릴리스 빌드에서 기본으로 메트릭을 외부 발신한다

`OtelConfig::default()`의 `metrics_exporter: OtelExporterKind::Statsig` →
`https://ab.chatgpt.com/otlp/v1/metrics` (키 임베드). `cfg!(debug_assertions)`에서만
억제되므로 **릴리스 바이너리는 기본 ON**이다. `[otel] metrics_exporter = "none"`으로
차단 가능하고, 폐쇄망에서는 조용히 실패한다(차단이 아니라 무해한 실패).

→ 폐쇄망 감사 항목이지 결격 사유는 아니다. **다만 기본값이 ON이라는 점이 요점이다.**

### 1-3. 포크 유지비는 현실적이지 않다

codex: 124 크레이트 · 2,642개 `.rs` · 테스트 제외 약 766,000 LOC · Apache-2.0 ·
릴리스 640회 이상. loco 대비 3자릿수 규모 차이. 상류 속도 하루 1릴리스.

### 1-4. grok-build은 codex보다 훨씬 나은 후보였다

| | codex | grok-build |
|---|---|---|
| 라이선스 | Apache-2.0 | Apache-2.0 |
| chat/completions | 제거됨 | **기본값** |
| 로컬 llama.cpp | 불가 | 문서에 예제 (`base_url = "http://localhost:8080/v1"`) |
| 텔레메트리 차단 | 가능(기본 ON) | `[features] telemetry = false` 문서화 |
| 로그인 | 커스텀 provider면 불필요 | `models_base_url` 경로에서 불필요 |

바이너리 임베드 문서 원문: *"When `models_base_url` is set, Grok uses API key auth
instead of session auth. `grok login` is not required."*

또한 grok-build 트리에는 codex·opencode의 툴 구현이 **소스 포팅**되어 있다
(THIRD_PARTY_NOTICES). codex를 포크해서 얻을 것의 상당 부분이 이미 안에 있다.

---

## 2. 실측 — 컨텍스트 예산

계측기: llama.cpp 8192 ctx 서버의 `exceed_context_size_error`가 **요청 토큰 수를
정확히 보고**한다. 아래는 `fix-off-by-one` 픽스처(3줄) + 한 문장 과제의 턴 0 크기.

| 조치 | 턴 0 | 델타 |
|---|---|---|
| grok 기본 | **10,209** | — |
| `--no-plan --no-subagents --disable-web-search --no-memory` | 10,209 | **0** |
| + `--tools` 화이트리스트(loco 6종 상당) | 8,322 | −1,887 |
| + `--system-prompt-override`(짧게) | 8192 미만 통과 | 나머지 |
| loco (참고) | **약 800~900** ⚠추정 | — |

⚠ loco 값은 프롬프트 템플릿 2.2KB + 툴 문서 + 트리를 **4자/토큰으로 근사한 추정치**다.
실측이 아니다. 정확한 비교가 필요하면 재야 한다(→ §4 후보 C).

**해석 — 이것이 질문 A의 핵심 근거다.** grok이 제공하는 축소 스위치는 프롬프트를
**1토큰도 줄이지 못한다.** 줄어든 것은 전부 툴을 빼고 시스템 프롬프트를 버려서
나왔다. 즉 **다이어트 성공 지점 = 가져다 쓰려던 것을 버린 지점**이다.
두 CLI 모두 새 모델의 `context_window` 기본값이 200,000이다 — 풍요를 전제로
프롬프트 엔지니어링이 되어 있다.

⚠ **질문 B(포크)에서는 이 논증이 뒤집힌다.** 포크에서 프롬프트·툴셋을 갈아끼우는 것은
비용이 아니라 **하려던 일 그 자체**다. 그리고 §3-3이 관련 신호를 준다 — 툴셋 교체 +
시스템 프롬프트 전면 교체 상태에서도 **grok의 에이전트 루프는 정상 완주했다.**
이 표를 포크 반대 근거로 인용하지 말 것.

---

## 3. 단일 관측 — 정황. 확정 아님

### 3-1. codex + gemma-4-e4b (LM Studio, 8K): 거짓 성공

완주했고(20,351 토큰) 이렇게 보고했다:

> "I have corrected the implementation in `src/lib.rs`… **This correction now passes
> all tests:** For n=5, the sum is now correctly 15…"

실제: `src/lib.rs` **미변경**, `cargo test` → `0 passed; 3 failed`.

**요약만 믿었으면 성공으로 기록됐다.** 컨트롤러가 `cargo test`를 직접 돌려서 잡았다.
loco가 M12 전체를 들여 대응한 실패 모드가 성숙한 에이전트에서 첫 과제에 나왔다.

⚠ **n=1이다.** "codex가 소형모델에서 실패한다"로 일반화하면 M13이 저지른 오류의 반복이다.

### 3-2. codex + ornith-1.0-9b (LM Studio, 8K): 턴 0 사망

`400: System message must be at the beginning` — 모델 Jinja 템플릿의
`raise_exception`. codex의 메시지 배치가 소형모델 템플릿의 전제를 위반한다.
**모델 1종에서만 관측** — 템플릿 문제인지 codex 문제인지 분리하지 않았다.

### 3-3. grok 다이어트(8K): 출력 여유 고갈로 중단

`response truncated by max_tokens`, `modelCalls: 2`, 파일 미변경.
입력이 컨텍스트를 거의 다 먹어 모델이 턴 하나를 뱉을 자리가 없었던 것으로 보인다
(`outputTokens: 136`). grok은 이를 **치명적 에러로 보고 세션을 중단**한다 —
loco는 같은 상황(`finish_reason: length`)을 유계 루프로 처리하고 계속 간다.

⚠ **컨트롤러 교란 확정.** `max_completion_tokens=2048`·`temperature=0.0`은 컨트롤러가
넣은 값이고 8192 서버에서 최적이라고 확인한 바 없다. **가설이지 관측이 아니다.**

**→ 이 가설은 이후 기각됐다.** 동일 다이어트 구성을 32K에서 재실행하니 정상 완주
(`(1..=n)` 편집 → `cargo test` → 통과). 8K 실패는 **순수 컨텍스트 예산 문제**였고
"grok이 출력 절단에 취약하다"는 서술은 틀렸다. 이 절은 기각된 가설의 기록으로 남긴다.

### 3-4. grok 무개조 32K: 통과

`context_window=32768`, `max_completion_tokens=4096`, 축소 스위치 없음.
`(1..n)` → `(1..=n)` 편집 후 `cargo test` 실행, **3/3 통과**. loco 기준으로 엄격 통과에 해당.

⚠ **1차 시도는 무효였고 폐기했다.** 컨트롤러가 `HOME`을 가짜로 덮어써서 grok에게
cargo가 보이지 않았고("Rust/Cargo가 설정되어 있지 않아 실행할 수 없었습니다"),
검증 기회를 주지 않은 상태의 결과였다. `CARGO_HOME`/`RUSTUP_HOME`/`PATH`를 실제 값으로
주고 재실행한 것이 위 결과다.

**이 관측이 §0의 결론을 "기성품이 못 한다"로 쓰지 못하게 만든다.** 8K 실패는
컨텍스트 예산 문제였지 근본적 무능이 아니었다.

### 3-5. grok 무개조 32K, 경로 미지정 대형 레포: 통과 — 그리고 loco도 통과

**M13이 지목한 축을 직접 겨눈 관측이다.** `tasks-large/find-definition-large`
(83개 `.rs`, 5크레이트 워크스페이스, 심볼명만 주고 경로 미지정 — M13 2-1에서
실레포 0/7이었던 형태).

| | 결과 |
|---|---|
| grok 무개조 32K | `inv-core/src/rules/mod.rs` — **정답**(710행), 전 테스트 통과 |
| loco 32K (n=1, seed 0) | **통과**, 엄격 통과, 12턴, 85.8s |

정답 근거: `grep -rn "fn restock_threshold" fixture` → `inv-core/src/rules/mod.rs:710`이
유일한 정의(나머지 2건은 `inv-core/tests/core_basic.rs`의 테스트 함수명).

**해석에 주의.** 이것은 **동점이지 loco의 승리가 아니며, grok의 승리도 아니다.**
각 1런이고 시드도 조건도 완전히 정렬되지 않았다(grok은 자체 툴셋·자체 프롬프트,
loco는 eval 샌드박스 경유). **"grok이 M13의 실패 축을 푼다"로 일반화하지 말 것** —
이 픽스처는 합성이고, M13의 0/7은 실제 OSS 레포(2.8K~64.7K LOC)에서 나왔다.
M13 2-1이 이미 경고한 그대로다: **합성 세트는 이 축을 구별하지 못한다.**

---

## 4. M15 후보

### A. [설계] 과제별 툴 표면 축소

grok의 `--tools` 화이트리스트에 해당. loco는 항상 6종 전부를 노출한다.
§2에서 툴 정의 제거만으로 1,887토큰이 빠진 것은 **툴 표면이 프롬프트 예산의
실질 항목**임을 보여준다. 소형모델의 선택지 축소라는 별도 이득도 예상된다.

**착수점 (코드 확인 완료)**:
- `Registry::guided()` — `src/tools/mod.rs:134`. 6종 고정 구성.
- `Registry::docs()` — `src/tools/mod.rs:86`. 프롬프트에 들어가는 툴 문서 생성.
- `prompt::system_prompt(tool_docs, root)` — `src/agent/prompt.rs:11`. 소비 지점.
- **`protocol::response_format(tool_names)` — `src/agent/protocol.rs:159`.**
  json_schema의 `tool` enum이 **이미 툴 이름 목록으로 만들어진다.**

**→ 이것이 이 후보의 핵심이다.** 툴 집합을 줄이면 프롬프트 토큰과 **디코딩 제약이
함께** 좁아진다(스키마 enum이 자동으로 따라옴). 두 효과가 한 지점에서 나오므로
설계 단위가 하나다.

**아직 미정 — 설계 라운드 필요**: "어떤 과제에 어떤 부분집합인가"를 누가 정하는가.
모델이 고르게 할지, 과제 정의(`task.toml`)가 지정할지, 휴리스틱인지. 실사용에서는
과제 정의가 없으므로 **eval에서만 되는 설계는 답이 아니다.**

**반증 가능한 수용 기준 초안**: 축소 구성이 (1) 턴 0 토큰을 유의하게 줄이고
(2) `tasks/` 통과율을 떨어뜨리지 않는다. (2)는 사전등록 배치가 필요하다.

### B. [설계] 문법 제약을 **편집 페이로드까지** 확장

⚠ **초판의 전제가 틀렸다. loco는 이미 문법 제약을 하고 있다.**
`ChatRequest.response_format`(`src/llm/types.rs:29`)에 `json_schema`를 실어 보내고,
`protocol::response_format()`(`src/agent/protocol.rs:159`)가 그 스키마를 만든다.
주석이 명시하듯 **"의도적으로 얕은 스키마"**(스펙 §4)다 — 턴 봉투
(`thought`/`action.tool`)만 강제하고 `args` 내부는 자유다.

**codex와의 실제 차이는 여기다.** codex의 `apply_patch`는 Lark 문법을 쓰는
`ToolSpec::Freeform`으로 **패치 본문 자체를 문법으로 강제**한다. loco에서 대응되는
것은 `edit_file`의 `search`/`replace` 페이로드이고, **거기엔 제약이 없다.**

M9~M12가 S/R 실패에 쏟은 장치들(`SR_CORRECTION`, 온도 섭동, 동일 텍스트 거부,
0-매치/다중-매치 오류 메시지)은 전부 **사후 교정**이다. 문법 제약은 **사전 예방**이라
축이 다르다. llama.cpp는 GBNF를 지원한다.

**착수 전 확인 필요 (미검증)**:
- llama.cpp의 OpenAI 호환 엔드포인트가 `response_format`을 넘어 **GBNF grammar 필드**를
  받는지, 받는다면 `ChatRequest`에 필드 추가가 필요한지.
- `search`는 **기존 파일의 정확한 발췌**여야 한다 — 문법으로 표현 가능한 성질인지가
  불분명하다(문법은 형태를 강제하지 내용 일치를 강제하지 못한다). **이 후보의 가치가
  여기서 갈린다.** 형태만 강제해서 실제 S/R 실패가 줄어드는지는 근거가 없다.
- codex에서도 이 툴은 `model_info.apply_patch_tool_type`가 `Some`일 때만 등록되고
  기본값은 `None`이다 — **임의 로컬 모델에는 등록되지 않는다.** 산업 사례이지
  검증된 소형모델 해법이 아니다.

### C. [티켓] 토큰 회계 계측

grok은 매 턴 `promptUsage`로 `inputTokens`/`outputTokens`/`cachedReadTokens`/
`modelCalls`를 보고한다. **`scripts/exp_metrics.py`에는 토큰 회계가 전혀 없다.**

**이것이 계측 부채에 직접 걸린다**: M13의 `pack()` 버그(사용자 과제 메시지 삭제)는
세션 1의 **토큰 산술로만 사후 확정**됐다(예산 3686에 3751). 턴별 토큰이 트랜스크립트에
있었다면 그 버그는 사후 재구성이 아니라 관측이었다. §2의 loco 추정치(800~900)를
실측으로 바꾸는 것도 같은 작업이다.

**수용 기준**: 트랜스크립트에 턴별 입력/출력 토큰이 기록되고, `exp_metrics.py`가
런당 최대 입력 토큰과 예산 대비 비율을 집계한다. `pack()` 발동 턴이 식별 가능해진다.

---

### D. [티켓] `xai-ratatui-inline` 벤더링 검토 — TUI 조사의 유일한 산출

**"전체는 못 써도 TUI는?"에 대한 답: 추출은 못 한다. 단 잎 크레이트 4개는 깨끗하다.**

⚠ **이 절은 질문 A(추출)의 판정이다.** 아래 `aws-lc-sys` 논거는 **포크에서는 약해진다** —
§5-2가 보였듯 grok 자체 크레이트는 이미 `default-features = false, features = ["ring"]`로
올바르고, `aws-lc`는 전부 **외부 크레이트의 default 피처**에서 온다. 포크에서는 그것을
고칠 수 있다(다만 §5는 6회 편집으로 해결에 도달하지 못했다). "318개 프로토콜 임포트"·
"총 크레이트 870" 같은 수치도 **추출 비용이지 포크 비용이 아니다.**

**두 TUI 모두 loco의 하드 제약을 정면 위반한다** — `aws-lc-sys`가 normal 의존 그래프에 있다:
- codex: `aws-lc-sys 0.39.0 ← aws-lc-rs ← rcgen ← rama-tls-rustls ← codex-network-proxy ← codex-config ← … ← codex-tui`. `codex-config`는 TUI의 **직접** 의존이다.
- grok: `aws-lc-rs ← jsonwebtoken ← gcloud-auth ← gcloud-storage ← xai-file-utils ← … ← xai-grok-pager-render`. **TUI 렌더 계층 그래프에 GCS 클라이언트가 있다.**

grok 자신의 `Cargo.toml` 주석이 결정적이다: *"the workspace pin enables aws-lc-rs,
under which `rustls::crypto::ring` does not exist."* — **loco의 rustls+ring 핀과 직접 충돌한다.**
`aws-lc-sys`는 `links` + `[build-dependencies.cmake]` + Windows NASM 요구다(스펙 §하드제약 위반 3중).

규모도 이식 가능한 종류가 아니다:

| | LOC | 워크스페이스 내부 의존 | 총 크레이트 |
|---|---|---|---|
| `codex-tui` | 228,423 | 106 | **870** |
| `xai-grok-pager` | 431,374 | 71 | 844 |
| `xai-grok-pager-render` ("extracted") | 36,667 | 49 | 728 |

codex TUI는 `codex_app_server_protocol` 타입을 **318개 경로**에서 임포트한다 — 위젯이
에이전트 와이어 프로토콜에 직접 쓰여 있어 자를 수 있는 이음매가 아니다. 게다가 codex는
`ratatui`/`crossterm`을 **git 포크로 `[patch]`** 하고 있어(오프라인 Windows 빌드와 상충)
포크까지 벤더링해야 한다. 둘 다 crates.io 미배포이고 공개 위젯 API가 없다(바이너리 사설 모듈).

**그러나 잎 크레이트 4개는 검증상 깨끗하다**(aws-lc-sys·openssl·cmake·NASM·`cc` 없음):

| 크레이트 | LOC | 총 크레이트 | 비고 |
|---|---|---|---|
| **`xai-ratatui-inline`** | **3,713** | 53 | **인라인/스크롤백 렌더(`insert_before`)** |
| `xai-grok-markdown-core` | 1,070 | 8 | `pulldown-cmark` 만 |
| `xai-tty-utils` | 1,245 | 21 | |
| `xai-ratatui-textarea` | 14,618 | 62 | |

**`xai-ratatui-inline`이 loco에 맞는다.** 화면을 통째로 점유하지 않고 터미널 기본
스크롤백에 렌더하는 방식 — alt-screen 전면 TUI보다 **loco의 REPL 형태에 훨씬 가깝다.**
스톡 `ratatui`/`crossterm` 위 3,713줄이라 실제로 벤더링 가능하다.

**정직한 기준선**: `ratatui` + `crossterm`을 그냥 직접 추가하면 약 50 크레이트이고
지원·문서화·semver된 API를 얻는다. 그 위에 `xai-ratatui-inline`을 얹으면 +3.
**두 에이전트 TUI를 경유하는 어떤 경로든 700~870이다.**

⚠ **의존성 추가는 스펙 하드 제약이다** — `ratatui`/`crossterm`조차 사용자 승인이 먼저다.
그리고 **UX가 지금 병목이라는 근거가 없다**: M13 파일럿의 실패는 `pack()`의 과제 메시지
삭제, `reasoning_content` 미파싱, `read_file` 푸터 크롤 유도, 거짓 성공이었다 — **하나도
TUI가 아니다.** 이 항목은 "가능하다"의 기록이지 "다음에 할 것"이 아니다.

⚠ 미검증: 실제 Windows 오프라인 빌드는 시도하지 않았다(cmake/NASM 판정은 `aws-lc-sys`의
`Cargo.toml`·README 근거). 의존성 해석은 `aarch64-apple-darwin` 기본 피처 기준.

### E. [질문] 32K를 기본 운용점으로 삼을 것인가

M8에서 ornith가 32K일 때 대형 레포 88.9%(8K 55.6%)였고, §3-4에서 grok은 32K에서만
돌았다. loco의 기본값은 여전히 `context_tokens=8192`다.

**수선 후보가 아니라 범위 결정이다.** 8K를 고수하는 것이 제약 조건인지(폐쇄망 하드웨어
가정) 아니면 관성인지 명시된 적이 없다.

---

## 5. 포크 타당성 프로브 — 질문 B, **미결로 중단**

`git clone --depth 1 https://github.com/xai-org/grok-build.git`, 74MB, 2,272개 `.rs`,
워크스페이스 멤버 79개, 메인 바이너리 `xai-grok-pager-bin`(866 크레이트 그래프).
**레포 밖(스크래치패드)에서 수행했고 loco 트리는 건드리지 않았다.**

### 5-1. 목표 — loco 하드 제약과의 충돌 해소가 가능한가

스펙 하드 제약: OpenSSL 없음 · **aws-lc-sys 없음** · rustls+ring · cmake/NASM 없이
Windows 오프라인 빌드. 기준선: `aws-lc` 5건(`aws-lc-sys v0.39.1` 포함), `openssl` 0건.

### 5-2. 실측 — 6회 편집, 5 → 5 (변화 없음)

| # | 편집 | 결과 |
|---|---|---|
| 1 | `gcloud-storage`: `jwt-aws-lc-rs` → `jwt-ring` | **해석 실패** — 그런 피처 없음 |
| 2 | 같은 곳 → `jwt-rust-crypto` (정답) | 해석 성공, aws-lc **5** |
| 3 | `tonic`: `tls-aws-lc` → `tls-ring` | aws-lc 5 |
| 4 | 워크스페이스 `reqwest` → `rustls-tls-webpki-roots-no-provider` | aws-lc 5 |
| 5 | `xai-grok-tools`의 `rustls-tls` 2곳 동일 교체 | aws-lc 5 |
| 6 | `async-openai`·`oauth2` `default-features = false` | aws-lc 5 |

**기전**: `rustls` 0.23의 `default`가 `aws_lc_rs`를 포함하고, cargo 피처는 **가산적**이라
866 크레이트 중 **하나만 켜도 전역**이다. `rustls` 직접 소비자가 13개이고 `reqwest`는
0.12/0.13 두 메이저가 공존한다. enabler를 하나 막으면 다음 것이 드러난다.

**중요**: grok **자신의** 크레이트들은 이미 올바르다 —
`rustls = { default-features = false, features = ["ring", ...] }`. 문제는 전부
**외부 크레이트의 default 피처**다(`oauth2`는 `rustls-tls = ["reqwest/rustls-tls"]`로
하드와이어라 no-provider 변형 자체가 없다).

### 5-3. 판정 — "불가능"이 아니라 "세금이 얼마인지 아직 모름"

**구조적으로 막힌 지점은 하나도 관측되지 않았다.** 각 enabler는 원리상
`default-features = false` + 명시 피처로 처리 가능했다. 다만 6회로 수치가 안 움직였다.

### 5-4. 시도하지 않은 것 — **이것이 진짜 시험이었다**

프로브는 **피처 수술**을 했는데, 포크에서 할 일은 **크레이트 삭제**다. 폐쇄망에서
불필요한 것들을 워크스페이스에서 통째로 빼면 enabler가 낱개가 아니라 뭉치로 사라진다:

`xai-grok-voice`(마이크 캡처, `cpal`/`coreaudio-sys`) · `xai-grok-telemetry` ·
`xai-file-utils`(S3+GCS 업로드 — aws-smithy 사슬의 뿌리) · `xai-grok-update`(자동 업데이트) ·
`xai-grok-plugin-marketplace` · `xai-grok-announcements` · `xai-grok-mermaid`

**이 경로는 미검증이다.** §5-2의 "6회 편집으로 안 움직임"을 **포크 불가의 근거로
쓰지 말 것** — 잘못된 축을 민 결과다.

### 5-5. 그 밖에 확인 안 한 것

- **빌드를 한 번도 돌리지 않았다.** 피처 해석 성공 ≠ 컴파일 성공.
- 잔존 네이티브 의존: `libgit2-sys`·`libsqlite3-sys`·`zstd-sys`·`libz-sys`·
  `coreaudio-sys`·`tikv-jemalloc-sys`·`libmimalloc-sys`. Windows 오프라인 빌드 영향 미평가.
- 프롬프트 10,209토큰을 **소형모델용으로 다시 쓰는 비용**을 산정하지 않았다.
  (단 §3-3이 관련 신호를 준다: 툴셋 교체 + 시스템 프롬프트 전면 교체 상태에서도
  grok의 에이전트 루프는 정상 완주했다 — 프롬프트 교체 자체는 루프를 깨지 않는다.)

### 5-6. 포크의 가장 무거운 항목 — 의존성이 아니다

레포는 **일방향 monorepo 익스포트**다(`SOURCE_REV`, "Synced from monorepo").
**상류 기여 경로가 없고 리베이스가 성립하지 않는다.** §5-2의 수술이 얼마가 되든
**동기화마다 손으로 다시 해야 한다.** 상류는 하루 1릴리스급이다.

뒤집어 읽을 수도 있다: 어차피 혼자 유지한다면 이것은 **"성숙한 코드베이스를 스냅샷으로
상속"**이고, loco를 13마일스톤 들여 만든 것과 비교하면 나쁜 거래가 아닐 수 있다.
**이 판단은 이 문서가 내리지 않는다.**

### 5-7. 컨트롤러 방법론 실패 (승계할 것)

`cargo tree ... 2>/dev/null | grep -c aws-lc` 로 "aws-lc 0건 — 스왑 성공"을 보고했으나
**실제로는 `cargo tree`가 에러로 죽어 있었고 `2>/dev/null`이 그것을 감췄다.**
0은 "없음"이 아니라 "출력 없음"이었다. 프로젝트 메모리
"검증 환경이 검증을 무효화한다"의 정확한 재현이다.

**교정**: 이후 stderr를 표시하고, **총 고유 크레이트 수를 sanity check로 병기**했다
(866이 나오면 명령이 살아 있다는 뜻). 계수 grep은 반드시 비-0 대조군과 함께 쓸 것.

---

## 6. 하지 않은 것 — 다음 분석이 상속할 공백

**⚠ 이 절 전체는 질문 A(추출·재사용)에만 유효하다.** 질문 B(포크)에서는 §5를 볼 것.

**메운 것**(초판의 공백 2건):
- ~~grok 다이어트를 32K에서 재시도하지 않았다~~ → §3-3에서 **기각 완료**
- ~~경로 미지정 축을 grok에 던지지 않았다~~ → §3-5에서 **양쪽 통과 관측**

**포크 타당성은 §5에 별도로 기록한다** (질문 B).

**남은 것**:
- **codex 턴 0 크기를 재지 않았다.** grok만 쟀다. codex도 LM Studio 경유로 잴 수 있다.
- **grok과 loco를 동일 과제 집합으로 비교하지 않았다.** `tasks/` 12과제 × 반복을
  양쪽에 돌리는 것은 **사전등록이 필요한 측정**이지 탐색이 아니다. §3-4·§3-5의 동점을
  "동등한 성능"으로 승격시키지 말 것 — n=1 두 쌍이다.
- **실레포에서 grok을 돌리지 않았다.** M13의 0/7은 실제 OSS 레포에서 나왔고,
  §3-5는 합성 픽스처다. `scripts/pilot.sh`가 이미 있으므로 grok을 같은 원장에
  기록하는 것은 기술적으로 가능하다 — 다만 그것도 측정이지 탐색이 아니다.
- **32K에서 grok의 실효 작업 예산을 재지 않았다.** 10,209/32,768 = 31%가 시작 전에
  나가지만, 그것이 실제 과제에서 병목이 되는지는 관측하지 않았다.

[discussion #7782]: https://github.com/openai/codex/discussions/7782
[#19138]: https://github.com/ggml-org/llama.cpp/issues/19138
[#14702]: https://github.com/ggml-org/llama.cpp/issues/14702
