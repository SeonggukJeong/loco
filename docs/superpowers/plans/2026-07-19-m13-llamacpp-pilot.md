# M13 llama.cpp 전환과 실사용 파일럿 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 측정·배포 스택을 LM Studio에서 llama.cpp로 옮기고, 그 위에서 실사용 파일럿을 돌려 북극성 판정을 합성 평가 통과율에서 실사용 신호로 전환한다.

**Architecture:** 세 갈래가 아니라 하나의 사슬이다 — 전환(T1~T4) → 앵커 확립(T5~T6) → 저비용 수선과 회귀 확인(T7~T8) → 파일럿 계측·수행·판정(T9~T12). 스택 전환과 코드 변경은 절대 한 배치에 섞지 않는다(귀속 불가). loco 프로덕션 코드 변경은 최소이며, 파일럿 계측은 전부 `scripts/` 밖 스크립트로 두어 "에이전트 코드 동결 = 비교가능성" 관례를 지킨다.

**Tech Stack:** Rust (edition 2024), `llama-server` (llama.cpp b9960+), Python 3 stdlib (지표 스크립트), POSIX sh (서버·파일럿 래퍼)

**스펙:** `docs/superpowers/specs/2026-07-19-m13-llamacpp-pilot-design.md` (`9466418`) — 리뷰 2R Ready=Yes, 사용자 승인 완료. **스펙이 유일한 진실이며, 이 플랜과 스펙이 충돌하면 스펙이 이긴다. 충돌을 발견하면 스스로 판단하지 말고 에스컬레이션할 것.**

## Global Constraints

이 절의 모든 항목은 **모든 태스크의 요구사항에 암묵적으로 포함된다.**

- **브랜치**: T1 시작 시 `main`(`9466418`)에서 `m13/llamacpp-pilot` 생성. T1~T12 전부 이 브랜치. `main` 병합은 T12 판정 후에만
- **Edition 2024.** 의존성 목록은 스펙이 고정한다 — **신규 크레이트 추가 금지**(사용자에게 먼저 물을 것)
- **Python 스크립트는 stdlib 전용** (`scripts/exp_metrics.py` 선례). pip 설치 금지
- **모델 대면 텍스트는 전부 영문**, 사용자 대면 CLI 메시지는 한국어, 식별자·`SYSTEM_PROMPT`는 영문
- **게이트 (모든 코드 태스크)**: `cargo test` 전건 통과 + `cargo clippy --all-targets -- -D warnings` 무경고. `--all-targets`가 중요하다(테스트 코드도 린트)
- **`tasks/`·`tasks-large/` 변경 시**: `cargo run -- eval tasks --verify`(12/12)와 `cargo run -- eval tasks-large --verify`(3/3). **이 마일스톤은 두 디렉토리를 건드리지 않는다** — 건드려야 할 것 같으면 에스컬레이션
- **상태선 마커 계약**: `"[status] "` 접두 + 9칸 연속 들여쓰기. `src/agent/status_note.rs`·`src/session.rs`·`scripts/exp_metrics.py` **3파일이 축자로 공유**한다. 한 곳을 바꾸면 셋 다 바꿔야 한다
- **`exp_metrics.py`의 Python↔Rust 손복사**(`MAX_SR_CORRECTIONS`·`BADARGS_KEY_PREFIX`·`normalize`)는 드리프트 자동 검출이 없다 — **`src/agent/repetition.rs` 수정 시 `exp_metrics.py` 수동 미러 필수**
- **커밋**: Conventional Commits (제목 한국어 허용), 태스크당 최소 1커밋
- **측정 태스크(T6·T8·T11)는 GPU 시간을 쓴다** — 아래 "측정 규율" 준수

### 측정 규율 (T6·T8·T11 필수)

`docs/experiments/PROTOCOL.md` 승계 + 스펙 §3-4:

1. **사전등록 없이 배치를 돌리지 않는다.** T5의 사전등록 문서에 대한 **사용자 승인 커밋**이 있어야 T6·T8이 돈다. 전언 승인은 승인이 아니다(M11·M12 전례)
2. 배치 전 스모크 7항(스펙 §3-4) 전건 통과 — 특히 **json_schema 요청 1건이 HTTP 200**
3. `.loco/config.toml`이 이번 배치 조건인지 확인. **현재 M12 배치 2의 `command_timeout_secs = 240`이 남아 있다**
4. `ls ${TMPDIR}/.cargo` — 존재하면 수동 제거(하네스 전체 중단 사유)
5. **측정 중 `cargo build`/`test` 병행 금지** (CPU 경합이 타이밍 민감 판정을 왜곡)
6. 데몬화: **macOS에 `setsid`가 없다.** `python3 -c "import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])" <cmd>...` 래퍼를 쓴다(하네스 백그라운드 60분 수명 상한)
7. 종료 확인은 **통지에 의존하지 말고** exit code와 스탬프 디렉토리로 직접 확인(M10 운영 교훈)

## File Structure

| 파일 | 책임 | 태스크 |
|---|---|---|
| `src/llm/client.rs` (수정) | 사용자 대면 안내 문구를 서버 불문으로 일반화 | T1 |
| `README.md` (수정) | llama-server 기동 경로를 1급으로 | T1 |
| `scripts/serve.sh` (신설) | llama-server 기동 조건을 핀 4개로 고정 — 배포 산출물 겸 실험 조건 기록 | T2 |
| `docs/experiments/PROTOCOL.md` (수정) | 배치 전 점검을 llama.cpp 기준으로 교체 | T2 |
| `src/agent/mod.rs` (수정) | `schema_fallback_fired()` 게터 노출 | T3 |
| `src/eval/report.rs` (수정) | `RunRecord.schema_fallback: bool` 추가 | T3 |
| `src/eval/mod.rs` (수정) | 게터→`RunRecord` 배선 | T3 |
| `scripts/exp_metrics.py` (수정) | `parse_fail_first` 컬럼 — C1 포착 기계 검사 | T4 |
| `docs/experiments/2026-07-19-llamacpp-anchor/pre-registration.md` (신설) | 앵커+회귀 게이트 사전등록 — **사용자 승인 게이트** | T5 |
| `src/agent/status_note.rs` (수정) | 케이던스 조밀화 + 무뮤테이션 분기 검증 줄 | T7 |
| `scripts/pilot.sh` (신설) | 실사용 세션 래퍼 — 원장 JSONL 한 행 append | T9 |
| `scripts/pilot_tally.py` (신설) | 줄 생존율 + 사전 선언 범주별 분류표 | T10 |
| `docs/baselines.md`·`CLAUDE.md`·`README.md` (수정) | 앵커·판정·파일럿 결과 기록 | T12 |

---

### Task 1: 사용자 대면 문구를 서버 불문으로 일반화

**Files:**
- Modify: `src/llm/client.rs:11-19` (Connect 오류), `src/llm/client.rs:184-188` (`resolve_model`), `src/llm/client.rs:297` (단언 테스트)
- Modify: `README.md` (서버 기동 절)

**Interfaces:**
- Consumes: 없음 (첫 태스크)
- Produces: 없음 (문자열만 변경 — 후속 태스크가 의존하는 시그니처 없음)

**배경:** `grep -rn "LM Studio" src/`는 정확히 이 3곳 + `src/agent/mod.rs`의 주석 1곳을 반환한다. 주석은 이미 llama.cpp를 포함하므로 건드리지 않는다.

- [ ] **Step 1: 브랜치 생성**

```bash
git checkout main && git pull --ff-only 2>/dev/null || true
git rev-parse HEAD   # 9466418 이어야 함
git checkout -b m13/llamacpp-pilot
```

- [ ] **Step 2: 실패하는 테스트로 바꾼다**

`src/llm/client.rs:297`의 단언을 서버 불문 문구로 바꾼다. 지금 코드:

```rust
        assert!(msg.contains("LM Studio"), "실행 가능한 안내 포함: {msg}");
```

이렇게 바꾼다:

```rust
        assert!(msg.contains("llama-server"), "실행 가능한 안내 포함: {msg}");
```

- [ ] **Step 3: 테스트가 실패하는지 확인**

Run: `cargo test --lib llm::client::tests::chat_connect_error 2>&1 | tail -20`

정확한 테스트명을 모르면 먼저 확인:

```bash
cargo test --lib llm::client 2>&1 | grep "^test "
```

Expected: 해당 테스트가 FAIL — 메시지에 `llama-server`가 없다.

- [ ] **Step 4: 문구 2곳을 일반화**

`src/llm/client.rs:11-19`의 `Connect` 변형:

```rust
    #[error(
        "서버에 연결할 수 없습니다 ({base_url}).\nllama-server(또는 LM Studio 등 사용 중인 서버)가 켜져 있고 주소/포트가 맞는지 확인하세요.\n원인: {source}"
    )]
    Connect {
        base_url: String,
        #[source]
        source: reqwest::Error,
    },
```

`src/llm/client.rs:184-188`의 `resolve_model`:

```rust
    let models = client.list_models().await?;
    models.into_iter().next().ok_or_else(|| {
        anyhow::anyhow!(
            "서버에 로드된 모델이 없습니다. llama-server를 모델과 함께 기동하거나(scripts/serve.sh), 설정 파일에 model을 지정하세요."
        )
    })
```

- [ ] **Step 5: 테스트 통과 확인**

Run: `cargo test --lib llm::client 2>&1 | tail -5`
Expected: 전건 PASS

- [ ] **Step 6: README 서버 기동 절 수정**

`README.md:163-164` 근방(현재 "LM Studio(또는 Ollama, llama.cpp server 등)에서 모델을 로드하고 서버 시작")을 llama-server 1급으로 바꾼다. 정확한 현재 문구를 먼저 읽고, 다음 취지로 교체한다:

```markdown
1. 모델 서버를 기동한다
   - **llama.cpp (권장, 배포 기준 스택)**: `scripts/serve.sh` — 측정·배포 조건을
     핀으로 고정해 기동한다. 기본 `http://localhost:8080/v1`
   - LM Studio: 기본 주소 `http://localhost:1234/v1` 는 설정 없이 바로 동작
   - `base_url` 기본값은 `http://localhost:1234/v1` 이므로, llama-server를 쓰면
     `./.loco/config.toml` 에 `base_url = "http://localhost:8080/v1"` 를 둔다
```

`scripts/serve.sh`는 T2에서 만든다 — README가 먼저 언급해도 무방하다(같은 브랜치 안에서 T2가 곧 채운다).

- [ ] **Step 7: 전체 게이트**

Run: `cargo test 2>&1 | tail -5 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 테스트 전건 PASS, clippy 무경고

- [ ] **Step 8: 커밋**

```bash
git add src/llm/client.rs README.md
git commit -m "refactor(llm): 사용자 대면 안내를 서버 불문으로 일반화

배포 타깃이 llama.cpp로 바뀌면서 'LM Studio를 확인하라'는 안내가
오도가 된다. grep이 반환한 3곳(오류 문구 2 + 단언 1) 전부 교체.
agent/mod.rs의 오버플로 휴리스틱 주석은 이미 llama.cpp를 포함해 불변."
```

---

### Task 2: `scripts/serve.sh` + PROTOCOL.md 갱신

**Files:**
- Create: `scripts/serve.sh`
- Modify: `docs/experiments/PROTOCOL.md` (항목 4)

**Interfaces:**
- Consumes: 없음
- Produces: `scripts/serve.sh` — T6·T8·T11이 이 스크립트로 서버를 띄운다. 환경변수 인터페이스: `LOCO_MODEL_GGUF`(필수, GGUF 경로), `LOCO_CTX`(기본 8192), `LOCO_PORT`(기본 8080), `LOCO_ALIAS`(기본 `ornith`)

**배경 (스펙 §3-3):** loco가 요청에 싣는 것은 `model`/`messages`/`temperature`/`max_tokens`/`stream`/`response_format`/`seed` 뿐이다(`src/llm/types.rs:22-35`). 나머지 샘플러는 서버 기본값에 좌우되므로 명시 고정한다.

**⚠️ 절대 넣지 말 것: `--reasoning-format none`.** `response_format: json_schema`와 병용하면 b9960에서 **모든 요청이 400**(`Failed to initialize samplers: std::exception`)이 되고, 본문에 `"context"`가 없어 오버플로 감지를 비켜간 뒤 json_schema 폴백 사다리가 영구 발동한다. 배치가 정상 종료하면서 매 턴 파싱이 실패한다.

