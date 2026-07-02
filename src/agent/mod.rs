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
            let resp: ChatResponse = self.client.chat(&self.build_request(history)).await?;
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
