pub mod approval;
pub mod bounded;
pub mod protocol;
pub mod prompt;
pub mod repetition;

use crate::config::Config;
use crate::llm::LlmClient;
use crate::llm::client::LlmError;
use crate::llm::types::{ChatMessage, ChatRequest, ChatResponse};
use crate::session::{Session, tool_result_message};
use crate::tools::{Registry, ToolCtx};
pub use approval::{ApprovalRequest, Approver, Decision};
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
    /// 취소 플래그 감지 후 자발 종료 (M4 — -p Ctrl+C·eval 타임아웃).
    /// REPL은 퓨처 드롭으로 취소하므로 보통 이 변형을 보지 않는다
    Cancelled,
}

/// 턴당 파싱 총 시도 횟수 (초기 1 + 재시도 2, 스펙 §9). max_turns에 계상 안 됨
pub const PARSE_ATTEMPTS: usize = 3;

/// 반복 3회째에 주입하는 교정 (스펙 §3). 모델 대상 — 영어
pub const REPEAT_CORRECTION: &str = "You are repeating the same tool call with the same arguments. \
Its result will not change. Try a different action, or call `finish` with your answer.";

/// salvage 발동 시 툴 결과에 붙이는 교정 노트 (M5 §5.1). 모델 대상 — 영어
pub const SALVAGE_NOTE: &str =
    "note: fields outside \"args\" were accepted this time - put them inside \"args\".";

/// 무검증 finish 1회 반려 (M5 §7.1). 모델 대상 — 영어
pub const VERIFY_NUDGE: &str = "You modified files but never ran a verification command. Run the project's tests (e.g. cargo test) with run_command, then finish.";

