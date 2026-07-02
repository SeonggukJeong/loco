# loco M2 — 읽기 에이전트 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 구조화 출력(JSON) 루프 + read_file/list_files/grep/finish 툴로 코드베이스 질문에 답하는 에이전트 — 스펙의 M2 마일스톤. 답변은 `finish.summary`로 전달된다.

**Architecture:** M1의 lib+thin bin 위에 두 모듈을 얹는다. `tools`(Tool 트레이트 + 경로 확인 + 읽기 툴 3종)와 `agent`(ReAct 루프 — 매 턴 `{thought, action}` JSON 하나를 `response_format: json_schema`로 강제, 파싱 사다리와 폴백 포함). REPL의 기본 입력은 에이전트 루프로 가고, M1 스트리밍 채팅은 `/chat`으로 유지. 에이전트 턴은 비스트리밍(스피너), 툴 결과는 `role:"user"` 메시지로 래핑(스펙 §3 — `role:"tool"` 금지).

**Tech Stack:** M1 스택 + `regex`, `ignore`(스펙 §2 크레이트 목록에 있음 — 추가 승인 불필요), `rustls`(ring 프로바이더 — 사용자 승인됨), tokio `signal` 피처. 테스트: 스크립트된 가짜 `LlmClient`(agent), tempfile(tools), wiremock(llm).

**스펙:** `docs/superpowers/specs/2026-07-02-loco-design.md`

## Global Constraints

- Rust edition 2024. 의존성은 위 Tech Stack까지가 전부 — 그 외 크레이트 추가 필요 시 사용자 확인
- reqwest는 Task 1 이후 `default-features = false, features = ["json", "stream", "rustls-no-provider"]` + `rustls`(ring) 직접 의존 — OpenSSL/aws-lc-sys 금지, `main()`에서 ring 프로바이더 설치
- HTTP 클라이언트의 `.no_proxy()` 유지 — 네트워크는 설정된 엔드포인트로만
- 언어 규칙: 사용자 대상 CLI 메시지는 한국어. 식별자·시스템 프롬프트는 영어. **모델에게 반환되는 텍스트(툴 결과, 툴 에러, 교정/피드백 메시지)도 영어** — 소형 모델 지시 이행률(스펙 §4)
- 에러 타입: `llm`/`tools` 모듈은 `thiserror`, 앱 레벨은 `anyhow`. 툴 실행 에러는 크래시가 아니라 모델에게 반환되는 데이터(스펙 §9)
- 각 태스크 완료 시점에 `cargo test` 전체 통과 + `cargo clippy --all-targets -- -D warnings` 클린
- 커밋 메시지는 conventional commits (제목 한국어 가능)
- 작업 브랜치: `feat/m2-read-agent` (Task 1 시작 전 `git checkout -b feat/m2-read-agent`)
- 작업 디렉터리: `/Users/sgj/develop/loco`

## M2 범위 밖 (M3 이후로 이연 — 구현 금지)

- write_file/edit_file/run_command, 확인 게이트, diff 미리보기, `--auto`, `auto_deny_patterns`
- 반복(루프) 감지 — 스펙 §12가 명시적으로 M3에 배정
- 히스토리 절삭·컨텍스트 예산(§6 공식)·컨텍스트 초과 에러 안내 — M3. M2는 히스토리를 그대로 쌓는다(`/clear`가 우회책, README에 명시)
- 세션 기록(`./.loco/sessions/*.jsonl`)과 `.loco/.gitignore` 자동 생성 — session.rs와 함께 M3
- `-p`에서 mutating 툴 거부 에러 — M2 툴은 전부 읽기 전용이라 해당 없음
- 다중 피드백을 한 user 메시지로 병합하는 규칙(스펙 §3) — M2는 턴당 피드백이 항상 하나(툴 결과 또는 파싱 피드백 또는 length 교정)라 자연 충족. M3에서 병합 헬퍼 필요

## 파일 구조

```
src/
├── llm/            (수정) LlmClient 트레이트, ChatRequest.response_format, 헬퍼 정비
├── tools/          (신규)
│   ├── mod.rs      Tool 트레이트, ToolError, ToolCtx, Registry
│   ├── path.rs     confine() — 경로 확인 (스펙 §4)
│   ├── read_file.rs / list_files.rs / grep.rs
├── agent/          (신규)
│   ├── mod.rs      Agent 루프, AgentEvent, AgentOutcome
│   ├── protocol.rs ModelTurn 파싱 사다리, response_format 스키마 빌더
│   └── prompt.rs   영어 시스템 프롬프트 + 디렉터리 트리 주입
├── ui/
│   ├── repl.rs     (수정) 기본 입력→에이전트, /chat, Ctrl+C 취소
│   └── status.rs   (신규) Spinner, format_action
└── main.rs         (수정) -p 에이전트 모드, 종료 코드 0/1/2, ring 프로바이더 설치
```

---

### Task 1: TLS 프로바이더를 ring으로 전환

배경: 현재 rustls의 기본 프로바이더 aws-lc-rs(aws-lc-sys)가 그래프에 있어 Windows 오프라인 빌드에 cmake+NASM이 필요 — 스펙 §2 `cargo vendor` 목표와 충돌. 사용자가 M2 Task 1로 전환을 승인함. reqwest 0.13의 `rustls-no-provider` 피처는 `rustls` 피처에서 aws-lc 배선만 뺀 것(platform-verifier는 유지됨 — cargo metadata로 확인 완료).

**Files:**
- Modify: `Cargo.toml`, `src/main.rs`, `CLAUDE.md`

**Interfaces:**
- Consumes: 없음
- Produces: aws-lc-sys가 없는 의존성 그래프. `main()` 최상단의 프로바이더 설치 한 줄 (Task 13이 main을 재작성할 때 이 줄을 보존해야 함)

- [ ] **Step 1: 브랜치 생성**

```bash
cd /Users/sgj/develop/loco
git checkout -b feat/m2-read-agent
```

- [ ] **Step 2: 의존성 변경**

```bash
cargo remove reqwest
cargo add reqwest@0.13.4 --no-default-features --features json,stream,rustls-no-provider
cargo add rustls@0.23 --no-default-features --features ring,logging,std,tls12
```

- [ ] **Step 3: main()에 프로바이더 설치**

`src/main.rs`의 `async fn main()` 첫 줄에 추가:

```rust
    // ring을 프로세스 기본 TLS 프로바이더로 설치 (aws-lc-sys 제거 — Windows 오프라인 빌드 대응).
    // 테스트는 이 설치 없이도 동작한다: 그래프에 프로바이더가 ring 하나뿐이면 rustls가 자동 선택.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("rustls crypto provider 설치 실패");
```

- [ ] **Step 4: 그래프 검증**

Run: `cargo tree -i aws-lc-sys 2>&1 | head -2`
Expected: `warning: nothing to print.` (aws-lc-sys 없음)

Run: `cargo tree -i ring 2>&1 | head -3`
Expected: `ring v0.17.x` 가 rustls 아래에 표시됨

- [ ] **Step 5: 테스트/린트 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 기존 26개 테스트 전부 PASS, clippy 클린. (만약 wiremock 테스트가 `CryptoProvider` 관련 패닉을 내면 — 예상되지는 않음 — 해당 테스트 파일 헬퍼에서 `let _ = rustls::crypto::ring::default_provider().install_default();`를 호출하도록 수정)

- [ ] **Step 6: CLAUDE.md 갱신**

Hard constraints 섹션의 reqwest 줄을 다음으로 교체:

```markdown
- reqwest stays `default-features = false, features = ["json", "stream", "rustls-no-provider"]`; TLS crypto provider is rustls+ring (direct dep, user-approved) — no OpenSSL and no aws-lc-sys in the graph (Windows offline builds need no cmake/NASM). `main()` installs the ring provider at startup
```

- [ ] **Step 7: 커밋**

```bash
git add -A
git commit -m "chore: TLS 크립토 프로바이더를 ring으로 전환 (Windows 오프라인 빌드 대응)"
```

---

### Task 2: llm 정비 — LlmClient 트레이트, response_format, 헬퍼 통합

M1 리뷰에서 이연된 정비 3건(Http 에러 한국어 래핑, parse 헬퍼, get 헬퍼) + M2가 필요로 하는 확장(트레이트 경계, response_format, finish_reason 접근자).

**Files:**
- Modify: `src/llm/mod.rs`, `src/llm/client.rs`, `src/llm/types.rs`
- Modify(컴파일 유지): `src/main.rs`, `src/ui/repl.rs` — `ChatRequest` 리터럴에 `response_format: None` 추가

**Interfaces:**
- Consumes: 기존 `OpenAiClient`, `ChatRequest`, `ChatResponse`
- Produces (agent가 소비할 것들):
  - `llm/mod.rs`: `pub trait LlmClient { async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError>; }` + `impl<T: LlmClient> LlmClient for &T` (블랭킷 — REPL이 `Agent<&OpenAiClient>`를 만들 수 있게) + `impl LlmClient for OpenAiClient`
  - `ChatRequest`에 `pub response_format: Option<serde_json::Value>` 필드 (None이면 직렬화 생략)
  - `ChatResponse::finish_reason(&self) -> Option<&str>` — 첫 choice의 finish_reason
  - `LlmError::Http` 표시가 `"HTTP 요청 실패: ..."`로 시작

- [ ] **Step 1: 실패하는 테스트 작성**

`src/llm/types.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn response_format_is_omitted_when_none() {
        let req = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
            response_format: None,
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert!(v.get("response_format").is_none());
    }

    #[test]
    fn response_format_serializes_when_set() {
        let req = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
            response_format: Some(serde_json::json!({"type": "json_schema"})),
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["response_format"]["type"], "json_schema");
    }

    #[test]
    fn finish_reason_reads_first_choice() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": "x"}, "finish_reason": "length"}]}"#;
        let resp: ChatResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.finish_reason(), Some("length"));

        let none = r#"{"choices": []}"#;
        let resp: ChatResponse = serde_json::from_str(none).unwrap();
        assert_eq!(resp.finish_reason(), None);
    }
```

`src/llm/client.rs` 테스트 모듈에 추가:

```rust
    #[tokio::test]
    async fn http_error_message_is_korean() {
        // 잘못된 URL → 연결 이전의 reqwest 빌더 에러 → LlmError::Http 경로
        let client = OpenAiClient::new("not-a-url", None);
        let err = client.chat(&sample_request()).await.unwrap_err();
        assert!(err.to_string().starts_with("HTTP 요청 실패"), "{err}");
    }

    async fn call_via_trait<C: crate::llm::LlmClient>(c: &C, req: &ChatRequest) -> String {
        c.chat(req).await.unwrap().text().to_string()
    }

    #[tokio::test]
    async fn openai_client_implements_llm_client_trait() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .mount(&server)
            .await;
        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        assert_eq!(call_via_trait(&client, &sample_request()).await, "hello");
    }
```

- [ ] **Step 2: 테스트가 실패(컴파일 에러)하는지 확인**

Run: `cargo test --lib llm 2>&1 | head -20`
Expected: FAIL — `response_format` 필드 없음, `finish_reason` 없음, `LlmClient` 없음

- [ ] **Step 3: 구현**

`src/llm/types.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    pub stream: bool,
    /// 에이전트 턴의 json_schema 강제 (스펙 §4). None이면 필드 자체를 보내지 않는다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
}
```

`ChatResponse` impl에 추가:

```rust
    /// 첫 번째 choice의 finish_reason ("stop", "length" 등)
    pub fn finish_reason(&self) -> Option<&str> {
        self.choices.first().and_then(|c| c.finish_reason.as_deref())
    }
```

`src/llm/mod.rs`:

```rust
pub mod client;
pub mod sse;
pub mod types;

use client::LlmError;
use types::{ChatRequest, ChatResponse};

/// agent 루프가 의존하는 최소 경계 (스펙 §3 핵심 트레이트).
/// 테스트에서 스크립트된 가짜 클라이언트를 주입할 수 있게 한다.
/// 크레이트 내부 전용 트레이트라 AFIT(Send 바운드 없음) 경고는 무시한다.
#[allow(async_fn_in_trait)]
pub trait LlmClient {
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError>;
}

/// &OpenAiClient / &Scripted 형태로도 Agent에 넣을 수 있게 하는 블랭킷 impl
impl<T: LlmClient> LlmClient for &T {
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        (**self).chat(req).await
    }
}
```

`src/llm/client.rs`:

1. `LlmError::Http` variant를 교체 — `#[error(transparent)]` 를 `#[error("HTTP 요청 실패: {0}")]` 로
2. 모듈 레벨 헬퍼 추가 + `chat`/`chat_stream`/`list_models`의 중복 `serde_json::from_str(...).map_err(...)` 세 곳을 이 헬퍼로 교체:

```rust
/// serde_json 파싱 실패를 원문과 함께 LlmError::Parse로 감싼다 (M1 중복 제거)
fn parse_json<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, LlmError> {
    serde_json::from_str(body).map_err(|e| LlmError::Parse(format!("{e}: {body}")))
}
```

3. `post()`와 대칭인 `get()` 헬퍼 추가 + `list_models`의 수동 bearer GET 빌드를 교체:

```rust
    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        let mut rb = self.http.get(url);
        if let Some(key) = &self.api_key {
            rb = rb.bearer_auth(key);
        }
        rb
    }
```

4. 트레이트 impl 추가 (인헌트 `chat`과 이름이 같아도 명시 호출로 재귀 없음):

```rust
impl crate::llm::LlmClient for OpenAiClient {
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        OpenAiClient::chat(self, req).await
    }
}
```