- [ ] **Step 1: 두 스택의 샘플러 실효 기본값 조사**

llama-server의 기본값을 실측으로 확인한다:

```bash
/opt/homebrew/bin/llama-server --help 2>&1 | grep -E "^\s*--(top-k|top-p|min-p|repeat-penalty)" -A 1
```

기록해 둔 값을 Step 2의 주석에 그대로 옮긴다. **LM Studio 쪽 값을 확보할 수 없으면 "확보 실패"라고 주석에 적고 llama-server 기본값을 명시 고정한다** — 스펙 §3-3이 말하듯 목적은 흉내가 아니라 "우리가 무엇을 돌리는지 아는 것"이다.

- [ ] **Step 2: `scripts/serve.sh` 작성**

```sh
#!/bin/sh
# loco 측정·배포용 llama-server 기동 — 조건을 핀으로 고정한다 (M13 스펙 §3-3).
# 이 스크립트가 곧 배포 산출물이자 실험 조건 기록이다. 값을 바꾸면 그 배치는
# 이전 배치와 비교 불가능해진다 — 반드시 사전등록 문서에 반영할 것.
#
# 사용법:
#   LOCO_MODEL_GGUF=/path/to/model.gguf scripts/serve.sh
#   LOCO_CTX=32768 LOCO_MODEL_GGUF=... scripts/serve.sh
set -eu

: "${LOCO_MODEL_GGUF:?LOCO_MODEL_GGUF (GGUF 경로)를 지정하세요}"
LOCO_CTX="${LOCO_CTX:-8192}"
LOCO_PORT="${LOCO_PORT:-8080}"
LOCO_ALIAS="${LOCO_ALIAS:-ornith}"
LLAMA_SERVER="${LLAMA_SERVER:-llama-server}"

# --- 핀 1: -np 1 -------------------------------------------------------------
# llama.cpp에서 -c는 병렬 슬롯이 나눠 쓰는 총량이다. -np 1로 n_ctx_slot == -c 를
# 결정론적으로 만든다. (기본 -np -1(auto)은 n_slots=4이지만 kv_unified=true라
# 분할하지 않는다 — 그 동작에 의존하지 않는다. 진짜 함정은 반대 방향이다:
# "안전하게" -np 4를 주면 슬롯당 컨텍스트가 1/4로 조용히 줄어든다.)
#
# --- 핀 2: 샘플러 4종 --------------------------------------------------------
# loco는 temperature만 보내고 top-k/top-p/min-p/repeat-penalty는 안 보낸다
# (src/llm/types.rs:22-35). 서버 기본값에 좌우되므로 명시 고정한다.
# 조사 근거(Step 1 실측): <여기에 실측한 llama-server 기본값을 적을 것>
#
# --- 핀 3: reasoning 처리 = 기본값(auto, reasoning_content로 분리) -----------
# --reasoning-format none 을 절대 쓰지 말 것: response_format: json_schema 와
# 병용 시 b9960에서 전 요청이 400 "Failed to initialize samplers: std::exception"
# 이 된다. 본문에 "context"가 없어 오버플로 감지를 비켜가고 json_schema 폴백이
# 영구 발동해, 배치는 정상 종료하면서 매 턴 파싱이 실패한다(M13 스펙 §3-3-1).
# 주의: 이 핀은 사고 토큰의 예산 잠식을 해결하지 않는다. --reasoning-format은
# 토큰을 어디에 "보고"할지만 정하고 생성을 막지 않는다. 실효 레버는
# .loco/config.toml 의 max_output_tokens 상향뿐이다(스펙 §3-2).
#
# --- 핀 4: --alias -----------------------------------------------------------
# alias가 없으면 /v1/models 의 id가 GGUF 전체 경로가 되고, 그 문자열이
# report.json 최상위 model 필드에 그대로 박힌다.
exec "$LLAMA_SERVER" \
  -m "$LOCO_MODEL_GGUF" \
  -c "$LOCO_CTX" \
  -np 1 \
  --alias "$LOCO_ALIAS" \
  --host 127.0.0.1 \
  --port "$LOCO_PORT" \
  --top-k 40 \
  --top-p 0.95 \
  --min-p 0.05 \
  --repeat-penalty 1.0
```

**Step 1에서 실측한 값이 위 4개 샘플러 값과 다르면, 실측값으로 바꾸고 주석에 근거를 적는다.** 위 값은 자리표시가 아니라 llama.cpp 통상 기본값이지만, 실측이 이긴다.

- [ ] **Step 3: 실행 권한 부여 + 기동 확인**

```bash
chmod +x scripts/serve.sh
LOCO_MODEL_GGUF=/Users/sgj/.lmstudio/models/deepreinforce-ai/Ornith-1.0-9B-GGUF/ornith-1.0-9b-Q4_K_M.gguf \
  LOCO_PORT=8081 scripts/serve.sh > /tmp/serve-test.log 2>&1 &
sleep 25
grep -E "n_ctx_slot|listening" /tmp/serve-test.log
```

Expected: `n_slots = 1, n_ctx_slot = 8192` 와 `listening on http://127.0.0.1:8081`

- [ ] **Step 4: alias와 json_schema를 함께 확인**

```bash
curl -s http://127.0.0.1:8081/v1/models | python3 -c "
import json,sys; d=json.load(sys.stdin)
print('id =', d['data'][0]['id'] if 'data' in d else d['models'][0].get('name'))
"
```

Expected: `id = ornith` (전체 경로가 아님)

- [ ] **Step 5: 서버 종료**

```bash
pkill -f "llama-server.*8081" || true
sleep 1
pgrep -f llama-server || echo "종료 확인"
```

- [ ] **Step 6: PROTOCOL.md 항목 4 교체**

`docs/experiments/PROTOCOL.md`의 항목 4를 먼저 읽는다. 현재 ②③은 LM Studio 전제(`lms unload --all` → `lms load …`, `curl -s localhost:1234/api/v0/models`)로 **llama.cpp에서 전부 죽은 명령**이다. 다음으로 교체한다:

```markdown
② 모델 서버 기동: `LOCO_MODEL_GGUF=<gguf> LOCO_CTX=<ctx> scripts/serve.sh`
   (이전 서버가 떠 있으면 먼저 내린다 — `pkill -f llama-server`)
③ 배치 전 스모크 (전건 통과해야 배치를 시작한다):
   - json_schema 요청 1건이 **HTTP 200** — 실패하면 배치를 시작하지 말 것.
     이 검사가 M12→M13 전환에서 발견된 조용한 전면 실패(스펙 §3-3-1)를 막는다
   - 서버 기동 로그의 `n_ctx_slot` == config의 `context_tokens`
   - `curl -s localhost:<port>/v1/models` 의 `data[0].id` == `--alias` 값
   - `.loco/config.toml` 이 이번 배치 조건인지 (직전 배치 잔재는 GPU 시간 전체를 무효화)
   - `ls ${TMPDIR}/.cargo` — 존재하면 수동 제거
④ 데몬화: macOS에 `setsid`가 없다.
   `python3 -c "import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])" <cmd>...`
```

json_schema 스모크의 구체 명령도 함께 적는다:

```bash
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/chat/completions \
  -H 'Content-Type: application/json' -d '{
  "model":"ornith","messages":[{"role":"user","content":"hi"}],
  "temperature":0.1,"max_tokens":64,"stream":false,
  "response_format":{"type":"json_schema","json_schema":{"name":"agent_turn","schema":{
    "type":"object","properties":{"thought":{"type":"string"},
    "action":{"type":"object","properties":{"tool":{"type":"string","enum":["finish"]},
    "args":{"type":"object"}},"required":["tool","args"]}},
    "required":["thought","action"]}}}}'
```

Expected: `200`

- [ ] **Step 7: 커밋**

```bash
git add scripts/serve.sh docs/experiments/PROTOCOL.md
git commit -m "feat(scripts): serve.sh — llama-server 기동 조건 핀 4개 고정

-np 1(컨텍스트 슬롯 산식)·샘플러 4종 명시·reasoning 기본값 고정·--alias.
--reasoning-format none은 json_schema와 병용 시 전 요청 400이 되고 폴백
사다리가 영구 발동해 조용히 실패하므로 금지를 주석에 근거와 함께 박았다.

PROTOCOL 항목 4의 LM Studio 전제(lms·api/v0/models)를 llama.cpp 기준으로
교체하고, 배치 전 json_schema 200 스모크를 필수 항목으로 추가."
```

---

### Task 3: json_schema 폴백 발동을 `report.json`에 기록

**Files:**
- Modify: `src/agent/mod.rs` (게터 추가)
- Modify: `src/eval/report.rs:31-40` (`RunRecord`)
- Modify: `src/eval/mod.rs:227,265` (`judge` 시그니처와 `RunRecord` 생성)

**Interfaces:**
- Consumes: 없음
- Produces:
  - `Agent::schema_fallback_fired(&self) -> bool` — json_schema 폴백이 이 런에서 발동했는가
  - `RunRecord.schema_fallback: bool` — `report.json`의 `tasks[].runs[].schema_fallback`

**배경 (스펙 §3-6-1):** 앵커 배치의 기계 검사 중 "json_schema 폴백이 발동하지 않았음"을 확인해야 하는데, 현재 `AgentEvent::Notice`는 eval에 영속되지 않아 `report.json`에도 트랜스크립트에도 흔적이 없다. `Agent::new`는 런마다 호출되므로(`src/eval/mod.rs:176`) `use_json_schema`는 런 지역 상태다 — 게터로 노출하면 된다.

- [ ] **Step 1: 실패하는 테스트를 쓴다**

`src/agent/mod.rs`의 `#[cfg(test)] mod tests`에 추가한다(기존 테스트 모듈 안에, 기존 테스트 헬퍼 사용법을 먼저 읽고 맞출 것):

기존 헬퍼는 `fn make_agent(script: &Scripted, root: PathBuf, max_turns: usize) -> Agent<&Scripted>`(`src/agent/mod.rs:701`)다. 이를 쓴다:

```rust
    #[test]
    fn schema_fallback_fired_is_false_on_a_fresh_agent() {
        // 폴백 게터의 초기 상태 핀 — use_json_schema가 true로 시작하므로 false여야
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![]);
        let agent = make_agent(&script, dir.path().to_path_buf(), 25);
        assert!(!agent.schema_fallback_fired(), "새 에이전트는 폴백 미발동");
    }
```

- [ ] **Step 2: 테스트가 실패(컴파일 실패)하는지 확인**

Run: `cargo test --lib agent::tests::schema_fallback 2>&1 | tail -20`
Expected: 컴파일 에러 — `no method named 'schema_fallback_fired'`

- [ ] **Step 3: 게터를 추가**

`src/agent/mod.rs`의 `impl Agent` 블록 안, `use_json_schema` 필드 선언 근처가 아니라 공개 메서드들과 같은 곳에 넣는다:

```rust
    /// 이 런에서 json_schema 폴백(400 → response_format 제거)이 발동했는가.
    /// eval이 report.json에 기록해 "조용한 전면 실패"를 배치 후 기계적으로
    /// 판별할 수 있게 한다 (M13 스펙 §3-6-1). Agent는 런마다 새로 만들어지므로
    /// (src/eval/mod.rs) 이 값은 런 지역이다.
    pub fn schema_fallback_fired(&self) -> bool {
        !self.use_json_schema
    }
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test --lib agent::tests::schema_fallback 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: `RunRecord`에 필드 추가**

`src/eval/report.rs:31-40`:

```rust
pub struct RunRecord {
    pub repeat: usize,
    /// base_seed + repeat — 개별 실행 재현용 (스펙 §8)
    pub seed: u64,
    pub passed: bool,
    pub outcome: RunOutcome,
    pub turns: usize,
    /// 에이전트 실행 시간(agent.run)만 — 판정 check·샌드박스 준비 제외 (M7 §4)
    pub duration_secs: f64,
    /// json_schema 폴백이 이 런에서 발동했는가 — true면 그 런은 스키마 강제 없이
    /// 돈 것이라 측정값으로 신뢰할 수 없다 (M13 스펙 §3-6-1 기계 검사)
    pub schema_fallback: bool,
}
```

`src/eval/report.rs:164-169`의 테스트 헬퍼 2곳도 고쳐야 컴파일된다:

```rust
    fn run(passed: bool, turns: usize, secs: f64) -> RunRecord {
        RunRecord {
            repeat: 0, seed: 0, passed, outcome: RunOutcome::Finished, turns,
            duration_secs: secs, schema_fallback: false,
        }
    }

    fn run_with(passed: bool, outcome: RunOutcome) -> RunRecord {
        RunRecord {
            repeat: 0, seed: 0, passed, outcome, turns: 1,
            duration_secs: 1.0, schema_fallback: false,
        }
    }