pub struct Agent<C: LlmClient> {
    client: C,
    registry: std::sync::Arc<Registry>,
    ctx: std::sync::Arc<ToolCtx>,
    model: String,
    temperature: f32,
    max_output_tokens: u32,
    max_turns: usize,
    context_tokens: usize,
    /// json_schema 폴백 상태 — 400을 만나면 끈다 (스펙 §4). Task 10에서 사용
    use_json_schema: bool,
    /// system role 폴백 상태 — 400을 만나면 첫 user에 병합 (스펙 §3).
    inline_system: bool,
    /// 평가 하네스가 반복마다 다른 시드를 주입한다 (스펙 §8)
    seed: Option<u64>,
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
            registry: std::sync::Arc::new(registry),
            ctx: std::sync::Arc::new(ctx),
            model,
            temperature: config.temperature,
            max_output_tokens: config.max_output_tokens as u32,
            max_turns: config.max_turns,
            context_tokens: config.context_tokens,
            use_json_schema: true,
            inline_system: false,
            seed: None,
        }
    }

    /// 시스템 프롬프트(툴 목록 + 프로젝트 트리)만 담긴 초기 히스토리
    pub fn initial_history(&self) -> Vec<ChatMessage> {
        vec![ChatMessage::system(prompt::system_prompt(
            &self.registry.docs(),
            &self.ctx.root,
        ))]
    }

    /// 평가 하네스가 반복마다 다른 시드를 주입한다 (스펙 §8)
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = Some(seed);
    }

    /// 스펙 §6: (context − max_output) × 0.9
    fn input_budget(&self) -> usize {
        self.context_tokens.saturating_sub(self.max_output_tokens as usize) * 9 / 10
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
            seed: self.seed,
        }
    }

    pub async fn run(
        &mut self,
        session: &mut Session,
        request: &str,
        approver: &mut dyn Approver,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) -> Result<AgentOutcome, LlmError> {
        session.push_user_request(request);
        let mut turns = 0;
        let mut tracker = repetition::RepetitionTracker::new();
        // 실행당(run 호출당) 최대 2회 — 턴을 넘나들며 누적, 턴마다 리셋하지 않는다 (스펙 §9)
        let mut overflow_shrinks: u32 = 0;
        let mut mutated_since_verify = false;
        let mut verify_nudged = false;
        while turns < self.max_turns {
            // 취소 신호 후에는 새 LLM 호출을 만들지 않는다 — run_bounded의 유예가
            // 이 경로로 빠르게 끝난다 (설계 §1). 진행 중이던 run_command는 자체
            // 폴링으로 이미 프로세스 그룹을 정리했다
            if self.ctx.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                return Ok(AgentOutcome::Cancelled);
            }
            session.pack(self.input_budget());
            let resp = loop {
                match self.chat_with_fallback(session.messages(), on_event).await {
                    Err(LlmError::Api { status: 400, body })
                        if looks_like_context_overflow(&body) && overflow_shrinks < 2 =>
                    {
                        overflow_shrinks += 1;
                        on_event(AgentEvent::Notice(format!(
                            "(컨텍스트 초과로 보임 — 히스토리 절삭 후 재시도 {overflow_shrinks}/2)"
                        )));
                        session.pack(self.input_budget() >> overflow_shrinks);
                    }
                    Err(LlmError::Api { status: 400, body }) if looks_like_context_overflow(&body) => {
                        on_event(AgentEvent::Notice(
                            "(컨텍스트 초과 — context_tokens 설정과 서버 로드 설정을 확인하세요)".to_string(),
                        ));
                        return Err(LlmError::Api { status: 400, body });
                    }
                    other => break other?,
                }
            };

            // 출력 잘림은 파싱 실패와 구분해 교정한다 (스펙 §9). 같은 요청 재시도는
            // 같은 지점에서 다시 잘리므로 "더 짧게"를 지시. 턴을 소모해 max_turns가
            // length 반복의 상한이 되게 한다 (스펙 §3 사각지대)
            if resp.finish_reason() == Some("length") {
                let t = resp.text();
                session.push(ChatMessage::assistant(if t.is_empty() { "(empty)" } else { t }));
                session.push(ChatMessage::user(
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
                        session.push(ChatMessage::assistant(if text.is_empty() {
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
                        session.push(ChatMessage::user(feedback));
                        // NOTE (M3 known gap): unlike the primary chat call above, this parse-retry is
                        // NOT wrapped by the context-overflow shrink-retry loop. An overflow 400 here
                        // propagates directly (clean error + rollback). Deferred to M4 — extract a
                        // shared `chat_packed` helper wrapping both call sites. (plan-scoped: the plan
                        // wrapped only "the turn's chat call".)
                        let retry = self.chat_with_fallback(session.messages(), on_event).await?;
                        text = retry.text().to_string();
                    }
                }
            };
            session.push(ChatMessage::assistant(text));
            on_event(AgentEvent::Thought(&turn.thought));

            if turn.action.tool == "finish" {
                match turn.action.args.get("summary").and_then(|v| v.as_str()) {
                    Some(s) => {
                        if mutated_since_verify && !verify_nudged {
                            verify_nudged = true;
                            on_event(AgentEvent::Notice("(검증 없는 종료 — 확인 요청 주입)".to_string()));
                            session.push(tool_result_message("finish", VERIFY_NUDGE));
                            turns += 1;
                            continue;
                        }
                        return Ok(AgentOutcome::Finished(s.to_string()));
                    }
                    None => {
                        const FINISH_ERR: &str = "Error: finish requires a string `summary` argument, e.g. {\"tool\": \"finish\", \"args\": {\"summary\": \"<your final answer>\"}}";
                        // summary 없는 finish도 반복 계수에 편입 (M5 §7.3 — 기존 §3 사각지대 폐지)
                        let key = format!("finish|{}", turn.action.args);
                        let verdict = tracker.record(&key, FINISH_ERR);
                        // InjectCorrection을 버리면 record()가 래치한 실행당 1회 교정 기회가
                        // 소모된다 — 같은 user 메시지에 병합해 반드시 전달 (본선 스펙 §3 연속 user 금지)
                        let body = match verdict {
                            repetition::RepetitionVerdict::InjectCorrection => {
                                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
                                format!("{FINISH_ERR}\n{REPEAT_CORRECTION}")
                            }
                            _ => FINISH_ERR.to_string(),
                        };
                        session.push(tool_result_message("finish", &body));
                        if matches!(verdict, repetition::RepetitionVerdict::Stop) {
                            on_event(AgentEvent::Notice("(같은 툴 호출이 반복돼 조기 종료합니다)".to_string()));
                            return Ok(AgentOutcome::RepetitionStop);
                        }
                        turns += 1;
                        continue;
                    }
                }
            }

            on_event(AgentEvent::Action {
                tool: &turn.action.tool,
                args: &turn.action.args,
            });

            // 확인 게이트 (스펙 §5): mutating이고 미리보기가 가능할 때만.
            // preview Err → 게이트 생략, 아래 디스패치가 같은 에러를 되먹인다
            let gate_preview = self
                .registry
                .get(&turn.action.tool)
                .filter(|t| t.is_mutating())
                .map(|t| t.preview(&turn.action.args, &self.ctx));
            if let Some(Ok(preview)) = gate_preview {
                let req = ApprovalRequest { tool: &turn.action.tool, args: &turn.action.args, preview: &preview };
                if let Decision::Deny { reason } = approver.approve(&req) {
                    on_event(AgentEvent::Notice("(거부됨 — 모델에 전달)".to_string()));
                    let body = format!("Denied: {reason}");
                    let (note, stop) = self.track_and_note(&mut tracker, &turn, &body, on_event);
                    session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
                    if stop {
                        return Ok(AgentOutcome::RepetitionStop);
                    }
                    turns += 1;
                    continue;
                }
            }
            // 툴 에러도 모델에 되먹이는 데이터 — 루프는 계속 (스펙 §9).
            // 디스패치는 spawn_blocking으로 — 동기 툴(향후 run_command 등)이 async
            // 런타임을 막지 않고, Ctrl+C가 REPL 쪽에서 즉시 select! 밖으로 빠질 수 있게 한다
            let registry = std::sync::Arc::clone(&self.registry);
            let ctx = std::sync::Arc::clone(&self.ctx);
            let tool_name = turn.action.tool.clone();
            let tool_args = turn.action.args.clone();
            let dispatched =
                tokio::task::spawn_blocking(move || registry.dispatch(&tool_name, &tool_args, &ctx)).await;
            let dispatch_ok = matches!(&dispatched, Ok(Ok(_)));
            let body = match dispatched {
                Ok(Ok(s)) if s.is_empty() => "(no output)".to_string(),
                Ok(Ok(s)) => s,
                Ok(Err(e)) => format!("Error: {e}"),
                Err(join) => format!("Error: tool execution panicked: {join}"),
            };
            if dispatch_ok {
                if turn.action.tool == "run_command" {
                    mutated_since_verify = false; // 검증 실행으로 인정 — 종료 코드 무관 (M5 §7.1)
                } else if self.registry.get(&turn.action.tool).is_some_and(|t| t.is_mutating()) {
                    mutated_since_verify = true;
                }
            }
            let (note, stop) = self.track_and_note(&mut tracker, &turn, &body, on_event);
            session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
            if stop {
                return Ok(AgentOutcome::RepetitionStop);
            }
            turns += 1;
        }
        Ok(AgentOutcome::MaxTurns)
    }

    /// 디스패치 후 반복 계수 + 노트 조립 (M5 §7.2). 반환: (병합 노트, RepetitionStop 여부)
    fn track_and_note(
        &self,
        tracker: &mut repetition::RepetitionTracker,
        turn: &protocol::ModelTurn,
        body: &str,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) -> (Option<String>, bool) {
        let mut notes: Vec<&str> = Vec::new();
        if turn.salvaged {
            notes.push(SALVAGE_NOTE);
        }
        let key = format!("{}|{}", turn.action.tool, turn.action.args);
        match tracker.record(&key, body) {
            repetition::RepetitionVerdict::Stop => {
                on_event(AgentEvent::Notice("(같은 툴 호출이 반복돼 조기 종료합니다)".to_string()));
                return (None, true);
            }
            repetition::RepetitionVerdict::InjectCorrection => {
                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
                notes.push(REPEAT_CORRECTION);
            }
            repetition::RepetitionVerdict::Ok => {}
        }
        if let Some(strategy) = tracker.error_correction(&turn.action.tool, body) {
            on_event(AgentEvent::Notice("(동일 에러 반복 — 전략 교정 주입)".to_string()));
            notes.push(strategy);
        }
        let joined = notes.join("\n");
        ((!joined.is_empty()).then_some(joined), false)
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
                // 휴리스틱 매치 시 즉시 전파 — 절삭 후 재시도는 run()의 상위 루프가
                // 처리한다(Notice도 거기서 낸다), 여기서는 그대로 반환만 (스펙 §9)
                Err(LlmError::Api { status: 400, body }) if looks_like_context_overflow(&body) => {
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

/// 서버 컨텍스트 초과 400 감지 휴리스틱 — LM Studio/llama.cpp/vLLM 모두 에러 메시지에
/// "context"가 들어간다. 완전하지 않은 최선 노력이며, 자동 절삭 대응(pack + 재시도)은
/// run()에 구현되어 있다 (스펙 §9)
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
    use crate::session::{Session, Transcript};
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
        Agent::new(script, Registry::read_only(), ToolCtx::new(root), "test-model".into(), &config)
    }

    fn new_session(agent: &Agent<&Scripted>) -> Session {
        Session::new(agent.initial_history(), Transcript::disabled())
    }

    async fn run_quiet(
        agent: &mut Agent<&Scripted>,
        session: &mut Session,
        request: &str,
    ) -> Result<AgentOutcome, LlmError> {
        agent.run(session, request, &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await
    }

    #[tokio::test]
    async fn salvaged_turn_gets_a_note_with_the_tool_result() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        // path를 action 레벨에 둔 salvage 대상 턴
        let bad_shape = r#"{"thought": "read", "action": {"tool": "read_file", "args": {}, "path": "a.txt"}}"#;
        let script = Scripted::new(vec![ok(bad_shape), ok(&finish("done"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let note = session.messages().iter().find(|m| m.content.contains("fields outside"));
        let note = note.expect("salvage 노트가 툴 결과에 병합");
        assert_eq!(note.role, "user");
        assert!(note.content.contains("hi"), "툴 결과(파일 내용)와 같은 메시지: {}", note.content);
    }

    #[tokio::test]
    async fn set_seed_reaches_the_request() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        agent.set_seed(7);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "질문").await.unwrap();
        assert_eq!(script.requests.lock().unwrap()[0].seed, Some(7));
    }

    #[tokio::test]
    async fn finish_returns_summary_and_sends_wellformed_request() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(&finish("답변입니다"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
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
        let mut session = new_session(&agent);
        let mut events: Vec<String> = Vec::new();
        let outcome = agent
            .run(&mut session, "hello.txt 읽어줘", &mut crate::agent::approval::AutoApprover::default(), &mut |ev| {
                events.push(match ev {
                    AgentEvent::Thought(t) => format!("thought:{t}"),
                    AgentEvent::Action { tool, .. } => format!("action:{tool}"),
                    AgentEvent::Notice(n) => format!("notice:{n}"),
                });
            })
            .await
            .unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));

        let wrapped = session.messages().iter().find(|m| m.content.contains("<tool_result")).unwrap();
        assert_eq!(wrapped.role, "user", "role:'tool' 금지 (스펙 §3)");
        assert!(wrapped.content.contains("<tool_result name=\"read_file\">"));
        assert!(wrapped.content.contains("세계"));
        assert_eq!(events[0], "thought:t");
        assert_eq!(events[1], "action:read_file");
        // 히스토리 role 교대: system, user, assistant, user(tool_result), assistant(finish)
        let roles: Vec<&str> = session.messages().iter().map(|m| m.role.as_str()).collect();
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
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "읽어").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let fed = session.messages().iter().find(|m| m.content.contains("Error: not found")).unwrap();
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
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(session.messages().iter().any(|m| m.content.contains("Error: unknown tool: teleport")));
    }

    #[tokio::test]
    async fn max_turns_returns_control() {
        let dir = tempfile::tempdir().unwrap();
        let list = || ok(&turn("list_files", serde_json::json!({})));
        let script = Scripted::new(vec![list(), list(), list()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 2);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
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
        let mut session = new_session(&agent);
        let first = run_quiet(&mut agent, &mut session, "첫 요청").await.unwrap();
        assert!(matches!(first, AgentOutcome::MaxTurns));
        let second = run_quiet(&mut agent, &mut session, "이어서").await.unwrap();
        assert!(matches!(second, AgentOutcome::Finished(_)));
        // role 교대 보존 (스펙 §3) — 연속 user 금지
        for w in session.messages().windows(2) {
            assert!(!(w[0].role == "user" && w[1].role == "user"), "연속 user 메시지");
        }
        let merged = session.messages().iter().find(|m| m.content.contains("이어서")).unwrap();
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
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(s) if s == "이제 됐다"));
        assert!(session.messages().iter().any(|m| m.content.contains("`summary`")));
    }

    fn api_400() -> Result<ChatResponse, LlmError> {
        Err(LlmError::Api { status: 400, body: "unsupported".into() })
    }

    #[tokio::test]
    async fn parse_failure_is_fed_back_then_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok("JSON 아님"), ok(&finish("복구"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(s) if s == "복구"));
        // 되먹임: assistant(원문) + user(형식 힌트 피드백)가 히스토리에 남는다
        assert!(session.messages().iter().any(|m| m.role == "assistant" && m.content == "JSON 아님"));
        assert!(session.messages().iter().any(|m| m.role == "user" && m.content.contains("JSON object")));
        assert_eq!(script.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn parse_failure_three_times_returns_raw_text() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok("쓰레기1"), ok("쓰레기2"), ok("쓰레기3")]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
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
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        // 파싱 재시도 피드백이 아니라 "잘렸으니 더 짧게" 교정 (스펙 §9)
        assert!(session.messages().iter().any(|m| m.role == "user" && m.content.contains("cut off")));
    }

    #[tokio::test]
    async fn length_consumes_a_turn_so_it_cannot_loop_forever() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok_with_reason("잘림", "length")]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 1);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::MaxTurns), "max_turns가 length 반복의 상한 (스펙 §3)");
    }

    #[tokio::test]
    async fn first_400_disables_json_schema_and_retries() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![api_400(), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let mut notices = Vec::new();
        let outcome = agent
            .run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |ev| {
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
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
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
        let mut session = new_session(&agent);
        let err = run_quiet(&mut agent, &mut session, "x").await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 400, .. }));
    }

    #[tokio::test]
    async fn context_overflow_packs_and_retries_then_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let overflow = || Err(LlmError::Api { status: 400, body: "exceeds the available context size".into() });
        let script = Scripted::new(vec![overflow(), ok(&finish("살아남"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        // 크기 산정 주의: 기본 예산 5529토큰은 "통과"하되 축소 예산(>>1 = 2764)은
        // 초과하도록 심는다 — 첫 턴의 일반 패킹이 아니라 초과-재시도 경로가 절삭해야 함.
        // "빅".repeat(5000) = 15000바이트 ≈ 3750토큰
        session.push(ChatMessage::user("빅".repeat(5000)));
        session.push(ChatMessage::assistant("이전 답"));
        let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "절삭 후 재시도로 회복");
        assert_eq!(script.requests.lock().unwrap().len(), 2);
        let reqs = script.requests.lock().unwrap();
        assert!(reqs[1].messages.len() < reqs[0].messages.len(), "재시도는 절삭된 히스토리");
    }

    #[tokio::test]
    async fn context_overflow_three_times_propagates_with_schema_intact() {
        let dir = tempfile::tempdir().unwrap();
        let overflow = || Err(LlmError::Api { status: 400, body: "context overflow".into() });
        let script = Scripted::new(vec![overflow(), overflow(), overflow()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let err = run_quiet(&mut agent, &mut session, "x").await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 400, .. }));
        assert_eq!(script.requests.lock().unwrap().len(), 3, "절삭 재시도 2회 후 전파 (스펙 §9)");
        let reqs = script.requests.lock().unwrap();
        assert!(reqs[2].response_format.is_some(), "폴백 사다리 오분류 금지 — json_schema 유지");
    }

    #[tokio::test]
    async fn every_turn_packs_to_budget() {
        // 툴 결과 2개를 쌓으면 세 번째 턴의 패킹이 "오래된" 쪽(마지막 메시지가 아닌)을
        // 생략해야 한다. pack은 마지막 메시지(방금 받은 결과)는 건드리지 않으므로
        // 결과가 하나뿐이면 이 테스트는 성립하지 않는다 — 반드시 두 번 읽는다.
        // 수치: 결과 각 ≈1500토큰, 예산 = (2500−100)×0.9 = 2160 → 둘 다 온전히는 못 담음.
        // 주의: 실측 시스템 프롬프트(~400토큰)도 예산에 계상된다 — 여유 ~230토큰.
        // 후속 마일스톤에서 프롬프트가 크게 자라면 이 수치를 재조정해야 한다
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), "z".repeat(6_000)).unwrap();
        let read = || ok(&turn("read_file", serde_json::json!({"path": "big.txt"})));
        let script = Scripted::new(vec![read(), read(), ok(&finish("done"))]);
        let config = Config { context_tokens: 2_500, max_output_tokens: 100, ..Default::default() };
        let mut agent = Agent::new(
            &script, Registry::read_only(), ToolCtx::new(dir.path().to_path_buf()),
            "test-model".into(), &config,
        );
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "읽어").await.unwrap();
        let reqs = script.requests.lock().unwrap();
        let third = &reqs[2].messages;
        assert!(
            third.iter().any(|m| m.content.contains(crate::session::ELIDED)),
            "오래된 툴 결과는 생략된 채 전송"
        );
        assert!(
            third.iter().filter(|m| m.content.contains("zzzz")).count() >= 1,
            "최신 툴 결과는 온전히 유지"
        );
    }

    #[tokio::test]
    async fn empty_response_counts_as_parse_failure() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(""), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(session.messages().iter().any(|m| m.content.contains("empty")));
    }

    #[tokio::test]
    async fn five_identical_calls_stop_early_with_one_correction() {
        let dir = tempfile::tempdir().unwrap();
        let same = || ok(&turn("list_files", serde_json::json!({})));
        let script = Scripted::new(vec![same(), same(), same(), same(), same()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::RepetitionStop));
        assert_eq!(script.requests.lock().unwrap().len(), 5, "5회째 응답까지 받고 종료");
        // 교정은 3회째에 정확히 1번, 툴 결과와 같은 user 메시지에 병합 (스펙 §3 다중 피드백 병합)
        let corrections: Vec<_> = session.messages()
            .iter()
            .filter(|m| m.content.contains("repeating the same tool call"))
            .collect();
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].role, "user");
        assert!(corrections[0].content.contains("</tool_result>"), "툴 결과 메시지에 병합");
    }

    #[tokio::test]
    async fn alternation_no_longer_resets_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let a = || ok(&turn("list_files", serde_json::json!({})));
        let b = || ok(&turn("list_files", serde_json::json!({"depth": 1})));
        let script = Scripted::new(vec![a(), a(), b(), a(), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "3회는 교정, 정지는 아님");
        assert_eq!(
            session.messages().iter().filter(|m| m.content.contains("repeating the same tool call")).count(),
            1,
            "교대에도 불구하고 윈도가 3회째를 잡아 교정 1회"
        );
    }

    #[tokio::test]
    async fn four_reads_then_edit_then_reread_is_not_stopped() {
        // 스펙 §7.2·§8: 결과 해시가 "편집 후 달라진 재읽기"를 정당한 반복으로 구제
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "old").unwrap();
        let read = || ok(&turn("read_file", serde_json::json!({"path": "f.txt"})));
        let write = ok(&turn("write_file", serde_json::json!({"path": "f.txt", "content": "CHANGED"})));
        // finish 2개: Task 15의 검증 넛지가 1차 finish를 반려한다 (Task 14 시점엔 2번째가 남아도 무해)
        let script = Scripted::new(vec![read(), read(), read(), read(), write, read(), ok(&finish("done")), ok(&finish("done"))]);
        let config = Config { max_turns: 25, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
        let mut session = new_session(&agent);
        let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
    }

    #[tokio::test]
    async fn summary_less_finish_loop_ends_in_repetition_stop() {
        // gemma chain-edits-0 실측: summary 없는 finish 14연속 — 이제 5회째 정지 (스펙 §7.3)
        let dir = tempfile::tempdir().unwrap();
        let bad = || ok(&turn("finish", serde_json::json!({})));
        let script = Scripted::new(vec![bad(), bad(), bad(), bad(), bad()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::RepetitionStop), "{outcome:?}");
    }

    #[tokio::test]
    async fn same_error_three_times_injects_strategy_correction() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "content").unwrap();
        // 서로 다른 args의 edit_file이 같은 에러(첫 줄)를 3연속 수신
        let e = |s: &str| ok(&turn("edit_file", serde_json::json!({"path": "f.txt", "search": s, "replace": "y"})));
        let script = Scripted::new(vec![e("no1"), e("no2"), e("no3"), ok(&finish("giving up"))]);
        let config = Config { max_turns: 25, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
        let mut session = new_session(&agent);
        let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(
            session.messages().iter().any(|m| m.content.contains("rewrite it completely with write_file")),
            "전략 교정 주입"
        );
    }

    #[tokio::test]
    async fn empty_length_response_gets_placeholder_content() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok_with_reason("", "length"), ok(&finish("ok"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        // 빈 assistant content를 거부하는 템플릿 대비 (파싱 실패 경로와 동일 정책)
        assert!(!session.messages().iter().any(|m| m.role == "assistant" && m.content.is_empty()));
    }

    use crate::agent::approval::{ApprovalRequest, Approver, Decision};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    /// 실행 횟수를 세는 가짜 mutating 툴
    struct MutTool(Arc<AtomicUsize>);
    impl crate::tools::Tool for MutTool {
        fn name(&self) -> &'static str { "mut_tool" }
        fn doc(&self) -> &'static str { "mut_tool(): test." }
        fn is_mutating(&self) -> bool { true }
        fn preview(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
            Ok("PREVIEW-TEXT".to_string())
        }
        fn run(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("mutated".to_string())
        }
    }

    struct ScriptedApprover {
        decisions: Mutex<VecDeque<Decision>>,
        seen: Mutex<Vec<(String, String)>>, // (tool, preview)
    }
    impl Approver for &ScriptedApprover {
        fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision {
            self.seen.lock().unwrap().push((req.tool.to_string(), req.preview.to_string()));
            self.decisions.lock().unwrap().pop_front().expect("결정 스크립트 소진")
        }
    }

    fn mut_agent(script: &Scripted, hits: Arc<AtomicUsize>, root: std::path::PathBuf) -> Agent<&Scripted> {
        let config = Config::default();
        let reg = Registry::new(vec![Box::new(MutTool(hits))]);
        Agent::new(script, reg, ToolCtx::new(root), "test-model".into(), &config)
    }

    #[tokio::test]
    async fn denied_action_is_not_executed_and_reason_reaches_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let script = Scripted::new(vec![ok(&turn("mut_tool", serde_json::json!({}))), ok(&finish("ok"))]);
        let mut agent = mut_agent(&script, hits.clone(), dir.path().to_path_buf());
        let approver = ScriptedApprover {
            decisions: Mutex::new(vec![Decision::Deny { reason: "nope".into() }].into()),
            seen: Mutex::new(Vec::new()),
        };
        let mut session = new_session(&agent);
        let outcome = agent.run(&mut session, "x", &mut (&approver), &mut |_| {}).await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert_eq!(hits.load(Ordering::SeqCst), 0, "거부된 액션은 실행 금지");
        assert!(session.messages().iter().any(|m| m.role == "user" && m.content.contains("Denied: nope")));
        let seen = approver.seen.lock().unwrap();
        assert_eq!(seen[0], ("mut_tool".to_string(), "PREVIEW-TEXT".to_string()));
    }

    #[tokio::test]
    async fn approved_action_executes() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        // finish 2개: 검증 넛지가 mutating 실행 후의 1차 finish를 반려한다 (M5 §7.1)
        let script = Scripted::new(vec![ok(&turn("mut_tool", serde_json::json!({}))), ok(&finish("ok")), ok(&finish("ok"))]);
        let mut agent = mut_agent(&script, hits.clone(), dir.path().to_path_buf());
        let approver = ScriptedApprover {
            decisions: Mutex::new(vec![Decision::Approve].into()),
            seen: Mutex::new(Vec::new()),
        };
        let mut session = new_session(&agent);
        agent.run(&mut session, "x", &mut (&approver), &mut |_| {}).await.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert!(session.messages().iter().any(|m| m.content.contains("mutated")));
    }

    /// 읽기 툴은 approver를 부르지 않는다 — 불리면 패닉
    struct PanicApprover;
    impl Approver for PanicApprover {
        fn approve(&mut self, _r: &ApprovalRequest<'_>) -> Decision {
            panic!("읽기 툴에 게이트가 걸림");
        }
    }

    #[tokio::test]
    async fn read_tools_bypass_the_gate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "a.txt"}))),
            ok(&finish("ok")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = agent.run(&mut session, "x", &mut PanicApprover, &mut |_| {}).await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
    }

    struct EmptyTool;
    impl crate::tools::Tool for EmptyTool {
        fn name(&self) -> &'static str { "empty_tool" }
        fn doc(&self) -> &'static str { "empty_tool(): returns nothing." }
        fn run(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn empty_tool_output_becomes_no_output_marker() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(&turn("empty_tool", serde_json::json!({}))), ok(&finish("ok"))]);
        let config = Config::default();
        let mut agent = Agent::new(
            &script, Registry::new(vec![Box::new(EmptyTool)]),
            ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config,
        );
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session.messages().iter().any(|m| m.content.contains("(no output)")));
    }

    #[tokio::test]
    async fn preset_cancel_flag_returns_cancelled_without_llm_calls() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![]); // 호출되면 스크립트 고갈로 패닉
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        ctx.cancel.store(true, Ordering::SeqCst);
        let config = Config::default();
        let mut agent = Agent::new(&script, Registry::read_only(), ctx, "test-model".into(), &config);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Cancelled));
        assert_eq!(script.requests.lock().unwrap().len(), 0, "LLM 호출 없이 반환");
    }

    /// 실행되면 cancel 플래그를 세우는 가짜 툴 — 툴 실행 후 다음 LLM 호출 전에 멈추는지 검증
    struct CancelTool(Arc<AtomicBool>);
    impl crate::tools::Tool for CancelTool {
        fn name(&self) -> &'static str { "cancel_tool" }
        fn doc(&self) -> &'static str { "cancel_tool(): test." }
        fn run(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
            self.0.store(true, Ordering::SeqCst);
            Ok("ok".to_string())
        }
    }

    #[tokio::test]
    async fn cancel_during_tool_stops_before_next_llm_call() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(&turn("cancel_tool", serde_json::json!({})))]); // 응답 1개뿐
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let flag = ctx.cancel.clone();
        let config = Config::default();
        let mut agent = Agent::new(
            &script, Registry::new(vec![Box::new(CancelTool(flag))]), ctx, "test-model".into(), &config,
        );
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Cancelled));
        assert_eq!(script.requests.lock().unwrap().len(), 1, "취소 후 추가 LLM 호출 금지");
    }

    #[tokio::test]
    async fn finish_after_edit_without_verification_is_nudged_once() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "new.txt", "content": "x"}))),
            ok(&finish("done without verify")),   // 1차 — 반려
            ok(&finish("done anyway")),           // 2차 — 통과
        ]);
        let config = Config { max_turns: 25, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
        let mut session = new_session(&agent);
        let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
        match outcome {
            AgentOutcome::Finished(s) => assert_eq!(s, "done anyway", "2차 finish는 무조건 통과"),
            other => panic!("{other:?}"),
        }
        assert!(session.messages().iter().any(|m| m.content.contains("never ran a verification command")));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn finish_after_edit_and_run_command_is_not_nudged() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "new.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": "exit 3"}))), // 실패해도 "검증 실행"
            ok(&finish("verified")),
        ]);
        let config = Config { max_turns: 25, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
        let mut session = new_session(&agent);
        let outcome = agent.run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(!session.messages().iter().any(|m| m.content.contains("never ran a verification command")));
    }

    #[tokio::test]
    async fn finish_without_any_edit_is_not_nudged() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(&finish("answer only"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(!session.messages().iter().any(|m| m.content.contains("verification command")));
    }
}