5. 기존 `ChatRequest` 리터럴 전부에 `response_format: None` 추가 — 위치: `client.rs`의 `sample_request()`, `types.rs` 테스트 2곳, `src/main.rs`의 -p 요청, `src/ui/repl.rs`의 채팅 요청

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test`
Expected: 전체 PASS (기존 + 신규 5개)

- [ ] **Step 5: clippy + 커밋**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: 클린

```bash
git add -A
git commit -m "refactor: llm 정비 — LlmClient 트레이트, response_format, 헬퍼 통합"
```

---

### Task 3: tools 뼈대 — Tool 트레이트, 레지스트리, 경로 확인

**Files:**
- Create: `src/tools/mod.rs`, `src/tools/path.rs`
- Modify: `src/lib.rs` (`pub mod tools;` 추가), `Cargo.toml`

**Interfaces:**
- Consumes: 없음
- Produces (이후 모든 툴/agent가 소비):
  - `trait Tool { fn name(&self) -> &'static str; fn doc(&self) -> &'static str; fn is_mutating(&self) -> bool { false }; fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError>; }`
  - `pub struct ToolCtx { pub root: PathBuf }`
  - `enum ToolError { BadArgs(String), PathViolation(String), NotFound(String), NotUtf8(String), Io(std::io::Error), UnknownTool(String) }` — 표시 메시지는 전부 영어 (모델에 되먹이는 데이터)
  - `Registry::new(Vec<Box<dyn Tool>>)`, `Registry::names() -> Vec<&'static str>`, `Registry::docs() -> String`, `Registry::dispatch(name, args, ctx) -> Result<String, ToolError>`
  - `tools::path::confine(root: &Path, raw: &str) -> Result<PathBuf, ToolError>` — 검증 통과 시 canonicalize된 실제 경로 반환

- [ ] **Step 1: 의존성 추가 (스펙 §2 목록에 있는 크레이트)**

```bash
cargo add regex ignore
```

- [ ] **Step 2: 실패하는 테스트 작성**

`src/lib.rs`에 `pub mod tools;` 추가. `src/tools/mod.rs` 생성, 테스트 먼저:

```rust
pub mod path;

use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl Tool for Echo {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn doc(&self) -> &'static str {
            "echo(text): Echo back `text`."
        }
        fn run(&self, args: &serde_json::Value, _ctx: &ToolCtx) -> Result<String, ToolError> {
            Ok(args["text"].as_str().unwrap_or("").to_string())
        }
    }

    fn ctx() -> ToolCtx {
        ToolCtx { root: PathBuf::from(".") }
    }

    #[test]
    fn registry_dispatches_by_name() {
        let reg = Registry::new(vec![Box::new(Echo)]);
        let out = reg
            .dispatch("echo", &serde_json::json!({"text": "hi"}), &ctx())
            .unwrap();
        assert_eq!(out, "hi");
    }

    #[test]
    fn registry_unknown_tool_is_error() {
        let reg = Registry::new(vec![Box::new(Echo)]);
        let err = reg.dispatch("teleport", &serde_json::json!({}), &ctx()).unwrap_err();
        assert!(matches!(err, ToolError::UnknownTool(_)));
        assert!(err.to_string().contains("teleport"));
    }

    #[test]
    fn registry_docs_and_names_list_tools() {
        let reg = Registry::new(vec![Box::new(Echo)]);
        assert_eq!(reg.names(), vec!["echo"]);
        assert!(reg.docs().contains("- echo(text)"));
    }
}
```

`src/tools/path.rs` 생성, 테스트 먼저:

```rust
use std::path::{Component, Path, PathBuf};

use super::ToolError;

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/sub")).unwrap();
        std::fs::write(dir.path().join("src/sub/a.txt"), "x").unwrap();
        dir
    }

    #[test]
    fn accepts_relative_path() {
        let dir = root();
        let p = confine(dir.path(), "src/sub/a.txt").unwrap();
        assert!(p.ends_with("src/sub/a.txt"));
    }

    #[test]
    fn accepts_backslash_separators() {
        let dir = root();
        assert!(confine(dir.path(), "src\\sub\\a.txt").is_ok());
    }

    #[test]
    fn accepts_parent_dir_that_stays_inside() {
        let dir = root();
        // src/sub/../sub/a.txt → src/sub/a.txt (루트 안)
        assert!(confine(dir.path(), "src/sub/../sub/a.txt").is_ok());
    }

    #[test]
    fn rejects_escape_via_parent_dir() {
        let dir = root();
        for p in ["../x", "src/../../x", "..\\x"] {
            let err = confine(dir.path(), p).unwrap_err();
            assert!(matches!(err, ToolError::PathViolation(_)), "{p}");
        }
    }

    #[test]
    fn rejects_absolute_drive_and_unc_paths() {
        let dir = root();
        for p in ["/etc/passwd", "C:/x", "C:\\x", "c:x", "\\\\server\\share", "//server/share"] {
            let err = confine(dir.path(), p).unwrap_err();
            assert!(matches!(err, ToolError::PathViolation(_)), "{p}");
        }
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = root();
        let err = confine(dir.path(), "no/such.txt").unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_pointing_outside_root() {
        let dir = root();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "s").unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), dir.path().join("link.txt"))
            .unwrap();
        let err = confine(dir.path(), "link.txt").unwrap_err();
        assert!(matches!(err, ToolError::PathViolation(_)));
    }
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test --lib tools 2>&1 | head -20`
Expected: FAIL (컴파일 에러 — `Tool`, `confine` 미정의)

- [ ] **Step 4: 구현**

(Step 1에서 작성한 테스트 모듈은 파일에 그대로 유지하고, 아래 구현 코드를 그 위에 추가한다 — 파일 전체 교체 아님)

`src/tools/mod.rs`:

```rust
pub mod path;

use std::path::PathBuf;

/// 툴 실행 에러 — 크래시가 아니라 모델에게 되먹이는 데이터 (스펙 §9).
/// 표시 메시지는 영어: 모델 대상 텍스트이기 때문 (스펙 §4).
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid arguments: {0}")]
    BadArgs(String),
    #[error("path not allowed: {0}")]
    PathViolation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("not a UTF-8 text file: {0}")]
    NotUtf8(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
}

/// 툴 실행 문맥. 모든 경로는 이 루트 기준 (스펙 §4 경로 확인)
pub struct ToolCtx {
    pub root: PathBuf,
}

pub trait Tool {
    /// 스키마 enum과 디스패치에 쓰이는 이름
    fn name(&self) -> &'static str;
    /// 시스템 프롬프트에 들어갈 한 줄 설명 (영어, 시그니처 포함)
    fn doc(&self) -> &'static str;
    /// M3 확인 게이트 대상 여부. M2 툴은 전부 읽기 전용
    fn is_mutating(&self) -> bool {
        false
    }
    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError>;
}

pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
}

impl Registry {
    pub fn new(tools: Vec<Box<dyn Tool>>) -> Self {
        Self { tools }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    /// 시스템 프롬프트용 툴 설명 목록 ("- name(args): ..." 줄들)
    pub fn docs(&self) -> String {
        self.tools
            .iter()
            .map(|t| format!("- {}", t.doc()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn dispatch(
        &self,
        name: &str,
        args: &serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<String, ToolError> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| ToolError::UnknownTool(name.to_string()))?;
        tool.run(args, ctx)
    }
}
```

`src/tools/path.rs`:

```rust
/// 모델이 준 경로를 프로젝트 루트 안으로 확인(confine)한다 (스펙 §4).
///
/// - `\` 구분자도 수용 (`/`로 정규화 — 스펙: 받을 때는 둘 다 허용)
/// - 절대 경로, Windows 드라이브 문자(`C:` 등), UNC(`\\server`) 거부
/// - 렉시컬 정규화에서 `..`가 루트를 벗어나면 거부
/// - canonicalize 후 루트 prefix 재검사 — 루트 밖을 가리키는 심볼릭 링크 거부
/// - 반환은 canonicalize된 실제 경로. 존재하지 않으면 NotFound
///   (M2 툴은 전부 읽기라 대상이 존재해야 함; M3 write_file은 부모 canonicalize로 확장 예정)
pub fn confine(root: &Path, raw: &str) -> Result<PathBuf, ToolError> {
    let normalized = raw.replace('\\', "/");
    if normalized.starts_with('/') || has_drive_prefix(&normalized) {
        return Err(ToolError::PathViolation(format!(
            "absolute paths are not allowed: {raw}"
        )));
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(c) => parts.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(ToolError::PathViolation(format!(
                        "path escapes the project root: {raw}"
                    )));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ToolError::PathViolation(format!(
                    "absolute paths are not allowed: {raw}"
                )));
            }
        }
    }
    let mut joined = root.to_path_buf();
    for p in &parts {
        joined.push(p);
    }
    let canon_root = root.canonicalize()?;
    let canon = joined
        .canonicalize()
        .map_err(|_| ToolError::NotFound(raw.to_string()))?;
    if !canon.starts_with(&canon_root) {
        return Err(ToolError::PathViolation(format!(
            "path resolves outside the project root (symlink?): {raw}"
        )));
    }
    Ok(canon)
}

/// "C:/..." 또는 "c:x" 같은 드라이브 문자 접두를 감지 (Unix에서도 거부해야 함)
fn has_drive_prefix(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}
```

참고: `//server/share`는 `starts_with('/')`에 걸리고, `\\server\share`는 정규화 후 `//...`가 되어 같은 검사에 걸린다.

- [ ] **Step 5: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 6: 커밋**

```bash
git add -A
git commit -m "feat: tools 뼈대 — Tool 트레이트/레지스트리/경로 확인"
```

---

### Task 4: read_file 툴

**Files:**
- Create: `src/tools/read_file.rs`
- Modify: `src/tools/mod.rs` (`pub mod read_file;` 추가)

**Interfaces:**
- Consumes: `Tool`, `ToolCtx`, `ToolError`, `path::confine` (Task 3)
- Produces: `pub struct ReadFile;` (Tool impl), `pub const MAX_LINES: usize = 200;`
  - 인자: `{path: string, offset?: usize(1-기준), limit?: usize}` — 초과 인자는 무시(소형 모델의 환각 인자에 관대), 필수 인자 누락/타입 오류는 BadArgs
  - 출력: **라인 번호 없는** 본문 (스펙 §4 — search 블록 오염 방지). 파일이 더 길면 마지막에 `[showing lines X-Y of N; call read_file again with offset=Z to continue]` 안내

- [ ] **Step 1: 실패하는 테스트 작성**

`src/tools/read_file.rs` 생성, 테스트 먼저:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Tool, ToolCtx, ToolError};

    fn setup(content: &str) -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), content).unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        (dir, ctx)
    }

    fn run(ctx: &ToolCtx, args: serde_json::Value) -> Result<String, ToolError> {
        ReadFile.run(&args, ctx)
    }

    #[test]
    fn reads_content_without_line_numbers() {
        let (_d, ctx) = setup("fn main() {}\nline two");
        let out = run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap();
        assert_eq!(out, "fn main() {}\nline two"); // 라인 번호 없음 (스펙 §4)
    }

    #[test]
    fn caps_at_200_lines_and_tells_how_to_continue() {
        let content: String = (1..=250).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap();
        assert!(out.contains("line200"));
        assert!(!out.contains("line201\n"));
        assert!(out.contains("offset=201"), "이어 읽기 안내: {out}");
    }

    #[test]
    fn offset_continues_reading() {
        let content: String = (1..=250).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt", "offset": 201})).unwrap();
        assert!(out.starts_with("line201"));
        assert!(out.contains("line250"));
        assert!(!out.contains("[showing"), "끝까지 읽으면 안내 없음");
    }

    #[test]
    fn crlf_file_reads_fine() {
        let (_d, ctx) = setup("a\r\nb\r\n");
        let out = run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap();
        assert_eq!(out, "a\nb");
    }

    #[test]
    fn non_utf8_is_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bin.dat"), [0xFF, 0xFE, 0x00, 0x01]).unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        let err = run(&ctx, serde_json::json!({"path": "bin.dat"})).unwrap_err();
        assert!(matches!(err, ToolError::NotUtf8(_)));
    }

    #[test]
    fn missing_file_and_escape_and_bad_args() {
        let (_d, ctx) = setup("x");
        assert!(matches!(
            run(&ctx, serde_json::json!({"path": "nope.txt"})).unwrap_err(),
            ToolError::NotFound(_)
        ));
        assert!(matches!(
            run(&ctx, serde_json::json!({"path": "../f.txt"})).unwrap_err(),
            ToolError::PathViolation(_)
        ));
        assert!(matches!(
            run(&ctx, serde_json::json!({})).unwrap_err(),
            ToolError::BadArgs(_)
        ));
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tools::read_file 2>&1 | head -10`
Expected: FAIL (컴파일 에러 — `ReadFile` 미정의)

- [ ] **Step 3: 구현**

(Step 1에서 작성한 테스트 모듈은 파일에 그대로 유지하고, 아래 구현 코드를 그 위에 추가한다 — 파일 전체 교체 아님)

```rust
use serde::Deserialize;

use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

/// 한 번에 읽는 최대 줄 수 (스펙 §4). limit 인자로도 이 값을 넘을 수 없다
pub const MAX_LINES: usize = 200;

pub struct ReadFile;

#[derive(Deserialize)]
struct Args {
    path: String,
    /// 1-기준 시작 줄
    offset: Option<usize>,
    limit: Option<usize>,
}

impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn doc(&self) -> &'static str {
        "read_file(path, offset?, limit?): Read a UTF-8 text file. Returns up to 200 lines starting at line `offset` (1-based). If the file is longer, the output ends with how to continue."
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let path = confine(&ctx.root, &args.path)?;
        let bytes = std::fs::read(&path)?;
        let text =
            String::from_utf8(bytes).map_err(|_| ToolError::NotUtf8(args.path.clone()))?;
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();
        if total == 0 {
            return Ok("(empty file)".to_string());
        }
        let offset = args.offset.unwrap_or(1).max(1);
        let limit = args.limit.unwrap_or(MAX_LINES).clamp(1, MAX_LINES);
        if offset > total {
            return Err(ToolError::BadArgs(format!(
                "offset {offset} is past the end of the file ({total} lines)"
            )));
        }
        let start = offset - 1;
        let end = (start + limit).min(total);
        let mut out = lines[start..end].join("\n");
        if end < total {
            out.push_str(&format!(
                "\n[showing lines {offset}-{end} of {total}; call read_file again with offset={} to continue]",
                end + 1
            ));
        }
        Ok(out)
    }
}
```

`src/tools/mod.rs` 상단에 `pub mod read_file;` 추가.

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: read_file 툴"
```