```

- [ ] **Step 6: `judge`에 배선**

⚠️ **`judge`와 `run_once`의 시그니처 꼬리가 바이트 동일하다.** 둘 다 이렇게 끝난다:

```rust
    cargo_snapshot: &integrity::CargoConfigSnapshot,
) -> anyhow::Result<Option<RunRecord>> {
```

**고칠 것은 `async fn judge`(`src/eval/mod.rs:237` 근방)이고, `run_once`(`:154` 근방)가 아니다.** 잘못 잡으면 컴파일 에러 4개가 난다(플랜 리뷰에서 실제로 발생). 함수 이름을 확인하고 편집할 것.

`judge` 시그니처에 인자를 추가한다(이미 `#[allow(clippy::too_many_arguments)]`가 붙어 있다):

```rust
#[allow(clippy::too_many_arguments)]
async fn judge(
    sb: &Sandbox,
    t: &Task,
    opts: &EvalOptions,
    outcome: RunOutcome,
    turns: usize,
    elapsed: Duration,
    seed: u64,
    repeat: usize,
    interrupt: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    cargo_snapshot: &integrity::CargoConfigSnapshot,
    schema_fallback: bool,
) -> anyhow::Result<Option<RunRecord>> {
```

마지막 `Ok(Some(...))`:

```rust
    Ok(Some(RunRecord {
        repeat, seed, passed, outcome, turns,
        duration_secs: elapsed.as_secs_f64(),
        schema_fallback,
    }))
```

**`judge` 호출부는 2곳이다** — 타임아웃 경로(`Err(Stopped::TimedOut)` 안)와 정상 경로. 둘 다 `agent.schema_fallback_fired()`를 마지막 인자로 넘긴다. `agent`는 그 시점에 살아 있는 지역 변수다(`src/eval/mod.rs:176`에서 생성).

- [ ] **Step 7: 전체 게이트**

Run: `cargo test 2>&1 | tail -5 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 테스트 전건 PASS, clippy 무경고

- [ ] **Step 8: 커밋**

```bash
git add src/agent/mod.rs src/eval/report.rs src/eval/mod.rs
git commit -m "feat(eval): json_schema 폴백 발동을 report.json에 기록

M13 스펙 §3-6-1의 기계 검사 하나가 구현을 요구한다. 현재 폴백은
AgentEvent::Notice로만 알려지는데 eval이 Notice를 영속하지 않아
report.json에도 트랜스크립트에도 흔적이 없다 — 배치가 스키마 없이
정상 종료해도 사후 판별이 불가능했다.

Agent::schema_fallback_fired() 게터 + RunRecord.schema_fallback.
Agent는 런마다 새로 만들어지므로 이 값은 런 지역이다."
```

---

### Task 4: `exp_metrics.py`에 `parse_fail_first` 컬럼

**Files:**
- Modify: `scripts/exp_metrics.py` (`run_metrics`, `COLS`, `process`, `selftest`)

**Interfaces:**
- Consumes: T3의 `report.json` 스키마 변경 (읽지는 않음 — 이 컬럼은 트랜스크립트 기반)
- Produces: `parse_fail_first` 컬럼 — 그 런의 **첫 assistant 메시지가 유효한 턴으로 파싱되지 않으면 1**

**배경 (스펙 §3-6-1):** C1형 조용한 실패에서는 모든 assistant 응답이 스키마 없이 나와 `parse_turn`이 매 턴 실패한다. 첫 메시지만 봐도 잡힌다. 이 컬럼이 없으면 C1을 닫는 검사가 문장으로만 존재한다.

**⚠️ 이 컬럼은 Rust의 `parse_turn`을 완전 재현하지 않는다** — 판별력 있는 최소 검사만 한다. `exp_metrics.py`가 스스로 그렇게 문서화해야 한다.

- [ ] **Step 1: 현재 구조를 읽는다**

```bash
sed -n '1,80p' scripts/exp_metrics.py
grep -n "^COLS" -A 12 scripts/exp_metrics.py
grep -n "def run_metrics" -A 5 scripts/exp_metrics.py
grep -n "return counts" scripts/exp_metrics.py
```

`run_metrics`는 튜플을 반환하고 `process`가 그것을 언패킹한다(`src/`가 아니라 `scripts/exp_metrics.py:215` 근방). **반환 튜플에 항목을 추가하면 언패킹부도 같이 고쳐야 한다.**

- [ ] **Step 2: selftest에 실패하는 기대값을 먼저 넣는다**

`selftest()`의 내장 픽스처 트랜스크립트에 대해 `parse_fail_first == 0`을 단언하고, 별도로 "깨진 첫 assistant" 픽스처에 대해 `1`을 단언한다. `selftest()` 안, 기존 단언들과 같은 스타일로:

```python
    # M13 — parse_fail_first: 첫 assistant가 유효 턴이 아니면 1 (C1형 조용한 실패 포착)
    broken = [
        {"kind": "system", "content": "sys", "ts": "t"},
        {"kind": "user", "content": "do it", "ts": "t"},
        # 스키마 강제가 꺼진 응답 — action이 객체가 아니라 문자열
        {"kind": "assistant", "content": '```json\n{"action": "read_file", "path": "a.rs"}\n```', "ts": "t"},
    ]
    assert parse_fail_first(broken) == 1, "깨진 첫 assistant는 1"
    ok = [
        {"kind": "system", "content": "sys", "ts": "t"},
        {"kind": "user", "content": "do it", "ts": "t"},
        {"kind": "assistant",
         "content": '{"thought": "look", "action": {"tool": "read_file", "args": {"path": "a.rs"}}}',
         "ts": "t"},
    ]
    assert parse_fail_first(ok) == 0, "정상 첫 assistant는 0"
    # assistant가 아예 없는 런(즉시 취소 등)은 판정 불가 → 0 (거짓 양성 금지)
    assert parse_fail_first(ok[:2]) == 0, "assistant 없으면 0"
```

- [ ] **Step 3: selftest가 실패하는지 확인**

Run: `python3 scripts/exp_metrics.py --selftest 2>&1 | tail -10`
Expected: `NameError: name 'parse_fail_first' is not defined`

- [ ] **Step 4: 함수를 구현**

`normalize_path` 근처(모듈 상단 헬퍼 구역)에 추가한다:

```python
def parse_fail_first(events):
    """첫 assistant 메시지가 유효한 에이전트 턴으로 파싱되지 않으면 1.

    M13 스펙 §3-6-1의 기계 검사 — C1형 조용한 전면 실패(json_schema 폴백이
    영구 발동해 매 턴 파싱이 실패하는데 배치는 정상 종료)를 배치 후에
    기계적으로 잡는다.

    Rust의 protocol.rs::parse_turn을 완전 재현하지 않는다. 판별력 있는 최소
    검사만 한다: 코드펜스를 벗기고 JSON 객체를 찾은 뒤, thought가 있고
    action이 tool 키를 가진 객체인지 본다. 실제 parse_turn은 salvage 관용이
    더 넓으므로 이 검사는 **거짓 양성을 내지 않는 쪽으로만 보수적**이다 —
    판정 불가(assistant 없음, JSON 못 찾음)는 전부 0으로 둔다.
    """
    for ev in events:
        if ev.get("kind") != "assistant":
            continue
        text = ev.get("content") or ""
        # 코드펜스 제거
        if "```" in text:
            parts = text.split("```")
            for p in parts:
                p = p.lstrip()
                if p.startswith("json"):
                    p = p[4:]
                if p.lstrip().startswith("{"):
                    text = p
                    break
        start = text.find("{")
        end = text.rfind("}")
        if start < 0 or end <= start:
            return 0  # JSON을 못 찾음 — 판정 불가, 거짓 양성 금지
        try:
            obj = json.loads(text[start:end + 1])
        except (ValueError, TypeError):
            return 0  # 파싱 불가 — 판정 불가
        if not isinstance(obj, dict):
            return 0
        action = obj.get("action")
        if not isinstance(action, dict) or "tool" not in action:
            return 1  # action이 객체가 아니거나 tool이 없다 = 스키마 미강제 형태
        if "thought" not in obj:
            return 1
        return 0
    return 0  # assistant 없음 — 판정 불가
```

- [ ] **Step 5: selftest 통과 확인**

Run: `python3 scripts/exp_metrics.py --selftest 2>&1 | tail -5`
Expected: `selftest ok`

- [ ] **Step 6: 컬럼으로 노출**

`COLS` 리스트에 `"parse_fail_first"`를 추가하고, `process()`의 행 생성부에서 `parse_fail_first(events)`를 호출해 값을 넣는다. `run_metrics`의 반환 튜플은 **건드리지 않는다** — 이 함수는 `events`만 받는 독립 함수라 `process()`에서 따로 부르면 된다(반환 튜플 언패킹부를 흔들지 않는 쪽이 안전하다).

또한 `process()`의 요약부에 총계를 더한다 — 기존 요약 출력 형식을 읽고 맞춘다:

```python
    parse_fail_total = 0
    # ... 루프 안에서
        pff = parse_fail_first(events)
        parse_fail_total += pff
    # ... 루프 후 요약에
    print(f"parse_fail_first(총): {parse_fail_total}  <- 0이 아니면 그 배치는 앵커/게이트로 쓸 수 없다")
```

- [ ] **Step 7: 실제 배치로 회귀 확인**

M12 배치들은 정상이었으므로 0이어야 한다:

```bash
python3 scripts/exp_metrics.py .loco/eval/20260718T222824Z 2>&1 | tail -8
python3 scripts/exp_metrics.py .loco/eval/20260718T115152Z 2>&1 | tail -8
```

Expected: 두 배치 모두 `parse_fail_first(총): 0`

**0이 아니면 멈추고 보고할 것** — 구현이 틀렸거나(가능성 높음), 과거 배치에 몰랐던 문제가 있다는 뜻이다. 어느 쪽인지 확인 없이 넘어가지 말 것.

- [ ] **Step 8: 커밋**

```bash
git add scripts/exp_metrics.py
git commit -m "feat(metrics): parse_fail_first — C1형 조용한 실패 기계 검사

앵커 배치의 기계 검사(스펙 §3-6-1) 중 'parse_turn 실패 런 0건'에 소유자가
없었다. json_schema 폴백이 영구 발동하면 매 턴 파싱이 실패하는데 배치는
정상 종료하므로, 배치 후 기계적으로 판별할 수단이 필요하다.

parse_turn을 완전 재현하지 않고 판별력 있는 최소 검사만 한다 — 판정 불가는
전부 0으로 두어 거짓 양성을 내지 않는다. M12 배치 2종에서 0 확인."
```

---

### Task 5: 사전등록 문서 — **사용자 승인 게이트**

**Files:**
- Create: `docs/experiments/2026-07-19-llamacpp-anchor/pre-registration.md`

**Interfaces:**
- Consumes: T1~T4의 커밋 해시(대상 커밋 동일성 조항에 적는다)
- Produces: T6·T8이 따르는 판정 규칙. **이 문서에 대한 사용자 승인 커밋 없이는 T6를 시작할 수 없다**

**⚠️ 이 태스크는 문서 작성 후 정지한다. 사용자 승인 없이 T6(GPU 배치)로 넘어가지 말 것.** 승인은 **문서 상태 행 커밋**으로만 성립한다 — 전언 승인은 승인이 아니다(M11·M12 전례).

- [ ] **Step 1: 기존 사전등록 문서를 읽고 형식을 맞춘다**

```bash
cat docs/experiments/PROTOCOL.md
cat docs/experiments/2026-07-18-honest-harness/pre-registration.md
ls docs/experiments/  # TEMPLATE이 있으면 그것을 따를 것
```

- [ ] **Step 2: 사전등록 문서를 쓴다**

`docs/experiments/2026-07-19-llamacpp-anchor/pre-registration.md`. **두 배치(앵커 T6 + 회귀 게이트 T8)를 한 문서로 묶는다** — 스펙 §6. 반드시 포함할 것:

- **상태 행**: `상태: 초안 — 사용자 승인 대기` (승인 시 이 줄을 커밋으로 바꾼다)
- **대상 커밋 동일성**: 앵커 배치는 T4까지의 커밋 `<hash>`, 회귀 게이트는 T7까지의 커밋. **두 배치 사이에 이 외의 코드 변경이 없음을 보장한다**
- **배치 조건**: `cargo run -- eval tasks --repeats 3` (36런), ornith@8K, seed 0, `scripts/serve.sh` 핀 적용, `.loco/config.toml`의 정확한 내용을 그대로 전재. **디버그 빌드 — `--release`를 쓰지 않는다**(대조 배치가 디버그였고, 빌드 프로파일은 `report.json`에 남지 않아 나중에 발견할 수 없는 차이가 된다)
- **대조**: `20260718T222824Z` (33/36, 엄격 32)
- **결함 하한 (판정 규칙보다 먼저)**: 앵커가 **< 27/36**이면 앵커로 기록하지 않고 마일스톤 정지·진단
- **기계 검사 3종** (전건 통과해야 앵커로 인정). **동등한 셋이 아니다 — 우선순위를 명시한다**:
  1. **주 검사**: `report.json`의 모든 런에서 `schema_fallback == false` (T3). 최종 상태를 읽으므로 **몇 번째 턴에서 폴백이 났든 잡는다**
  2. 보조: `parse_fail_first(총) == 0` (T4). 첫 assistant만 보고, 거짓 양성을 내지 않는 쪽으로 보수적이라(산문·JSON 없음은 전부 0) 단독으로는 불충분
  3. 환경: 서버 기동 로그의 `n_ctx_slot == 8192`
- **동등성 판정 규칙**: 총합이 대조 대비 **±4런 이내**면 동등 성립. 실효 구간 29~36(대조 33이므로 상한 37은 도달 불가)
- **안정 집합 = 분류 방아쇠, 판정 항 아님**: 아래 6개 중 하나라도 3/3 아래로 떨어지면 **판정을 기록하기 전에 해당 런의 트랜스크립트를 정독**한다. 읽고 스택 차이로 귀속될 때에만 판정을 불성립으로 뒤집는다
  ```
  add-function, chain-edits, count-usages, create-module,
  find-definition, fix-off-by-one
  ```
- **회귀 게이트(T8) 규칙**: 앵커 대비 총합 −4런 이내. 안정 집합은 여기서도 분류 방아쇠. 미달 시 재측정 **1회** 사전 공약, 재측정도 미달이면 T7 변경을 **되돌리고** 사용자 보고(추가 재측정 없음)
- **관측 항목(판정 아님)**: **빈-content 턴 수**(`"(empty)"` grep) — 스펙 §3-2의 사고 토큰 예산 잠식에 대한 **대리 지표**다. `finish_reason`은 트랜스크립트에 영속되지 않으므로 직접 셀 수 없고, 일부 content가 남은 절단은 이 지표에 안 잡힌다(다만 §3-2 실측상 예산 소진 시 content는 부분이 아니라 완전히 비어 지배적 경우는 포착된다). 그 외 `empty_test_note`, 상태선 렌더 수
- **T7 이후 지표 비교가능성**: 회귀 게이트 배치의 `verify_total`/`verify_zero`/`verify_allpass`/`verify_failed`는 T7의 수선 B로 **구조적으로 상향**된다(무뮤테이션 노트에도 검증 줄이 실리므로). 앵커 배치(T7 이전)와 직접 비교하지 않는다
- **"배치 사망"의 정의**: 정상 종료했으나 통과 수가 낮은 배치는 **사망이 아니다** — 재수행 대상이 아니다. 사망은 하네스 에러(exit 1)·서버 다운·Ctrl+C 부분 리포트에 한한다. (M12 사전등록 승인 리뷰가 이 미정의를 seam으로 지적한 전례)

- [ ] **Step 3: 커밋하고 정지**

```bash
git add docs/experiments/2026-07-19-llamacpp-anchor/
git commit -m "docs: M13 앵커·회귀 게이트 사전등록 초안

앵커 배치와 회귀 게이트를 한 문서로 묶었다(스펙 §6). 결함 하한(<27/36)과
기계 검사 3종을 판정 규칙 앞에 두고, 안정 집합 6개는 판정 항이 아니라
분류 방아쇠로 둔다 — 연언 항으로 쓰면 18개 시행 전승을 요구하게 되어
오경보율이 추정 불가능해진다.

사용자 승인 대기 — 승인 없이 배치를 돌리지 않는다."
```

- [ ] **Step 4: 사용자에게 보고하고 승인을 기다린다**

사용자에게 다음을 보고한다: 사전등록 문서 경로, 결함 하한과 판정 규칙 요지, 예상 GPU 시간(배치당 ~40분, 총 2회), 승인이 **문서 상태 행 커밋**으로 이뤄져야 한다는 점.

**여기서 정지한다.** 승인 커밋 전에 T6로 진행하지 말 것.

---

### Task 6: 앵커 배치 수행·판정

**Files:**
- Modify: `docs/experiments/2026-07-19-llamacpp-anchor/report.md` (신설 또는 갱신)
- Modify: `.loco/config.toml` (배치 조건으로 정리 — git-ignored, 커밋되지 않음)

**Interfaces:**
- Consumes: T5의 승인된 사전등록, `scripts/serve.sh`(T2), `parse_fail_first`(T4), `schema_fallback`(T3)
- Produces: llama.cpp 앵커 수치와 스탬프 — T8의 회귀 게이트 기준이 된다

**⚠️ 전제 확인: T5의 사용자 승인 커밋이 있는가.** 없으면 이 태스크를 시작하지 말 것.

- [ ] **Step 1: `.loco/config.toml`을 배치 조건으로 정리**

현재 M12 배치 2 조건(`command_timeout_secs = 240`)이 남아 있다. 사전등록 문서에 전재한 내용과 **정확히 일치**하도록 만든다. 앵커 배치는 M12 게이트 배치(`20260718T222824Z`)와 대조하므로 **그 배치의 `effective_config`를 그대로 따른다**:

```bash
python3 -c "
import json; d=json.load(open('.loco/eval/20260718T222824Z/report.json'))
print(json.dumps(d['effective_config'], indent=2, ensure_ascii=False))"
```

출력된 값에 맞춰 `.loco/config.toml`을 쓴다. `base_url`은 llama-server 포트로 바꾼다(그것이 이 배치의 유일한 의도된 차이다).

- [ ] **Step 2: 서버 기동 + 스모크 7항**

PROTOCOL 항목 4(T2에서 갱신)를 그대로 수행한다. **json_schema 스모크가 200이 아니면 배치를 시작하지 말 것.**

- [ ] **Step 3: 배치 수행 (데몬화)**

**`--release`를 쓰지 않는다.** 대조 배치 `20260718T222824Z`는 `cargo run -- eval tasks --repeats 3 --seed 0`(디버그 빌드)로 돌았다(M12 리포트 §1 표에서 확인). 앵커 배치의 대조 대비 **의도된 차이는 스택 하나뿐**이어야 하는데, `--release`는 두 번째 차이이고 **`report.json`에 남지 않는다**(`effective_config`는 `loco_version`은 기록하지만 빌드 프로파일은 기록하지 않는다). 나중에 아무도 이 차이를 발견할 수 없다.

`--seed 0`은 생략해도 된다 — `src/main.rs:38-40`의 `#[arg(long, default_value_t = 0)]`가 기본값이다.

```bash
ls ${TMPDIR}/.cargo 2>/dev/null && echo "!!! 수동 제거 필요" || echo "트립와이어 clear"
cd /Users/sgj/develop/loco
python3 -c "import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])" \
  cargo run -- eval tasks --repeats 3 > /tmp/m13-anchor.log 2>&1 &
```

**측정 중 `cargo build`/`test`를 병행하지 말 것.** 종료는 통지가 아니라 스탬프 디렉토리와 로그로 직접 확인한다:

```bash
ls -t .loco/eval | head -3
tail -20 /tmp/m13-anchor.log
```

- [ ] **Step 4: 기계 검사 3종**

```bash
STAMP=.loco/eval/<새 스탬프>
python3 scripts/exp_metrics.py $STAMP 2>&1 | tail -10   # parse_fail_first(총) == 0
python3 -c "
import json,sys
d=json.load(open('$STAMP/report.json'))
fb=[f\"{t['name']}-{r['repeat']}\" for t in d['tasks'] for r in t['runs'] if r.get('schema_fallback')]
print('schema_fallback 발동 런:', fb or '없음')
print('통과:', d['passed_count'], '/ 36, 엄격:', d['passed_strict_count'])
"
grep -E "n_ctx_slot" /tmp/serve-*.log | tail -2
```

Expected: `parse_fail_first(총): 0`, `schema_fallback 발동 런: 없음`, `n_ctx_slot = 8192`

**셋 중 하나라도 실패하면 그 배치를 앵커로 기록하지 말고 정지·보고한다.**

- [ ] **Step 5: 결함 하한 → 판정**

```bash
python3 - <<'EOF'
import json, glob
STAMP = "<새 스탬프 디렉토리>"
a = json.load(open(f"{STAMP}/report.json"))
b = json.load(open(".loco/eval/20260718T222824Z/report.json"))
pa, pb = a["passed_count"], b["passed_count"]
print(f"앵커 {pa}/36  대조 {pb}/36  차이 {pa-pb:+d}")
if pa < 27:
    print("!!! 결함 하한 미달 (<27) — 앵커로 기록하지 말고 정지·진단")
elif abs(pa - pb) <= 4:
    print("동등 성립 (±4 이내)")
else:
    print("동등 불성립 — 새 앵커로 기록하고 각주. 수선은 M13 범위 밖")
STABLE = ["add-function","chain-edits","count-usages","create-module","find-definition","fix-off-by-one"]
pt = {t["name"]: t["passed_count"] for t in a["tasks"]}
viol = [(n, pt.get(n)) for n in STABLE if pt.get(n) != 3]
print("안정 집합 위반:", viol or "없음",
      "\n(위반이 있으면 판정을 기록하기 전에 해당 런 트랜스크립트를 정독할 것)")
EOF
```

- [ ] **Step 6: 빈-content length 턴 관측 (대리 지표, 판정 아님)**

스펙 §3-2의 1순위 관측 항목은 `finish_reason == "length"` 턴 수인데, **`finish_reason`은 트랜스크립트에 영속되지 않는다**(기록되는 `kind`는 `system`/`user`/`assistant`/`tool_result` 뿐). 따라서 대리 지표를 쓴다:

```bash
grep -c '"(empty)"' $STAMP/run-*.jsonl 2>/dev/null | grep -v ':0$' || echo "빈 응답 없음"
```

**이것이 대리 지표임을 report.md에 명시한다.** 사각지대: 일부 content가 남은 채 잘린 절단은 정상 턴과 구별되지 않는다. 다만 이 위험에 한해서는 대리 지표가 잘 겨냥돼 있다 — 스펙 §3-2의 실측에서 사고가 예산을 소진하면 content가 **부분이 아니라 완전히 빈** 문자열로 왔다. 지배적 경우는 잡힌다.

(`finish_reason`을 제대로 기록하는 것은 T3 규모의 변경이며 이 마일스톤의 스코프 크리프다 — 하지 않는다.)

결과를 report.md에 기록한다 — 대조 배치와 비교하되 **판정에는 쓰지 않는다.**

- [ ] **Step 7: report.md 작성 + 커밋**

`docs/experiments/2026-07-19-llamacpp-anchor/report.md`에 스탬프, 기계 검사 결과, 판정, 안정 집합 상태, 관측 항목을 기록한다. **러너 보고를 그대로 옮기지 말고 `report.json`을 직접 대조한 값을 쓴다**(M12 교훈).

```bash
git add docs/experiments/2026-07-19-llamacpp-anchor/report.md
git commit -m "docs: llama.cpp 앵커 배치 결과 — <통과수>/36, 동등 <성립|불성립>"
```

---

### Task 7: 상태선 무뮤테이션 접지 — 수선 A·B

**Files:**
- Modify: `src/agent/status_note.rs:15` (케이던스), `src/agent/status_note.rs:94-99` (무뮤테이션 렌더), 같은 파일의 테스트 3건
- Modify: `src/agent/mod.rs` (테스트 2건 — `repetition_stop_still_fires_with_status_note_active:2168`, `status_note_cadence_fires_at_turn_5_when_nothing_edited`)
- Modify: `docs/baselines.md` (M13 절에 `verify_*` 비교가능성 각주 — Step 7)

**Interfaces:**
- Consumes: 기존 `StatusNote::verification_line(&self) -> String` (`src/agent/status_note.rs:117`) — 시그니처 불변, 재사용만 한다
- Produces: 없음 (내부 렌더 변경). 단 **`exp_metrics.py`의 `verify_*` 지표 의미가 바뀐다** — Step 7 참조

**배경 (스펙 §5-2):** 수선 A만으로는 "너는 아무것도 안 고쳤고 지금 3턴째다"를 더 일찍 말할 뿐이다. 값진 접지는 조기 반환이 막고 있다 — 검증 줄은 뮤테이션이 있을 때만 만들어진다. **두 수선을 함께 넣는다.**

**주장하지 않는 것**: 이 수선이 뮤테이션 0회 거짓 finish 갭을 닫는다고 주장하지 않는다. turn2에서 finish하는 런은 어떤 케이던스로도 도달 불가다.

- [ ] **Step 1: 실패하는 테스트 3건을 쓴다**

`src/agent/status_note.rs`의 `#[cfg(test)] mod tests`에 추가한다. 기존 헬퍼 `ctx(turn, mutation_ok, channel, msv)`(`src/agent/status_note.rs:176`)를 쓴다:

```rust
    #[test]
    fn zero_mutation_cadence_fires_at_3_5_7_10_15_20() {
        let mut s = StatusNote::new();
        for t in 1..=25 {
            let got = s.on_turn(&ctx(t, false, true, false)).is_some();
            let want = matches!(t, 3 | 5 | 7 | 10 | 15 | 20);
            assert_eq!(got, want, "turn {t}");
        }
    }

    #[test]
    fn zero_mutation_note_renders_verification_line() {
        // 수선 B — 뮤테이션 0회에서도 마지막 cargo test 결과를 접지한다.
        // fix-failing-test-1 재현: turn1 cargo test가 1 failed(max_of_list)
        let mut s = StatusNote::new();
        s.record_command_result(
            Some("101".to_string()),
            Some(TestSummary {
                ran: 5,
                passed: 4,
                failed: 1,
                failed_names: vec!["max_of_list".to_string()],
                filtered_out: 0,
            }),
        );
        let note = s.on_turn(&ctx(3, false, true, false)).expect("케이던스 3 발동");
        assert!(note.contains("files edited: none yet"), "{note}");
        assert!(note.contains("1 failed (max_of_list)"), "검증 줄이 실려야 함: {note}");
        assert!(note.contains("turns: 3 of 25 used"), "{note}");
    }

    #[test]
    fn zero_mutation_note_without_any_command_keeps_the_old_shape() {
        // run_command가 한 번도 없었으면 검증 줄은 규칙 5로 떨어진다 —
        // 없는 사실을 지어내지 않는지 핀
        let mut s = StatusNote::new();
        let note = s.on_turn(&ctx(3, false, true, false)).expect("케이던스 3 발동");
        assert!(note.contains("files edited: none yet"), "{note}");
        assert!(note.contains("gave no exit code"), "{note}");
    }
```

시그니처는 실측으로 확인했다(플랜 작성 시점):

- `StatusNote::new()` — `Default`도 있으나 기존 테스트가 전부 `new()`를 쓴다
- `pub fn record_command_result(&mut self, exit: Option<String>, summary: Option<TestSummary>)`
  — `String`과 `TestSummary`를 **값으로** 받는다(참조 아님)
- `pub struct TestSummary { ran, passed, failed, failed_names: Vec<String>, filtered_out }`
  (`src/test_summary.rs:9-17`)

`TestSummary`를 쓰려면 테스트 모듈에 임포트가 필요할 수 있다 — 파일 상단이 이미
`use crate::test_summary::TestSummary;`(`src/agent/status_note.rs:7`)이므로
`use super::*;`가 있는 테스트 모듈에서는 그대로 보인다. 안 보이면 임포트를 추가한다.

- [ ] **Step 2: 기존 테스트 5건이 깨진다 — 각각 다르게 처분한다**

수선 A·B를 적용하면 **정확히 5건**이 실패한다(플랜 작성 중 실측 확인):

```
agent::status_note::tests::zero_mutation_cadence_fires_at_5_10_15_20_only
agent::status_note::tests::zero_mutation_render_is_single_line
agent::status_note::tests::threshold_on_channelless_turn_carries_over_once
agent::tests::repetition_stop_still_fires_with_status_note_active
agent::tests::status_note_cadence_fires_at_turn_5_when_nothing_edited
```

**처분이 테스트마다 다르다. 일괄 삭제하지 말 것** — 셋은 여전히 유효한 불변식을 지키고 있고, 그중 하나는 지우면 M11의 의도된 보증이 조용히 사라진다.

| 테스트 | 처분 |
|---|---|
| `zero_mutation_cadence_fires_at_5_10_15_20_only` | **삭제하고 Step 1의 새 테스트로 대체.** 같은 사실의 갱신이므로 둘 다 두면 모순 |
| `zero_mutation_render_is_single_line` | **기댓값 문자열만 갱신.** 노트는 **여전히 한 줄**이므로 테스트의 이름과 목적은 그대로 유효하다 — 삭제 금지 |
| `threshold_on_channelless_turn_carries_over_once` | **세 번째 프로브를 케이던스 아닌 턴으로 옮긴다.** 현재 turn 7에서 `is_none()`("이월은 1회로 소진")을 보는데 turn 7이 이제 케이던스 지점이라 "이월 소진"과 "새 케이던스 발동"을 구별하지 못한다. **turn 8이나 9로 옮긴다**(불변식 자체는 여전히 참) |
| `repetition_stop_still_fires_with_status_note_active` | **단언을 정지 턴으로 좁힌다.** ⚠️ 가장 위험한 건이다 |
| `status_note_cadence_fires_at_turn_5_when_nothing_edited` | **기댓값 문자열만 갱신.** `assert_eq!(with_status.len(), 1)`은 **그대로 통과한다** — `remove_status_note`가 최신만 유지하므로 케이던스를 조밀화해도 히스토리는 늘지 않는다 |

**`repetition_stop_still_fires_with_status_note_active`를 반드시 이해하고 고칠 것.** 현재(`src/agent/mod.rs:2168-2180`):

```rust
        assert!(!session_contains(&session, "[status]"), "정지 턴 주입 억제");
```

이것은 M11의 `!stop` 가드 — "RepetitionStop 턴에는 상태선을 주입하지 않는다" — 를 핀으로 박은 것이다. 케이던스에 `3`이 들어오면 **정지(turn 5) 이전인 turn 3에 노트가 주입되므로** 히스토리가 비어 있지 않게 되어 이 단언이 깨진다. **가드 자체는 여전히 정상 동작한다.**

**이 단언을 지우면 의도된 M11 불변식이 조용히 은퇴한다.** 지우지 말고, "정지 턴에" 주입되지 않았음을 보는 형태로 좁힌다 — 예컨대 마지막 tool_result에 `[status]`가 없음을 보거나, 정지 직전 시점의 노트 수와 정지 후 노트 수가 같음을 본다. 구현자는 `session_contains`와 `new_session`의 실제 정의를 읽고 그에 맞는 형태를 고른다.

Run: `cargo test 2>&1 | grep -E "^test result|^    agent::"`
Expected (수정 전): `FAILED. 360 passed; 5 failed` + 위 5개 이름

**`src/agent/mod.rs`도 수정 대상이다** — 이 태스크의 Files 목록에 포함된다.

- [ ] **Step 3: 케이던스 조밀화 (수선 A)**

`src/agent/status_note.rs:15`:

```rust
/// 뮤테이션 0회 케이던스 (조건 2 — 탐색 루프 겨냥).
/// M13 §5-2-1에서 [5,10,15,20] → 초기 조밀화. M12 법의학이 확인한 두 사례
/// (fix-failing-test-1·update-vat-rate-0)가 모두 turn5를 finish가 소비해
/// 렌더되지 못했고, 3이 있었다면 둘 다 turn3에서 렌더됐을 것이다.
/// (turn2에서 finish하는 런은 어떤 케이던스로도 도달 불가 — 이 수선의 상한)
const ZERO_MUT_CADENCE: [usize; 6] = [3, 5, 7, 10, 15, 20];
```

- [ ] **Step 4: 무뮤테이션 분기에 검증 줄 (수선 B)**

`src/agent/status_note.rs:94-99`의 `render`. 현재:

```rust
    fn render(&self, ctx: &TurnCtx) -> String {
        let turns_line = format!("turns: {} of {} used", ctx.turn, ctx.max_turns);
        if self.mutated_paths.is_empty() {
            return format!("{STATUS_MARKER}files edited: none yet | {turns_line}");
        }
```

이렇게 바꾼다:

```rust
    fn render(&self, ctx: &TurnCtx) -> String {
        let turns_line = format!("turns: {} of {} used", ctx.turn, ctx.max_turns);
        if self.mutated_paths.is_empty() {
            // M13 §5-2-2 — 뮤테이션이 없어도 마지막 검증 결과는 접지한다.
            // 규칙 1(mutated_since_verify)은 뮤테이션을 전제하므로 여기선 도달 불가:
            // verification_line()의 규칙 2~5만 탄다.
            let verification = self.verification_line();
            return format!("{STATUS_MARKER}files edited: none yet | {verification} | {turns_line}");
        }
```

**한 줄 형태를 유지한다** — 뮤테이션 분기의 3행(마커 + 9칸 들여쓰기 2행) 형태로 바꾸지 말 것. 무뮤테이션 노트는 파이프 구분 1행이 기존 계약이고, `session.remove_status_note`의 블록 경계 판정(마커 줄 + 9칸 들여쓰기 연속 줄)이 그대로 동작한다.

- [ ] **Step 5: 테스트 통과 확인**

Run: `cargo test --lib agent::status_note 2>&1 | tail -10`
Expected: 전건 PASS

- [ ] **Step 6: 뮤테이션 테스트 — 변이가 죽는지 확인**

M12 교훈: 확대 태스크는 기존 분기도 변이 대상에 포함시켜야 한다. 다음 3개 변이를 **손으로 넣었다 되돌리며** 각각 테스트가 빨간불이 되는지 확인한다:

1. `ZERO_MUT_CADENCE`에서 `3`을 뺀다 → `zero_mutation_cadence_fires_at_3_5_7_10_15_20`이 FAIL해야 함
2. 수선 B의 `verification` 보간을 지우고 원래 문자열로 되돌린다 → `zero_mutation_note_renders_verification_line`이 FAIL해야 함
3. `PACING`을 `[15, 20]`에서 `[15]`로 바꾼다 → **기존** 페이싱 테스트가 FAIL해야 함(확대가 기존 분기 핀을 지우지 않았는지)

각 변이가 **실제로 빨간불을 만드는지** 확인하고 되돌린다. 죽지 않는 변이가 있으면 그 테스트가 공허하다는 뜻이다 — 테스트를 보강할 것.

- [ ] **Step 7: `verify_*` 비교가능성 각주 기록**

수선 B는 **`exp_metrics.py`가 세는 것의 의미를 바꾼다.** 그 스크립트는 평문 부분문자열로 지표를 뽑는다:

```
verify_total   <- "verification: last cargo test: "
verify_zero    <- "verification: last cargo test ran 0 tests"
verify_allpass <- "verification: last cargo test: all "
verify_failed  =  verify_total - verify_allpass
```

T7 이전에는 이 문자열이 **뮤테이션 분기에서만** 나올 수 있었다(무뮤테이션 분기는 조기 반환이라 검증 줄이 없었다). T7 이후에는 무뮤테이션 케이던스 노트마다 나온다. 따라서 **M13 이후 배치의 `verify_*` 수는 M12 기록치와 구조적으로 비교 불가**다.

`docs/baselines.md`의 M13 절에 각주를 남긴다 — M12가 `sr_error`에 대해 같은 성격의 각주를 남긴 전례를 따른다:

```markdown
⚠️ `verify_total`/`verify_zero`/`verify_allpass`/`verify_failed`는 M13 T7 이후
구조적으로 상향된다. 그 이전에는 검증 줄이 뮤테이션 분기에서만 렌더됐으나
M13부터 무뮤테이션 케이던스 노트에도 실린다. M12 실험 리포트 §4의 관측 지표
②와 직접 비교하지 말 것.
```

`exp_metrics.py` **코드는 고치지 않는다** — 세는 대상이 늘어난 것이지 세는 방식이 틀린 것이 아니다.

- [ ] **Step 8: 전체 게이트**

Run: `cargo test 2>&1 | tail -5 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 테스트 전건 PASS(365건), clippy 무경고

- [ ] **Step 9: 커밋**

```bash
git add src/agent/status_note.rs src/agent/mod.rs docs/baselines.md
git commit -m "feat(agent): 무뮤테이션 상태선 접지 — 케이던스 조밀화 + 검증 줄

M12 법의학 재검토가 당초 가정을 정정했다: 거짓 finish를 놓친 원인은
케이던스가 성겨서가 아니라 turn5를 finish가 소비해 붙일 tool_result가
없었던 구조 문제다. 그래도 케이던스에 3이 있었다면 확인된 두 사례 모두
turn3에서 렌더됐을 것이다.

수선 A: [5,10,15,20] -> [3,5,7,10,15,20].
수선 B: 무뮤테이션 분기가 조기 반환이라 검증 줄이 렌더되지 않고 있었다.
A만으로는 '너는 아무것도 안 고쳤다'를 더 일찍 말할 뿐 정보가 거의 없다.
B를 넣으면 fix-failing-test-1에서 모델이 곧 환각으로 고칠 그 테스트 이름
(max_of_list)이 turn3에 모델 앞에 놓인다. 기존 규칙 2~5 재사용.

케이던스 조밀화의 컨텍스트 비용은 0이다 — remove_status_note가 최신만
유지하므로 노트가 누적되지 않는다(기존 테스트의 len==1 단언이 그대로 통과).

이 수선이 뮤테이션 0회 거짓 finish 갭을 닫는다고 주장하지 않는다 —
turn2 finish는 어떤 케이던스로도 도달 불가하며 구조적 해법은 M14다.

exp_metrics의 verify_* 지표는 이 변경 이후 구조적으로 상향된다 —
baselines.md M13 절에 비교가능성 각주."
```

---

### Task 8: 회귀 게이트 배치

**Files:**
- Modify: `docs/experiments/2026-07-19-llamacpp-anchor/report.md` (게이트 절 추가)

**Interfaces:**
- Consumes: T6의 앵커 수치, T7의 상태선 변경
- Produces: 병합 가부 판정

**⚠️ T5의 승인된 사전등록에 이 배치가 포함돼 있다.** 조건은 앵커 배치와 **동일**해야 한다 — `.loco/config.toml`, `serve.sh` 인자, seed 전부.

- [ ] **Step 1: 배치 전 점검**

T6 Step 1~2와 동일. `.loco/config.toml`이 앵커 배치와 **바이트 동일**한지 확인한다(직전 배치 잔재가 아니라 *같은* 조건이어야 한다는 점이 여기선 오히려 요구사항이다).

- [ ] **Step 2: 배치 수행**

T6 Step 3과 동일 (`/tmp/m13-gate.log`로 로그 분리).

- [ ] **Step 3: 기계 검사 3종** — T6 Step 4와 동일

- [ ] **Step 4: 판정**

```bash
python3 - <<'EOF'
import json
GATE = "<게이트 스탬프>"; ANCHOR = "<앵커 스탬프>"
g = json.load(open(f".loco/eval/{GATE}/report.json"))
a = json.load(open(f".loco/eval/{ANCHOR}/report.json"))
pg, pa = g["passed_count"], a["passed_count"]
print(f"게이트 {pg}/36  앵커 {pa}/36  차이 {pg-pa:+d}")
print("통과" if pg >= pa - 4 else "미달 — 재측정 1회 사전 공약")
STABLE = ["add-function","chain-edits","count-usages","create-module","find-definition","fix-off-by-one"]
pt = {t["name"]: t["passed_count"] for t in g["tasks"]}
viol = [(n, pt.get(n)) for n in STABLE if pt.get(n) != 3]
print("안정 집합 위반:", viol or "없음")
print("(위반은 판정 항이 아니다 — 재측정을 걸기 전에 트랜스크립트를 먼저 읽고,")
print(" T7 변경으로 귀속될 때에만 미달로 취급한다)")
EOF
```

- [ ] **Step 5: 미달 시 — 재측정 1회, 그래도 미달이면 되돌림**

재측정도 미달이면 T7 커밋을 되돌린다:

```bash
git revert --no-edit <T7 커밋 해시>
cargo test && cargo clippy --all-targets -- -D warnings
```

파일럿(T11)은 T4까지의 코드로 진행한다. **추가 재측정은 없다.** 사용자에게 보고한다.

- [ ] **Step 6: 상태선 실측 관측 (판정 아님)**

수선 B가 실제로 렌더됐는지 확인한다:

```bash
grep -h "files edited: none yet" .loco/eval/<게이트 스탬프>/run-*.jsonl | head -5
grep -ch "files edited: none yet | verification:" .loco/eval/<게이트 스탬프>/run-*.jsonl | paste -sd+ | bc
```

Expected: 검증 줄이 실린 무뮤테이션 노트가 1건 이상. **0건이면 배선이 안 된 것이므로 보고할 것**(게이트 통과 여부와 무관하게).

- [ ] **Step 7: report.md 갱신 + 커밋**

```bash
git add docs/experiments/2026-07-19-llamacpp-anchor/report.md
git commit -m "docs: M13 회귀 게이트 결과 — <통과수>/36 (앵커 <앵커수>/36 대비 <차이>)"
```

---

### Task 9: `scripts/pilot.sh` — 실사용 세션 래퍼

**Files:**
- Create: `scripts/pilot.sh`

**Interfaces:**
- Consumes: 없음 (loco 프로덕션 코드 변경 0)
- Produces: 원장 JSONL — T10이 읽는다. 한 행의 스키마:

```json
{"session_id":"20260719T140000Z","repo":"/path/to/repo","start_rev":"abc1234",
 "end_rev":"abc1234","task_type":"bugfix","difficulty":"중","task":"한 줄 설명",
 "transcript":"/path/to/.loco/sessions/xxx.jsonl","diff":"...",
 "duration_secs":312.4,"verdict":"수정해서 씀","reason":"S/R 루프로 두 번 헤맴"}
```

**배경 (스펙 §4-2):** loco 프로덕션 코드는 **0 변경**이다 — "에이전트 코드 동결 = 비교가능성" 관례. 파일럿 종료 후 잔존물은 스크립트뿐이다.

**난이도를 세션 *전에* 받는 이유**: 사후에 매기면 결과를 아는 상태의 추정이 되어 분모로 못 쓴다.

- [ ] **Step 1: 스크립트 작성**

```sh
#!/bin/sh
# loco 실사용 파일럿 세션 래퍼 (M13 스펙 §4-2).
# loco 프로덕션 코드는 건드리지 않는다 — 세션을 감싸기만 한다.
#
# 사용법 (대상 레포 안에서):
#   LOCO_BIN=/path/to/loco PILOT_LEDGER=/path/to/ledger.jsonl scripts/pilot.sh
set -eu

LOCO_BIN="${LOCO_BIN:-loco}"
PILOT_LEDGER="${PILOT_LEDGER:?PILOT_LEDGER (원장 JSONL 경로)를 지정하세요}"
REPO="$(pwd)"

command -v git >/dev/null || { echo "git이 필요합니다"; exit 1; }
git rev-parse --git-dir >/dev/null 2>&1 || { echo "git 레포 안에서 실행하세요"; exit 1; }

if [ -n "$(git status --porcelain)" ]; then
  printf '워킹트리가 더럽습니다. 세션 diff가 오염됩니다. 계속할까요? [y/N] '
  read -r ans
  [ "$ans" = "y" ] || exit 1
fi

# --- 세션 전 수집: 결과를 알기 전에 받아야 분모로 쓸 수 있다 -----------------
printf '과제 유형 한 단어 (bugfix/feature/refactor/explore/test/other): '
read -r TASK_TYPE
printf '난이도 추정 (상/중/하) — 지금 추정해야 의미가 있습니다: '
read -r DIFFICULTY
printf '과제 한 줄: '
read -r TASK_DESC

SESSION_ID="$(date -u +%Y%m%dT%H%M%SZ)"
START_REV="$(git rev-parse HEAD)"
START_TS="$(date +%s)"

# --- 세션 ---------------------------------------------------------------------
"$LOCO_BIN" || true   # 비정상 종료도 기록 대상이다

END_TS="$(date +%s)"
END_REV="$(git rev-parse HEAD)"
DURATION=$((END_TS - START_TS))

# 세션이 만든 변경 = 미커밋 워킹트리 diff + 세션 중 생긴 커밋
DIFF="$(git diff "$START_REV" 2>/dev/null || true)"

# 가장 최근 loco 세션 트랜스크립트
TRANSCRIPT="$(ls -t "$REPO"/.loco/sessions/*.jsonl 2>/dev/null | head -1 || echo "")"

# --- 세션 후 판정 -------------------------------------------------------------
printf '판정 (1=성공 2=수정해서 씀 3=버림): '
read -r V
case "$V" in
  1) VERDICT="성공" ;;
  2) VERDICT="수정해서 씀" ;;
  3) VERDICT="버림" ;;
  *) VERDICT="미기재" ;;
