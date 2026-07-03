pub mod protocol;
pub mod prompt;

use crate::config::Config;
use crate::llm::LlmClient;
use crate::llm::client::LlmError;
use crate::llm::types::{ChatMessage, ChatRequest, ChatResponse};
use crate::tools::{Registry, ToolCtx};
use protocol::{parse_turn, response_format};

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
    /// 동일 (tool, args) 5회 연속 — 조기 종료 (스펙 §3), -p 종료 코드 2
    RepetitionStop,
}

/// 턴당 파싱 총 시도 횟수 (초기 1 + 재시도 2, 스펙 §9). max_turns에 계상 안 됨
pub const PARSE_ATTEMPTS: usize = 3;

/// 반복 3회째에 주입하는 교정 (스펙 §3). 모델 대상 — 영어
pub const REPEAT_CORRECTION: &str = "You are repeating the same tool call with the same arguments. \
Its result will not change. Try a different action, or call `finish` with your answer.";

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
        let mut last_action_key: Option<String> = None;
        let mut repeat_count = 0usize;
        let mut corrected = false;
        while turns < self.max_turns {
            let resp = self.chat_with_fallback(history, on_event).await?;

            // 출력 잘림은 파싱 실패와 구분해 교정한다 (스펙 §9). 같은 요청 재시도는
            // 같은 지점에서 다시 잘리므로 "더 짧게"를 지시. 턴을 소모해 max_turns가
            // length 반복의 상한이 되게 한다 (스펙 §3 사각지대)
            if resp.finish_reason() == Some("length") {
                let t = resp.text();
                history.push(ChatMessage::assistant(if t.is_empty() { "(empty)" } else { t }));
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

            // 반복 감지 (스펙 §3). finish는 위에서 이미 return/continue 했으므로
            // 계수 대상이 아니다 — summary 없는 finish 반복은 max_turns가 상한
            // (A/B 교대·length 반복과 함께 §3이 명시한 v1 사각지대, 의도된 것)
            let key = format!("{}|{}", turn.action.tool, turn.action.args);
            if last_action_key.as_deref() == Some(key.as_str()) {
                repeat_count += 1;
            } else {
                last_action_key = Some(key);
                repeat_count = 1;
                corrected = false;
            }
            if repeat_count >= 5 {
                on_event(AgentEvent::Notice(
                    "(같은 툴 호출이 5회 반복돼 조기 종료합니다)".to_string(),
                ));
                return Ok(AgentOutcome::RepetitionStop);
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
            // 교정은 툴 결과와 하나의 user 메시지로 병합 (스펙 §3 — 연속 user 금지)
            let mut msg = tool_result_message(&turn.action.tool, &body);
            if repeat_count == 3 && !corrected {
                corrected = true;
                msg.content = format!("{}\n\n{}", msg.content, REPEAT_CORRECTION);
                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
            }
            history.push(msg);
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
}

/// 툴 결과를 role:"user" 메시지로 감싼다 — role:"tool"은 Gemma 챗템플릿에서
/// 깨지므로 금지 (스펙 §3). 구분자는 <tool_result name="...">
fn tool_result_message(tool: &str, body: &str) -> ChatMessage {
    ChatMessage::user(format!("<tool_result name=\"{tool}\">\n{body}\n</tool_result>"))
}

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

    #[tokio::test]
    async fn five_identical_calls_stop_early_with_one_correction() {
        let dir = tempfile::tempdir().unwrap();
        let same = || ok(&turn("list_files", serde_json::json!({})));
        let script = Scripted::new(vec![same(), same(), same(), same(), same()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::RepetitionStop));
        assert_eq!(script.requests.lock().unwrap().len(), 5, "5회째 응답까지 받고 종료");
        // 교정은 3회째에 정확히 1번, 툴 결과와 같은 user 메시지에 병합 (스펙 §3 다중 피드백 병합)
        let corrections: Vec<_> = history
            .iter()
            .filter(|m| m.content.contains("repeating the same tool call"))
            .collect();
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].role, "user");
        assert!(corrections[0].content.contains("</tool_result>"), "툴 결과 메시지에 병합");
    }

    #[tokio::test]
    async fn different_args_reset_the_repeat_counter() {
        let dir = tempfile::tempdir().unwrap();
        let a = || ok(&turn("list_files", serde_json::json!({})));
        let b = || ok(&turn("list_files", serde_json::json!({"depth": 1})));
        let script = Scripted::new(vec![a(), a(), b(), a(), a(), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "교대하면 감지 안 됨");
        assert!(!history.iter().any(|m| m.content.contains("repeating the same tool call")));
    }

    #[tokio::test]
    async fn empty_length_response_gets_placeholder_content() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok_with_reason("", "length"), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        run_quiet(&mut agent, &mut history, "x").await.unwrap();
        // 빈 assistant content를 거부하는 템플릿 대비 (파싱 실패 경로와 동일 정책)
        assert!(!history.iter().any(|m| m.role == "assistant" && m.content.is_empty()));
    }
}