---

### Task 5: list_files 툴 + 공용 워커

**Files:**
- Create: `src/tools/list_files.rs`
- Modify: `src/tools/mod.rs` (`pub mod list_files;` 추가)

**Interfaces:**
- Consumes: `Tool`, `ToolCtx`, `ToolError`, `path::confine` (Task 3)
- Produces:
  - `pub struct ListFiles;` (Tool impl), `pub const MAX_ENTRIES: usize = 200;`
  - `pub(crate) fn walker(base: &Path, depth: Option<usize>) -> ignore::Walk` — gitignore 존중 + 정렬(결정적) + `require_git(false)`. **Task 6(grep)이 재사용**
  - `pub fn walk_entries(root: &Path, base: &Path, depth: Option<usize>, max_entries: usize) -> Vec<String>` — 루트 기준 상대 경로 목록, 디렉터리는 `/` 접미, 구분자 `/` 정규화. **Task 8(트리 주입)이 재사용**
  - 참고: ignore 크레이트 기본값이 숨김 파일(`.git`, `.loco` 등)을 건너뛴다 — 의도된 동작

- [ ] **Step 1: 실패하는 테스트 작성**

`src/tools/list_files.rs` 생성, 테스트 먼저:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Tool, ToolCtx};

    fn setup() -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/deep/deeper")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/deep/a.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/deep/deeper/b.rs"), "").unwrap();
        std::fs::write(dir.path().join("README.md"), "").unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        (dir, ctx)
    }

    #[test]
    fn lists_files_and_dirs_with_slash_suffix() {
        let (_d, ctx) = setup();
        let out = ListFiles.run(&serde_json::json!({}), &ctx).unwrap();
        assert!(out.contains("README.md"));
        assert!(out.lines().any(|l| l == "src/"), "디렉터리는 / 접미: {out}");
        assert!(out.contains("src/deep/deeper/b.rs"));
    }

    #[test]
    fn respects_gitignore_without_git_repo() {
        let (dir, ctx) = setup();
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/junk.o"), "").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/target\n").unwrap();
        let out = ListFiles.run(&serde_json::json!({}), &ctx).unwrap();
        assert!(!out.contains("junk.o"), "{out}");
    }

    #[test]
    fn depth_limits_recursion() {
        let (_d, ctx) = setup();
        let out = ListFiles.run(&serde_json::json!({"depth": 1}), &ctx).unwrap();
        assert!(out.contains("src/"));
        assert!(!out.contains("src/main.rs"), "depth=1이면 루트 항목만: {out}");
    }

    #[test]
    fn path_narrows_the_listing() {
        let (_d, ctx) = setup();
        let out = ListFiles.run(&serde_json::json!({"path": "src/deep"}), &ctx).unwrap();
        assert!(out.contains("src/deep/a.rs"));
        assert!(!out.contains("README.md"));
    }

    #[test]
    fn caps_entries_with_notice() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..210 {
            std::fs::write(dir.path().join(format!("f{i:03}.txt")), "").unwrap();
        }
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        let out = ListFiles.run(&serde_json::json!({}), &ctx).unwrap();
        assert_eq!(out.lines().filter(|l| l.ends_with(".txt")).count(), MAX_ENTRIES);
        assert!(out.contains("[truncated at 200 entries"), "{out}");
    }

    #[test]
    fn escape_is_rejected() {
        let (_d, ctx) = setup();
        assert!(ListFiles.run(&serde_json::json!({"path": "../"}), &ctx).is_err());
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tools::list_files 2>&1 | head -10`
Expected: FAIL (컴파일 에러)

- [ ] **Step 3: 구현**

(Step 1에서 작성한 테스트 모듈은 파일에 그대로 유지하고, 아래 구현 코드를 그 위에 추가한다 — 파일 전체 교체 아님)

```rust
use std::ffi::OsStr;
use std::path::Path;

use serde::Deserialize;

use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

/// 한 번에 나열하는 최대 항목 수 (스펙 §4 "항목 수 상한")
pub const MAX_ENTRIES: usize = 200;

pub struct ListFiles;

#[derive(Deserialize)]
struct Args {
    path: Option<String>,
    depth: Option<usize>,
}

/// gitignore를 존중하는 공용 워커 (grep과 프롬프트 트리 주입이 재사용).
/// require_git(false): git repo가 아니어도 .gitignore를 적용 (테스트 픽스처 포함).
/// 정렬은 출력 결정성 때문에 필요하다.
pub(crate) fn walker(base: &Path, depth: Option<usize>) -> ignore::Walk {
    let mut b = ignore::WalkBuilder::new(base);
    b.require_git(false)
        .sort_by_file_name(|a: &OsStr, b: &OsStr| a.cmp(b));
    if let Some(d) = depth {
        b.max_depth(Some(d));
    }
    b.build()
}

/// base 아래 항목을 루트 기준 상대 경로로 나열한다. 디렉터리는 `/` 접미,
/// 구분자는 `/`로 정규화 (스펙 §4: 모델에게 보여줄 때는 `/`). 최대 max_entries개.
pub fn walk_entries(
    root: &Path,
    base: &Path,
    depth: Option<usize>,
    max_entries: usize,
) -> Vec<String> {
    // 표시 경로 계산과 시작점 비교가 일관되도록 양쪽 다 canonicalize
    // (macOS의 /tmp → /private/tmp 심링크 등으로 걷기 경로와 어긋나는 것 방지)
    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let mut out = Vec::new();
    for entry in walker(&base, depth) {
        let Ok(entry) = entry else { continue }; // 읽기 실패 항목은 건너뜀
        let p = entry.path();
        if p == base {
            continue; // 시작점 자신은 제외
        }
        let rel = p.strip_prefix(&canon_root).unwrap_or(p);
        let mut s = rel.to_string_lossy().replace('\\', "/");
        if entry.file_type().is_some_and(|t| t.is_dir()) {
            s.push('/');
        }
        out.push(s);
        if out.len() >= max_entries {
            break;
        }
    }
    out
}

impl Tool for ListFiles {
    fn name(&self) -> &'static str {
        "list_files"
    }

    fn doc(&self) -> &'static str {
        "list_files(path?, depth?): List files under `path` (default: project root), honoring .gitignore. Directories end with `/`."
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let base = confine(&ctx.root, args.path.as_deref().unwrap_or(""))?;
        let mut entries = walk_entries(&ctx.root, &base, args.depth, MAX_ENTRIES + 1);
        if entries.is_empty() {
            return Ok("(empty)".to_string());
        }
        let truncated = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);
        let mut out = entries.join("\n");
        if truncated {
            out.push_str(&format!(
                "\n[truncated at {MAX_ENTRIES} entries; pass `path` or `depth` to narrow]"
            ));
        }
        Ok(out)
    }
}
```

`src/tools/mod.rs`에 `pub mod list_files;` 추가.

참고: `confine(root, "")`은 루트 자신을 반환한다 (빈 경로 → 컴포넌트 없음 → 루트 canonicalize). `depth` 의미는 ignore 크레이트의 `max_depth`를 그대로 따른다 — `depth: 1`은 base 바로 아래 항목까지.

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: list_files 툴 + 공용 워커"
```

---

### Task 6: grep 툴 + 읽기 전용 레지스트리

**Files:**
- Create: `src/tools/grep.rs`
- Modify: `src/tools/mod.rs` (`pub mod grep;` + `Registry::read_only()` 추가)

**Interfaces:**
- Consumes: `Tool`/`ToolCtx`/`ToolError`/`confine` (Task 3), `list_files::walker` (Task 5)
- Produces:
  - `pub struct Grep;` (Tool impl), `pub const MAX_MATCHES: usize = 50;`
  - 출력 형식: 매치 줄 `상대경로:줄번호: 내용`, 전후 2줄 컨텍스트는 `상대경로-줄번호- 내용`, 매치 그룹 사이 `--` 구분. 매치 없으면 `"no matches"`
  - `Registry::read_only() -> Registry` — `[ReadFile, ListFiles, Grep]`. **finish는 레지스트리에 없다** — agent 루프가 직접 처리 (Task 9)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/tools/grep.rs` 생성, 테스트 먼저:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Tool, ToolCtx, ToolError};

    fn setup() -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/a.rs"),
            "line one\nfn target() {}\nline three\nline four\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("src/b.rs"), "nothing here\n").unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        (dir, ctx)
    }

    fn run(ctx: &ToolCtx, args: serde_json::Value) -> Result<String, ToolError> {
        Grep.run(&args, ctx)
    }

    #[test]
    fn match_shows_line_number_and_context() {
        let (_d, ctx) = setup();
        let out = run(&ctx, serde_json::json!({"pattern": "fn target"})).unwrap();
        assert!(out.contains("src/a.rs:2: fn target() {}"), "{out}");
        assert!(out.contains("src/a.rs-1- line one"), "앞 컨텍스트: {out}");
        assert!(out.contains("src/a.rs-4- line four"), "뒤 컨텍스트 2줄: {out}");
        assert!(!out.contains("b.rs"), "매치 없는 파일 제외: {out}");
    }

    #[test]
    fn caps_matches_at_50() {
        let dir = tempfile::tempdir().unwrap();
        let body: String = (1..=60).map(|i| format!("hit {i}\n")).collect();
        std::fs::write(dir.path().join("many.txt"), body).unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        let out = run(&ctx, serde_json::json!({"pattern": "hit"})).unwrap();
        assert_eq!(out.matches("many.txt:").count(), MAX_MATCHES, "{out}");
        assert!(out.contains("[more matches truncated at 50]"), "{out}");
    }

    #[test]
    fn invalid_regex_is_bad_args() {
        let (_d, ctx) = setup();
        let err = run(&ctx, serde_json::json!({"pattern": "["})).unwrap_err();
        assert!(matches!(err, ToolError::BadArgs(_)));
        assert!(err.to_string().contains("invalid regex"));
    }

    #[test]
    fn binary_files_are_skipped() {
        let (dir, ctx) = setup();
        std::fs::write(dir.path().join("bin.dat"), [0xFF, 0x00, b'f', b'n']).unwrap();
        let out = run(&ctx, serde_json::json!({"pattern": "fn"})).unwrap();
        assert!(!out.contains("bin.dat"), "{out}");
    }

    #[test]
    fn no_match_says_so() {
        let (_d, ctx) = setup();
        assert_eq!(run(&ctx, serde_json::json!({"pattern": "zzz_none"})).unwrap(), "no matches");
    }

    #[test]
    fn path_can_target_a_single_file() {
        let (_d, ctx) = setup();
        let out = run(&ctx, serde_json::json!({"pattern": "one", "path": "src/a.rs"})).unwrap();
        assert!(out.contains("src/a.rs:1"), "{out}");
    }
}
```

`src/tools/mod.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn read_only_registry_has_the_three_read_tools() {
        let reg = Registry::read_only();
        assert_eq!(reg.names(), vec!["read_file", "list_files", "grep"]);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib tools 2>&1 | head -10`
Expected: FAIL (컴파일 에러 — `Grep`, `read_only` 미정의)

- [ ] **Step 3: 구현**

(Step 1에서 작성한 테스트 모듈은 파일에 그대로 유지하고, 아래 구현 코드를 그 위에 추가한다 — 파일 전체 교체 아님)

`src/tools/grep.rs`:

```rust
use std::path::PathBuf;

use serde::Deserialize;

use super::list_files::walker;
use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

/// 최대 매치 수 (스펙 §4)
pub const MAX_MATCHES: usize = 50;
/// 매치당 전후 컨텍스트 줄 수 (스펙 §4)
const CONTEXT: usize = 2;

pub struct Grep;

#[derive(Deserialize)]
struct Args {
    pattern: String,
    path: Option<String>,
}

impl Tool for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn doc(&self) -> &'static str {
        "grep(pattern, path?): Search file contents with a regex under `path` (default: project root). Shows up to 50 matching lines with 2 context lines, formatted `file:line: text`."
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let re = regex::Regex::new(&args.pattern)
            .map_err(|e| ToolError::BadArgs(format!("invalid regex: {e}")))?;
        let base = confine(&ctx.root, args.path.as_deref().unwrap_or(""))?;
        let canon_root = ctx.root.canonicalize()?;

        let files: Vec<PathBuf> = if base.is_file() {
            vec![base]
        } else {
            walker(&base, None)
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
                .map(|e| e.into_path())
                .collect()
        };

        let mut out = String::new();
        let mut matches = 0;
        let mut truncated = false;
        'files: for file in files {
            let Ok(bytes) = std::fs::read(&file) else { continue };
            // 바이너리/비UTF-8 파일은 조용히 건너뛴다
            let Ok(text) = String::from_utf8(bytes) else { continue };
            let rel = file
                .strip_prefix(&canon_root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if !re.is_match(line) {
                    continue;
                }
                if matches == MAX_MATCHES {
                    truncated = true;
                    break 'files;
                }
                matches += 1;
                if !out.is_empty() {
                    out.push_str("--\n");
                }
                let start = i.saturating_sub(CONTEXT);
                let end = (i + CONTEXT + 1).min(lines.len());
                // 인덱스 루프는 clippy::needless_range_loop에 걸린다 (-D warnings 게이트)
                for (j, ctx_line) in lines.iter().enumerate().take(end).skip(start) {
                    let sep = if j == i { ':' } else { '-' };
                    out.push_str(&format!("{rel}{sep}{}{sep} {}\n", j + 1, ctx_line));
                }
            }
        }
        if matches == 0 {
            return Ok("no matches".to_string());
        }
        if truncated {
            out.push_str(&format!("[more matches truncated at {MAX_MATCHES}]\n"));
        }
        Ok(out.trim_end().to_string())
    }
}
```

`src/tools/mod.rs`에 `pub mod grep;` 추가 + `Registry` impl에 추가:

```rust
    /// M2 읽기 전용 툴 세트. finish는 agent 루프가 직접 처리한다 (스펙 §4)
    pub fn read_only() -> Self {
        Self::new(vec![
            Box::new(read_file::ReadFile),
            Box::new(list_files::ListFiles),
            Box::new(grep::Grep),
        ])
    }
```

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: grep 툴 + 읽기 전용 레지스트리"
```

---

### Task 7: agent 프로토콜 — 파싱 사다리 + json_schema 빌더

**Files:**
- Create: `src/agent/mod.rs` (일단 `pub mod protocol;` 선언만), `src/agent/protocol.rs`
- Modify: `src/lib.rs` (`pub mod agent;` 추가)

**Interfaces:**
- Consumes: 없음 (serde_json만)
- Produces (Task 9~10이 소비):
  - `pub struct ModelTurn { pub thought: String, pub action: Action }`, `pub struct Action { pub tool: String, pub args: serde_json::Value }` (args 누락 시 `Value::Null`)
  - `pub fn parse_turn(text: &str) -> Result<ModelTurn, String>` — Err는 모델에 되먹일 영어 피드백. 사다리: 그대로 파싱 → 마크다운 펜스 제거 → 첫 `{...}` 균형 스캔
  - `pub fn response_format(tool_names: &[&str]) -> serde_json::Value` — 얕은 스키마 (tool은 enum, args는 자유 오브젝트 — 스펙 §4)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/lib.rs`에 `pub mod agent;` 추가. `src/agent/mod.rs`는 일단 한 줄:

```rust
pub mod protocol;
```

`src/agent/protocol.rs` 생성, 테스트 먼저:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_json_turn() {
        let t = parse_turn(r#"{"thought": "look", "action": {"tool": "read_file", "args": {"path": "a.rs"}}}"#)
            .unwrap();
        assert_eq!(t.thought, "look");
        assert_eq!(t.action.tool, "read_file");
        assert_eq!(t.action.args["path"], "a.rs");
    }

    #[test]
    fn missing_args_defaults_to_null() {
        let t = parse_turn(r#"{"thought": "done", "action": {"tool": "finish"}}"#).unwrap();
        assert!(t.action.args.is_null());
    }

    #[test]
    fn parses_a_fenced_turn() {
        let text = "```json\n{\"thought\": \"t\", \"action\": {\"tool\": \"finish\", \"args\": {}}}\n```";
        assert_eq!(parse_turn(text).unwrap().action.tool, "finish");
    }

    #[test]
    fn extracts_object_from_surrounding_prose() {
        let text = "Sure! Here is my move: {\"thought\": \"t\", \"action\": {\"tool\": \"finish\", \"args\": {}}} hope that helps";
        assert_eq!(parse_turn(text).unwrap().action.tool, "finish");
    }

    #[test]
    fn handles_braces_and_escapes_inside_strings() {
        let text = r#"{"thought": "regex like {\"x\": 1}", "action": {"tool": "grep", "args": {"pattern": "fn \\w+ \\{"}}}"#;
        let t = parse_turn(text).unwrap();
        assert_eq!(t.action.tool, "grep");
        assert_eq!(t.action.args["pattern"], "fn \\w+ \\{");
        // 산문에 싸여 있으면 스캐너 경로를 타는데, 문자열 안 중괄호에 속지 않아야 한다
        let wrapped = format!("Here is my move: {text} — done");
        assert_eq!(parse_turn(&wrapped).unwrap().action.tool, "grep");
    }

    #[test]
    fn garbage_and_empty_are_errors_with_format_hint() {
        for bad in ["no json here", "", "   "] {
            let err = parse_turn(bad).unwrap_err();
            assert!(err.contains("JSON object"), "{err}");
            assert!(err.contains("thought"), "형식 힌트 포함: {err}");
        }
    }

    #[test]
    fn valid_json_with_wrong_shape_is_an_error() {
        let err = parse_turn(r#"{"answer": 42}"#).unwrap_err();
        assert!(err.contains("thought"), "{err}");
    }

    #[test]
    fn schema_is_shallow_with_tool_enum() {
        let v = response_format(&["read_file", "list_files", "grep", "finish"]);
        assert_eq!(v["type"], "json_schema");
        let schema = &v["json_schema"]["schema"];
        assert_eq!(schema["required"], serde_json::json!(["thought", "action"]));
        assert_eq!(
            schema["properties"]["action"]["properties"]["tool"]["enum"],
            serde_json::json!(["read_file", "list_files", "grep", "finish"])
        );
        // args는 자유 오브젝트 — 깊은 oneOf 유니온 금지 (스펙 §4)
        assert_eq!(
            schema["properties"]["action"]["properties"]["args"],
            serde_json::json!({"type": "object"})
        );
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib agent 2>&1 | head -10`
Expected: FAIL (컴파일 에러)

- [ ] **Step 3: 구현**

(Step 1에서 작성한 테스트 모듈은 파일에 그대로 유지하고, 아래 구현 코드를 그 위에 추가한다 — 파일 전체 교체 아님)

```rust
use serde::Deserialize;

/// 매 턴 모델이 출력해야 하는 구조 (스펙 §4)
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelTurn {
    pub thought: String,
    pub action: Action,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Action {
    pub tool: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

/// 모델 출력에서 턴 하나를 파싱한다. Err는 모델에 되먹일 영어 피드백 (스펙 §9).
/// 사다리: 그대로 → 펜스 제거 → 첫 JSON 오브젝트 스캔.
/// json_schema 강제가 꺼진 폴백 모드에서도 이 사다리가 동작해야 한다 (스펙 §4)
pub fn parse_turn(text: &str) -> Result<ModelTurn, String> {
    const FORMAT_HINT: &str = r#"Reply with exactly one JSON object: {"thought": "...", "action": {"tool": "...", "args": {...}}}"#;
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("Your reply was empty. {FORMAT_HINT}"));
    }
    if let Ok(turn) = serde_json::from_str::<ModelTurn>(text) {
        return Ok(turn);
    }
    // let 체인 (edition 2024) — 중첩 if let은 clippy::collapsible_if에 걸린다
    if let Some(inner) = strip_fence(text)
        && let Ok(turn) = serde_json::from_str::<ModelTurn>(inner)
    {
        return Ok(turn);
    }
    if let Some(obj) = first_json_object(text) {
        return match serde_json::from_str::<ModelTurn>(obj) {
            Ok(turn) => Ok(turn),
            Err(e) => Err(format!("Your reply was not a valid turn ({e}). {FORMAT_HINT}")),
        };
    }
    Err(format!("Your reply contained no JSON object. {FORMAT_HINT}"))
}

/// ```json ... ``` 펜스 내부를 꺼낸다
fn strip_fence(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("```")?;
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    let end = rest.rfind("```")?;
    Some(rest[..end].trim())
}

/// 문자열/이스케이프를 인지하며 첫 번째 균형 잡힌 {...}를 찾는다.
/// 인덱스는 전부 ASCII 바이트(`{`, `}`, `"`, `\`) 위치라 char 경계 안전.
fn first_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, &b) in text.as_bytes().iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// response_format: json_schema — 의도적으로 얕은 스키마 (스펙 §4).
/// tool만 enum으로 제약하고 args는 자유 오브젝트로 둔다. 툴별 인자 검증은
/// 앱 쪽(serde)에서 하고 위반 시 에러를 되먹인다. strict 플래그는 쓰지 않는다
/// (백엔드가 additionalProperties: false를 요구해 자유 args와 충돌할 수 있음)
pub fn response_format(tool_names: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": "agent_turn",
            "schema": {
                "type": "object",
                "properties": {
                    "thought": {"type": "string"},
                    "action": {
                        "type": "object",
                        "properties": {
                            "tool": {"type": "string", "enum": tool_names},
                            "args": {"type": "object"}
                        },
                        "required": ["tool", "args"]
                    }
                },
                "required": ["thought", "action"]
            }
        }
    })
}
```

주의: `valid_json_with_wrong_shape_is_an_error` 테스트가 통과하려면 `{"answer": 42}`가 1단계(그대로 파싱)에서 실패한 뒤 3단계(오브젝트 스캔)에서 잡혀 `"not a valid turn"` 에러가 나와야 한다 — 에러 문구에 FORMAT_HINT가 포함되므로 "thought" 검사를 만족한다.

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: 에이전트 턴 프로토콜 — 파싱 사다리 + json_schema 빌더"
```

---

### Task 8: 에이전트 시스템 프롬프트 + 디렉터리 트리 주입

**Files:**
- Create: `src/agent/prompt.rs`
- Modify: `src/agent/mod.rs` (`pub mod prompt;` 추가)

**Interfaces:**
- Consumes: `tools::list_files::walk_entries` (Task 5)
- Produces (Task 9가 소비):
  - `pub fn system_prompt(tool_docs: &str, root: &Path) -> String` — 영어, 프로토콜 규칙 + 툴 목록 + few-shot 1개 + 프로젝트 트리. `tool_docs`는 `Registry::docs()` 출력
  - `pub fn project_tree(root: &Path) -> String` — depth 3, 최대 100항목, 초과 시 `[tree truncated]` (스펙 §6 "상한 있음")

- [ ] **Step 1: 실패하는 테스트 작성**

`src/agent/prompt.rs` 생성, 테스트 먼저:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_states_protocol_and_finish_channel() {
        let dir = tempfile::tempdir().unwrap();
        let p = system_prompt("- read_file(path): Read a file.", dir.path());
        assert!(p.contains("\"thought\""), "프로토콜 형태 명시");
        assert!(p.contains("- read_file(path)"), "툴 목록 주입");
        assert!(p.contains("finish"), "답변 채널 명시 (스펙 §4)");
        assert!(p.contains("summary"), "summary가 사용자에게 가는 유일한 채널");
        assert!(p.contains("Example"), "few-shot 예시 (스펙 §4)");
        assert!(p.is_ascii(), "시스템 프롬프트는 영어 (스펙 §4)");
    }

    #[test]
    fn tree_lists_files_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/junk.o"), "").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/target\n").unwrap();
        let tree = project_tree(dir.path());
        assert!(tree.contains("src/main.rs"), "{tree}");
        assert!(!tree.contains("junk.o"), "{tree}");
    }

    #[test]
    fn tree_is_capped_at_100_entries() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..110 {
            std::fs::write(dir.path().join(format!("f{i:03}.txt")), "").unwrap();
        }
        let tree = project_tree(dir.path());
        assert_eq!(tree.lines().count(), 101, "100항목 + 절삭 표시\n{tree}");
        assert_eq!(tree.lines().last().unwrap(), "[tree truncated]");
    }

    #[test]
    fn empty_project_says_no_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(project_tree(dir.path()), "(no files)");
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib agent::prompt 2>&1 | head -10`
Expected: FAIL (컴파일 에러)

- [ ] **Step 3: 구현**

(Step 1에서 작성한 테스트 모듈은 파일에 그대로 유지하고, 아래 구현 코드를 그 위에 추가한다 — 파일 전체 교체 아님)

```rust
use std::path::Path;

use crate::tools::list_files::walk_entries;

/// 트리 주입 상한 (스펙 §6 "상한 있음"). 8K 컨텍스트 예산을 고려해 보수적으로
const TREE_MAX_ENTRIES: usize = 100;
const TREE_DEPTH: usize = 3;

/// 에이전트 시스템 프롬프트 (영어 — 소형 모델의 지시 이행률, 스펙 §4).
/// 매 턴 JSON 하나, 답변 채널은 finish.summary, few-shot 1개 포함
pub fn system_prompt(tool_docs: &str, root: &Path) -> String {
    let tree = project_tree(root);
    format!(
        "You are loco, a coding agent working inside the user's project directory. \
You interact with the project ONLY by calling tools.\n\
\n\
Respond with exactly ONE JSON object per turn and nothing else:\n\
{{\"thought\": \"<one short sentence of reasoning, in English>\", \"action\": {{\"tool\": \"<name>\", \"args\": {{...}}}}}}\n\
\n\
Rules:\n\
- One tool call per turn.\n\
- File paths are relative to the project root. Explore with list_files or grep before reading whole files.\n\
- When you know the answer (or cannot proceed), call `finish`. Its `summary` is the ONLY text shown to the user - put the complete answer there, written in the user's language.\n\
\n\
Tools:\n\
{tool_docs}\n\
- finish(summary): End the task and give `summary` to the user as the final answer.\n\
\n\
Example turn:\n\
{{\"thought\": \"I need to find where the config is loaded.\", \"action\": {{\"tool\": \"grep\", \"args\": {{\"pattern\": \"fn load\", \"path\": \"src\"}}}}}}\n\
\n\
Project files (partial, gitignore respected):\n\
{tree}"
    )
}

/// 프롬프트 주입용 파일 목록. list_files의 워커를 재사용한다 (DRY)
pub fn project_tree(root: &Path) -> String {
    let entries = walk_entries(root, root, Some(TREE_DEPTH), TREE_MAX_ENTRIES + 1);
    if entries.is_empty() {
        return "(no files)".to_string();
    }
    let truncated = entries.len() > TREE_MAX_ENTRIES;
    let mut out: Vec<String> = entries.into_iter().take(TREE_MAX_ENTRIES).collect();
    if truncated {
        out.push("[tree truncated]".to_string());
    }
    out.join("\n")
}
```

주의: `system_prompt`의 `format!` 문자열 안 JSON 중괄호는 전부 `{{` `}}`로 이스케이프되어 있어야 한다 (플레이스홀더는 `{tool_docs}`와 `{tree}` 둘뿐).

`project_tree`가 base로 canonicalize 안 된 루트를 넘겨도 안전하다 — `walk_entries`가 내부에서 root/base 둘 다 canonicalize한다 (Task 5).

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: 에이전트 시스템 프롬프트 + 디렉터리 트리 주입"
```

---

### Task 9: 에이전트 루프 핵심 — 툴 디스패치/finish/max_turns