esac
printf '사유 한 줄: '
read -r REASON

# 값은 반드시 환경변수로 넘긴다 — 셸 변수를 파이썬 소스에 보간하면 안 된다.
# 이유(실측 확인): 파이썬 삼중따옴표는 백슬래시 이스케이프를 해석하므로
#   diff의  \"  ->  "        (백슬래시 소실)
#   diff의  \n  ->  실제 개행 (줄 구조 파괴)
#   diff의  \t  ->  탭
# 이 되고, diff에 """ 가 들어 있으면 아예 SyntaxError로 세션이 통째로 유실된다.
# 더 나쁜 것은 조용한 쪽이다: 손상된 diff도 유효한 JSON이고 길이가 0이 아니라
# "검증 통과"로 보인다. 그리고 T10의 survival()은 git grep -F 고정 문자열
# 대조라 손상된 줄이 전부 불일치 처리되어 생존율이 체계적으로 과소 계상된다.
DIFF="$DIFF" REPO="$REPO" TASK_TYPE="$TASK_TYPE" DIFFICULTY="$DIFFICULTY" \
TASK_DESC="$TASK_DESC" TRANSCRIPT="$TRANSCRIPT" VERDICT="$VERDICT" \
REASON="$REASON" SESSION_ID="$SESSION_ID" START_REV="$START_REV" \
END_REV="$END_REV" DURATION="$DURATION" \
python3 - "$PILOT_LEDGER" <<'PYEOF'
import json, os, sys
row = {
    "session_id": os.environ["SESSION_ID"],
    "repo": os.environ["REPO"],
    "start_rev": os.environ["START_REV"],
    "end_rev": os.environ["END_REV"],
    "task_type": os.environ["TASK_TYPE"],
    "difficulty": os.environ["DIFFICULTY"],
    "task": os.environ["TASK_DESC"],
    "transcript": os.environ["TRANSCRIPT"],
    "diff": os.environ["DIFF"],
    "duration_secs": int(os.environ["DURATION"]),
    "verdict": os.environ["VERDICT"],
    "reason": os.environ["REASON"],
}
with open(sys.argv[1], "a") as f:
    f.write(json.dumps(row, ensure_ascii=False) + "\n")