에이전트 한 사이클(스펙 §3): 히스토리로 요청 조립 → 비스트리밍 chat → `{thought, action}` 파싱 → thought 이벤트 → 툴 실행 → 결과를 `<tool_result>` user 메시지로 → finish 또는 max_turns까지 반복. 이 태스크는 행복 경로만 — 파싱 재시도/length/폴백은 Task 10.

**Files:**
- Modify: `src/agent/mod.rs`

**Interfaces:**
- Consumes: `LlmClient`(Task 2), `Registry`/`ToolCtx`(Task 3·6), `protocol`(Task 7), `prompt`(Task 8), `Config`
- Produces (Task 12·13이 소비):
  - `pub struct Agent<C: LlmClient>` + `Agent::new(client: C, registry: Registry, ctx: ToolCtx, model: String, config: &Config) -> Self`
  - `Agent::initial_history(&self) -> Vec<ChatMessage>` — `[system(시스템 프롬프트)]`
  - `Agent::run(&mut self, history: &mut Vec<ChatMessage>, request: &str, on_event: &mut dyn FnMut(AgentEvent<'_>)) -> Result<AgentOutcome, LlmError>`
  - `pub enum AgentEvent<'a> { Thought(&'a str), Action { tool: &'a str, args: &'a serde_json::Value }, Notice(String) }`
  - `pub enum AgentOutcome { Finished(String), MaxTurns, ParseFailed(String) }`
  - `pub const PARSE_ATTEMPTS: usize = 3;`
  - 병합 시맨틱: `run()`은 히스토리 꼬리가 user면(직전 MaxTurns 실행) 새 요청을 그 메시지에 병합해 role 교대를 보존한다 (스펙 §3)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/agent/mod.rs`를 다음으로 교체 (선언 + 테스트 먼저, 구현은 Step 3):

```rust
pub mod protocol;
pub mod prompt;

use crate::config::Config;
use crate::llm::LlmClient;
use crate::llm::client::LlmError;
use crate::llm::types::{ChatMessage, ChatRequest, ChatResponse};
use crate::tools::{Registry, ToolCtx};
use protocol::{parse_turn, response_format};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{Choice, ResponseMessage};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// 스크립트된 가짜 LLM (스펙 §11 — agent는 LlmClient 트레이트만 의존)
    struct Scripted {
        responses: Mutex<VecDeque<Result<ChatResponse, LlmError>>>,
        requests: Mutex<Vec<ChatRequest>>,
    }

    impl Scripted {
        fn new(responses: Vec<Result<ChatResponse, LlmError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl LlmClient for Scripted {
        async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
            self.requests.lock().unwrap().push(req.clone());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("스크립트에 남은 응답이 없음")
        }
    }

    fn ok_with_reason(text: &str, reason: &str) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: Some(text.into()),
                },
                finish_reason: Some(reason.into()),
            }],
        })
    }

    fn ok(text: &str) -> Result<ChatResponse, LlmError> {
        ok_with_reason(text, "stop")
    }

    fn turn(tool: &str, args: serde_json::Value) -> String {
        serde_json::json!({"thought": "t", "action": {"tool": tool, "args": args}}).to_string()
    }

    fn finish(summary: &str) -> String {
        turn("finish", serde_json::json!({"summary": summary}))
    }

    fn make_agent(script: &Scripted, root: std::path::PathBuf, max_turns: usize) -> Agent<&Scripted> {
        let config = Config { max_turns, ..Default::default() };
        Agent::new(script, Registry::read_only(), ToolCtx { root }, "test-model".into(), &config)
    }

    async fn run_quiet(
        agent: &mut Agent<&Scripted>,
        history: &mut Vec<ChatMessage>,
        request: &str,
    ) -> Result<AgentOutcome, LlmError> {
        agent.run(history, request, &mut |_| {}).await
    }

    #[tokio::test]
    async fn finish_returns_summary_and_sends_wellformed_request() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(&finish("답변입니다"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "질문").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(s) if s == "답변입니다"));

        let reqs = script.requests.lock().unwrap();
        assert_eq!(reqs.len(), 1);
        assert!(!reqs[0].stream, "에이전트 턴은 비스트리밍 (스펙 §3)");
        assert!(reqs[0].response_format.is_some(), "json_schema 강제 (스펙 §4)");
        assert_eq!(reqs[0].messages[0].role, "system");
        assert_eq!(reqs[0].messages.last().unwrap().content, "질문");
        // 스키마 enum에 finish 포함
        let rf = reqs[0].response_format.as_ref().unwrap();
        let enum_names = &rf["json_schema"]["schema"]["properties"]["action"]["properties"]["tool"]["enum"];
        assert!(enum_names.as_array().unwrap().contains(&serde_json::json!("finish")));
    }

    #[tokio::test]
    async fn tool_result_is_wrapped_user_message_and_events_fire() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "세계").unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "hello.txt"}))),
            ok(&finish("done")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let mut events: Vec<String> = Vec::new();
        let outcome = agent
            .run(&mut history, "hello.txt 읽어줘", &mut |ev| {
                events.push(match ev {
                    AgentEvent::Thought(t) => format!("thought:{t}"),
                    AgentEvent::Action { tool, .. } => format!("action:{tool}"),
                    AgentEvent::Notice(n) => format!("notice:{n}"),
                });
            })
            .await
            .unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));

        let wrapped = history.iter().find(|m| m.content.contains("<tool_result")).unwrap();
        assert_eq!(wrapped.role, "user", "role:'tool' 금지 (스펙 §3)");
        assert!(wrapped.content.contains("<tool_result name=\"read_file\">"));
        assert!(wrapped.content.contains("세계"));
        assert_eq!(events[0], "thought:t");
        assert_eq!(events[1], "action:read_file");
        // 히스토리 role 교대: system, user, assistant, user(tool_result), assistant(finish)
        let roles: Vec<&str> = history.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "user", "assistant"]);
    }

    #[tokio::test]
    async fn tool_error_is_fed_back_not_crashed() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "no-such.txt"}))),
            ok(&finish("없네요")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "읽어").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let fed = history.iter().find(|m| m.content.contains("Error: not found")).unwrap();
        assert_eq!(fed.role, "user", "툴 에러는 모델에 되먹이는 데이터 (스펙 §9)");
    }

    #[tokio::test]
    async fn unknown_tool_is_fed_back() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("teleport", serde_json::json!({}))),
            ok(&finish("ok")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(history.iter().any(|m| m.content.contains("Error: unknown tool: teleport")));
    }

    #[tokio::test]
    async fn max_turns_returns_control() {
        let dir = tempfile::tempdir().unwrap();
        let list = || ok(&turn("list_files", serde_json::json!({})));
        let script = Scripted::new(vec![list(), list(), list()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 2);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::MaxTurns));
        assert_eq!(script.requests.lock().unwrap().len(), 2, "max_turns=2면 호출도 2회");
    }

    #[tokio::test]
    async fn request_after_max_turns_merges_into_trailing_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("list_files", serde_json::json!({}))),
            ok(&finish("ok")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 1);
        let mut history = agent.initial_history();
        let first = run_quiet(&mut agent, &mut history, "첫 요청").await.unwrap();
        assert!(matches!(first, AgentOutcome::MaxTurns));
        let second = run_quiet(&mut agent, &mut history, "이어서").await.unwrap();
        assert!(matches!(second, AgentOutcome::Finished(_)));
        // role 교대 보존 (스펙 §3) — 연속 user 금지
        for w in history.windows(2) {
            assert!(!(w[0].role == "user" && w[1].role == "user"), "연속 user 메시지");
        }
        let merged = history.iter().find(|m| m.content.contains("이어서")).unwrap();
        assert!(merged.content.contains("</tool_result>"), "직전 툴 결과 메시지에 병합");
    }

    #[tokio::test]
    async fn finish_without_summary_gets_feedback() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("finish", serde_json::json!({}))),
            ok(&finish("이제 됐다")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(s) if s == "이제 됐다"));
        assert!(history.iter().any(|m| m.content.contains("`summary`")));
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib agent 2>&1 | head -10`
Expected: FAIL (컴파일 에러 — `Agent` 미정의)

- [ ] **Step 3: 구현**

`src/agent/mod.rs`의 테스트 모듈 위에 추가:

```rust
/// run() 진행 상황 알림. UI가 렌더링을 담당한다 (agent는 출력하지 않음 — 테스트 용이성)
pub enum AgentEvent<'a> {
    /// 매 턴 모델의 사고 과정 — 사용자에게 표시 (스펙 §3-4)
    Thought(&'a str),
    /// 툴 실행 직전 알림 (스펙 §5 "→ read_file src/main.rs")
    Action {
        tool: &'a str,
        args: &'a serde_json::Value,
    },
    /// 재시도/폴백 등 진행 메시지 (한국어, 그대로 표시)
    Notice(String),
}

// Debug는 테스트의 unwrap_err()가 요구한다 (Result<AgentOutcome, _>)
#[derive(Debug)]
pub enum AgentOutcome {
    /// finish.summary — 사용자에게 전달되는 답변 (스펙 §4)
    Finished(String),
    /// 최대 턴 도달 (스펙 §3-7) — -p 종료 코드 2
    MaxTurns,
    /// 파싱 총 3회 실패 — 마지막 모델 원문 (스펙 §9), -p 종료 코드 1
    ParseFailed(String),
}

/// 턴당 파싱 총 시도 횟수 (초기 1 + 재시도 2, 스펙 §9). max_turns에 계상 안 됨
pub const PARSE_ATTEMPTS: usize = 3;

pub struct Agent<C: LlmClient> {
    client: C,
    registry: Registry,
    ctx: ToolCtx,
    model: String,
    temperature: f32,
    max_output_tokens: u32,
    max_turns: usize,
    /// json_schema 폴백 상태 — 400을 만나면 끈다 (스펙 §4). Task 10에서 사용
    use_json_schema: bool,
    /// system role 폴백 상태 — 400을 만나면 첫 user에 병합 (스펙 §3).
    /// Task 9 시점엔 읽는 곳이 없어 dead_code로 clippy 게이트가 깨진다 — Task 10에서 attribute 제거
    #[allow(dead_code)]
    inline_system: bool,
}

impl<C: LlmClient> Agent<C> {
    pub fn new(
        client: C,
        registry: Registry,
        ctx: ToolCtx,
        model: String,
        config: &Config,
    ) -> Self {
        Self {
            client,
            registry,
            ctx,
            model,
            temperature: config.temperature,
            max_output_tokens: config.max_output_tokens as u32,
            max_turns: config.max_turns,
            use_json_schema: true,
            inline_system: false,
        }
    }

    /// 시스템 프롬프트(툴 목록 + 프로젝트 트리)만 담긴 초기 히스토리
    pub fn initial_history(&self) -> Vec<ChatMessage> {
        vec![ChatMessage::system(prompt::system_prompt(
            &self.registry.docs(),
            &self.ctx.root,
        ))]
    }

    fn schema_tool_names(&self) -> Vec<&'static str> {
        let mut names = self.registry.names();
        names.push("finish");
        names
    }

    fn build_request(&self, history: &[ChatMessage]) -> ChatRequest {
        ChatRequest {
            model: self.model.clone(),
            messages: history.to_vec(),
            temperature: self.temperature,
            max_tokens: Some(self.max_output_tokens),
            stream: false, // 에이전트 턴은 비스트리밍 (스펙 §3)
            response_format: self
                .use_json_schema
                .then(|| response_format(&self.schema_tool_names())),
        }
    }

    pub async fn run(
        &mut self,
        history: &mut Vec<ChatMessage>,
        request: &str,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) -> Result<AgentOutcome, LlmError> {
        // 직전 실행이 MaxTurns로 끝났으면 히스토리 꼬리가 user(tool_result)다.
        // user를 연속으로 쌓으면 role 교대를 요구하는 템플릿이 깨지므로 병합한다 (스펙 §3)
        match history.last_mut() {
            Some(m) if m.role == "user" => m.content = format!("{}\n\n{}", m.content, request),
            _ => history.push(ChatMessage::user(request)),
        }
        let mut turns = 0;
        while turns < self.max_turns {
            let resp = self.client.chat(&self.build_request(history)).await?;
            let text = resp.text().to_string();
            let turn = match parse_turn(&text) {
                Ok(t) => t,
                Err(_) => {
                    // Task 10에서 3회 재시도로 확장
                    history.push(ChatMessage::assistant(text.clone()));
                    return Ok(AgentOutcome::ParseFailed(text));
                }
            };
            history.push(ChatMessage::assistant(text));
            on_event(AgentEvent::Thought(&turn.thought));

            if turn.action.tool == "finish" {
                match turn.action.args.get("summary").and_then(|v| v.as_str()) {
                    Some(s) => return Ok(AgentOutcome::Finished(s.to_string())),
                    None => {
                        history.push(tool_result_message(
                            "finish",
                            "Error: finish requires a string `summary` argument containing the final answer.",
                        ));
                        turns += 1;
                        continue;
                    }
                }
            }

            on_event(AgentEvent::Action {
                tool: &turn.action.tool,
                args: &turn.action.args,
            });
            // 툴 에러도 모델에 되먹이는 데이터 — 루프는 계속 (스펙 §9)
            let body = match self.registry.dispatch(&turn.action.tool, &turn.action.args, &self.ctx) {
                Ok(s) if s.is_empty() => "(no output)".to_string(),
                Ok(s) => s,
                Err(e) => format!("Error: {e}"),
            };
            history.push(tool_result_message(&turn.action.tool, &body));
            turns += 1;
        }
        Ok(AgentOutcome::MaxTurns)
    }
}

/// 툴 결과를 role:"user" 메시지로 감싼다 — role:"tool"은 Gemma 챗템플릿에서
/// 깨지므로 금지 (스펙 §3). 구분자는 <tool_result name="...">
fn tool_result_message(tool: &str, body: &str) -> ChatMessage {
    ChatMessage::user(format!("<tool_result name=\"{tool}\">\n{body}\n</tool_result>"))
}
```

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: 에이전트 루프 핵심 — 툴 디스패치/finish/max_turns"
```