print(f"원장에 기록: {row['session_id']} ({row['verdict']})")
PYEOF
```

**heredoc 구분자가 `<<'PYEOF'`(따옴표)인 것이 핵심이다** — 따옴표가 셸 보간을 끄고, 값은 전부 환경변수로만 들어온다. 따옴표를 빼면 위 주석의 손상이 그대로 돌아온다.

- [ ] **Step 2: 적대적 diff로 바이트 동일성 검증**

**판정 기준은 "유효한 JSON이고 길이가 0이 아니다"가 아니다.** 그 기준은 손상된 diff도 통과시킨다(실측 확인: `\"` → `"`로 소실돼도 JSON은 유효하고 길이도 0이 아니다). **기록된 diff가 `git diff` 출력과 바이트 동일한지**를 본다.

세 가지 적대적 요소를 전부 넣은 픽스처를 만든다 — 이스케이프된 따옴표, 백슬래시, **그리고 파이썬 삼중따옴표**:

```bash
chmod +x scripts/pilot.sh
rm -rf /tmp/pilot-test && mkdir -p /tmp/pilot-test && cd /tmp/pilot-test
git init -q && git config user.email t@t && git config user.name t
printf 'fn a() {}\n' > a.rs && git add -A && git commit -qm init
cat > a.rs <<'FIXTURE'
fn a() {
    let s = "he said \"hi\"";
    let p = "C:\temp\new";
    /* """ triple quote in a comment """ */
}
FIXTURE
cd /Users/sgj/develop/loco
```

```bash
cd /tmp/pilot-test
printf 'bugfix\n중\n테스트 과제\n1\n사유 """따옴표""" 포함\n' | \
  LOCO_BIN=true PILOT_LEDGER=/tmp/pilot-test/ledger.jsonl \
  /Users/sgj/develop/loco/scripts/pilot.sh