---

### Task 10: 에이전트 루프 에러 경로 — 파싱 재시도, length 교정, 400 폴백 사다리

**Files:**
- Modify: `src/agent/mod.rs`

**Interfaces:**
- Consumes: Task 9의 `Agent`
- Produces: 동일 시그니처의 `run()` — 동작 확장:
  - 파싱 실패 시 에러를 되먹여 턴당 총 `PARSE_ATTEMPTS`(3)회 시도, 소진 시 `ParseFailed` (max_turns에 계상 안 됨 — 스펙 §9)
  - `finish_reason == "length"`(턴의 첫 응답)면 재시도가 아니라 "더 짧게" 교정 메시지 (스펙 §9). length 교정은 턴을 소모한다 — length 반복은 감지 못 하는 사각지대라 max_turns가 상한이 되게 (스펙 §3). 파싱 재시도 중의 length는 단순화를 위해 파싱 실패로 취급 (잘린 JSON은 어차피 파싱 실패 → 결국 수렴)
  - 400 폴백 사다리 (블라인드 — 원인 판별이 서버마다 달라 순서대로 시도): 첫 400 → `use_json_schema = false` 재요청, 둘째 400 → `inline_system = true`(시스템 프롬프트를 첫 user 메시지 앞에 병합) 재요청, 셋째 400 → 에러 전파. 플래그는 Agent 수명 동안 유지 (세션 내 재협상 없음)
  - 예외: body에 "context"가 들어간 400은 컨텍스트 초과로 보고 사다리를 타지 않고 즉시 전파한다 — M2는 히스토리 절삭이 없어 긴 세션에서 실제로 발생하며, 사다리를 타면 use_json_schema가 세션 내내 꺼지는 오분류가 된다

- [ ] **Step 1: 실패하는 테스트 작성**

`src/agent/mod.rs` 테스트 모듈에 추가:

```rust
    fn api_400() -> Result<ChatResponse, LlmError> {
        Err(LlmError::Api { status: 400, body: "unsupported".into() })
    }

    #[tokio::test]
    async fn parse_failure_is_fed_back_then_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok("JSON 아님"), ok(&finish("복구"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(s) if s == "복구"));
        // 되먹임: assistant(원문) + user(형식 힌트 피드백)가 히스토리에 남는다
        assert!(history.iter().any(|m| m.role == "assistant" && m.content == "JSON 아님"));
        assert!(history.iter().any(|m| m.role == "user" && m.content.contains("JSON object")));
        assert_eq!(script.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn parse_failure_three_times_returns_raw_text() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok("쓰레기1"), ok("쓰레기2"), ok("쓰레기3")]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::ParseFailed(raw) if raw == "쓰레기3"));
        assert_eq!(script.requests.lock().unwrap().len(), 3, "총 3회 시도 (스펙 §9)");
    }

    #[tokio::test]
    async fn length_gets_correction_not_a_retry() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok_with_reason("잘린 응답...", "length"),
            ok(&finish("짧게 답함")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        // 파싱 재시도 피드백이 아니라 "잘렸으니 더 짧게" 교정 (스펙 §9)
        assert!(history.iter().any(|m| m.role == "user" && m.content.contains("cut off")));
    }

    #[tokio::test]
    async fn length_consumes_a_turn_so_it_cannot_loop_forever() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok_with_reason("잘림", "length")]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 1);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::MaxTurns), "max_turns가 length 반복의 상한 (스펙 §3)");
    }

    #[tokio::test]
    async fn first_400_disables_json_schema_and_retries() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![api_400(), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let mut notices = Vec::new();
        let outcome = agent
            .run(&mut history, "x", &mut |ev| {
                if let AgentEvent::Notice(n) = ev {
                    notices.push(n);
                }
            })
            .await
            .unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let reqs = script.requests.lock().unwrap();
        assert!(reqs[0].response_format.is_some());
        assert!(reqs[1].response_format.is_none(), "폴백: json_schema 끔 (스펙 §4)");
        assert!(!notices.is_empty(), "폴백 알림 이벤트");
    }

    #[tokio::test]
    async fn second_400_inlines_system_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![api_400(), api_400(), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "질문").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let reqs = script.requests.lock().unwrap();
        let third = &reqs[2].messages;
        assert!(third.iter().all(|m| m.role != "system"), "system role 제거 (스펙 §3 폴백)");
        assert_eq!(third[0].role, "user");
        assert!(third[0].content.contains("You are loco"), "시스템 프롬프트가 첫 user 앞에 병합");
        assert!(third[0].content.contains("질문"), "원래 사용자 요청 보존");
    }

    #[tokio::test]
    async fn third_400_propagates_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![api_400(), api_400(), api_400()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let err = run_quiet(&mut agent, &mut history, "x").await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 400, .. }));
    }

    #[tokio::test]
    async fn context_overflow_400_propagates_without_touching_fallback_flags() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![Err(LlmError::Api {
            status: 400,
            body: "the request exceeds the available context size".into(),
        })]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let err = run_quiet(&mut agent, &mut history, "x").await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 400, .. }));
        assert_eq!(
            script.requests.lock().unwrap().len(),
            1,
            "폴백 사다리를 타지 않고 즉시 전파 (json_schema 유지)"
        );
    }

    #[tokio::test]
    async fn empty_response_counts_as_parse_failure() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(""), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(history.iter().any(|m| m.content.contains("empty")));
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib agent 2>&1 | tail -20`
Expected: 파싱 재시도·length·400 폴백 사다리 테스트 FAIL. 단 `third_400_propagates_the_error`와 `context_overflow_400_propagates_without_touching_fallback_flags`는 Task 9 구현(모든 에러 즉시 전파)으로도 이미 통과한다 — 회귀 가드로 유지

- [ ] **Step 3: 구현**

`run()`을 다음으로 교체하고 `chat_with_fallback`/`inline_system_into_first_user`를 추가:

```rust
    pub async fn run(
        &mut self,
        history: &mut Vec<ChatMessage>,
        request: &str,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) -> Result<AgentOutcome, LlmError> {
        // 직전 실행이 MaxTurns로 끝났으면 히스토리 꼬리가 user(tool_result)다.
        // user를 연속으로 쌓으면 role 교대를 요구하는 템플릿이 깨지므로 병합한다 (스펙 §3)
        match history.last_mut() {
            Some(m) if m.role == "user" => m.content = format!("{}\n\n{}", m.content, request),
            _ => history.push(ChatMessage::user(request)),
        }
        let mut turns = 0;
        while turns < self.max_turns {
            let resp = self.chat_with_fallback(history, on_event).await?;

            // 출력 잘림은 파싱 실패와 구분해 교정한다 (스펙 §9). 같은 요청 재시도는
            // 같은 지점에서 다시 잘리므로 "더 짧게"를 지시. 턴을 소모해 max_turns가
            // length 반복의 상한이 되게 한다 (스펙 §3 사각지대)
            if resp.finish_reason() == Some("length") {
                history.push(ChatMessage::assistant(resp.text()));
                history.push(ChatMessage::user(
                    "Your previous response was cut off by the output token limit. \
                     Respond again with exactly one, much shorter JSON turn.",
                ));
                on_event(AgentEvent::Notice("(응답이 잘림 — 더 짧게 다시 요청)".to_string()));
                turns += 1;
                continue;
            }

            // 파싱 실패는 에러를 되먹여 턴당 총 PARSE_ATTEMPTS회 시도 (스펙 §9).
            // 되먹임(assistant 원문 + user 피드백)은 히스토리에 남는다 — 모델이
            // 자기 실패를 문맥으로 보는 것이 의도. max_turns에는 계상하지 않는다
            let mut text = resp.text().to_string();
            let mut attempts = 1;
            let turn = loop {
                match parse_turn(&text) {
                    Ok(t) => break t,
                    Err(feedback) => {
                        // 빈 assistant content를 거부하는 템플릿이 있어 자리표시자로 대체
                        history.push(ChatMessage::assistant(if text.is_empty() {
                            "(empty)".to_string()
                        } else {
                            text.clone()
                        }));
                        if attempts >= PARSE_ATTEMPTS {
                            return Ok(AgentOutcome::ParseFailed(text));
                        }
                        attempts += 1;
                        on_event(AgentEvent::Notice(format!(
                            "(응답 파싱 실패 — 재시도 {attempts}/{PARSE_ATTEMPTS})"
                        )));
                        history.push(ChatMessage::user(feedback));
                        let retry = self.chat_with_fallback(history, on_event).await?;
                        text = retry.text().to_string();
                    }
                }
            };
            history.push(ChatMessage::assistant(text));
            on_event(AgentEvent::Thought(&turn.thought));

            if turn.action.tool == "finish" {
                match turn.action.args.get("summary").and_then(|v| v.as_str()) {
                    Some(s) => return Ok(AgentOutcome::Finished(s.to_string())),
                    None => {
                        history.push(tool_result_message(
                            "finish",
                            "Error: finish requires a string `summary` argument containing the final answer.",
                        ));
                        turns += 1;
                        continue;
                    }
                }
            }

            on_event(AgentEvent::Action {
                tool: &turn.action.tool,
                args: &turn.action.args,
            });
            let body = match self.registry.dispatch(&turn.action.tool, &turn.action.args, &self.ctx) {
                Ok(s) if s.is_empty() => "(no output)".to_string(),
                Ok(s) => s,
                Err(e) => format!("Error: {e}"),
            };
            history.push(tool_result_message(&turn.action.tool, &body));
            turns += 1;
        }
        Ok(AgentOutcome::MaxTurns)
    }

    /// 400 폴백 사다리 (스펙 §3·§4): 서버가 무엇을 거부했는지 표준적으로 알 수 없어
    /// 순서대로 하나씩 끄며 재시도한다. 두 플래그가 다 꺼진 뒤의 400은 그대로 전파
    async fn chat_with_fallback(
        &mut self,
        history: &[ChatMessage],
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) -> Result<ChatResponse, LlmError> {
        loop {
            let req = self.build_request(history);
            match self.client.chat(&req).await {
                // 컨텍스트 초과 400은 폴백 대상이 아니다 — 사다리를 타면 use_json_schema가
                // 세션 내내 꺼지는 오분류가 된다 (M2는 절삭이 없어 긴 세션에서 실제 발생).
                // 휴리스틱 매치 시 안내와 함께 즉시 전파. 자동 절삭·재시도는 M3 (스펙 §9)
                Err(LlmError::Api { status: 400, body }) if looks_like_context_overflow(&body) => {
                    on_event(AgentEvent::Notice(
                        "(컨텍스트 초과로 보입니다 — 히스토리를 비우거나(REPL: /clear) context_tokens 설정과 서버 로드 설정을 확인하세요)".to_string(),
                    ));
                    return Err(LlmError::Api { status: 400, body });
                }
                Err(LlmError::Api { status: 400, .. }) if self.use_json_schema => {
                    self.use_json_schema = false;
                    on_event(AgentEvent::Notice(
                        "(서버가 요청을 거부 — response_format 없이 재시도)".to_string(),
                    ));
                }
                Err(LlmError::Api { status: 400, .. }) if !self.inline_system => {
                    self.inline_system = true;
                    on_event(AgentEvent::Notice(
                        "(서버가 요청을 거부 — 시스템 프롬프트를 user 메시지로 병합해 재시도)".to_string(),
                    ));
                }
                other => return other,
            }
        }
    }
```

`build_request`를 교체 (inline_system 반영 — 이제 필드가 읽히므로 구조체 `inline_system` 필드의 `#[allow(dead_code)]` attribute와 그 주석 줄을 함께 제거한다):

```rust
    fn build_request(&self, history: &[ChatMessage]) -> ChatRequest {
        // Gemma 순정 템플릿엔 system role이 없다 — 폴백 모드에선 시스템 프롬프트를
        // 첫 user 메시지 앞에 병합한다 (스펙 §3). history 자체는 건드리지 않는다
        let messages = if self.inline_system {
            inline_system_into_first_user(history)
        } else {
            history.to_vec()
        };
        ChatRequest {
            model: self.model.clone(),
            messages,
            temperature: self.temperature,
            max_tokens: Some(self.max_output_tokens),
            stream: false,
            response_format: self
                .use_json_schema
                .then(|| response_format(&self.schema_tool_names())),
        }
    }
```

모듈 레벨 함수 추가:

```rust
/// 서버 컨텍스트 초과 400 감지 휴리스틱 — LM Studio/llama.cpp/vLLM 모두 에러 메시지에
/// "context"가 들어간다. 완전하지 않은 최선 노력이며, 자동 절삭 대응은 M3 (스펙 §9)
fn looks_like_context_overflow(body: &str) -> bool {
    body.to_lowercase().contains("context")
}

/// system 메시지를 제거하고 그 내용을 첫 user 메시지 앞에 붙인다 (스펙 §3 폴백)
fn inline_system_into_first_user(history: &[ChatMessage]) -> Vec<ChatMessage> {
    let Some((first, rest)) = history.split_first() else {
        return Vec::new();
    };
    if first.role != "system" {
        return history.to_vec();
    }
    let mut msgs: Vec<ChatMessage> = rest.to_vec();
    match msgs.iter_mut().find(|m| m.role == "user") {
        Some(u) => u.content = format!("{}\n\n{}", first.content, u.content),
        None => msgs.insert(0, ChatMessage::user(first.content.clone())),
    }
    msgs
}
```

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS (Task 9 테스트 포함 — 행복 경로 회귀 없음), clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: 에이전트 루프 에러 경로 — 파싱 재시도/length 교정/400 폴백"
```

---

### Task 11: 스피너와 액션 알림 렌더링 (ui::status)

**Files:**
- Create: `src/ui/status.rs`
- Modify: `src/ui/mod.rs` (`pub mod status;` 추가)

**Interfaces:**
- Consumes: 없음 (tokio, serde_json만)
- Produces (Task 12·13이 소비):
  - `pub struct Spinner` — `Spinner::start(label: &str) -> Spinner`, `spinner.stop()`, `spinner.is_active() -> bool`, Drop 시 자동 stop. **stderr**에 그리며, `stdout`이 TTY가 아니면 아무것도 그리지 않는다 (스펙 §7). 프레임은 ASCII `|/-\` — 한국어 Windows 콘솔(CP949)에서도 안전
  - `pub fn format_action(tool: &str, args: &serde_json::Value) -> String` — `"→ read_file src/main.rs"` 식 한 줄 (스펙 §5)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/ui/status.rs` 생성, 테스트 먼저:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_lines_are_compact() {
        assert_eq!(
            format_action("read_file", &serde_json::json!({"path": "src/main.rs"})),
            "→ read_file src/main.rs"
        );
        assert_eq!(
            format_action("list_files", &serde_json::json!({})),
            "→ list_files ."
        );
        assert_eq!(
            format_action("grep", &serde_json::json!({"pattern": "fn load", "path": "src"})),
            "→ grep \"fn load\" src"
        );
        assert_eq!(
            format_action("grep", &serde_json::json!({"pattern": "x"})),
            "→ grep \"x\""
        );
        // 모르는 툴은 인자 원문 (M3에서 툴 늘어나도 동작)
        assert_eq!(
            format_action("run_command", &serde_json::json!({"command": "ls"})),
            "→ run_command {\"command\":\"ls\"}"
        );
    }

    #[tokio::test]
    async fn spinner_activity_follows_stdout_tty() {
        // libtest의 출력 캡처는 매크로 수준이라 fd는 그대로다 — 터미널에서 직접
        // cargo test를 치면 TTY일 수 있으므로, 절대값 대신 is_terminal()과의 일치를 검증
        use std::io::IsTerminal;
        let mut s = Spinner::start("생각 중");
        assert_eq!(s.is_active(), std::io::stdout().is_terminal());
        s.stop();
        assert!(!s.is_active(), "stop 후에는 항상 비활성");
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib ui::status 2>&1 | head -10`
Expected: FAIL (컴파일 에러)

- [ ] **Step 3: 구현**

(Step 1에서 작성한 테스트 모듈은 파일에 그대로 유지하고, 아래 구현 코드를 그 위에 추가한다 — 파일 전체 교체 아님)

```rust
use std::io::{IsTerminal, Write};

/// 에이전트 턴 대기 표시 (스펙 §3 — 구조화 출력은 스트리밍 불가라 스피너).
/// stderr에 그린다. stdout이 TTY가 아니면(-p 파이프 등) 아무것도 그리지 않는다 (스펙 §7)
pub struct Spinner {
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Spinner {
    pub fn start(label: &str) -> Self {
        if !std::io::stdout().is_terminal() {
            return Self { task: None };
        }
        let label = label.to_string();
        let task = tokio::spawn(async move {
            // ASCII 프레임 — 한국어 Windows 콘솔(CP949)에서도 안 깨진다
            const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
            for i in 0.. {
                eprint!("\r{} {label}", FRAMES[i % FRAMES.len()]);
                let _ = std::io::stderr().flush();
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            }
        });
        Self { task: Some(task) }
    }

    pub fn is_active(&self) -> bool {
        self.task.is_some()
    }

    pub fn stop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
            // 스피너 줄을 공백으로 덮어 지운다 (ANSI 없이 — 레거시 콘솔 호환)
            eprint!("\r{:60}\r", "");
            let _ = std::io::stderr().flush();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 읽기 툴 자동 실행 알림 한 줄 (스펙 §5: "→ read_file src/main.rs")
pub fn format_action(tool: &str, args: &serde_json::Value) -> String {
    let detail = match tool {
        "read_file" | "list_files" => args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".")
            .to_string(),
        "grep" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => format!("{pattern:?} {p}"),
                None => format!("{pattern:?}"),
            }
        }
        _ => args.to_string(),
    };
    format!("→ {tool} {detail}")
}
```

`src/ui/mod.rs`에 `pub mod status;` 추가.

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "feat: 스피너와 액션 알림 렌더링"
```

---

### Task 12: REPL 에이전트 통합 — 기본 입력→에이전트, /chat 분리, Ctrl+C 취소

M2부터 REPL의 기본 입력은 에이전트 루프로 간다. M1 스트리밍 채팅은 `/chat <메시지>`로 유지 (스펙 §7). M1 이연 항목 2건도 여기서 해소: `/`로 시작하는 채팅은 `/chat /foo`로 보낼 수 있고, Ctrl+C가 진행 중인 에이전트 실행/스트리밍을 취소한다.

**Files:**
- Modify: `src/ui/repl.rs`, `src/main.rs`(최소 수정 — 이름 변경 반영), `Cargo.toml`(tokio `signal` 피처)

**Interfaces:**
- Consumes: `Agent`/`AgentEvent`/`AgentOutcome`(Task 9·10), `Registry::read_only`(Task 6), `Spinner`/`format_action`(Task 11)
- Produces:
  - `pub const CHAT_SYSTEM_PROMPT: &str` — 기존 `SYSTEM_PROMPT`의 새 이름 (/chat 전용)
  - `Input::Agent(String)`(기본 입력), `Input::Chat(String)`(`/chat <msg>`) — 나머지 variant 유지
  - `run_repl(client: &OpenAiClient, config: &Config, model: &str)` — 시그니처 동일, 내부에서 `Agent<&OpenAiClient>` 구성
  - 히스토리 설계: **에이전트 히스토리와 /chat 히스토리는 분리** (JSON 프로토콜 히스토리에 자유 산문이 섞이면 소형 모델이 형식을 잃는다). `/clear`는 둘 다 초기화
  - 취소/실패 시맨틱: Ctrl+C·LlmError·ParseFailed → 에이전트 히스토리를 요청 이전 길이로 truncate하고 꼬리 메시지를 원복 (병합 경로 — 직전 MaxTurns — 에서는 길이가 안 변하므로 원복이 필수). MaxTurns는 히스토리 유지 (실제 진행이 있었음)

- [ ] **Step 1: tokio signal 피처 추가**

```bash
cargo add tokio --features macros,rt-multi-thread,time,signal
```

참고: `tokio::signal::ctrl_c()`를 한 번이라도 폴링하면 프로세스 기본 SIGINT 종료가 비활성화되지만, rustyline 프롬프트는 raw mode에서 ^C를 키 입력(`ReadlineError::Interrupted`)으로 직접 받으므로 프롬프트에서의 Ctrl+C 동작(REPL 종료)은 영향 없다. `-p` 경로는 ctrl_c를 폴링하지 않으므로 기본 종료가 유지된다. select 대기 중이 아닐 때(결과 출력 직후 등) 눌린 Ctrl+C는 어디에도 전달되지 않고 무시된다 — 알려진 M2 동작.

- [ ] **Step 2: parse_input 테스트 갱신 (실패 확인)**

`src/ui/repl.rs` 테스트를 다음으로 교체:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_goes_to_the_agent() {
        assert_eq!(parse_input("config 어디서 읽어?"), Input::Agent("config 어디서 읽어?".to_string()));
    }

    #[test]
    fn chat_command_bypasses_the_agent() {
        assert_eq!(parse_input("/chat 안녕"), Input::Chat("안녕".to_string()));
        // 슬래시로 시작하는 채팅도 /chat으로 보낼 수 있다 (M1 이연 항목 해소)
        assert_eq!(parse_input("/chat /help가 뭐야"), Input::Chat("/help가 뭐야".to_string()));
    }

    #[test]
    fn bare_chat_is_unknown() {
        assert_eq!(parse_input("/chat"), Input::Unknown("chat".to_string()));
        assert_eq!(parse_input("/chat   "), Input::Unknown("chat".to_string()));
    }

    #[test]
    fn slash_commands_parse() {
        assert_eq!(parse_input("/help"), Input::Help);
        assert_eq!(parse_input("/clear"), Input::Clear);
        assert_eq!(parse_input("/config"), Input::Config);
        assert_eq!(parse_input("/quit"), Input::Quit);
        assert_eq!(parse_input("/exit"), Input::Quit);
        assert_eq!(parse_input(" /help "), Input::Help);
    }

    #[test]
    fn unknown_slash_command() {
        assert_eq!(parse_input("/foo"), Input::Unknown("foo".to_string()));
    }
}
```

Run: `cargo test --lib ui::repl 2>&1 | head -10`
Expected: FAIL (`Input::Agent` 없음)

- [ ] **Step 3: 구현 — repl.rs 재작성**

`src/ui/repl.rs`의 비테스트 부분을 다음으로 교체:

```rust
use std::cell::RefCell;
use std::io::Write;

use rustyline::error::ReadlineError;

use crate::agent::{Agent, AgentEvent, AgentOutcome, PARSE_ATTEMPTS};
use crate::config::Config;
use crate::llm::client::OpenAiClient;
use crate::llm::types::{ChatMessage, ChatRequest};
use crate::tools::{Registry, ToolCtx};
use crate::ui::status::{format_action, Spinner};

/// /chat 경로(M1 스트리밍 채팅) 전용 시스템 프롬프트
pub const CHAT_SYSTEM_PROMPT: &str = "You are loco, a concise coding assistant running on a local model. \
Answer briefly and accurately. Reply in the user's language.";

#[derive(Debug, PartialEq)]
pub enum Input {
    /// 기본 입력 — 에이전트 루프로 (스펙 §7, M2부터)
    Agent(String),
    /// /chat <메시지> — M1 스트리밍 채팅 경로 (빠른 질문용)
    Chat(String),
    Help,
    Clear,
    Config,
    Quit,
    Unknown(String),
}

pub fn parse_input(line: &str) -> Input {
    let line = line.trim();
    if let Some(msg) = line.strip_prefix("/chat ") {
        let msg = msg.trim();
        if !msg.is_empty() {
            return Input::Chat(msg.to_string());
        }
    }
    if let Some(cmd) = line.strip_prefix('/') {
        return match cmd.trim() {
            "help" => Input::Help,
            "clear" => Input::Clear,
            "config" => Input::Config,
            "quit" | "exit" => Input::Quit,
            other => Input::Unknown(other.to_string()),
        };
    }
    Input::Agent(line.to_string())
}

fn print_help() {
    println!("입력한 내용은 에이전트가 툴(read_file/list_files/grep)로 조사해 답합니다.");
    println!("명령어:");
    println!("  /chat <메시지>  에이전트 없이 모델과 바로 대화 (스트리밍)");
    println!("  /clear          에이전트·채팅 히스토리 초기화");
    println!("  /config         현재 설정 표시");
    println!("  /quit           종료");
    println!("실행 중 Ctrl+C 는 진행 중인 요청을 취소합니다.");
}

fn print_config(config: &Config, model: &str) {
    println!("base_url: {}", config.base_url);
    println!("model: {model}");
    println!("temperature: {}", config.temperature);
    println!("context_tokens: {}", config.context_tokens);
    println!("max_output_tokens: {}", config.max_output_tokens);
    println!("max_turns: {}", config.max_turns);
    println!("command_timeout_secs: {}", config.command_timeout_secs);
    println!(
        "api_key: {}",
        if config.api_key.is_some() { "(설정됨)" } else { "(없음)" }
    );
    if let Some(p) = Config::default_global_path() {
        println!("전역 설정 파일: {}", p.display());
    }
}

pub async fn run_repl(
    client: &OpenAiClient,
    config: &Config,
    model: &str,
) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let mut agent = Agent::new(
        client,
        Registry::read_only(),
        ToolCtx { root },
        model.to_string(),
        config,
    );
    let mut agent_history = agent.initial_history();
    let mut chat_history = vec![ChatMessage::system(CHAT_SYSTEM_PROMPT)];
    let mut rl = rustyline::DefaultEditor::new()?;
    println!("loco — 로컬 모델 코딩 에이전트 (모델: {model}, /help 참고)");

    loop {
        let line = match rl.readline("loco> ") {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => return Err(e.into()),
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let _ = rl.add_history_entry(line);

        match parse_input(line) {
            Input::Quit => break,
            Input::Help => print_help(),
            Input::Config => print_config(config, model),
            Input::Unknown(cmd) => println!("알 수 없는 명령: /{cmd} — /help 참고"),
            Input::Clear => {
                // 에이전트/채팅 히스토리는 분리 운영 — 둘 다 초기화
                agent_history = agent.initial_history();
                chat_history.truncate(1);
                println!("(히스토리 초기화)");
            }
            Input::Chat(text) => {
                run_chat_turn(client, config, model, &mut chat_history, text).await;
            }
            Input::Agent(text) => {
                run_agent_turn(&mut agent, &mut agent_history, config, &text).await;
            }
        }
    }
    println!("안녕히 가세요.");
    Ok(())
}

/// M1 스트리밍 채팅 경로 (/chat). Ctrl+C로 취소 가능
async fn run_chat_turn(
    client: &OpenAiClient,
    config: &Config,
    model: &str,
    history: &mut Vec<ChatMessage>,
    text: String,
) {
    history.push(ChatMessage::user(text));
    let req = ChatRequest {
        model: model.to_string(),
        messages: history.clone(),
        temperature: config.temperature,
        max_tokens: Some(config.max_output_tokens as u32),
        stream: true,
        response_format: None,
    };
    let result = tokio::select! {
        r = client.chat_stream(&req, &mut |delta| {
            print!("{delta}");
            let _ = std::io::stdout().flush();
        }) => r,
        _ = tokio::signal::ctrl_c() => {
            history.pop();
            println!("\n(중단됨)");
            return;
        }
    };
    match result {
        Ok(full) if full.is_empty() => {
            history.pop();
            println!("(빈 응답 — 히스토리에 남기지 않음)");
        }
        Ok(full) => {
            println!();
            history.push(ChatMessage::assistant(full));
        }
        Err(e) => {
            history.pop();
            println!("\n오류: {e}");
        }
    }
}