python3 - <<'EOF'
import json, subprocess
row = json.loads(open('/tmp/pilot-test/ledger.jsonl').readline())
expected = subprocess.run(
    ["git", "-C", "/tmp/pilot-test", "diff", row["start_rev"]],
    capture_output=True, text=True).stdout
print("바이트 동일:", row["diff"] == expected)
print("기록 길이", len(row["diff"]), "| 기대 길이", len(expected))
if row["diff"] != expected:
    print("!!! 손상됨 — Step 1의 환경변수 방식이 제대로 적용됐는지 확인할 것")
EOF
cd /Users/sgj/develop/loco
```

Expected: `바이트 동일: True`

**`False`가 나오면 멈춘다.** 이 검증이 통과할 때까지 T9를 끝내지 말 것 — 여기서 새는 데이터는 사용자가 자기 시간을 들여 만든 세션이고, 파일럿이 끝난 뒤에는 복구할 수 없다.

- [ ] **Step 3: 정리 + 커밋**

```bash
rm -rf /tmp/pilot-test
git add scripts/pilot.sh
git commit -m "feat(scripts): pilot.sh — 실사용 세션 래퍼

loco 프로덕션 코드 0 변경으로 세션을 감싼다(스펙 §4-2). 과제 유형과 난이도
추정을 세션 '전에' 받는다 — 사후 추정은 결과를 아는 상태의 추정이라
분모로 쓸 수 없다. 따옴표·백슬래시가 든 diff로 원장 JSON 유효성 검증."
```

---

### Task 10: `scripts/pilot_tally.py` — 생존율과 분류표

**Files:**
- Create: `scripts/pilot_tally.py`

**Interfaces:**
- Consumes: T9의 원장 JSONL
- Produces: 채택률(줄 생존율, 기술 통계)과 사전 선언 범주별 건수(주 산출물)

**배경 (스펙 §4-3·§4-4):** 줄 생존율은 **대리 지표**이며 알려진 왜곡이 5종이다. 주 산출물은 채택률이 아니라 **사전 선언된 범주별 건수**다.

**분류 절차 (스펙 §4-4):** 증거 출처를 분리해 기록하고(기계 판정 vs 사용자 사유), **다중 라벨을 허용하며**(따라서 범주 합 ≠ 세션 수), `실패 없음`을 명시적 범주로 두어 분모를 확보한다.

- [ ] **Step 1: 스크립트 작성**

```python
#!/usr/bin/env python3
"""M13 파일럿 원장 집계 (스펙 §4-3·§4-4). stdlib 전용.

  python3 scripts/pilot_tally.py <ledger.jsonl> <repo-path>

산출:
  1) 범주별 건수 — 주 산출물. 다중 라벨이므로 합 != 세션 수
  2) 줄 생존율 — 기술 통계. 대리 지표이며 왜곡 5종이 알려져 있다
"""
import json, os, subprocess, sys
from collections import Counter

# 스펙 §4-4 — 세션 1 이전에 확정된 범주. 신규 추가 시 추가 시점을 원장에 기록할 것
CATEGORIES = [
    "실패 없음", "S/R 루프", "뮤테이션 0회 거짓 finish", "뮤테이션 없는 탐색 루프",
    "컨텍스트 오버플로", "엉뚱한 파일 편집", "length 루프", "인자 누락(BadArgs)",
]


def added_lines(diff):
    """diff에서 유의미한 추가 줄만 — 공백/괄호/짧은 줄은 생존 판정 노이즈."""
    out = []
    for line in diff.splitlines():
        if not line.startswith("+") or line.startswith("+++"):
            continue
        body = line[1:].strip()
        if len(body) > 10 and body not in ("{", "}", "*/"):
            out.append(body)
    return out


def survival(repo, diff):
    """추가 줄 중 현재 HEAD 트리에 남아 있는 비율. (None, 0) = 판정 대상 없음."""
    lines = added_lines(diff)
    if not lines:
        return None, 0
    alive = 0
    for body in lines:
        # git grep -F: 고정 문자열, HEAD 트리 전체
        r = subprocess.run(["git", "-C", repo, "grep", "-qF", body, "HEAD"],
                           capture_output=True)
        if r.returncode == 0:
            alive += 1
    return alive / len(lines), len(lines)


def classify(row):
    """(범주 리스트, 증거 출처) — 기계 판정은 트랜스크립트, 나머지는 사유 한 줄.

    증거 출처를 섞지 않는 이유(스펙 §4-4): 기계 판정과 사용자 판정이 어긋나는
    것이 §4-3이 말한 가장 값진 축이므로, 섞으면 그 축이 사라진다.
    """
    cats, source = [], "user"
    tpath = row.get("transcript") or ""
    if tpath and os.path.exists(tpath):
        source = "machine"
        try:
            events = [json.loads(l) for l in open(tpath)]
        except (ValueError, OSError):
            events = []
        bodies = " ".join((e.get("content") or "") for e in events)
        muts = sum(1 for e in events
                   if e.get("kind") == "tool" and e.get("tool") in ("edit_file", "write_file")
                   and not (e.get("content") or "").startswith("Error"))
        if "still contains your search text" in bodies:
            cats.append("S/R 루프")
        if "context" in bodies.lower() and "exceed" in bodies.lower():
            cats.append("컨텍스트 오버플로")
        if "missing field" in bodies:
            cats.append("인자 누락(BadArgs)")
        if muts == 0 and row.get("verdict") != "성공":
            cats.append("뮤테이션 없는 탐색 루프")
    if not cats and row.get("verdict") == "성공":
        cats.append("실패 없음")
    if not cats:
        cats.append("기타")
        source = "user"
    return cats, source


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(1)
    ledger, repo = sys.argv[1], sys.argv[2]
    rows = [json.loads(l) for l in open(ledger)]
    print(f"# 파일럿 집계 — 세션 {len(rows)}개\n")

    cat_counts, src_counts = Counter(), Counter()
    for r in rows:
        cats, src = classify(r)
        cat_counts.update(cats)
        src_counts[src] += 1

    print("## 범주별 건수 (주 산출물)")
    print("다중 라벨 — 합계는 세션 수와 같지 않다\n")
    for c in CATEGORIES + [k for k in cat_counts if k not in CATEGORIES]:
        if cat_counts[c]:
            print(f"  {c:<28} {cat_counts[c]}")
    print(f"\n  증거 출처: 기계 {src_counts['machine']} / 사용자 {src_counts['user']}")

    print("\n## 판정 분포 (기술 통계)")
    for v, n in Counter(r.get("verdict") for r in rows).most_common():
        print(f"  {v:<16} {n}")

    print("\n## 난이도 × 판정 (분모 — 세션 전 수집)")
    for d in ("상", "중", "하"):
        sub = [r for r in rows if r.get("difficulty") == d]
        if sub:
            ok = sum(1 for r in sub if r.get("verdict") == "성공")
            print(f"  난이도 {d}: {len(sub)}세션, 성공 {ok}")

    print("\n## 줄 생존율 (대리 지표 — 왜곡 5종 알려짐, 스펙 §4-3)")
    tot_alive, tot_lines, judged = 0.0, 0, 0
    for r in rows:
        rate, n = survival(repo, r.get("diff") or "")
        if rate is None:
            continue
        judged += 1
        tot_alive += rate * n
        tot_lines += n
        print(f"  {r['session_id']}  {rate:5.1%} ({n}줄)  {r.get('verdict')}")
    if tot_lines:
        print(f"\n  가중 생존율: {tot_alive / tot_lines:.1%} ({judged}세션, {tot_lines}줄)")
    print("\n  경고: 생존율은 채택의 대리 지표다. 삭제가 가치였던 세션·진단만")
    print("  내놓은 세션은 0으로 잡히고, 무관한 리팩터링에 휩쓸린 채택은")
    print("  미채택으로 잡힌다. 반드시 판정 분포와 교차해 읽을 것.")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: 합성 원장으로 검증**

```bash
mkdir -p /tmp/tally-test && cd /tmp/tally-test && git init -q
printf 'fn kept() { println!("this line survives the pilot"); }\n' > a.rs
git add -A && git commit -qm init
cd /Users/sgj/develop/loco
python3 - <<'EOF'
import json
rows = [
 {"session_id":"s1","difficulty":"중","verdict":"성공","transcript":"",
  "diff":'--- a/a.rs\n+++ b/a.rs\n+fn kept() { println!("this line survives the pilot"); }\n'},
 {"session_id":"s2","difficulty":"상","verdict":"버림","transcript":"",
  "diff":'--- a/a.rs\n+++ b/a.rs\n+fn gone() { println!("this line was thrown away"); }\n'},
]
open('/tmp/tally-test/ledger.jsonl','w').write("\n".join(json.dumps(r,ensure_ascii=False) for r in rows)+"\n")
EOF
python3 scripts/pilot_tally.py /tmp/tally-test/ledger.jsonl /tmp/tally-test
```

Expected: s1 생존율 100%, s2 생존율 0%, 범주에 `실패 없음 1`, 난이도 표에 상/중 각 1

- [ ] **Step 3: 정리 + 커밋**

```bash
rm -rf /tmp/tally-test
git add scripts/pilot_tally.py
git commit -m "feat(scripts): pilot_tally.py — 범주별 건수와 줄 생존율

주 산출물은 사전 선언 범주별 건수다(스펙 §4-4). 채택률은 기술 통계로
강등했다 — 줄 생존율은 대리 지표이고 왜곡 5종이 알려져 있다.
증거 출처(기계/사용자)를 분리 기록하고 다중 라벨을 허용하며,
'실패 없음'을 명시 범주로 두어 분모를 확보한다."
```

---

### Task 11: 파일럿 20세션 수행

**Files:**
- Create: 대상 레포 밖의 원장 JSONL (loco 레포에 커밋하지 않는다 — 개인 코드 diff가 들어간다)

**Interfaces:**
- Consumes: `scripts/pilot.sh`(T9), T7까지의 loco 빌드
- Produces: 원장 — T12가 집계한다

**⚠️ 이 태스크는 사용자 시간에 의존한다. GPU 배치가 아니다.** 서브에이전트가 대신 수행할 수 없다.

- [ ] **Step 1: 대상 레포 확정**

사용자에게 묻는다: 개인/OSS 레포 중 어디서 돌릴 것인가. **loco 자신은 안 된다**(마일스톤 중인 레포를 그 마일스톤의 도구로 고치는 오염).

- [ ] **Step 2: 사전 약속 — 히스토리 재작성 금지**

파일럿 기간 중 대상 레포에서 **squash 머지·rebase를 하지 않는다.** 스펙 §4-3의 왜곡 4를 막는 유일한 수단이다. 사용자에게 명시적으로 확인받는다.

- [ ] **Step 3: 릴리스 빌드와 서버 기동**

파일럿은 **릴리스 빌드를 쓴다** — 실사용이므로 그것이 맞다. T6·T8의 배치가 디버그인 것과 의도적으로 다르며, 파일럿 수치는 배치 통과율과 **수치 비교 대상이 아니므로** 문제되지 않는다(배치끼리는 서로 비교하므로 프로파일이 같아야 한다).

```bash
cargo build --release
LOCO_MODEL_GGUF=<gguf> scripts/serve.sh > /tmp/pilot-serve.log 2>&1 &
sleep 25 && grep -E "n_ctx_slot|listening" /tmp/pilot-serve.log
```

대상 레포에 `.loco/config.toml`을 두어 `base_url`을 llama-server 포트로 맞춘다.