/// 에이전트 턴. Ctrl+C·에러·파싱 실패 시 히스토리를 요청 이전 상태로 되돌린다
async fn run_agent_turn(
    agent: &mut Agent<&OpenAiClient>,
    history: &mut Vec<ChatMessage>,
    config: &Config,
    text: &str,
) {
    let snapshot_len = history.len();
    // 직전 실행이 MaxTurns였다면 이번 요청은 push가 아니라 꼬리 user 메시지에
    // in-place 병합된다(agent::run 진입부) — 길이가 안 변하므로 truncate만으로는
    // 취소된 요청 텍스트가 남는다. 꼬리 내용까지 스냅샷해 원복한다
    let snapshot_tail = history.last().cloned();
    let spinner = RefCell::new(Spinner::start("생각 중"));
    let mut on_event = |ev: AgentEvent<'_>| {
        spinner.borrow_mut().stop();
        match ev {
            AgentEvent::Thought(t) => println!("· {t}"),
            AgentEvent::Action { tool, args } => println!("{}", format_action(tool, args)),
            AgentEvent::Notice(n) => println!("{n}"),
        }
        *spinner.borrow_mut() = Spinner::start("생각 중");
    };
    let result = tokio::select! {
        r = agent.run(history, text, &mut on_event) => Some(r),
        _ = tokio::signal::ctrl_c() => None,
    };
    // on_event는 &RefCell만 캡처해 Copy — drop()은 clippy::dropping_copy_types에 걸리고
    // 애초에 불필요하다 (NLL이 차용을 끝낸다)
    spinner.borrow_mut().stop();

    match result {
        None => {
            rollback(history, snapshot_len, snapshot_tail);
            println!("\n(중단됨 — 이번 요청은 히스토리에서 제거)");
        }
        Some(Ok(AgentOutcome::Finished(summary))) => println!("\n{summary}"),
        Some(Ok(AgentOutcome::MaxTurns)) => println!(
            "(최대 턴 {}회에 도달했습니다 — 작업을 더 작게 나눠 다시 시도하세요)",
            config.max_turns
        ),
        Some(Ok(AgentOutcome::ParseFailed(raw))) => {
            rollback(history, snapshot_len, snapshot_tail);
            println!("(모델 응답을 {PARSE_ATTEMPTS}회 파싱하지 못했습니다. 마지막 원문:)\n{raw}");
        }
        Some(Err(e)) => {
            rollback(history, snapshot_len, snapshot_tail);
            println!("오류: {e}");
        }
    }
}

/// 실패/중단 롤백 — 길이 절단 + 꼬리 메시지 원복.
/// 병합 경로(직전 MaxTurns)에선 길이가 그대로라 truncate만으로는 부족하다.
/// snapshot_tail은 세 arm 중 하나에서만 소비되므로 move해도 안전
fn rollback(history: &mut Vec<ChatMessage>, len: usize, tail: Option<ChatMessage>) {
    history.truncate(len);
    if let (Some(slot), Some(orig)) = (history.last_mut(), tail) {
        *slot = orig;
    }
}
```

`src/main.rs`의 `use loco::ui::repl::{run_repl, SYSTEM_PROMPT};`를 `use loco::ui::repl::{run_repl, CHAT_SYSTEM_PROMPT};`로, `-p` 경로의 `SYSTEM_PROMPT` 참조를 `CHAT_SYSTEM_PROMPT`로 바꿔 컴파일 유지 (-p의 에이전트 전환은 Task 13).

- [ ] **Step 4: 테스트 통과 + clippy 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 5: 수동 스모크 (LM Studio 서버 필요 — 없으면 Task 14에서 일괄 수행)**

```bash
cargo run
```
- `src에 어떤 파일이 있어?` 입력 → `→ list_files ...` 알림과 thought가 표시되고 finish summary가 답으로 출력
- 에이전트 실행 중 Ctrl+C → `(중단됨 ...)` 후 프롬프트 복귀
- `/chat 안녕` → M1처럼 스트리밍 응답
- `/clear` → 초기화 메시지
- (히스토리 오염 확인) max_turns를 낮춘 설정으로 MaxTurns를 유도한 뒤 재요청을 Ctrl+C로 중단 → 이어지는 질문에서 취소된 요청 내용이 언급되지 않아야 함

- [ ] **Step 6: 커밋**

```bash
git add -A
git commit -m "feat: REPL 에이전트 통합 — /chat 분리 + Ctrl+C 취소"
```

---

### Task 13: -p 에이전트 단발 모드 — stdout 계약과 종료 코드

스펙 §7 계약: 최종 답변(`finish.summary`)만 **stdout**, thought/툴 알림/진행 표시는 **stderr**. 종료 코드 `0` = finish 정상 종료, `1` = 에러(연결 실패, 파싱 3회 실패 등), `2` = max_turns 조기 종료 (반복 감지는 M3에서 합류).

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `Agent`(Task 9·10), `Registry::read_only`(Task 6), `Spinner`/`format_action`(Task 11), `run_repl`(Task 12)
- Produces: `main() -> ExitCode`. Task 1의 ring 프로바이더 설치 줄은 main 최상단에 보존

- [ ] **Step 1: 구현 — main.rs 재작성**

```rust
use std::cell::RefCell;
use std::process::ExitCode;

use clap::Parser;

use loco::agent::{Agent, AgentEvent, AgentOutcome, PARSE_ATTEMPTS};
use loco::config::Config;
use loco::llm::client::{resolve_model, OpenAiClient};
use loco::tools::{Registry, ToolCtx};
use loco::ui::repl::run_repl;
use loco::ui::status::{format_action, Spinner};

#[derive(Parser)]
#[command(name = "loco", version, about = "폐쇄망 소형모델 코딩 CLI")]
struct Cli {
    /// 단발 실행 프롬프트 (비대화형 에이전트 — 최종 답변만 stdout)
    #[arg(short, long)]
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    // ring을 프로세스 기본 TLS 프로바이더로 설치 (aws-lc-sys 제거 — Windows 오프라인 빌드 대응).
    // 테스트는 이 설치 없이도 동작한다: 그래프에 프로바이더가 ring 하나뿐이면 rustls가 자동 선택.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("rustls crypto provider 설치 실패");
    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            // 연결 실패·설정 오류 등 — 스펙 §7 종료 코드 1
            eprintln!("오류: {e:#}");
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let config = Config::load_default()?;
    let client = OpenAiClient::new(&config.base_url, config.api_key.clone());
    let model = resolve_model(&client, &config).await?;
    match cli.prompt {
        Some(prompt) => run_oneshot(&client, &config, &model, &prompt).await,
        None => {
            run_repl(&client, &config, &model).await?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// -p 출력 계약 (스펙 §7): 최종 답변만 stdout, 진행 표시는 전부 stderr.
/// 스피너는 stdout이 TTY가 아니면 Spinner::start 내부에서 자동으로 꺼진다
async fn run_oneshot(
    client: &OpenAiClient,
    config: &Config,
    model: &str,
    prompt: &str,
) -> anyhow::Result<ExitCode> {
    let root = std::env::current_dir()?;
    let mut agent = Agent::new(
        client,
        Registry::read_only(),
        ToolCtx { root },
        model.to_string(),
        config,
    );
    let mut history = agent.initial_history();
    let spinner = RefCell::new(Spinner::start("생각 중"));
    let mut on_event = |ev: AgentEvent<'_>| {
        spinner.borrow_mut().stop();
        match ev {
            AgentEvent::Thought(t) => eprintln!("· {t}"),
            AgentEvent::Action { tool, args } => eprintln!("{}", format_action(tool, args)),
            AgentEvent::Notice(n) => eprintln!("{n}"),
        }
        *spinner.borrow_mut() = Spinner::start("생각 중");
    };
    let outcome = agent.run(&mut history, prompt, &mut on_event).await;
    spinner.borrow_mut().stop();

    match outcome? {
        AgentOutcome::Finished(summary) => {
            println!("{summary}");
            Ok(ExitCode::SUCCESS)
        }
        AgentOutcome::MaxTurns => {
            eprintln!("(최대 턴 {}회 도달 — 조기 종료)", config.max_turns);
            Ok(ExitCode::from(2))
        }
        AgentOutcome::ParseFailed(raw) => {
            eprintln!("(모델 응답을 {PARSE_ATTEMPTS}회 파싱하지 못했습니다. 마지막 원문:)\n{raw}");
            Ok(ExitCode::from(1))
        }
    }
}
```

- [ ] **Step 2: 빌드/테스트 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 3: 서버-다운 스모크 (자동화 가능 — LM Studio 불필요)**

```bash
cargo build
tmp=$(mktemp -d)
mkdir "$tmp/.loco"
printf 'base_url = "http://127.0.0.1:1/v1"\n' > "$tmp/.loco/config.toml"
(cd "$tmp" && /Users/sgj/develop/loco/target/debug/loco -p "안녕" > out.txt; echo "exit=$?"; cat out.txt)
```
Expected: stderr에 `서버에 연결할 수 없습니다` 계열 한국어 메시지, `exit=1`, `out.txt` 비어 있음 (stdout 오염 없음)

- [ ] **Step 4: 커밋**

```bash
git add -A
git commit -m "feat: -p 에이전트 단발 모드 — stdout 계약과 종료 코드"
```

---

### Task 14: 문서 갱신 + 라이브 스모크

**Files:**
- Modify: `README.md`, `CLAUDE.md`

**Interfaces:**
- Consumes: 완성된 M2 기능 전부
- Produces: 문서 최신화 + 실모델 검증 완료

- [ ] **Step 1: README 갱신**

`## 시작하기`의 실행 블록을 다음으로 교체:

```markdown
2. 실행:

   ```
   cargo run                 # 대화형 에이전트 REPL
   cargo run -- -p "질문"    # 단발 실행 (답변만 stdout, 종료코드 0/1/2)
   ```

## 사용법

REPL에 입력한 내용은 에이전트가 처리한다 — 모델이 read_file/list_files/grep
툴로 프로젝트를 조사하고, 답은 마지막에 한 번에 출력된다 (`finish`).
진행 중 Ctrl+C 로 취소할 수 있다.

- `/chat <메시지>` — 에이전트 없이 모델과 바로 스트리밍 대화 (빠른 질문용)
- `/clear` — 히스토리 초기화. M2는 히스토리 절삭이 없으므로 긴 세션에서
  컨텍스트가 넘치면 이 명령으로 비운다 (자동 절삭은 M3)
- `/config`, `/help`, `/quit`

`-p` 모드 종료 코드: `0` 정상(finish), `1` 에러(연결 실패·파싱 실패),
`2` 최대 턴 도달. 진행 표시는 stderr로 가므로 stdout만 파이프하면 답변만 남는다.

## 빌드 노트

TLS는 rustls+ring 고정 — OpenSSL도 aws-lc-sys(cmake/NASM)도 그래프에 없어
Windows 폐쇄망에서 `cargo vendor` 후 Rust 툴체인만으로 빌드된다.
```

`## 현재 상태`의 M2 항목을 `[x]`로.

- [ ] **Step 2: CLAUDE.md 갱신**

(reqwest 줄은 Task 1에서 이미 갱신됨.) 다음을 반영:

- 헤더 요약: `M1 (streaming chat REPL) merged to main; M2 (read-tool agent) is next.` → `M1-M2 done (streaming /chat + read-tool agent REPL); M3 (mutating tools + confirmation gate) is next.`
- Commands: `cargo run` 설명을 `agent REPL (default input runs the tool loop; /chat for plain streaming chat)`으로, `-p` 설명에 `one-shot agent; summary to stdout, progress to stderr, exit codes 0/1/2` 추가
- Architecture 불릿에 추가:

```markdown
- tools: `Tool` trait + `Registry::read_only()` (read_file/list_files/grep); `path::confine` rejects absolute/drive/UNC/`..`/symlink-escape paths and accepts `\` separators; model-facing tool output/errors are English
- agent: one JSON `{thought, action}` per turn forced via `response_format: json_schema` (shallow schema); tool results wrapped in `<tool_result>` user messages (no `role:"tool"`); per-turn parse retry x3, `finish_reason: length` gets a "shorter" correction, blind 400 fallback ladder (drop json_schema → inline system prompt); finish is handled by the loop, not the registry
- REPL keeps separate agent and /chat histories; Ctrl+C cancels an in-flight run (history rolls back to the pre-request snapshot)
```

- [ ] **Step 3: 전체 검증**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS, clippy 클린

- [ ] **Step 4: 라이브 스모크 (수동 — LM Studio에 모델 로드 후)**

1. `cargo run` → `src/llm 모듈에 어떤 파일이 있고 각각 뭘 하는지 알려줘` 입력
   - 기대: `→ list_files`/`→ read_file` 알림, `· thought` 줄들, 마지막에 한국어 요약
2. 에이전트 실행 중 Ctrl+C → `(중단됨 ...)` 후 프롬프트 복귀, 이어서 다른 질문 정상 동작
3. `/chat 안녕` → 스트리밍 응답
4. `cargo run -- -p "이 프로젝트의 max_turns 기본값은?" > /tmp/loco-out.txt; echo $?`
   - 기대: 종료코드 0, `/tmp/loco-out.txt`에 답변만, 진행 표시는 터미널(stderr)에만
5. 결과를 `.superpowers/sdd/progress.md` 원장에 기록 (실패 항목은 이슈로)

- [ ] **Step 5: 커밋**

```bash
git add -A
git commit -m "docs: M2 사용법/아키텍처 문서 갱신"
```

---

## 완료 기준 (스펙 §12 M2)

- [ ] `cargo run` 후 코드베이스 질문에 툴 사용을 거쳐 finish.summary로 답한다
- [ ] `cargo run -- -p "질문"`이 답변만 stdout으로, 종료코드 계약(0/1/2)을 지킨다
- [ ] 서버-다운 스모크: 한국어 연결 실패 메시지 + 종료코드 1
- [ ] `cargo test` 전체 통과, `cargo clippy --all-targets -- -D warnings` 클린
- [ ] M1 이연 항목 해소: Http 한국어 래핑, parse/get 헬퍼, ring 전환, /chat 이스케이프, Ctrl+C 취소