- [ ] **Step 4: 세션 수행**

대상 레포에서 평소처럼 작업하되 loco를 `scripts/pilot.sh`로 감싸 부른다:

```bash
cd <대상 레포>
LOCO_BIN=/Users/sgj/develop/loco/target/release/loco \
PILOT_LEDGER=<원장 경로> /Users/sgj/develop/loco/scripts/pilot.sh
```

목표 20세션. **탐색형이므로 미달도 산출을 낸다** — 사용자 시간이 막히면 그 수로 보고한다.

- [ ] **Step 5: 중간 점검 (5세션·10세션 시점)**

```bash
python3 scripts/pilot_tally.py <원장> <대상 레포>
```

새 실패 유형이 보이면 `CATEGORIES`에 추가하되 **추가 시점을 원장에 기록**한다(스펙 §4-4 — 사후에 범주를 그리는 것을 막는 장치).

---

### Task 12: 판정·문서화·M14 입력

**Files:**
- Modify: `docs/baselines.md` (M13 절 신설)
- Modify: `CLAUDE.md` (M1-M13 갱신)
- Modify: `README.md` (M13 절)
- Create: `docs/experiments/2026-07-19-llamacpp-anchor/report.md` (파일럿 절 추가)

**Interfaces:**
- Consumes: T6·T8의 배치 결과, T11의 원장
- Produces: M14 입력 후보 목록

- [ ] **Step 1: 파일럿 집계**

```bash
python3 scripts/pilot_tally.py <원장> <대상 레포> > /tmp/pilot-summary.txt
cat /tmp/pilot-summary.txt
```

- [ ] **Step 2: `docs/baselines.md`에 M13 절**

기존 절의 형식을 따라 쓴다. 반드시 포함:

- **llama.cpp 앵커**: 스탬프·통과수·엄격·거짓finish, `serve.sh` 핀 값, 동등 성립 여부와 근거
- **비교가능성 각주**: 동등 불성립이면 v2 기준선과 직접 비교 불가임을 명시
- **회귀 게이트**: 스탬프·앵커 대비 차이·판정
- **관측 항목**: `finish_reason: length` 턴 수, 무뮤테이션 검증 줄 렌더 수
- **파일럿**: 세션 수, 범주별 건수(주), 난이도 분포, 판정 분포, 생존율(기술 통계, 대리 지표 경고와 함께)

**러너·스크립트 출력을 그대로 옮기지 말고 `report.json`을 직접 대조한 값을 쓴다**(M12 교훈).

- [ ] **Step 3: `CLAUDE.md` 갱신**

첫 문단의 "M1-M12 done"을 "M1-M13 done"으로 바꾸고, M13 스펙 경로와 요지 한 문단을 추가한다. Commands 절에 `scripts/serve.sh`·`scripts/pilot.sh`·`scripts/pilot_tally.py` 사용법을 추가한다. **CLAUDE.md는 영문 유지.**

- [ ] **Step 4: `README.md`에 M13 절**

M12 절의 4단 구조(문제·수선·측정·정직 기록)를 따른다. 정직 기록에 반드시 넣을 것: 상태선 수선이 뮤테이션 0회 갭을 닫지 않는다는 점, 파일럿이 n=1 비맹검 자기 평가라는 점, 생존율이 대리 지표라는 점.

- [ ] **Step 5: M14 입력 도출**

`docs/m14-candidates.md`를 신설하고 다음을 우선순위와 함께 적는다:

- 파일럿 범주별 건수에서 나온 후보 (**실사용 증거가 우선순위를 정한다**)
- 이월 확정분: 뮤테이션 0회 finish 게이트(구조적 해법), Vulkan iGPU 오프로드
- 동등 불성립이었다면 그 원인 분석
- M12 이월 Minor 중 파일럿이 실제로 걸린 것

- [ ] **Step 6: 전체 게이트 + 커밋**

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
cargo run -- eval tasks --verify 2>&1 | tail -3       # 12/12
cargo run -- eval tasks-large --verify 2>&1 | tail -3 # 3/3
git status --short
```

```bash
git add docs/ CLAUDE.md README.md
git commit -m "docs: M13 결과 — llama.cpp 앵커 확립과 실사용 파일럿 <N>세션"
```

- [ ] **Step 7: 최종 브랜치 리뷰 요청**

`superpowers:requesting-code-review`로 브랜치 전체 리뷰를 요청한다. 리뷰어에게 **코드 실측 대조를 요구**한다(이 프로젝트 관례 — 스펙 1R이 기함 결함을 실행으로 잡은 전례).

- [ ] **Step 8: 병합**

리뷰 Ready=Yes 후, 사용자 지시가 있을 때에만:

```bash
git checkout main
git merge --no-ff m13/llamacpp-pilot
cargo test && cargo clippy --all-targets -- -D warnings
```

**origin 푸시는 사용자가 명시적으로 요청할 때만.**

---

## Self-Review

**1. 스펙 커버리지**

| 스펙 절 | 태스크 |
|---|---|
| §3-3 핀 4개 + `serve.sh` | T2 |
| §3-4 배치 전 스모크 7항 + PROTOCOL 갱신 | T2(문서), T6·T8(수행) |
| §3-5 `report.json` 모델 기록 | T2 핀 4(`--alias`)로 해소 — 코드 변경 불필요 |
| §3-6-1 결함 하한 + 기계 검사 3종 | T3(schema_fallback), T4(parse_fail_first), T5(문서화), T6(수행) |
| §3-6-2 판정 규칙 | T5(사전등록), T6(적용) |
| §4-2 `pilot.sh` + 세션 전 난이도 | T9 |
| §4-3 생존율 2단 + 왜곡 5종 | T10 |
| §4-4 사전 선언 범주 + 분류 절차 | T10 |
| §5-1 전환 부수물 | T1 |
| §5-2-1 케이던스 조밀화 | T7 |
| §5-2-2 무뮤테이션 검증 줄 | T7 |
| §6-3 회귀 게이트 | T5(사전등록), T8(수행) |
| §7 성공 기준 | T12 |

빠진 것 없음. §3-5는 코드 변경이 아니라 T2의 `--alias` 핀으로 해소되는 것이 스펙의 결론이므로 별도 태스크가 없는 것이 맞다.

**2. 자리표시자 스캔**

`scripts/serve.sh`의 샘플러 4개 값은 T2 Step 1의 실측으로 확정하도록 지시했고, 통상 기본값을 미리 채워 두어 "빈 칸"이 아니다. `<새 스탬프>`류는 실행 시점에만 알 수 있는 값이므로 자리표시자가 아니라 변수다.

**3. 타입 일관성**

- `Agent::schema_fallback_fired(&self) -> bool` (T3 정의) → T6 Step 4에서 `report.json`의 `schema_fallback` 필드로 읽음 ✓
- `RunRecord.schema_fallback: bool` (T3) → `judge`의 11번째 인자 ✓
- `parse_fail_first(events) -> int` (T4) → T6 Step 4의 요약 출력 ✓
- `StatusNote::verification_line(&self) -> String` — 기존 시그니처, T7이 재사용만 함 ✓
- T9 원장 스키마의 키(`session_id`/`difficulty`/`verdict`/`diff`/`transcript`) → T10의 `classify`·`survival`·`main`이 읽는 키와 일치 ✓
- `ZERO_MUT_CADENCE: [usize; 6]` — 배열 길이를 4에서 6으로 바꿔야 컴파일된다(T7 Step 3에 명시됨) ✓

**플랜 작성 중 자체 적발**: T7 Step 1의 테스트가 처음에 `StatusNote::default()`와 `record_command(Some("101"), Some(&TestSummary{...}))`를 썼는데, 실제 API는 `StatusNote::new()`와 `record_command_result(Option<String>, Option<TestSummary>)`(값 전달)였다.

## 플랜 리뷰 1R 처분 (`ab124de` 대상, Ready=No → 전건 반영)

리뷰어가 T3·T7을 워킹트리에 실제로 적용해 빌드·클리피·전체 테스트를 돌리고, T4를 `.loco/eval/`의 **803개 트랜스크립트 전수**에 적용했으며, T9의 heredoc을 적대적 diff로 실행했다. 컨트롤러가 C1·I1·I3를 독립 재현했다.

| # | 등급 | 처분 |
|---|---|---|
| C1 | Critical | 수용 — `pilot.sh`의 heredoc 보간이 diff를 손상시킨다. **두 모드**: `"""`가 들어오면 SyntaxError로 세션 통째 유실, 그 외에는 `\"`→`"`·`\n`→개행으로 **조용히 손상**되는데 원래 Step 2 기준(JSON 유효 + len>0)이 이를 통과시킨다(컨트롤러 재현 확인 — 원래 Step 2 픽스처가 정확히 그 형태였다). 손상된 줄은 T10의 `git grep -F` 대조에서 전부 불일치해 **생존율이 loco에 불리한 방향으로 체계적 과소 계상**된다. → 환경변수 방식을 1급으로, Step 2 기준을 **`git diff`와 바이트 동일**로 |
| I1 | Important | 수용 — T7이 **5건**을 깨는데 플랜은 1건만 명명했다(컨트롤러 재현: `FAILED. 360 passed; 5 failed`). 셋은 문자열 갱신이 아니라 재설계가 필요하고, 그중 `repetition_stop_still_fires_with_status_note_active`는 단언을 지우면 **M11의 `!stop` 가드 불변식이 조용히 은퇴**한다. 테스트별 처분표 신설 + Files에 `src/agent/mod.rs` 추가 |
| I2 | Important | 수용 — T7이 `exp_metrics.py`의 `verify_*` 지표 의미를 바꾼다(검증 줄이 무뮤테이션 노트에도 실림). M12가 `sr_error`에 남긴 것과 같은 성격의 비교가능성 각주를 T7 Step 7로 신설, T12가 승계 |
| I3 | Important | 수용 — T6이 `--release`를 썼으나 대조 배치는 디버그였다(M12 리포트 §1 표 확인). 빌드 프로파일은 `report.json`에 안 남아 **나중에 발견 불가능한 두 번째 차이**가 된다. 제거. T11의 릴리스 빌드는 실사용이라 의도적임을 명시 |
| I4 | Important | 수용 — 스펙 §3-2의 1순위 관측 항목 `finish_reason == "length"`가 **트랜스크립트에 영속되지 않는다**. `"(empty)"` grep은 대리 지표이며 그렇게 라벨링(§3-2 실측상 예산 소진 시 content가 완전히 비어 지배적 경우는 포착) |
| M1 | Minor | 수용 — `test_agent()`는 없다. 실제 헬퍼 `make_agent(&Scripted, PathBuf, usize)`(`mod.rs:701`)로 동작하는 스니펫 제공 |
| M2 | Minor | 수용 — `judge`와 `run_once`의 시그니처 꼬리가 바이트 동일해 리뷰어가 실제로 `run_once`를 잘못 고쳐 컴파일 에러 4개를 냈다. 경고 추가 |
| M3 | Minor | 수용 — 기계 검사 3종에 우선순위 명시(`schema_fallback`이 주 검사 — 최종 상태를 읽어 어느 턴의 폴백이든 잡는다) |
| M4 | Minor | 수용 — 케이던스 조밀화의 컨텍스트 비용이 **0**임이 확인됐다(`remove_status_note`가 최신만 유지). T7 커밋 메시지에 반영 |
| N1 | Nit | 미채택 — 안정 집합 6개가 3곳(사전등록·T6·T8)에 하드코딩된 것은 드리프트 위험이나, 사전등록 문서가 단일 진실이고 스니펫은 그 사본이다. 세 목록 일치는 리뷰어가 확인함 |

**리뷰어가 확인해 준 정상 항목**: T3은 단독 적용 시 빌드·클리피·테스트 전건 통과(호출부 2곳 모두 `agent` 스코프 유효). `parse_fail_first`는 **803런 전수 거짓 양성 0**이며 실제 llama-server 스키마-오프 출력에는 1을 반환. 스펙 커버리지 표 감사 결과 누락 없음 — §3-5의 "코드 변경 불필요" 결론도 T3 이후 여전히 유효(T3은 `RunRecord`를 건드리고 최상위 `model` 키는 안 건드림). T6/T8 판정 스니펫의 JSON 키 전부 실재. 순서가 주장하는 귀속 성립(T5의 산출물은 Markdown뿐이라 런타임 표면 없음).
