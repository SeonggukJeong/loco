pub mod approval;
pub mod bounded;
pub mod finish_nudge;
pub mod protocol;
pub mod prompt;
pub mod repetition;
pub mod status_note;

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

/// M12 §3-2 — args 안의 잉여 `tool` 키를 제거했을 때 붙이는 노트. SALVAGE_NOTE는
/// "args 바깥의 필드를 안으로"라는 정반대 진술이라 이 오형에 재사용하면 오도한다
pub const ARGS_TOOL_KEY_NOTE: &str =
    "note: the `tool` key inside \"args\" is not a parameter - it was removed. \
     Put only the tool's own parameters inside \"args\".";

/// args가 다른 등록 도구를 지목해 그 도구로 교체 디스패치했을 때의 노트 (M12 §3-2)
pub const ARGS_TOOL_SWITCH_NOTE: &str =
    "note: \"args\" named a different tool, so this call was dispatched as that tool instead. \
     Put the tool name only in \"action\".\"tool\".";

/// 무검증 finish 1회 반려 (M5 §7.1). 모델 대상 — 영어
pub const VERIFY_NUDGE: &str = "You modified files but never ran a verification command since your last edit. Run the project's tests (e.g. cargo test) with run_command, then finish.";

/// §3-3-1 — 파이프 때문에 검증이 성립하지 않은 경우. 기본 문구는 "never ran"이라
/// 방금 파이프로 검증을 돌린 모델에게 거짓말이 된다
pub const VERIFY_NUDGE_PIPE: &str = "You ran a verification command, but it was a shell pipeline, so its exit code reflects only the last command in the pipe and does not tell whether the tests passed. Re-run it without a pipe, then finish.";

/// summary 없는 finish 2연속 시 1회 주입 (M9 §4-1) — 모델이 내보내야 하는
/// 전체 턴 형태를 제시한다 (인자 예시만 담은 FINISH_ERR 에코는 5연속 반복을
/// 못 막은 실측이 있다). 모델 대상 — 영어
pub const FINISH_ARGS_CORRECTION: &str = "Your finish call is missing `summary`. Respond with exactly this shape: \
{\"thought\": \"...\", \"action\": {\"tool\": \"finish\", \"args\": {\"summary\": \"<your final answer>\"}}}. \
Do not call finish with empty args again.";

/// S/R 스트릭 2연속 시 다음 요청의 temperature (M10 §5 — 저온 복사 어트랙터
/// 가설의 개입값, 0단계 확정). 스트릭이 끊기면 즉시 원복, 래치 없음
const SR_PERTURB_TEMPERATURE: f32 = 0.7;

/// length 턴 회복 문구 (M9~M13에서 문자열 동일 — 상수로 승격해 중복 제거가 참조한다)
pub(crate) const LENGTH_RECOVERY: &str = "Your previous response was cut off by the output token limit. \
                                          Respond again with exactly one, much shorter JSON turn.";

/// §4-2(c) 하한 — max_output_tokens가 context_tokens를 넘겨도 예산이 0이 되지
/// 않게 한다. 0이면 pack()이 시스템 프롬프트와 마지막 메시지만 남긴다
const MIN_INPUT_BUDGET: usize = 512;

/// 입력 예산 = (context − max_output) × 0.9, 하한 MIN_INPUT_BUDGET 적용.
/// input_budget()과 cramped_budget_warning()이 **같은 값**을 봐야 한다 —
/// 공식을 두 벌로 두면 경고가 하한 적용 전 값을 출력한다
fn floored_input_budget(context_tokens: usize, max_output_tokens: usize) -> usize {
    (context_tokens.saturating_sub(max_output_tokens) * 9 / 10).max(MIN_INPUT_BUDGET)
}

/// 출력 예산이 컨텍스트의 절반 이상을 먹으면 입력 예산이 좁아진다는 경고.
/// 사용자 대면이므로 한국어. None이면 경고 없음
fn cramped_budget_warning(context_tokens: usize, max_output_tokens: usize) -> Option<String> {
    if max_output_tokens * 2 < context_tokens {
        return None;
    }
    let budget = floored_input_budget(context_tokens, max_output_tokens);
    Some(format!(
        "경고: max_output_tokens={max_output_tokens}가 context_tokens={context_tokens}의 절반 이상입니다 \
         — 입력 예산이 {budget} 토큰으로 좁아져 오래된 대화가 일찍 잘립니다. \
         context_tokens를 올리거나 max_output_tokens를 낮추는 것을 검토하세요."
    ))
}

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
    /// 이 런에서 json_schema 폴백이 **실제로 발동했는가** (M13 스펙 §3-6-1).
    /// `!use_json_schema`로 파생하지 않는다 — 그러면 "처음부터 json_schema를
    /// 끄고 시작하는 설정"이 생기는 순간 전 런이 폴백 발동으로 기록돼
    /// 앵커 게이트가 배치 전체를 오차단한다(fail-closed 오작동).
    schema_fallback: bool,
    /// system role 폴백 상태 — 400을 만나면 첫 user에 병합 (스펙 §3).
    inline_system: bool,
    /// 평가 하네스가 반복마다 다른 시드를 주입한다 (스펙 §8)
    seed: Option<u64>,
    /// S/R 스트릭 중 일시 temperature 상향 (M10 §5). run() 지역 수명 —
    /// 진입 시 리셋해 REPL의 다음 런으로 새지 않는다 (리뷰 2R M-1)
    temperature_override: Option<f32>,
    /// M16 hierarchical repo notes — from `config.repo_notes`, not registry inference.
    repo_notes: bool,
}

/// 진단 가치가 있는 Notice는 트랜스크립트에도 남긴다 (M14 C-2).
/// M13에서 오버플로가 "0이 아니라 미측정"이 된 원인 — 프로세스가 죽으면 Notice는
/// 흔적 없이 사라졌다. `on_event(AgentEvent::Notice(...))` 호출은 19곳이지만
/// 최소 적용 지점 2곳(컨텍스트 오버플로·length 절단)에만 쓴다 — 범위를 넓히면
/// 트랜스크립트가 비대해진다
macro_rules! notice_recorded {
    ($session:expr, $on_event:expr, $msg:expr) => {{
        let m: String = $msg;
        $session.record_extra("notice", &m);
        $on_event(AgentEvent::Notice(m));
    }};
}

impl<C: LlmClient> Agent<C> {
    pub fn new(
        client: C,
        registry: Registry,
        ctx: ToolCtx,
        model: String,
        config: &Config,
    ) -> Self {
        // M14 §4-2c — run()엔 on_event 채널이 있지만 REPL은 run()을 요청마다
        // 다시 호출하므로 거기 두면 경고가 반복된다. Agent::new는 런당 1회만
        // 생성되므로 여기서 eprintln!으로 1회 낸다 (on_event 채널 부재)
        if let Some(w) = cramped_budget_warning(config.context_tokens, config.max_output_tokens) {
            eprintln!("{w}");
        }
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
            schema_fallback: false,
            inline_system: false,
            seed: None,
            temperature_override: None,
            repo_notes: config.repo_notes,
        }
    }

    /// 시스템 프롬프트(툴 목록 + 프로젝트 트리)만 담긴 초기 히스토리
    pub fn initial_history(&self) -> Vec<ChatMessage> {
        vec![ChatMessage::system(prompt::system_prompt(
            &self.registry.docs(),
            &self.ctx.root,
            self.repo_notes,
        ))]
    }

    /// 평가 하네스가 반복마다 다른 시드를 주입한다 (스펙 §8)
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = Some(seed);
    }

    /// 이 런에서 json_schema 폴백(400 → response_format 제거)이 발동했는가.
    /// eval이 report.json에 기록해 "조용한 전면 실패"를 배치 후 기계적으로
    /// 판별할 수 있게 한다 (M13 스펙 §3-6-1). Agent는 런마다 새로 만들어지므로
    /// (src/eval/mod.rs) 이 값은 런 지역이다.
    pub fn schema_fallback_fired(&self) -> bool {
        self.schema_fallback
    }

    /// 스펙 §6: (context − max_output) × 0.9, 하한 MIN_INPUT_BUDGET (M14 §4-2c) —
    /// max_output_tokens >= context_tokens인 병리적 config에서도 예산이 0으로
    /// 붕괴해 pack()이 히스토리를 지워버리는 것을 막는다
    fn input_budget(&self) -> usize {
        floored_input_budget(self.context_tokens, self.max_output_tokens as usize)
    }

    /// 이 에이전트가 **생성 시점에 스냅샷한** 컨텍스트 운용점 (M15 H9).
    /// eval의 `RunRecord`가 실효값을 자증할 때 `Config`가 아니라 여기서 읽어야 한다 —
    /// `Config` 쪽을 다시 읽으면 두 값이 같은 출처가 되어 오버라이드 순서 오류를
    /// 탐지하지 못한다(플랜 1R Critical 2)
    pub fn context_tokens(&self) -> usize {
        self.context_tokens
    }

    /// 이 에이전트가 생성 시점에 스냅샷한 턴 상한 (M15 H9). context_tokens()와 같은 이유로
    /// eval의 RunRecord는 cfg가 아니라 여기서 읽어야 순서 오류를 탐지한다
    pub fn max_turns(&self) -> usize {
        self.max_turns
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
            temperature: self.temperature_override.unwrap_or(self.temperature),
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
        self.temperature_override = None; // M10 §5 — run() 지역 수명
        let mut turns = 0;
        let mut tracker = repetition::RepetitionTracker::new();
        // 실행당(run 호출당) 최대 2회 — 턴을 넘나들며 누적, 턴마다 리셋하지 않는다 (스펙 §9)
        let mut overflow_shrinks: u32 = 0;
        let mut mutated_since_verify = false;
        let mut verify_nudged = false;
        // §3-3-1: "현재 미해제 상태를 만든 원인이 파이프인가" — 해제 술어를 평가할
        // 때마다 무조건 대입한다(조건부 건너뛰기 금지, 4R C2)
        let mut unreleased_due_to_pipe = false;
        let mut finish_nudge = finish_nudge::FinishNudge::new();
        // summary 없는 finish 연속 카운트 (M9 §4-1) — 무액션 턴은 유지, 디스패치·거부된
        // 다른 액션이 리셋
        let mut finish_missing_streak: usize = 0;
        let mut finish_args_corrected = false;
        let mut status = status_note::StatusNote::new();
        // M16: certified/dirty only when flag on — no start-scan / mut-gate / STALE when off
        let mut notes_state = if self.repo_notes {
            let ns = crate::notes::NotesState::scan(&self.ctx.root);
            // Always record after scan (0 if empty) — §5-3 notes_bytes_max source
            session.record_extra(
                crate::notes::NOTES_BYTES_MAX_KIND,
                &ns.bytes_max().to_string(),
            );
            Some(ns)
        } else {
            None
        };
        // M16: consecutive update_repo_notes schema rejects → one thrifty correction
        let mut notes_schema_streak: usize = 0;
        let mut notes_schema_corrected = false;
        // M16: length / notes-shaped parse fail → one mouth-small correction
        let mut notes_output_corrected = false;
        let mut saw_mut_gate = false;
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
                        notice_recorded!(
                            session,
                            on_event,
                            format!("(컨텍스트 초과로 보임 — 히스토리 절삭 후 재시도 {overflow_shrinks}/2)")
                        );
                        session.pack(self.input_budget() >> overflow_shrinks);
                    }
                    Err(LlmError::Api { status: 400, body }) if looks_like_context_overflow(&body) => {
                        // M15 H14: on_event만 타던 것을 트랜스크립트에도 남긴다.
                        // 축소 재시도(위 arm)는 M14가 이미 notice_recorded!로 기록하는데
                        // **포기한 런만 흔적이 없어** 배치 후 구분이 불가능했다 (§5-2 ⑤)
                        notice_recorded!(
                            session,
                            on_event,
                            "(컨텍스트 초과 — context_tokens 설정과 서버 로드 설정을 확인하세요)".to_string()
                        );
                        return Err(LlmError::Api { status: 400, body });
                    }
                    other => break other?,
                }
            };

            // M15 H13·§5-2 ②: 축 C의 원자료. 측정 지점 = **응답을 만들어낸
            // 마지막 반복의 직렬화 직전 히스토리**. 세 사실이 이것을 보장한다:
            // ① 오버플로 재시도는 위 loop 안에서 축소 예산으로 다시 pack하므로
            //    루프 종료 후 읽어야 마지막 반복의 상태다
            // ② chat_with_fallback은 inline_system을 켜기만 하고 끄지 못하므로
            //    (self.inline_system = true 한 곳뿐) 호출 후 값이 곧 성공한 요청이 쓴 값이다 —
            //    직렬화 메시지 집합을 바꾸므로 §5-3의 층화 키로 남긴다
            // ③ 루프 종료와 이 지점 사이에 히스토리를 바꾸는 문장이 없다
            //
            // ⚠ 기록만 한다 — 히스토리·상태선·모델 대면 텍스트 어디에도 안 나간다(§5-6)
            let est = session.total_tokens();
            let n_msgs = session.messages().len();
            session.record_extra(
                "usage",
                &serde_json::json!({
                    "prompt_tokens": resp.prompt_tokens(),
                    "completion_tokens": resp.completion_tokens(),
                    "estimate_tokens": est,
                    "messages": n_msgs,
                    "budget": self.input_budget(),
                    "inline_system": self.inline_system,
                    "overflow_shrinks": overflow_shrinks,
                })
                .to_string(),
            );

            // length 판정 **전**에 무조건 기록 — length 턴도 세야 하는 게 이 기록의
            // 목적이다(M14 C-2, M13이 "빈-content 턴"으로만 대리 추정하던 것)
            if let Some(fr) = resp.finish_reason() {
                session.record_extra("finish_reason", fr);
            }

            // 출력 잘림은 파싱 실패와 구분해 교정한다 (스펙 §9). 같은 요청 재시도는
            // 같은 지점에서 다시 잘리므로 "더 짧게"를 지시. 턴을 소모해 max_turns가
            // length 반복의 상한이 되게 한다 (스펙 §3 사각지대)
            if resp.finish_reason() == Some("length") {
                // §4-2(b): 절단 blob을 **히스토리에 push하지 않는다**. 예산 초과를
                // 만드는 것이 이 blob이고, 초과가 쌍 삭제를 부르면 사용자 과제가
                // 함께 사라진다(M13 세션 Z1). 트랜스크립트에는 남긴다 —
                // M13의 디코딩 퇴화 분석이 전부 이 blob을 읽어 만들어졌다
                // §4-2-2: 최종 상태는 "히스토리에 push 안 함 + 트랜스크립트에만".
                // B-1은 그 트랜스크립트 레코드의 **내용**을 "(empty)"에서 추론 꼬리로
                // 바꾼다 — content가 비어도 모델은 예산을 추론에 다 쓴 것이지
                // 아무것도 안 낸 것이 아니다
                let t = resp.text();
                let blob = if !t.is_empty() {
                    t
                } else {
                    let r = resp.reasoning();
                    if r.is_empty() { "(empty)" } else { r }
                };
                session.record_extra("assistant", blob);
                // push가 아니라 push_recovery_notice: ① assistant를 건너뛰었으므로
                // 꼬리가 user다 — 병합해야 role 교대가 유지되는데 교정 담당
                // merge_adjacent_same_role는 pack()의 쌍 삭제 루프 안에서만 돌고
                // (b)의 목적이 바로 pack 미발동이라 영영 교정되지 않는다
                // ② 연속 주입 중복은 pack()이 회수할 수 없는 자리에 쌓인다
                // M16: notes-shaped cut or post-mut-gate → thrifty notes recovery once
                let notes_cut = self.repo_notes
                    && !notes_output_corrected
                    && (saw_mut_gate
                        || crate::tools::update_repo_notes::looks_like_notes_output(blob));
                if notes_cut {
                    notes_output_corrected = true;
                    session.push_recovery_notice(
                        crate::tools::update_repo_notes::NOTES_OUTPUT_CORRECTION,
                    );
                    notice_recorded!(
                        session,
                        on_event,
                        "(응답 잘림 — notes thrifty 교정 주입)".to_string()
                    );
                } else {
                    session.push_recovery_notice(LENGTH_RECOVERY);
                    notice_recorded!(
                        session,
                        on_event,
                        "(응답이 잘림 — 더 짧게 다시 요청)".to_string()
                    );
                }
                let _ = status.on_turn(&status_note::TurnCtx {
                    turn: turns + 1,
                    max_turns: self.max_turns,
                    mutation_ok: false,
                    has_note_channel: false, // record_extra+push_recovery_notice 경로 — 이월
                    mutated_since_verify,
                });
                turns += 1;
                continue;
            }

            // 파싱 실패는 에러를 되먹여 턴당 총 PARSE_ATTEMPTS회 시도 (스펙 §9).
            // 되먹임(assistant 원문 + user 피드백)은 히스토리에 남는다 — 모델이
            // 자기 실패를 문맥으로 보는 것이 의도. max_turns에는 계상하지 않는다
            let mut text = resp.text().to_string();
            let mut attempts = 1;
            let mut turn = loop {
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
                        // M16: notes-shaped broken JSON → thrifty correction once
                        let fb = if self.repo_notes
                            && !notes_output_corrected
                            && crate::tools::update_repo_notes::looks_like_notes_output(&text)
                        {
                            notes_output_corrected = true;
                            format!(
                                "{feedback}\n\n{}",
                                crate::tools::update_repo_notes::NOTES_OUTPUT_CORRECTION
                            )
                        } else {
                            feedback.to_string()
                        };
                        session.push(ChatMessage::user(fb));
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

            // 최종 리뷰 Minor 3(의도된 동작, 문서화만): finish 분기가 아래 M12 §3-2
            // args.tool 정규화 블록보다 먼저이므로, action.tool == "finish"이면서
            // args.tool == "write_file"(또는 다른 등록 도구)인 턴은 정규화를 절대 타지
            // 않는다 — args.tool은 그대로 무시된 채 finish로 처리된다. 스펙 §3-2는
            // args.tool == "finish" 케이스만 다루므로 이 순서는 스펙 범위 밖이며,
            // "정규화 없음"이 안전한 방향(뮤테이션 도구로 오인해 finish를 억누르지
            // 않음)이라 의도적으로 그대로 둔다 — "고치지" 말 것
            if turn.action.tool == "finish" {
                match turn.action.args.get("summary").and_then(|v| v.as_str()) {
                    Some(s) => {
                        // M16 §3-6 finish order: VERIFY (once) → NOTES_STALE (once) → accept
                        if mutated_since_verify && !verify_nudged {
                            verify_nudged = true;
                            let nudge = if unreleased_due_to_pipe { VERIFY_NUDGE_PIPE } else { VERIFY_NUDGE };
                            on_event(AgentEvent::Notice("(검증 없는 종료 — 확인 요청 주입)".to_string()));
                            session.push(tool_result_message("finish", nudge));
                            finish_missing_streak = 0;
                            let _ = finish_nudge.on_turn(finish_nudge::TurnEvent::FinishAttempt); // idle만 리셋 — 발동 불가
                            let _ = status.on_turn(&status_note::TurnCtx {
                                turn: turns + 1,
                                max_turns: self.max_turns,
                                mutation_ok: false,
                                has_note_channel: false, // session.push 경로 — 이월
                                mutated_since_verify,
                            });
                            turns += 1;
                            continue;
                        }
                        if let Some(ref mut notes) = notes_state
                            && !notes.dirty.is_empty()
                            && !notes.notes_stale_nudged
                        {
                            notes.notes_stale_nudged = true;
                            let body = crate::notes::notes_stale_nudge(&notes.dirty);
                            on_event(AgentEvent::Notice(
                                "(notes 미갱신 — stale finish 반려)".to_string(),
                            ));
                            session.push(tool_result_message("finish", &body));
                            finish_missing_streak = 0;
                            let _ = finish_nudge.on_turn(finish_nudge::TurnEvent::FinishAttempt);
                            let _ = status.on_turn(&status_note::TurnCtx {
                                turn: turns + 1,
                                max_turns: self.max_turns,
                                mutation_ok: false,
                                has_note_channel: false,
                                mutated_since_verify,
                            });
                            turns += 1;
                            continue;
                        }
                        return Ok(AgentOutcome::Finished(s.to_string()));
                    }
                    None => {
                        const FINISH_ERR: &str = "Error: finish requires a string `summary` argument, e.g. {\"tool\": \"finish\", \"args\": {\"summary\": \"<your final answer>\"}}";
                        // summary 없는 finish도 반복 계수에 편입 (M5 §7.3 — 기존 §3 사각지대 폐지)
                        finish_missing_streak += 1;
                        // idle만 리셋 — 이 이벤트로는 발동 불가 (M9 §4-2 표 6행)
                        let _ = finish_nudge.on_turn(finish_nudge::TurnEvent::FinishAttempt);
                        let key = format!("finish|{}", turn.action.args);
                        let verdict = tracker.record(&key, FINISH_ERR);
                        // InjectCorrection을 버리면 record()가 래치한 실행당 1회 교정 기회가
                        // 소모된다 — 같은 user 메시지에 병합해 반드시 전달 (본선 스펙 §3 연속 user 금지)
                        let mut body = match verdict {
                            repetition::RepetitionVerdict::InjectCorrection => {
                                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
                                format!("{FINISH_ERR}\n{REPEAT_CORRECTION}")
                            }
                            _ => FINISH_ERR.to_string(),
                        };
                        if finish_missing_streak >= 2 && !finish_args_corrected {
                            finish_args_corrected = true;
                            on_event(AgentEvent::Notice("(finish 인자 누락 반복 — 교정 주입)".to_string()));
                            body = format!("{body}\n{FINISH_ARGS_CORRECTION}");
                        }
                        let _ = status.on_turn(&status_note::TurnCtx {
                            turn: turns + 1,
                            max_turns: self.max_turns,
                            mutation_ok: false,
                            has_note_channel: false, // session.push 경로 — 이월
                            mutated_since_verify,
                        });
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

            // M12 §3-2: args 안의 잉여 `tool` 키 정규화. 게이트·preview보다 **먼저** —
            // 규칙 2의 교체로 비뮤테이션 액션이 뮤테이션 도구가 될 수 있고,
            // 게이트는 교체 결과 도구를 기준으로 판정해야 한다
            // (최종 리뷰 Minor 3) action.tool == "finish"인 턴은 위에서 이미
            // return/continue로 빠져 여기 도달하지 않는다 — finish + args.tool =
            // "write_file" 같은 조합은 절대 정규화되지 않음. 의도된 동작이니 참조
            let mut args_tool_note: Option<&'static str> = None;
            if let Some(inner) =
                turn.action.args.get("tool").and_then(|v| v.as_str()).map(str::to_string)
            {
                if let Some(map) = turn.action.args.as_object_mut() {
                    map.remove("tool");
                }
                if inner == turn.action.tool {
                    args_tool_note = Some(ARGS_TOOL_KEY_NOTE); // 규칙 1
                } else if self.registry.get(&inner).is_some() {
                    turn.action.tool = inner; // 규칙 2 — 등록 도구면 교체
                    args_tool_note = Some(ARGS_TOOL_SWITCH_NOTE);
                } else {
                    args_tool_note = Some(ARGS_TOOL_KEY_NOTE); // 규칙 3 — 미등록(finish 포함): 키만 제거
                }
            }

            on_event(AgentEvent::Action {
                tool: &turn.action.tool,
                args: &turn.action.args,
            });

            // M16 §3-5 mut-gate hard order (after args.tool salvage, before preview/approve):
            // 1) ban edit/write under `.loco/notes/**`
            // 2) if repo_notes && !gate_ok → reject every time (not once-latch)
            // 3) else existing preview → approve → dispatch
            if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
                let path = turn
                    .action
                    .args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let ban_or_gate = if !path.is_empty()
                    && crate::notes::is_under_notes_dir(&self.ctx.root, std::path::Path::new(path))
                {
                    Some(crate::notes::notes_path_ban_body(&turn.action.tool))
                } else if let Some(ref notes) = notes_state {
                    if path.is_empty() || !notes.gate_ok(path) {
                        let p = if path.is_empty() { "<missing path>" } else { path };
                        Some(notes.mut_gate_body(p))
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(body) = ban_or_gate {
                    if body.starts_with(crate::notes::NOTES_MUT_GATE_MARK) {
                        saw_mut_gate = true;
                    }
                    finish_missing_streak = 0;
                    let ev = finish_nudge::TurnEvent::MutationAttempt;
                    let (mut note, stop) =
                        self.track_and_note(&mut tracker, &turn, &body, args_tool_note, on_event);
                    self.update_perturb(&tracker, on_event);
                    if !stop && let Some(nudge) = finish_nudge.on_turn(ev) {
                        let nudge = if unreleased_due_to_pipe && nudge == finish_nudge::FINISH_NUDGE
                        {
                            finish_nudge::FINISH_NUDGE_PIPE
                        } else {
                            nudge
                        };
                        on_event(AgentEvent::Notice(
                            "(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string(),
                        ));
                        note = merge_note(note, nudge);
                    }
                    if !stop
                        && let Some(s) = status.on_turn(&status_note::TurnCtx {
                            turn: turns + 1,
                            max_turns: self.max_turns,
                            mutation_ok: false,
                            has_note_channel: true,
                            mutated_since_verify,
                        })
                    {
                        session.remove_status_note();
                        note = merge_note(note, &s);
                    }
                    session.push_tool_result(
                        &turn.action.tool,
                        &turn.action.args,
                        &body,
                        note.as_deref(),
                    );
                    if stop {
                        return Ok(AgentOutcome::RepetitionStop);
                    }
                    turns += 1;
                    continue;
                }
            }

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
                    finish_missing_streak = 0;
                    let ev = match turn.action.tool.as_str() {
                        "edit_file" | "write_file" => finish_nudge::TurnEvent::MutationAttempt,
                        _ => finish_nudge::TurnEvent::Other, // 게이트 거부된 run_command — 불변 (§4-2 표)
                    };
                    let (mut note, stop) = self.track_and_note(&mut tracker, &turn, &body, args_tool_note, on_event);
                    self.update_perturb(&tracker, on_event);
                    // 반복정지 우선 (§4-2) — 정지 턴에는 넛지를 평가하지 않는다.
                    // M14 B-4: 이 `!stop` 가드는 finish_nudge 억제 절반이다. 거부 경로의
                    // 이벤트는 MutationAttempt/Other 둘뿐인데, MutationAttempt(write_file/
                    // edit_file)는 disarm()을 불러 armed가 참이 될 수 없다. Other(denied
                    // run_command)는 idle을 진전시키지 않는데, idle은 VerifyOk/ReadOnly
                    // 이벤트에만 진전하고 거부 경로는 둘 다 방출하지 않는다. 또한 stop은
                    // 해당 턴에서 run()을 즉시 종료하므로, armed 임계치를 넘으려면 승인
                    // 경로에서 이미 넘어야 한다(그 쪽도 !stop 가드 있음). 그래서 이 가드를
                    // 지워도 어떤 시나리오에서도 FINISH_NUDGE는 나오지 않는다(실측 확인,
                    // 태스크 7 5-b) —
                    // `a_rejected_action_on_a_repetition_stop_turn_emits_no_finish_nudge`는
                    // 핀이 아니라 관측점: 훗날 거부 경로의 이벤트 매핑이 바뀌어 armed가
                    // 참이 될 수 있게 되면 그 테스트가 빨간불이 된다. 이 사실을 이유로
                    // 가드를 지우지 말 것 — 옆의 finish_nudge 이벤트 매핑(VerifyOk 등)도
                    // 손대지 말 것(스펙 §3-3-3, 실측으로 기각된 대안: 손대면 재검증 루프
                    // 감지가 죽는다).
                    if !stop && let Some(nudge) = finish_nudge.on_turn(ev) {
                        let nudge = if unreleased_due_to_pipe && nudge == finish_nudge::FINISH_NUDGE {
                            finish_nudge::FINISH_NUDGE_PIPE
                        } else {
                            nudge
                        };
                        on_event(AgentEvent::Notice("(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string()));
                        note = merge_note(note, nudge);
                    }
                    // M14 B-4: 이 `!stop` 가드가 status 억제 절반이다 — RepetitionStop
                    // 턴에는 상태선을 붙이지 않는다(M11 불변식). 제거하면
                    // `a_rejected_mutation_on_a_repetition_stop_turn_emits_no_status_note`가
                    // 빨간불이 된다(실측 확인, 태스크 7 5-a) — 그 테스트가 이 가드의 핀이다.
                    if !stop
                        && let Some(s) = status.on_turn(&status_note::TurnCtx {
                            turn: turns + 1,
                            max_turns: self.max_turns,
                            mutation_ok: false, // 거부 — 뮤테이션 아님
                            has_note_channel: true,
                            mutated_since_verify,
                        })
                    {
                        session.remove_status_note();
                        note = merge_note(note, &s);
                    }
                    session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
                    if stop {
                        return Ok(AgentOutcome::RepetitionStop);
                    }
                    turns += 1;
                    continue;
                }
            }
            // M9 §4-2: 반복-호출 신호는 tracker.record()(track_and_note 내부) **전에**
            // 조회해야 자기-매치가 없다
            let call_key = format!("{}|{}", turn.action.tool, turn.action.args);
            let repeated_call = tracker.seen_key(&call_key);
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
            // M12 §2-3: run_command 결과를 여기서 **1회** 파싱해 상태선과
            // (T5에서) 두 술어가 공유한다. 저장 조건과 술어 조건의 계약이 한 지점에 모인다
            let cmd_exit = if turn.action.tool == "run_command" && dispatch_ok {
                body.lines().next().and_then(|l| l.strip_prefix("exit code: ")).map(str::to_string)
            } else {
                None
            };
            let cmd_summary = cmd_exit
                .as_ref()
                .and_then(|_| crate::test_summary::parse_test_summary(&body));
            // 공허 런 = 필터가 아무 테스트도 못 맞힌 실행. "검증"으로 인정하지 않는다 (M12 §2-4)
            let empty_verify = cmd_summary.as_ref().is_some_and(|s| s.ran == 0 && s.filtered_out > 0);
            // §3-3: 파이프가 섞이면 exit code가 파이프라인 마지막 명령의 것이라
            // 검증 성립을 알 수 없다. **해제 술어에만** 건다 — VerifyOk 매핑을
            // 건드리면 재검증 루프 감지가 죽는다(§3-3-3, 실측으로 기각된 대안)
            let is_piped = turn.action.tool == "run_command"
                && turn
                    .action
                    .args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .is_some_and(crate::tools::run_command::has_unquoted_pipe);
            if dispatch_ok {
                if turn.action.tool == "run_command" {
                    // M12 §2-4: 공허 런은 VERIFY_NUDGE를 해제하지 않는다
                    // (해제 조건이었던 "Ok이면 종료코드 무관"에서 공허 런과 파이프를 제외)
                    // §3-3-1: 플래그는 "현재 미해제 상태를 만든 원인이 파이프인가"다.
                    // 조건부 건너뛰기 금지 — 해제 성공 시에도 갱신해야 한다.
                    // 소비자가 둘이고 결합 조건이 다르기 때문: VERIFY_NUDGE는
                    // mutated_since_verify와 묶여 낡은 값을 못 읽지만
                    // FINISH_NUDGE는 묶여 있지 않아 그대로 읽는다 (4R C2)
                    let released = !empty_verify && !is_piped;
                    if released {
                        mutated_since_verify = false;
                    }
                    unreleased_due_to_pipe = !released && is_piped;
                    status.record_command_result(cmd_exit.clone(), cmd_summary.clone(), is_piped);
                } else if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
                    // M16 §3-3 VERIFY whitelist: only edit_file|write_file re-arm VERIFY.
                    // `update_repo_notes` is is_mutating but must NOT set mutated_since_verify.
                    mutated_since_verify = true;
                    unreleased_due_to_pipe = false; // 뮤테이션이 원인을 갈아치운다
                }
                if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
                    status.record_mutation(&turn.action.args);
                    if let Some(p) = turn.action.args.get("path").and_then(|v| v.as_str()) {
                        tracker.record_mutation_ok(p);
                        if let Some(ref mut notes) = notes_state {
                            notes.mark_dirty_for_path(p);
                        }
                    }
                }
                // M16: successful notes update → certify + exact dirty clear + bytes extra
                if turn.action.tool == "update_repo_notes"
                    && let Some(ref mut notes) = notes_state
                    && let Some(raw) = turn.action.args.get("path").and_then(|v| v.as_str())
                    && let Ok(key) = crate::notes::normalize_key(raw)
                {
                    let nbytes = turn
                        .action
                        .args
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.len())
                        .unwrap_or(0);
                    notes.certify(&key, nbytes);
                    notes.clear_dirty_key(&key);
                    session.record_extra(
                        crate::notes::NOTES_BYTES_MAX_KIND,
                        &notes.bytes_max().to_string(),
                    );
                }
                // M15 H10·§5-4: 항해/수선 지표의 원자료. **툴별로 분리**한다 —
                // 셋을 한 집합으로 합치면 §1-1 축의 근거인 M8 실패 분석의
                // "미열람(grep만)" 구분이 사라져 축과 계측기가 어긋난다.
                //
                // 항해 지표를 read_file 집합만으로 정의하는 것은 **축의 정의에서
                // 나오는 설계 결정**이지 기술적 제약이 아니다. §1-1이 이 트랙의
                // 축을 세운 근거가 M8 실패 분석의 "monthly.rs 미열람(grep만)"이고,
                // 그 구분은 "grep으로 스쳤는가"와 "열어서 읽었는가"를 **가르는 것이
                // 목적**이다 — 그래서 grep으로 오라클을 지목한 런을 항해 성공에서
                // 빼는 것은 부작용이 아니라 의도된 방향이다.
                // ⚠ grep은 path로 **파일 하나를 지목할 수 있다**(grep.rs의
                // `if base.is_file()` 분기). "grep은 경로를 못 준다"는 것은 사실이
                // 아니다 — 스펙 개정 10이 이 근거를 정정했다. list_files는 참이다
                // (walk_entries가 `if p == base { continue }`로 시작점을 버린다).
                // 경로는 status_note::normalize로 정규화해 표기 변형을 합산한다
                if matches!(
                    turn.action.tool.as_str(),
                    "read_file" | "edit_file" | "write_file" | "grep" | "list_files"
                ) {
                    let touched = matches!(
                        turn.action.tool.as_str(),
                        "read_file" | "edit_file" | "write_file"
                    )
                    .then(|| turn.action.args.get("path").and_then(|v| v.as_str()))
                    .flatten()
                    .map(status_note::normalize);
                    session.record_extra(
                        "touch",
                        &serde_json::json!({"tool": turn.action.tool, "path": touched}).to_string(),
                    );
                }
            }
            finish_missing_streak = 0;
            let ev = match turn.action.tool.as_str() {
                "edit_file" | "write_file" => {
                    if dispatch_ok {
                        finish_nudge::TurnEvent::MutationOk
                    } else {
                        finish_nudge::TurnEvent::MutationAttempt
                    }
                }
                // §4-2: "성공 검증" = Ok ∧ 첫 줄 exit code 0. 타임아웃·취소·Err 본문에는
                // 이 줄이 없어 자연 배제. M12 §2-4: 공허 런(필터 0매치)도 배제 —
                // VerifyOther로 떨어뜨려 기존 무장까지 내린다
                "run_command" => {
                    if dispatch_ok && cmd_exit.as_deref() == Some("0") && !empty_verify {
                        finish_nudge::TurnEvent::VerifyOk { repeat: repeated_call }
                    } else {
                        finish_nudge::TurnEvent::VerifyOther
                    }
                }
                "read_file" | "grep" | "list_files" => finish_nudge::TurnEvent::ReadOnly { repeat: repeated_call },
                _ => finish_nudge::TurnEvent::Other, // 미지 도구 (§4-2 표)
            };
            let (mut note, stop) = self.track_and_note(&mut tracker, &turn, &body, args_tool_note, on_event);
            self.update_perturb(&tracker, on_event);
            // M16: 2 consecutive notes schema rejects → thrifty correction once (before generic stop)
            if turn.action.tool == "update_repo_notes" {
                if body.contains(
                    crate::tools::update_repo_notes::NOTES_SCHEMA_REJECT_PREFIX,
                ) {
                    notes_schema_streak += 1;
                    if notes_schema_streak >= 2 && !notes_schema_corrected {
                        notes_schema_corrected = true;
                        on_event(AgentEvent::Notice(
                            "(notes 스키마 반복 실패 — thrifty 교정 주입)".to_string(),
                        ));
                        note = merge_note(
                            note,
                            crate::tools::update_repo_notes::NOTES_SCHEMA_CORRECTION,
                        );
                    }
                } else if dispatch_ok {
                    notes_schema_streak = 0;
                }
            }
            // 반복정지 우선 (§4-2) — 정지 턴에는 넛지를 평가하지 않는다
            if !stop && let Some(nudge) = finish_nudge.on_turn(ev) {
                let nudge = if unreleased_due_to_pipe && nudge == finish_nudge::FINISH_NUDGE {
                    finish_nudge::FINISH_NUDGE_PIPE
                } else {
                    nudge
                };
                on_event(AgentEvent::Notice("(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string()));
                note = merge_note(note, nudge);
            }
            if !stop
                && let Some(s) = status.on_turn(&status_note::TurnCtx {
                    turn: turns + 1,
                    max_turns: self.max_turns,
                    mutation_ok: dispatch_ok
                        && matches!(turn.action.tool.as_str(), "edit_file" | "write_file"),
                    has_note_channel: true,
                    mutated_since_verify,
                })
            {
                session.remove_status_note();
                note = merge_note(note, &s);
            }
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
        args_tool_note: Option<&'static str>,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) -> (Option<String>, bool) {
        let mut notes: Vec<&str> = Vec::new();
        // 액션 레벨 필드 salvage (기존 M5 경로)
        if turn.salvaged {
            notes.push(SALVAGE_NOTE);
        }
        // args 안 `tool` 키 (M12 §3-2) — 위와 배타가 아니다. 한 턴에 겹칠 수 있다
        if let Some(n) = args_tool_note {
            notes.push(n);
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
        if let Some(strategy) = tracker.error_correction(&turn.action.tool, &turn.action.args, body) {
            on_event(AgentEvent::Notice("(동일 에러 반복 — 전략 교정 주입)".to_string()));
            notes.push(strategy);
        }
        let joined = notes.join("\n");
        ((!joined.is_empty()).then_some(joined), false)
    }

    /// M10 §5: 스트릭 상태를 오버라이드에 반영 — track_and_note(error_correction
    /// 경유) 직후에만 호출한다. 무액션·finish 턴은 호출 지점에 닿지 않아 유지된다.
    /// M12 §3-1·§4-1: 트리거만 확대한다(메커니즘·수명·원복 규칙은 불변) —
    /// 파일별 비연속 S/R 재발과 missing-field 오형 복사 루프도 저온 어트랙터다
    fn update_perturb(
        &mut self,
        tracker: &repetition::RepetitionTracker,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) {
        let triggered =
            tracker.sr_streak() >= 2 || tracker.sr_file_streak() >= 2 || tracker.badargs_streak() >= 2;
        let want = triggered.then_some(SR_PERTURB_TEMPERATURE);
        if want.is_some() && self.temperature_override.is_none() {
            on_event(AgentEvent::Notice("(동일 오류 반복 감지 — temperature 일시 상향)".to_string()));
        }
        self.temperature_override = want;
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
                    self.schema_fallback = true;
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

/// 교정 노트에 문장 하나를 덧붙인다 (없으면 새로) — tool_result 병합 규칙 유지
fn merge_note(note: Option<String>, extra: &str) -> Option<String> {
    Some(match note {
        Some(n) => format!("{n}\n{extra}"),
        None => extra.to_string(),
    })
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
                    reasoning_content: None,
                },
                finish_reason: Some(reason.into()),
            }],
            usage: None,
        })
    }

    fn ok_with_reasoning(content: &str, reasoning: &str, reason: &str) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: Some(content.to_string()),
                    reasoning_content: Some(reasoning.to_string()),
                },
                finish_reason: Some(reason.to_string()),
            }],
            usage: None,
        })
    }

    fn ok(text: &str) -> Result<ChatResponse, LlmError> {
        ok_with_reason(text, "stop")
    }

    /// M15 H13 — usage를 실은 응답. 기존 `ok()`는 `usage: None`이다
    fn ok_with_usage(text: &str, prompt: u32, completion: u32) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: Some(text.into()),
                    reasoning_content: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(crate::llm::types::Usage {
                completion_tokens: Some(completion),
                prompt_tokens: Some(prompt),
            }),
        })
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

    fn make_guided_agent(script: &Scripted, root: std::path::PathBuf, max_turns: usize) -> Agent<&Scripted> {
        // M16: legacy mut tests opt out of notes (gate lands in T3; keep false-paired)
        let config = Config { max_turns, repo_notes: false, ..Default::default() };
        Agent::new(script, Registry::guided(false), ToolCtx::new(root), "test-model".into(), &config)
    }

    fn session_contains(session: &Session, needle: &str) -> bool {
        session.messages().iter().any(|m| m.content.contains(needle))
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

    /// M14 T4 — 스크립트를 guided 에이전트로 돌리고 (결과, 모든 tool_result 본문)을
    /// 반환한다. 파이프 검증 테스트들이 세션 메시지를 훑는 기존 패턴을 헬퍼로 뽑은 것
    async fn run_capturing_tool_results(
        script: Vec<Result<ChatResponse, LlmError>>,
    ) -> (AgentOutcome, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let scripted = Scripted::new(script);
        let mut agent = make_guided_agent(&scripted, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let notes = session
            .messages()
            .iter()
            .filter(|m| m.content.contains("<tool_result"))
            .map(|m| m.content.clone())
            .collect();
        (outcome, notes)
    }

    #[test]
    fn schema_fallback_fired_is_false_on_a_fresh_agent() {
        // 폴백 게터의 초기 상태 핀 — use_json_schema가 true로 시작하므로 false여야
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![]);
        let agent = make_agent(&script, dir.path().to_path_buf(), 25);
        assert!(!agent.schema_fallback_fired(), "새 에이전트는 폴백 미발동");
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
    async fn duplicate_tool_key_inside_args_is_stripped_with_a_note() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "a.txt", "tool": "read_file"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let mut dispatched_args: Vec<serde_json::Value> = Vec::new();
        agent
            .run(&mut session, "x", &mut crate::agent::approval::AutoApprover::default(), &mut |ev| {
                if let AgentEvent::Action { args, .. } = ev {
                    dispatched_args.push(args.clone());
                }
            })
            .await
            .unwrap();
        assert!(session_contains(&session, "hi"), "read_file은 정상 디스패치");
        assert!(session_contains(&session, ARGS_TOOL_KEY_NOTE), "전용 노트 (M12 §3-2 규칙 1)");
        assert!(!session_contains(&session, SALVAGE_NOTE), "SALVAGE_NOTE는 정반대 진술이라 붙으면 안 된다");
        assert!(
            dispatched_args[0].get("tool").is_none(),
            "잉여 `tool` 키가 실제로 args에서 제거돼야 한다(노트의 주장과 일치): {:?}",
            dispatched_args[0]
        );
    }

    #[tokio::test]
    async fn a_different_tool_named_in_args_switches_the_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/inner.rs"), "x").unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "sub", "tool": "list_files"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(!session_contains(&session, "is a directory, not a file"), "교체로 루프가 성립하지 않는다 (uv-2)");
        assert!(session_contains(&session, "inner.rs"), "list_files로 교체 디스패치");
        assert!(session_contains(&session, ARGS_TOOL_SWITCH_NOTE), "규칙 2 전용 노트");
    }

    #[tokio::test]
    async fn an_unknown_tool_name_in_args_is_only_stripped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let script = Scripted::new(vec![
            // finish는 레지스트리 밖 — 교체 대상이 아니다(규칙 3)
            ok(&turn("read_file", serde_json::json!({"path": "a.txt", "tool": "finish"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(s) if s == "d"), "액션 도구 유지");
        assert!(session_contains(&session, "hi"));
        assert!(session_contains(&session, ARGS_TOOL_KEY_NOTE));
    }

    #[tokio::test]
    async fn salvage_note_and_args_tool_key_note_are_not_mutually_exclusive() {
        // 플랜 §11 (line 1546): 두 노트는 서로 다른 오형을 가리키므로 배타로 두지
        // 않는다 — 액션 레벨 산재 필드 salvage(SALVAGE_NOTE)와 args 안 `tool` 키
        // 오형(ARGS_TOOL_KEY_NOTE)이 한 턴에 겹치면 둘 다 나가야 한다.
        // path는 action 레벨(산재 필드), args.tool은 action.tool과 같은 값(규칙 1) —
        // 두 규칙이 같은 턴에서 동시에 걸리는 조합
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let both = r#"{"thought": "read", "action": {"tool": "read_file", "args": {"tool": "read_file"}, "path": "a.txt"}}"#;
        let script = Scripted::new(vec![ok(both), ok(&finish("done"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "정상 디스패치 후 finish까지 도달");
        assert!(session_contains(&session, "hi"), "read_file은 정상 디스패치");
        assert!(session_contains(&session, SALVAGE_NOTE), "산재 필드 salvage 노트도 함께 나가야 한다");
        assert!(session_contains(&session, ARGS_TOOL_KEY_NOTE), "args.tool 전용 노트도 함께 나가야 한다");
    }

    #[tokio::test]
    async fn the_switched_tool_is_what_the_approval_gate_sees() {
        struct DenyAll;
        impl crate::agent::approval::Approver for DenyAll {
            fn approve(&mut self, _req: &ApprovalRequest) -> Decision {
                Decision::Deny { reason: "테스트 거부".into() }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let script = Scripted::new(vec![
            // 교체 전이면 read_file(비뮤테이션)이라 게이트를 아예 통과하지 못한다
            ok(&turn("read_file", serde_json::json!({"path": "a.txt", "content": "x", "tool": "write_file"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        agent.run(&mut session, "x", &mut DenyAll, &mut |_| {}).await.unwrap();
        assert!(session_contains(&session, "Denied:"), "게이트는 교체 결과 도구로 판정해야 한다 (M12 §3-2)");
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "hi", "거부됐으므로 미수정");
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

    #[tokio::test]
    async fn finish_missing_summary_twice_gets_args_correction_once() {
        let dir = tempfile::tempdir().unwrap();
        let empty = turn("finish", serde_json::json!({}));
        let script = Scripted::new(vec![ok(&empty), ok(&empty), ok(&finish("done"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let hits = session
            .messages()
            .iter()
            .filter(|m| m.content.contains("Do not call finish with empty args again"))
            .count();
        assert_eq!(hits, 1, "2연속에 정확히 1회 주입 (M9 §4-1)");
    }

    #[tokio::test]
    async fn dispatched_action_resets_finish_args_streak() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let empty = turn("finish", serde_json::json!({}));
        let read = turn("read_file", serde_json::json!({"path": "a.txt"}));
        let script = Scripted::new(vec![ok(&empty), ok(&read), ok(&empty), ok(&finish("done"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(!session_contains(&session, "Do not call finish with empty args again"), "사이에 디스패치된 액션 → 리셋 (§4-1)");
    }

    #[tokio::test]
    async fn length_cut_between_missing_finishes_keeps_the_streak() {
        let dir = tempfile::tempdir().unwrap();
        let empty = turn("finish", serde_json::json!({}));
        let script = Scripted::new(vec![
            ok(&empty),
            ok_with_reason("truncated...", "length"), // 무액션 턴 — 스트릭 유지 (§4-1)
            ok(&empty),
            ok(&finish("done")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session_contains(&session, "Do not call finish with empty args again"));
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
    async fn length_turns_keep_the_task_message_and_leave_the_blob_only_in_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");

        // ⚠ blob 크기가 이 테스트의 구별력을 정한다. estimate_tokens = len/4 이므로
        // 400자는 100토큰이고 5회 = 500토큰이라 예산 1843을 못 넘겨 **쌍 삭제가
        // 발동하지 않는다** — 수선을 되돌려도 단언 ①이 통과한다(플랜 리뷰 실측:
        // 400 구별불가 / 1500부터 구별). 1500자 이상을 쓸 것
        let mut v = Vec::new();
        for _ in 0..5 {
            // LlmError는 Clone을 파생하지 않는다 — vec![x; 5]가 아니라 루프로 push
            v.push(ok_with_reason(&"X".repeat(1500), "length"));
        }
        v.push(ok(&finish("done")));
        let script = Scripted::new(v);

        let config = Config {
            context_tokens: 4096,
            max_output_tokens: 2048, // 예산 1843
            repo_notes: false,
            ..Default::default()
        };
        let mut agent = Agent::new(
            &script,
            Registry::guided(false),
            ToolCtx::new(dir.path().to_path_buf()),
            "m".into(),
            &config,
        );
        let mut session = Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        // run()이 요청을 스스로 push한다 — 과제를 미리 push하지 말 것
        let out = run_quiet(&mut agent, &mut session, "TASK: 한국어 과제 원문").await.unwrap();

        // T1이 input_budget 공식을 고치는 태스크다. 하한이 예산을 끌어올리면 이 blob이
        // 조용히 구별력을 잃는다 — 요란하게 깨지도록 예산을 직접 단언한다
        // (임계 ~775자, 1500은 약 1.9배 여유 — 플랜 리뷰 실측)
        // `input_budget()`은 비공개지만 `mod tests`가 `agent`의 자식 모듈이라 보인다 —
        // **이 테스트는 반드시 `agent/mod.rs`의 `mod tests` 안에 있어야 한다**
        assert_eq!(
            agent.input_budget(),
            1843,
            "T1이 예산 공식을 바꿨다면 blob 크기를 재측정할 것(임계 ~775자)"
        );

        // ① 과제 생존
        let hist = session.messages().iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n");
        assert!(hist.contains("TASK: 한국어 과제 원문"), "과제가 삭제됐다:\n{hist}");

        // ④ **연속** length가 5회여도 회복 문구는 1벌
        assert_eq!(hist.matches(LENGTH_RECOVERY).count(), 1, "회복 문구 중복:\n{hist}");

        // ③ role 교대 무손상
        for w in session.messages().windows(2) {
            assert_ne!(w[0].role, w[1].role, "role 연속: {:?}", session.messages());
        }

        // ② 절단 blob은 트랜스크립트에 남는다 (실 파일이어야 한다 —
        //    Transcript::disabled()로는 이 단언이 공허하게 통과한다)
        let jsonl = std::fs::read_to_string(&tpath).unwrap();
        assert!(jsonl.contains(&"X".repeat(1500)), "절단 blob이 트랜스크립트에서 사라졌다");

        assert!(matches!(out, AgentOutcome::Finished(_)));
    }

    #[tokio::test]
    async fn a_length_turn_with_only_reasoning_records_the_reasoning_tail_not_empty() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");

        // content 공백 + reasoning_content 있음 + finish_reason length — 파일럿 형태
        let script = Scripted::new(vec![
            ok_with_reasoning("", "I was thinking about src/lib.rs", "length"),
            ok(&finish("done")),
        ]);
        let config = Config { repo_notes: false, ..Default::default() };
        let mut agent = Agent::new(
            &script,
            Registry::guided(false),
            ToolCtx::new(dir.path().to_path_buf()),
            "m".into(),
            &config,
        );
        let mut session = Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        let _ = run_quiet(&mut agent, &mut session, "TASK").await.unwrap();

        let jsonl = std::fs::read_to_string(&tpath).unwrap();
        assert!(jsonl.contains("I was thinking about src/lib.rs"), "추론 꼬리가 안 남았다");
        assert!(!jsonl.contains("\"(empty)\""), "(empty) 리터럴이 남았다:\n{jsonl}");
    }

    #[tokio::test]
    async fn finish_reason_and_overflow_notices_reach_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");

        // 오버플로 400(재시도로 회복) → length 턴 → finish. 세 신호 모두 한 런에서
        // 트랜스크립트로 나가는지 본다: finish_reason(스톱값 포함), length 값,
        // 오버플로 Notice(M13에서 "0이 아니라 미측정"이던 바로 그 신호)
        let overflow = || Err(LlmError::Api { status: 400, body: "context length exceeded".into() });
        let script = Scripted::new(vec![overflow(), ok_with_reason("cut", "length"), ok(&finish("done"))]);
        let config = Config { repo_notes: false, ..Default::default() };
        let mut agent = Agent::new(
            &script,
            Registry::guided(false),
            ToolCtx::new(dir.path().to_path_buf()),
            "m".into(),
            &config,
        );
        // new_session()은 Transcript::disabled()를 쓴다 — 이 테스트는 실 파일이 필요하다
        // (그래야 아래 파일 읽기 단언이 공허하게 통과하지 않는다)
        let mut session = Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        let out = run_quiet(&mut agent, &mut session, "TASK").await.unwrap();
        assert!(matches!(out, AgentOutcome::Finished(_)), "회복 후 정상 종료해야 단언 대상 신호가 다 남는다");

        let jsonl = std::fs::read_to_string(&tpath).unwrap();
        // finish_reason: length 턴에서도(판정 이전에) 남는다 — M13은 이걸 못 세서
        // "빈-content 턴"을 대리 지표로 썼다
        assert!(jsonl.contains("\"kind\":\"finish_reason\""), "finish_reason이 안 남았다:\n{jsonl}");
        assert!(jsonl.contains("\"content\":\"length\""), "length 값이 안 남았다:\n{jsonl}");

        // 오버플로 Notice: M13 스펙 리뷰 C1이 지적한 "배치는 끝났는데 흔적 0" 공백
        assert!(jsonl.contains("\"kind\":\"notice\""), "notice kind가 안 남았다:\n{jsonl}");
        assert!(jsonl.contains("컨텍스트 초과로 보임"), "오버플로 notice 본문이 안 남았다:\n{jsonl}");
    }

    /// M15 H13·§5-2 ② — 턴마다 서버 실측과 추정치를 나란히 남긴다.
    /// 측정 지점은 **응답을 만들어낸 마지막 반복의 직렬화 직전 히스토리**다.
    /// ⚠ `new_session`은 `Transcript::disabled()`라 이 테스트에 못 쓴다 —
    /// 실 파일이어야 단언이 공허하지 않다(M14가 같은 함정을 겪었다)
    #[tokio::test]
    async fn each_turn_records_usage_next_to_the_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        let script = Scripted::new(vec![ok_with_usage(&finish("done"), 999, 11)]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        let out = run_quiet(&mut agent, &mut session, "요청").await.unwrap();
        assert!(matches!(out, AgentOutcome::Finished(_)));

        let text = std::fs::read_to_string(&tpath).unwrap();
        let rec: serde_json::Value = text
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|v| v["kind"] == "usage")
            .expect("턴마다 usage 이벤트가 있어야 한다");
        let body: serde_json::Value =
            serde_json::from_str(rec["content"].as_str().unwrap()).unwrap();
        assert_eq!(body["prompt_tokens"], 999);
        assert_eq!(body["completion_tokens"], 11);
        assert!(body["estimate_tokens"].as_u64().unwrap() > 0, "추정치가 나란히 있어야 한다");
        assert!(body["budget"].as_u64().unwrap() > 0);
        assert_eq!(body["inline_system"], false, "§5-3 층화 키");
        assert_eq!(body["overflow_shrinks"], 0);
    }

    /// 짝 — 서버가 usage를 안 주면 `null`이어야 한다. 0으로 떨어지면 §5-3 회귀가
    /// 원점을 지나는 거짓 관측을 얻는다(T10의 `missing_usage_is_none_not_zero`와 같은 규율)
    #[tokio::test]
    async fn missing_usage_records_null_not_zero() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        let script = Scripted::new(vec![ok(&finish("done"))]); // usage: None
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        run_quiet(&mut agent, &mut session, "요청").await.unwrap();

        let text = std::fs::read_to_string(&tpath).unwrap();
        let rec: serde_json::Value = text
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|v| v["kind"] == "usage")
            .expect("usage 이벤트는 서버 응답과 무관하게 남아야 한다");
        let body: serde_json::Value =
            serde_json::from_str(rec["content"].as_str().unwrap()).unwrap();
        assert!(body["prompt_tokens"].is_null(), "0이 아니라 null: {body}");
        assert!(body["estimate_tokens"].as_u64().unwrap() > 0, "추정치는 서버와 무관하게 있다");
    }

    /// M15 H14·§5-2 ⑤ — 오버플로로 **포기**한 런이 트랜스크립트에 남아야 한다.
    /// M13이 △(계측 불가)로 분류한 2종 중 하나다. 축소 재시도(2회)는 M14가
    /// 이미 기록하므로 여기서 닫는 것은 3번째 400(최종 포기)뿐이다
    #[tokio::test]
    async fn overflow_giveup_is_recorded_in_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        // 컨텍스트 초과 400을 3번 — 2회는 축소 재시도, 3번째가 최종 포기.
        // LlmError는 Clone을 파생하지 않는다 — vec![x; 3]이 아니라 루프로 push
        let mut v = Vec::new();
        for _ in 0..3 {
            v.push(Err(LlmError::Api {
                status: 400,
                body: "context length exceeded".into(),
            }));
        }
        let script = Scripted::new(v);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        let err = run_quiet(&mut agent, &mut session, "요청").await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 400, .. }), "{err:?}");

        let text = std::fs::read_to_string(&tpath).unwrap();
        let notices: Vec<&str> = text.lines().filter(|l| l.contains("\"kind\":\"notice\"")).collect();
        assert_eq!(
            notices.iter().filter(|l| l.contains("히스토리 절삭 후 재시도")).count(),
            2,
            "축소 재시도 2회 (M14가 이미 기록): {notices:?}"
        );
        assert_eq!(
            notices.iter().filter(|l| l.contains("컨텍스트 초과 — context_tokens")).count(),
            1,
            "최종 포기도 기록돼야 한다 (M15 H14): {notices:?}"
        );
    }

    /// M15 H10·§5-4 — 툴별로 **분리**해 남긴다. 합치면 §1-1 축의 근거인
    /// M8 실패 분석의 "미열람(grep만)" 구분이 사라진다
    #[tokio::test]
    async fn touched_files_are_recorded_per_tool() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "fn main() {}\n").unwrap();

        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "src/lib.rs"}))),
            ok(&turn("grep", serde_json::json!({"pattern": "fn"}))),
            // 리뷰 지적(경계 케이스 누락): path를 **준** grep 호출. 내부 matches!가
            // grep을 뺀 올바른 구현에서만 null로 남는다 — 내부·외부 집합을 합친
            // 회귀 구현이라면 이 호출이 "src/lib.rs"로 기록돼 아래 단언이 잡아낸다
            ok(&turn("grep", serde_json::json!({"pattern": "fn", "path": "src/lib.rs"}))),
            // list_files도 호출 계수일 뿐임을 고정 — path를 줘도 null이어야 한다
            ok(&turn("list_files", serde_json::json!({"path": "src"}))),
            ok(&turn("write_file", serde_json::json!({"path": "src/lib.rs", "content": "fn main() {}\n// x\n"}))),
            ok(&turn("run_command", serde_json::json!({"command": "true"}))),
            ok(&finish("done")),
            ok(&finish("done")), // VERIFY_NUDGE가 1차 finish를 반려한다
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        run_quiet(&mut agent, &mut session, "요청").await.unwrap();

        let touches: Vec<serde_json::Value> = std::fs::read_to_string(&tpath)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .filter(|v| v["kind"] == "touch")
            .map(|v| serde_json::from_str(v["content"].as_str().unwrap()).unwrap())
            .collect();

        // read_file은 경로를 준다 — 항해 지표의 유일한 원천
        let read = touches.iter().find(|t| t["tool"] == "read_file").expect("read_file 기록");
        assert_eq!(read["path"], "src/lib.rs");
        // grep 호출은 두 건 — 하나는 path 없이, 하나는 path를 주고 호출했다.
        // 리뷰 지적: 기존 테스트는 path 없는 호출만 있어 "outer/inner matches!를
        // 합친" 회귀와 구별이 안 됐다(둘 다 null을 낸다). path를 **준** 쪽까지
        // 둘 다 null이어야 진짜 경계 단언이 된다 — path 있는 grep이 null이 아니라
        // "src/lib.rs"로 나오면 내부 matches!가 삭제된 회귀다.
        let grep_touches: Vec<&serde_json::Value> =
            touches.iter().filter(|t| t["tool"] == "grep").collect();
        assert_eq!(grep_touches.len(), 2, "grep 호출 두 건이 각각 기록돼야 한다: {grep_touches:?}");
        assert!(
            grep_touches.iter().all(|t| t["path"].is_null()),
            "path를 줘도 grep은 null로 기록된다 (합친-집합 회귀면 여기서 실패): {grep_touches:?}"
        );
        // list_files도 path를 줬지만 호출 계수일 뿐 — null로 남아야 한다
        let list = touches.iter().find(|t| t["tool"] == "list_files").expect("list_files 기록");
        assert!(list["path"].is_null(), "path를 줘도 list_files는 null로 기록된다: {list}");
        // write_file은 수선 지표의 원천
        let write = touches.iter().find(|t| t["tool"] == "write_file").expect("write_file 기록");
        assert_eq!(write["path"], "src/lib.rs");
        // finish·run_command는 기록 대상이 아니다
        assert!(touches.iter().all(|t| t["tool"] != "run_command"));
        assert!(touches.iter().all(|t| t["tool"] != "finish"));
        // 총 touch 이벤트 수 고정: read_file 1 + grep 2 + list_files 1 + write_file 1 = 5.
        // 임의의 중복 기록(예: 같은 턴에 두 번 push)이 있으면 여기서 잡힌다
        assert_eq!(touches.len(), 5, "touch 이벤트 총수가 스크립트와 어긋난다: {touches:?}");
    }

    /// 실패한 디스패치는 접촉이 아니다 — 기록은 `if dispatch_ok` **안**에 있어야 한다.
    ///
    /// ⚠ **이 테스트에 본문이 있어야 반증이 성립한다.** 초판은 이것을 주석 한 줄뿐인
    /// **빈 함수**로 출하했고, 빈 테스트는 항상 통과하므로 Step 4의 "블록을
    /// `dispatch_ok` 밖으로 옮기면 실패해야 한다"가 **절대 실패할 수 없었다**
    /// (플랜 1R 실현 I3). 본문을 넣으면 실제로 `left: 1, right: 0`으로 실패한다
    #[tokio::test]
    async fn failed_dispatch_is_not_recorded_as_a_touch() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        // 존재하지 않는 파일 → read_file이 Error를 돌려준다 (dispatch_ok=false)
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "nope.rs"}))),
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        run_quiet(&mut agent, &mut session, "요청").await.unwrap();

        let n = std::fs::read_to_string(&tpath)
            .unwrap()
            .lines()
            .filter(|l| l.contains("\"kind\":\"touch\""))
            .count();
        assert_eq!(n, 0, "실패한 디스패치는 접촉이 아니다 (기록이 dispatch_ok 밖으로 샜다)");
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
        assert!(agent.schema_fallback_fired(), "400 폴백 후 폴백 게터는 true (스펙 M13 §3-6-1)");
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
        // 사다리 2단계로 승격해도 1단계에서 켠 폴백 플래그는 유지돼야 한다 —
        // 누군가 사다리를 리팩터링하며 상태를 초기화하면 앵커 게이트가 조용히
        // 눈멀기 때문에 여기서 고정한다 (M13 §3-6-1).
        assert!(agent.schema_fallback_fired(), "json_schema 폴백 발동은 system 인라인 승격 후에도 유지");
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
        // 수치: 결과 각 ≈1500토큰. 예산은 시스템 프롬프트(brevity 규칙 포함)를
        // 담은 뒤에도 "둘 다 온전"은 불가능하고, elide(ELIDED) 경로는 남도록 잡는다.
        // context를 너무 낮추면 쌍 삭제만 일어나 ELIDED 없이 실패한다.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), "z".repeat(6_000)).unwrap();
        let read = || ok(&turn("read_file", serde_json::json!({"path": "big.txt"})));
        let script = Scripted::new(vec![read(), read(), ok(&finish("done"))]);
        // repo_notes=false: pack arithmetic without notes SYSTEM pointer
        let config = Config {
            context_tokens: 3_000,
            max_output_tokens: 100,
            repo_notes: false,
            ..Default::default()
        };
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
        let config = Config { max_turns: 25, repo_notes: false, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(false), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
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
    async fn repetition_key_uses_post_salvage_args() {
        // 스펙 §8 회귀: 키가 salvage 정규화 후 args 기준 — 상이한 원형이 같은 키로 합류.
        // 홀수 턴: action 레벨 스칼라 depth (salvage 대상, args:{} + depth:1 → 병합).
        // 짝수 턴: 이미 정규화된 args:{"depth":1}. 둘 다 build_turn 이후 args가
        // {"depth":1}로 동일해지고, list_files 결과도 변하지 않는 tempdir이라
        // (키, 결과해시)가 5회 일치 → RepetitionStop.
        let dir = tempfile::tempdir().unwrap();
        let malformed =
            r#"{"thought": "x", "action": {"tool": "list_files", "args": {}, "depth": 1}}"#;
        let clean = || turn("list_files", serde_json::json!({"depth": 1}));
        let script = Scripted::new(vec![
            ok(malformed),
            ok(&clean()),
            ok(malformed),
            ok(&clean()),
            ok(malformed),
        ]);
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
        let config = Config { max_turns: 25, repo_notes: false, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(false), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
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

    // M14 B-4: 거부 경로(Decision::Deny)에는 `!stop` 가드가 둘 있다 — status 억제와
    // finish_nudge 억제. 각각 독립적으로 제거해도 cargo test/clippy는 초록불이라
    // (1R 실측) 핀이 필요하다. 아래 두 테스트가 그 핀 쌍이다.

    #[tokio::test]
    async fn a_rejected_mutation_on_a_repetition_stop_turn_emits_no_status_note() {
        // 거부 경로의 **status 억제** `!stop` 가드를 핀한다. 제거하면 RepetitionStop
        // 턴에 상태선이 붙어 M11의 불변식이 깨진다
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(
            (0..5)
                .map(|_| ok(&turn("write_file", serde_json::json!({"path": "a.rs", "content": "x"}))))
                .collect::<Vec<_>>(),
        );
        let approver = ScriptedApprover {
            decisions: Mutex::new(
                (0..5).map(|_| Decision::Deny { reason: "no".into() }).collect::<VecDeque<_>>(),
            ),
            seen: Mutex::new(Vec::new()),
        };
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let out = agent.run(&mut session, "x", &mut &approver, &mut |_| {}).await.unwrap();

        assert!(matches!(out, AgentOutcome::RepetitionStop));
        let last = session
            .messages()
            .iter()
            .rfind(|m| m.role == "user")
            .expect("tool_result가 있어야 한다");
        assert!(
            !last.content.contains(status_note::STATUS_MARKER),
            "정지 턴에 상태선이 붙었다: {}",
            last.content
        );
    }

    #[tokio::test]
    async fn a_rejected_action_on_a_repetition_stop_turn_emits_no_finish_nudge() {
        // 거부 경로의 **finish_nudge 억제** `!stop` 가드를 지목한다.
        // ⚠ 이것은 핀이 아니라 **관측점**이다 — Step 5-a의 예외.
        //
        // 도달불가 논증: (:470 주석 참고) 거부 경로의 이벤트는 둘뿐(MutationAttempt/Other)
        // 인데, idle 진전은 VerifyOk/ReadOnly에만 연결되고 거부 경로는 둘 다 방출하지 않는다.
        // MutationAttempt(write_file 거부)는 또한 disarm()을 불러 armed가 참이 될 수 없다.
        // 따라서 이 경로에서는 어떤 시나리오로도 FINISH_NUDGE가 나올 수 없다. 이 가드를
        // 지워도(플랜 리뷰가 실측) 결과는 바뀌지 않으므로, 이 테스트는 뮤테이션 검사로
        // "핀"임을 증명할 수 없지만, 오늘의 이벤트 매핑 아래에서 이 가드의 도달불가를
        // 기록하는 관측점 역할을 한다(스펙 §7 기준 4). 훗날 거부 경로의 이벤트 매핑이
        // 바뀌어 armed가 참이 될 수 있게 되면 이 테스트가 빨간불이 되어 그 변경을 잡아낸다.
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(
            (0..5)
                .map(|_| ok(&turn("write_file", serde_json::json!({"path": "a.rs", "content": "x"}))))
                .collect::<Vec<_>>(),
        );
        let approver = ScriptedApprover {
            decisions: Mutex::new(
                (0..5).map(|_| Decision::Deny { reason: "no".into() }).collect::<VecDeque<_>>(),
            ),
            seen: Mutex::new(Vec::new()),
        };
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let out = agent.run(&mut session, "x", &mut &approver, &mut |_| {}).await.unwrap();

        assert!(matches!(out, AgentOutcome::RepetitionStop));
        assert!(
            !session.messages().iter().any(|m| m.content.contains(finish_nudge::FINISH_NUDGE)),
            "정지 턴에 FINISH_NUDGE가 붙었다"
        );
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
        let config = Config { max_turns: 25, repo_notes: false, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(false), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
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
        let config = Config { max_turns: 25, repo_notes: false, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(false), ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config);
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

    #[cfg(unix)]
    mod finish_nudge_loop {
        use super::*;

        fn write_turn(path: &str, content: &str) -> String {
            turn("write_file", serde_json::json!({"path": path, "content": content}))
        }
        fn run_turn(cmd: &str) -> String {
            turn("run_command", serde_json::json!({"command": cmd}))
        }
        fn read_turn(path: &str) -> String {
            turn("read_file", serde_json::json!({"path": path}))
        }
        fn grep_turn(pattern: &str) -> String {
            turn("grep", serde_json::json!({"pattern": pattern}))
        }

        #[tokio::test]
        async fn verified_then_repeated_rechecks_get_finish_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")), // exit 0 — 무장
                ok(&read_turn("a.txt")),
                ok(&grep_turn("answer")),
                ok(&read_turn("a.txt")), // 반복 호출
                ok(&turn("list_files", serde_json::json!({}))), // 4번째 카운트 턴 — 발동
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(matches!(outcome, AgentOutcome::Finished(_)));
            assert!(session_contains(&session, "do not re-verify"), "§4-2 발동");
        }

        #[tokio::test]
        async fn novel_exploration_after_verify_does_not_fire_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&grep_turn("p1")),
                ok(&grep_turn("p2")),
                ok(&turn("list_files", serde_json::json!({}))), // 4턴 전부 신규 — 불발
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "반복-호출 조건 (§4-2)");
        }

        #[tokio::test]
        async fn edit_attempt_after_verify_disarms_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&read_turn("a.txt")), // 반복 — idle 2
                ok(&turn("edit_file", serde_json::json!({"path": "a.txt", "search": "answer", "replace": "answer"}))), // S/R 실패 시도 — 무장 해제
                ok(&grep_turn("x")),
                ok(&grep_turn("y")),
                ok(&turn("list_files", serde_json::json!({}))),
                ok(&grep_turn("z")),
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "무장 해제 후 재검증 성공 없이는 불발 (§4-2 표 2행)");
        }

        #[tokio::test]
        async fn failing_verification_does_not_arm_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("false")), // exit 1 — 무장 안 함
                ok(&read_turn("a.txt")),
                ok(&grep_turn("x")),
                ok(&read_turn("a.txt")),
                ok(&turn("list_files", serde_json::json!({}))),
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "비0 종료코드는 무장하지 않음 (§4-2 표 4행)");
        }

        #[tokio::test]
        async fn timed_out_verification_does_not_arm_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let mut ctx = ToolCtx::new(dir.path().to_path_buf());
            ctx.command_timeout = std::time::Duration::from_millis(100);
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("sleep 5")), // 타임아웃 본문에는 exit code 줄이 없다 — VerifyOther (§4-2 표 4행, §6 ④ 타임아웃 몫)
                ok(&read_turn("a.txt")),
                ok(&grep_turn("x")),
                ok(&read_turn("a.txt")),
                ok(&turn("list_files", serde_json::json!({}))),
                ok(&finish("done")),
            ]);
            let config = Config { max_turns: 25, repo_notes: false, ..Default::default() };
            let mut agent = Agent::new(&script, Registry::guided(false), ctx, "test-model".into(), &config);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "타임아웃 검증은 무장하지 않음");
        }

        #[tokio::test]
        async fn invalid_finish_resets_nudge_idle_counter() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&read_turn("a.txt")), // 반복 — idle 2
                ok(&grep_turn("x")),     // idle 3
                ok(&turn("finish", serde_json::json!({}))), // 무효 finish — idle 리셋 (§4-2 표 6행)
                ok(&turn("list_files", serde_json::json!({}))), // 리셋이 없었다면 4번째 카운트 턴으로 발동했을 자리
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "무효 finish가 4-2 카운터를 리셋 (§6 ⑥)");
        }

        #[tokio::test]
        async fn no_action_turn_preserves_nudge_idle_counter() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&read_turn("a.txt")), // 반복 — idle 2
                ok(&grep_turn("x")),     // idle 3
                ok_with_reason("truncated...", "length"), // 무액션 턴 — 카운터 불변 (§4-2 표 7행)
                ok(&turn("list_files", serde_json::json!({}))), // idle 4 — 발동
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(session_contains(&session, "do not re-verify"), "무액션 턴은 카운터 불변 (§6 ⑦)");
        }

        #[tokio::test]
        async fn pure_identical_loop_prefers_repetition_stop() {
            let dir = tempfile::tempdir().unwrap();
            let echo = run_turn("echo hi");
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&echo), // 무장 (윈도 1회째)
                ok(&echo), // idle 1 (2회째)
                ok(&echo), // idle 2 (3회째 — REPEAT_CORRECTION)
                ok(&echo), // idle 3 (4회째)
                ok(&echo), // 5회째 — RepetitionStop (idle 4 도달 전에 정지가 선점, §4-2 우선순위)
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(matches!(outcome, AgentOutcome::RepetitionStop), "{outcome:?}");
            assert!(!session_contains(&session, "do not re-verify"), "정지 턴에는 넛지를 평가하지 않는다");
        }
    }

    // M12 §2-4 — 공허 런(필터 0매치) 배제: VerifyOk 무장·VERIFY_NUDGE 해제 모두 제외
    #[cfg(unix)]
    const EMPTY_RUN: &str = "printf 'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out\n'";
    #[cfg(unix)]
    const REAL_RUN: &str = "printf 'running 1 test\ntest alpha ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'";

    #[cfg(unix)]
    #[tokio::test]
    async fn empty_test_run_does_not_clear_the_verify_nudge_flag() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "a.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": EMPTY_RUN}))),
            ok(&finish("done")), // 공허 런은 검증 시도가 아니므로 1회 반려된다
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
        assert!(session_contains(&session, "never ran a verification command"), "M12 §2-4");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn empty_test_run_does_not_arm_the_finish_nudge() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "answer").unwrap();
        let read = turn("read_file", serde_json::json!({"path": "a.txt"}));
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "b.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": EMPTY_RUN}))),
            ok(&read),
            ok(&turn("grep", serde_json::json!({"pattern": "answer"}))),
            ok(&read), // 반복 호출
            ok(&turn("list_files", serde_json::json!({}))), // 무장돼 있었다면 4번째 카운트 턴
            ok(&finish("done")),
            ok(&finish("done")), // VERIFY_NUDGE 반려분
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(!session_contains(&session, "do not re-verify"), "공허 런은 무장하지 않는다 (M12 §2-4)");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn real_test_run_still_arms_the_finish_nudge() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "answer").unwrap();
        let read = turn("read_file", serde_json::json!({"path": "a.txt"}));
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "b.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": REAL_RUN}))),
            ok(&read),
            ok(&turn("grep", serde_json::json!({"pattern": "answer"}))),
            ok(&read),
            ok(&turn("list_files", serde_json::json!({}))),
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session_contains(&session, "do not re-verify"), "실질 통과는 기존대로 무장 (회귀 방어)");
    }

    // 최종 리뷰 Minor 1 — 에이전트 레벨 공허 런 술어(`s.ran == 0 && s.filtered_out > 0`)를
    // `s.ran == 0`로 뮤테이션하면 죽는지 핀. "테스트가 원래 없는 크레이트"(0 ran, 0
    // filtered out — 정당한 런)와 "필터 미스"(0 ran, N filtered out — M12가 잡으려는
    // 공허 런)를 구분하는 것이 이 술어의 존재 이유다. 뮤테이션되면 둘 다 공허 런으로
    // 오분류돼, 정당한 검증인데도 VERIFY_NUDGE가 불필요하게 반려한다
    #[cfg(unix)]
    const NO_TESTS_IN_CRATE_RUN: &str =
        "printf 'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'";

    #[cfg(unix)]
    #[tokio::test]
    async fn zero_ran_zero_filtered_out_is_a_real_verification_not_a_vacuous_run() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "a.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": NO_TESTS_IN_CRATE_RUN}))),
            ok(&finish("done")), // 진짜 테스트-프리 크레이트 — 반려 없이 바로 종결돼야 한다
            ok(&finish("done2")), // 뮤테이션 시에만 소모됨(VERIFY_NUDGE가 1회 더 반려)
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
        assert!(
            !session_contains(&session, "never ran a verification command"),
            "0 ran/0 filtered out은 공허 런이 아니다 — filtered_out>0 조건 없이는 오분류된다 (Minor 1)"
        );
    }

    // 최종 리뷰 Minor 2 — cmd_summary는 cmd_exit가 Some일 때만(= 본문 첫 줄이 실제로
    // "exit code: "일 때만) 파싱해야 한다는 계약을 핀. 가드를 제거하면 TimedOut/Cancelled
    // 처럼 "exit code: " 줄이 없는 본문에서도(그 이전에 찍힌 출력이 우연히 test-summary
    // 형태면) 그걸 유효 검증 신호로 읽어 공허 런 판정에 흘러든다
    #[cfg(unix)]
    #[tokio::test]
    async fn timed_out_body_is_not_scanned_for_a_stray_test_summary() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = ToolCtx::new(dir.path().to_path_buf());
        ctx.command_timeout = std::time::Duration::from_millis(100);
        // 요약과 닮은 텍스트를 찍고서 타임아웃 — 본문 첫 줄은 "command timed out..."이지
        // "exit code: "가 아니다. cmd_exit는 None이어야 한다
        let cmd = "printf 'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out\n'; sleep 5";
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "a.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": cmd}))),
            ok(&finish("done")), // 타임아웃은 empty_verify가 아니므로 VERIFY_NUDGE를 해제한다
            ok(&finish("done2")), // 뮤테이션 시에만 소모됨(오탐 empty_verify로 1회 더 반려)
        ]);
        let config = Config { max_turns: 25, repo_notes: false, ..Default::default() };
        let mut agent = Agent::new(&script, Registry::guided(false), ctx, "test-model".into(), &config);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
        assert!(
            !session_contains(&session, "never ran a verification command"),
            "타임아웃 본문에는 exit code 줄이 없다 — 우연히 섞인 요약 문구를 검증으로 읽으면 안 된다 (Minor 2)"
        );
    }

    // M14 A-1 — 파이프 가드는 해제 술어에만: VerifyOk 매핑을 건드리면 재검증 루프
    // 감지가 죽는다(§3-3-3, 실측으로 기각된 대안). 실셸 파이프 실행이 필요해 unix 한정
    #[cfg(unix)]
    #[tokio::test]
    async fn a_piped_verification_does_not_release_verify_nudge() {
        let script = vec![
            ok(&turn("write_file", serde_json::json!({"path":"a.rs","content":"fn a(){}"}))),
            ok(&turn("run_command", serde_json::json!({"command":"cargo test 2>&1 | tail -5"}))),
            ok(&finish("done")),  // 파이프 문구로 1회 반려
            ok(&finish("done2")),
        ];
        let (out, notes) = run_capturing_tool_results(script).await;
        assert!(matches!(out, AgentOutcome::Finished(ref s) if s == "done2"), "{out:?}");
        assert!(notes.iter().any(|n| n.contains(VERIFY_NUDGE_PIPE)), "파이프 문구가 안 나왔다: {notes:?}");
        assert!(!notes.iter().any(|n| n.contains(VERIFY_NUDGE)), "기본 문구가 나왔다 — 거짓말이다");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_pipe_still_counts_for_loop_detection() {
        // §3-3-3 가드: VerifyOk 매핑에 가드를 걸면 이 테스트가 실패한다.
        // 교대가 필수 — 동일 명령 반복은 5번째에 RepetitionStop이 !stop 가드로
        // FINISH_NUDGE 평가를 선점한다. 5회는 하한이다(#1 무장 + #2~#5가 K=4를 채움)
        let mut script = vec![ok(&turn("write_file", serde_json::json!({"path":"a.rs","content":"x"})))];
        for i in 0..5 {
            let cmd = if i % 2 == 0 { "cargo test 2>&1 | tail -5" } else { "cargo test 2>&1 | head -5" };
            script.push(ok(&turn("run_command", serde_json::json!({"command": cmd}))));
        }
        script.push(ok(&finish("done")));
        script.push(ok(&finish("done2")));
        let (_out, notes) = run_capturing_tool_results(script).await;
        assert!(
            notes.iter().any(|n| n.contains(finish_nudge::FINISH_NUDGE_PIPE)),
            "FINISH_NUDGE가 안 나왔다 — VerifyOk 매핑에 가드가 걸렸다: {notes:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_pipe_flag_is_cleared_once_a_clean_verification_releases() {
        // 4R C2: 해제 성공 시 플래그를 안 고치면 FINISH_NUDGE가 낡은 true를 읽는다
        let mut script = vec![ok(&turn("write_file", serde_json::json!({"path":"a.rs","content":"x"})))];
        script.push(ok(&turn("run_command", serde_json::json!({"command":"cargo test 2>&1 | tail -5"}))));
        // "cargo test" 단독은 이 스크래치 샌드박스에 Cargo.toml이 없어 exit 101로 실패하고
        // VerifyOther가 finish_nudge를 disarm해 버려 이 테스트가 무의미하게 통과한다
        // (RED 확인 중 실측 — verification-criteria-must-be-falsifiable). `|| true`로
        // exit 0을 강제해 "파이프 없는 성공한 검증"을 실제로 재현한다 (T4 편차, 사유는 리포트)
        script.push(ok(&turn("run_command", serde_json::json!({"command":"cargo test || true"})))); // 해제
        for _ in 0..4 {
            script.push(ok(&turn("read_file", serde_json::json!({"path":"a.rs"}))));
        }
        script.push(ok(&finish("done")));
        let (_out, notes) = run_capturing_tool_results(script).await;
        assert!(
            !notes.iter().any(|n| n.contains(finish_nudge::FINISH_NUDGE_PIPE)),
            "마지막 검증이 파이프가 아닌데 파이프 변형이 나왔다: {notes:?}"
        );
    }

    // M10 §5 — 암③ 디코딩 섭동: S/R 스트릭 2연속 시 다음 요청의 temperature 상향
    #[tokio::test]
    async fn sr_streak_of_two_raises_temperature_until_streak_breaks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
        let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
        let script = Scripted::new(vec![ok(&sr), ok(&sr), ok(&read), ok(&finish("d"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        // 요청0(첫 턴)·요청1(SR 1회 후) 기본값, 요청2(SR 2연속 후) 0.7, 요청3(read 성공 후) 원복
        assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1], "{temps:?}");
    }

    // 리뷰 Important(T7): §4-1 확대 후 sr_streak() >= 2 분기가 두 개의
    // 파일별 disjunct(sr_file_streak/badargs_streak)에 가려 무핀 상태였다.
    // 서로 다른 파일에서 연속 S/R 오류가 나면 연속 스트릭(sr_streak)은 2에
    // 도달하지만, 파일별 누적(sr_file_streak)은 각 파일에서 1회씩이라 2에
    // 못 미친다 — sr_streak() 분기가 없으면 이 케이스는 섭동하지 않는다.
    #[tokio::test]
    async fn sr_streak_across_different_files_raises_temperature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "x\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "y\n").unwrap();
        let sr_a = turn("edit_file", serde_json::json!({"path": "a.rs", "search": "x", "replace": "x"}));
        let sr_b = turn("edit_file", serde_json::json!({"path": "b.rs", "search": "y", "replace": "y"}));
        let script = Scripted::new(vec![ok(&sr_a), ok(&sr_b), ok(&finish("d"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        // 요청0(첫 턴) 기본값, 요청1(a.rs SR 1회 후 — 연속 1, 파일별 누적 1) 기본값,
        // 요청2(b.rs SR — 연속 2, 파일별 누적은 b.rs만 1) sr_streak() 단독으로 섭동
        assert_eq!(temps, vec![0.1, 0.1, 0.7], "{temps:?}");
    }

    #[tokio::test]
    async fn perturb_reactivates_without_latch_and_resets_per_run() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
        let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
        // 1런: SR×2 → read → SR×2 (재활성 확인 — SR_CORRECTION 래치와 무관) → finish
        let script = Scripted::new(vec![
            ok(&sr), ok(&sr), ok(&read), ok(&sr), ok(&sr), ok(&finish("d")),
            // 2런: 활성 상태로 끝난 뒤에도 진입 리셋 확인
            ok(&finish("d2")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        {
            let reqs = script.requests.lock().unwrap();
            let temps: Vec<f32> = reqs.iter().map(|r| r.temperature).collect();
            // M12 §4-1로 확대된 이후: f.rs의 파일별 누적은 read로 끊긴 연속 스트릭과
            // 무관하게 살아남는다(성공 뮤테이션 외에는 리셋되지 않음) — 두 번째
            // 서브스트릭의 1회차 시점에 이미 누적 3(=2+1)이라 사전 §3-1 술어 중
            // 파일별 disjunct가 즉시 켜진다(요청4). f.rs의 SR_CORRECTION 래치는 첫
            // 서브스트릭에서 이미 소진돼 텍스트 교정은 재발화하지 않지만(래치는
            // 파일별) 온도는 래치와 무관하게 재트리거된다 — 원래 이 테스트가 핀하려던
            // 불변(재활성은 교정 래치에 좌우되지 않음)은 그대로 유지, 다만 트리거
            // 시점이 확대된 술어만큼 앞당겨졌다
            assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1, 0.7, 0.7], "{temps:?}");
        }
        let mut session2 = new_session(&agent);
        run_quiet(&mut agent, &mut session2, "y").await.unwrap();
        let reqs = script.requests.lock().unwrap();
        assert_eq!(reqs.last().unwrap().temperature, 0.1, "활성 상태로 끝난 뒤에도 run() 진입 리셋 (리뷰 2R M-1)");
    }

    #[tokio::test]
    async fn successful_mutation_unlatches_the_files_sr_correction_for_a_later_recurrence() {
        // M12 §4-1 wiring 핀: mod.rs가 성공 디스패치 지점에서 tracker.record_mutation_ok를
        // 실제로 호출하는지 확인한다(단위 테스트는 tracker를 직접 호출해 이 배선을
        // 우회한다). f.rs에서 SR 2연속으로 1차 발화(래치) → 유효 편집으로 성공 뮤테이션
        // → 같은 파일에서 SR 2연속 재발 → 배선이 없으면 래치가 안 풀려 2차 발화가
        // 영원히 막힌다(§4-1 "편집 성공 후 재발한 루프는 별개 사건" 요구사항).
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        let sr_x = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let mutate = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "y"}));
        let sr_y = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "y", "replace": "y"}));
        let script = Scripted::new(vec![
            ok(&sr_x), ok(&sr_x), // 1차 SR 2연속 — 발화 + f.rs 래치
            ok(&mutate),          // 성공 뮤테이션 — record_mutation_ok가 와이어링돼 있어야 래치 해제
            ok(&sr_y), ok(&sr_y), // 2차 SR 2연속(같은 파일, 값만 y) — 배선돼 있어야 재발화
            ok(&finish("d")), // 무검증 finish 1차 — VERIFY_NUDGE가 반려(뮤테이션 후 run_command 없음)
            ok(&finish("d2")), // 2차로 종결 (VERIFY_NUDGE는 런당 1회)
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let fires = session
            .messages()
            .iter()
            .filter(|m| m.content.contains(repetition::SR_CORRECTION))
            .count();
        assert_eq!(fires, 2, "성공 뮤테이션이 파일별 래치를 풀어 2차 발화가 나가야 한다 (배선 누락 시 1회에 그침)");
    }

    #[tokio::test]
    async fn gate_denied_edit_clears_perturb_override() {
        // §5 핀: Denied: 본문은 스트릭 리셋 → 오버라이드 해제. 주의: SR 오류 2회는
        // preview(dry_run)가 같은 사다리를 타므로 Err → 게이트 생략 → 디스패치가 SR
        // 오류를 되먹여 스트릭이 쌓인다. 유효한 3번째 편집만 preview Ok → 거부 경로.
        struct DenyEdits;
        impl crate::agent::approval::Approver for DenyEdits {
            fn approve(&mut self, _req: &ApprovalRequest) -> Decision {
                Decision::Deny { reason: "테스트 거부".into() }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let valid = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "y"}));
        let script = Scripted::new(vec![ok(&sr), ok(&sr), ok(&valid), ok(&finish("d"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        agent.run(&mut session, "x", &mut DenyEdits, &mut |_| {}).await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        // 요청2까지 SR 2연속으로 0.7, 유효 편집이 게이트 거부(Denied:)되며 리셋 → 요청3 원복
        assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1], "{temps:?}");
    }

    #[tokio::test]
    async fn no_action_turns_preserve_perturb_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let script = Scripted::new(vec![
            ok(&sr),
            ok(&sr),
            ok_with_reason("cut off", "length"), // 무액션 턴 — 스트릭·오버라이드 불변 (§5 핀)
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        // length-cut 턴이 오버라이드를 건드리지 않아 그다음 요청도 0.7 유지
        assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.7], "{temps:?}");
    }

    #[tokio::test]
    async fn perturb_override_survives_finish_missing_summary_turn() {
        // M10 §5 원복 핀: finish 시도 턴은 S/R 스트릭 불변 → 오버라이드 유지 (M11 §7 이월 소품)
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "fn a() {}\n").unwrap();
        let sr = turn(
            "edit_file",
            serde_json::json!({"path": "f.rs", "search": "fn a() {}", "replace": "fn a() {}"}),
        );
        let script = Scripted::new(vec![
            ok(&sr),                                    // S/R 오류 1
            ok(&sr),                                    // S/R 오류 2 → 스트릭 2
            ok(&turn("finish", serde_json::json!({}))), // summary 없는 finish — 스트릭 불변
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let reqs = script.requests.lock().unwrap();
        assert!((reqs[2].temperature - 0.7).abs() < 1e-6, "스트릭 2 도달 후 요청은 섭동");
        assert!(
            (reqs[3].temperature - 0.7).abs() < 1e-6,
            "finish 인자누락 턴은 스트릭 불변 — 오버라이드 유지: {}",
            reqs[3].temperature
        );
    }

    // M12 §3-1: missing-field 오형 연속도 섭동 트리거에 포함 (기존 텍스트 교정에
    // 개입 없던 경로 — 082449Z에서 5연속 정지로 이어진 사각)
    #[tokio::test]
    async fn badargs_streak_of_two_raises_temperature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
        let bad = turn("write_file", serde_json::json!({"path": "a.txt"})); // content 누락
        let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
        let script = Scripted::new(vec![ok(&bad), ok(&bad), ok(&read), ok(&finish("d"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1], "missing-field 2연속 → 섭동, 성공 결과로 원복 {temps:?}");
    }

    // M12 §4-1: 파일별 누적 S/R도 섭동 트리거에 포함 (연속 스트릭은 read가 끊어도
    // 같은 파일의 비연속 재발이 저온 어트랙터인 것은 동일)
    #[tokio::test]
    async fn non_consecutive_sr_on_the_same_file_raises_temperature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
        let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
        let script = Scripted::new(vec![ok(&sr), ok(&read), ok(&sr), ok(&finish("d"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        // 연속 스트릭은 read가 끊었지만 f.rs 파일 누적 2로 섭동 (M12 §4-1)
        assert_eq!(temps, vec![0.1, 0.1, 0.1, 0.7], "{temps:?}");
    }

    // T7 원복 가드: sr_file_streak()가 last_sr_file 잔류로 영구 참이 되면 이
    // 케이스에서 요청4(성공 편집 이후)도 0.7로 걸린다 — 조건 해소 시 원복돼야 한다
    #[tokio::test]
    async fn sr_file_streak_trigger_reverts_after_a_successful_edit_clears_the_counter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let read = turn("read_file", serde_json::json!({"path": "f.rs"}));
        let mutate = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "y"}));
        let script = Scripted::new(vec![
            ok(&sr),
            ok(&read),
            ok(&sr),     // f.rs 파일 누적 2 → 섭동 켜짐
            ok(&mutate), // 성공 뮤테이션 — record_mutation_ok로 f.rs 카운터 리셋
            ok(&finish("d")), // 무검증 finish 1차 — VERIFY_NUDGE가 반려(뮤테이션 후 run_command 없음)
            ok(&finish("d2")), // 2차로 종결
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        assert_eq!(
            temps,
            vec![0.1, 0.1, 0.1, 0.7, 0.1, 0.1],
            "성공 뮤테이션으로 파일 카운터가 풀리면 다음 요청은 원복돼야 한다 {temps:?}"
        );
    }

    #[tokio::test]
    async fn status_note_cadence_fires_at_turn_5_when_nothing_edited() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["a", "b", "c", "d", "e"] {
            std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
        }
        let reads: Vec<_> = ["a", "b", "c", "d", "e"]
            .iter()
            .map(|n| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")}))))
            .collect();
        let mut script_vec = reads;
        script_vec.push(ok(&finish("done")));
        let script = Scripted::new(script_vec);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let with_status: Vec<_> = session
            .messages()
            .iter()
            .filter(|m| m.content.contains("[status]"))
            .collect();
        // remove_status_note가 최신만 유지 — 턴 3(케이던스)의 노트가 턴 5에서
        // 교체돼 히스토리에는 여전히 1개만 남는다 (M13 조밀화로 늘지 않음)
        assert_eq!(with_status.len(), 1, "턴 5에서 정확히 1회");
        // M14 A-2 리뷰 수정: run_command를 한 번도 안 썼으므로 last_cmd_exit도
        // None — "no test summary in output" 한정자는 명령이 실제로 실행된
        // 경우에만 붙으므로 여기서는 렌더되지 않는다
        assert!(
            with_status[0].content.contains(
                "[status] files edited: none yet | verification: last command gave no exit code | turns: 5 of 25 used"
            ),
            "{}",
            with_status[0].content
        );
    }

    #[tokio::test]
    async fn status_note_fires_on_mutation_and_keeps_only_latest() {
        let dir = tempfile::tempdir().unwrap();
        let w = |p: &str| turn("write_file", serde_json::json!({"path": p, "content": "x"}));
        // 무검증 finish는 VERIFY_NUDGE가 1회 반려한다 — 두 번째 finish로 종결 (M5 §7.1)
        let script = Scripted::new(vec![
            ok(&w("a.rs")),
            ok(&w("b.rs")),
            ok(&finish("done")),
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let with_status: Vec<_> =
            session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
        assert_eq!(with_status.len(), 1, "최신만 유지 — 히스토리에 상태선 1개");
        let c = &with_status.last().unwrap().content;
        assert!(c.contains("files edited: 2 (a.rs, b.rs)"), "{c}");
        assert!(c.contains("verification: none since your last edit"), "{c}");
    }

    #[tokio::test]
    async fn status_note_merges_after_existing_correction_notes() {
        // 스펙 §8 "기존 교정문과 병합 순서" — salvage 노트가 있는 뮤테이션 턴에서
        // 상태선은 같은 메시지의 마지막에 온다
        let dir = tempfile::tempdir().unwrap();
        let bad_shape =
            r#"{"thought": "w", "action": {"tool": "write_file", "args": {"path": "a.rs"}, "content": "x"}}"#;
        let script = Scripted::new(vec![ok(bad_shape), ok(&finish("done")), ok(&finish("done"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let msg = session
            .messages()
            .iter()
            .find(|m| m.content.contains("[status]"))
            .expect("salvage된 write_file 뮤테이션 턴에 상태선");
        let salvage_pos = msg.content.find("fields outside").expect("salvage 노트 공존");
        let status_pos = msg.content.find("[status]").unwrap();
        assert!(status_pos > salvage_pos, "상태선은 마지막 병합: {}", msg.content);
    }

    #[tokio::test]
    async fn status_note_threshold_on_length_turn_carries_to_next_tool_turn() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["a", "b", "c", "d", "e"] {
            std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
        }
        let rd = |n: &str| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")})));
        let script = Scripted::new(vec![
            rd("a"), rd("b"), rd("c"), rd("d"),
            ok_with_reason("truncated…", "length"), // 턴 5 — 채널 없음, 이월
            rd("e"),                                 // 턴 6 — 이월분 주입
            ok(&finish("done")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let with_status: Vec<_> =
            session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
        assert_eq!(with_status.len(), 1);
        assert!(with_status[0].content.contains("turns: 6 of 25"), "{}", with_status[0].content);
    }

    #[tokio::test]
    async fn repetition_stop_still_fires_with_status_note_active() {
        // 정지 우선순위: 동일 호출 5회 정지 턴(턴 5)에는 상태선을 주입하지 않는다
        // (!stop 가드). M13 조밀화로 턴 3이 케이던스 지점이 되어 정지 이전에
        // 상태선이 히스토리에 등장하므로(session_contains 전체 검사는 더 이상
        // "주입 안 됐다"를 핀할 수 없다) — 정지 턴 자체의 tool_result만 좁혀서 본다.
        // stop==true 경로는 push_tool_result 직후 바로 반환하므로 마지막 메시지가
        // 곧 정지 턴의 결과다.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let same = turn("read_file", serde_json::json!({"path": "a.txt"}));
        let script = Scripted::new(vec![ok(&same), ok(&same), ok(&same), ok(&same), ok(&same)]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::RepetitionStop));
        let stop_turn_result = session.messages().last().expect("정지 턴 tool_result 존재");
        assert!(
            !stop_turn_result.content.contains("[status]"),
            "정지 턴 자체에는 주입되지 않음: {}",
            stop_turn_result.content
        );
    }

    #[tokio::test]
    async fn status_note_on_a_repeated_result_does_not_break_repetition_hash() {
        // 채널 격리 실증 (스펙 §8): 턴 5에서 상태선이 병합된 결과가 이후 동일 반복돼도
        // 해시는 body만 보므로 5회째에 RepetitionStop 도달
        let dir = tempfile::tempdir().unwrap();
        for n in ["a", "b", "c", "d", "e"] {
            std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
        }
        let rd = |n: &str| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")})));
        // 턴 1-4 상이 read, 턴 5 = e.txt 1회차(케이던스 상태선 병합), 턴 6-9 = e.txt 반복
        let script = Scripted::new(vec![
            rd("a"), rd("b"), rd("c"), rd("d"),
            rd("e"), rd("e"), rd("e"), rd("e"), rd("e"),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::RepetitionStop), "e.txt 5회째(턴 9) 정지");
        assert!(session_contains(&session, "[status]"), "턴 5의 상태선이 반복 결과에 병합돼 있었음");
    }

    #[tokio::test]
    async fn status_note_threshold_on_finish_error_turn_carries_over() {
        // 이월 핀 경로 ③(finish 오류 턴 — session.push 경로) 통합 검증 (스펙 §8)
        let dir = tempfile::tempdir().unwrap();
        for n in ["a", "b", "c", "d", "e"] {
            std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
        }
        let rd = |n: &str| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")})));
        let script = Scripted::new(vec![
            rd("a"), rd("b"), rd("c"), rd("d"),
            ok(&turn("finish", serde_json::json!({}))), // 턴 5 — summary 없음, 채널 없음 → 이월
            rd("e"),                                     // 턴 6 — 이월분 주입
            ok(&finish("done")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let with_status: Vec<_> =
            session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
        assert_eq!(with_status.len(), 1);
        assert!(with_status[0].content.contains("turns: 6 of 25"), "{}", with_status[0].content);
    }

    #[tokio::test]
    async fn status_note_threshold_on_verify_nudge_turn_carries_over() {
        // 이월 핀 경로 ②(VERIFY_NUDGE 반려 턴) 통합 검증 (스펙 §8) — 뮤테이션으로
        // 케이던스가 꺼진 뒤 pacing 15를 반려 턴이 밟는 시나리오
        let dir = tempfile::tempdir().unwrap();
        let mut script_vec =
            vec![ok(&turn("write_file", serde_json::json!({"path": "a.rs", "content": "x"})))]; // 턴 1 — 뮤테이션(상태선 #1)
        for i in 0..13 {
            let name = format!("f{i}.txt");
            std::fs::write(dir.path().join(&name), "x").unwrap();
            script_vec.push(ok(&turn("read_file", serde_json::json!({"path": name})))); // 턴 2-14
        }
        script_vec.push(ok(&finish("done"))); // 턴 15 — VERIFY_NUDGE 반려(채널 없음) + pacing 15 → 이월
        script_vec.push(ok(&turn("read_file", serde_json::json!({"path": "f0.txt"})))); // 턴 16 — 이월분 주입
        script_vec.push(ok(&finish("done"))); // 종결 (VERIFY_NUDGE는 런당 1회)
        let script = Scripted::new(script_vec);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let with_status: Vec<_> =
            session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
        assert_eq!(with_status.len(), 1, "최신만 유지");
        let c = &with_status[0].content;
        assert!(c.contains("turns: 16 of 25"), "이월분이 턴 16에 주입: {c}");
        assert!(c.contains("verification: none since your last edit"), "{c}");
    }

    #[test]
    fn input_budget_has_a_floor_so_pathological_config_cannot_erase_history() {
        // max_output >= context면 saturating_sub가 0을 주고 예산이 0이 된다 —
        // pack()이 시스템 프롬프트와 마지막 메시지만 남기고 전부 지운다
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![]);
        let config = Config {
            context_tokens: 4096,
            max_output_tokens: 8192,
            repo_notes: false,
            ..Default::default()
        };
        let agent = Agent::new(
            &script,
            Registry::guided(false),
            ToolCtx::new(dir.path().to_path_buf()),
            "test-model".into(),
            &config,
        );
        assert!(
            agent.input_budget() >= MIN_INPUT_BUDGET,
            "예산이 {}로 붕괴했다 — 하한 {MIN_INPUT_BUDGET}이 없다",
            agent.input_budget()
        );
    }

    #[test]
    fn cramped_output_budget_is_reported_to_the_user() {
        // 파일럿 형태: 4096/8192 → 예산 3686. 하한에는 안 걸리지만 좁다
        assert!(cramped_budget_warning(8192, 4096).is_some(), "파일럿 형태는 경고 대상");
        assert!(cramped_budget_warning(8192, 2048).is_none(), "기본값은 경고 없음");
    }

    #[test]
    fn cramped_budget_warning_reports_floored_value_not_pre_floor() {
        // M14 B-2c: 하한이 발동하는 병리적 config(context=4096, output=8192)에서도
        // 경고는 실제 예산(512)을 보고해야 한다. 하한 적용 전 값(0)을 출력하면
        // 사용자가 도구가 부서졌다고 착각한다
        let warning = cramped_budget_warning(4096, 8192).expect("경고가 발동해야 함");
        assert!(
            warning.contains("512 토큰"),
            "경고는 floored_input_budget()의 결과(512)를 보고: {warning}"
        );
        assert!(
            !warning.contains("0 토큰"),
            "경고에 하한 미적용 값(0)이 나오면 안 된다: {warning}"
        );
    }

    // ── M16 T3: certified mut-gate · NOTES_STALE · VERIFY whitelist ──────────

    fn make_notes_agent(
        script: &Scripted,
        root: std::path::PathBuf,
        max_turns: usize,
    ) -> Agent<&Scripted> {
        let config = Config {
            max_turns,
            repo_notes: true,
            ..Default::default()
        };
        Agent::new(
            script,
            Registry::guided(true),
            ToolCtx::new(root),
            "test-model".into(),
            &config,
        )
    }

    fn valid_root_notes() -> String {
        crate::notes::ROOT_TEMPLATE.to_string()
    }

    fn valid_dir_notes() -> String {
        crate::notes::DIR_TEMPLATE.to_string()
    }

    fn notes_update(key: &str, content: &str) -> String {
        turn(
            "update_repo_notes",
            serde_json::json!({"path": key, "content": content}),
        )
    }

    /// Certify root (+ optional dir) via scripted tool turns, then run remaining script.
    #[tokio::test]
    async fn finish_scenario_a_verify_then_stale_then_accept() {
        // A: code mut → finish → VERIFY → finish → STALE → finish → accept
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"t\"\nversion=\"0.1.0\"\nedition=\"2021\"\n").unwrap();
        let root_body = valid_root_notes();
        let script = Scripted::new(vec![
            ok(&notes_update("_root", &root_body)),
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "Cargo.toml", "content": "[package]\nname=\"t\"\nversion=\"0.1.1\"\nedition=\"2021\"\n"}),
            )),
            ok(&finish("done")),  // VERIFY
            ok(&finish("done2")), // STALE
            ok(&finish("done3")), // accept
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(ref s) if s == "done3"));
        let finish_bodies: Vec<_> = session
            .messages()
            .iter()
            .filter(|m| m.content.contains("<tool_result name=\"finish\""))
            .map(|m| m.content.clone())
            .collect();
        assert_eq!(finish_bodies.len(), 2, "two rejected finishes before accept: {finish_bodies:?}");
        assert!(
            finish_bodies[0].contains(VERIFY_NUDGE),
            "1st reject is VERIFY: {}",
            finish_bodies[0]
        );
        assert!(
            finish_bodies[1].contains(crate::notes::NOTES_STALE_MARK),
            "2nd reject is STALE: {}",
            finish_bodies[1]
        );
        assert!(
            finish_bodies[1].contains("repo notes stale: you edited code but did not update notes for: _root."),
            "exact STALE shape: {}",
            finish_bodies[1]
        );
    }

    #[tokio::test]
    async fn finish_scenario_b_stale_only_after_verify_cleared() {
        // B: code mut → run_command verify clear → finish → STALE only → finish accept
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "x\n").unwrap();
        let root_body = valid_root_notes();
        let script = Scripted::new(vec![
            ok(&notes_update("_root", &root_body)),
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "a.rs", "content": "y\n"}),
            )),
            ok(&turn("run_command", serde_json::json!({"command": "true"}))),
            ok(&finish("almost")), // STALE only (verify already clear)
            ok(&finish("final")),  // accept
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(ref s) if s == "final"));
        let finish_bodies: Vec<_> = session
            .messages()
            .iter()
            .filter(|m| m.content.contains("<tool_result name=\"finish\""))
            .map(|m| m.content.clone())
            .collect();
        assert_eq!(finish_bodies.len(), 1, "only STALE, no VERIFY: {finish_bodies:?}");
        assert!(
            finish_bodies[0].contains(crate::notes::NOTES_STALE_MARK),
            "{}",
            finish_bodies[0]
        );
        assert!(
            !finish_bodies[0].contains(VERIFY_NUDGE),
            "VERIFY must not fire when verify cleared"
        );
    }

    #[tokio::test]
    async fn notes_update_after_green_test_does_not_set_mutated_since_verify() {
        // Green test clears VERIFY; notes update must not re-arm it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "x\n").unwrap();
        let root_body = valid_root_notes();
        let script = Scripted::new(vec![
            ok(&notes_update("_root", &root_body)),
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "a.rs", "content": "y\n"}),
            )),
            ok(&turn("run_command", serde_json::json!({"command": "true"}))),
            // update notes for dirty key after green verify
            ok(&notes_update("_root", &root_body)),
            ok(&finish("done")), // must accept — no VERIFY re-arm from notes
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(ref s) if s == "done"));
        assert!(
            !session_contains(&session, VERIFY_NUDGE),
            "notes update must not re-arm VERIFY"
        );
        assert!(
            !session_contains(&session, crate::notes::NOTES_STALE_MARK),
            "dirty cleared by exact-key notes update"
        );
        assert!(session_contains(
            &session,
            crate::tools::update_repo_notes::NOTES_UPDATE_OK_PREFIX
        ));
    }

    #[tokio::test]
    async fn mut_gate_blocks_edit_without_cert() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "x\n").unwrap();
        let script = Scripted::new(vec![
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "a.rs", "content": "y\n"}),
            )),
            ok(&finish("d")),
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(
            session_contains(&session, crate::notes::NOTES_MUT_GATE_MARK),
            "gate must fire"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "x\n",
            "file must not be written"
        );
    }

    #[tokio::test]
    async fn root_only_cargo_toml_with_only_root_cert() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "v1\n").unwrap();
        let root_body = valid_root_notes();
        let script = Scripted::new(vec![
            ok(&notes_update("_root", &root_body)),
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "Cargo.toml", "content": "v2\n"}),
            )),
            ok(&turn("run_command", serde_json::json!({"command": "true"}))),
            ok(&notes_update("_root", &root_body)), // clear dirty
            ok(&finish("ok")),
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap(),
            "v2\n"
        );
        assert!(!session_contains(&session, crate::notes::NOTES_MUT_GATE_MARK));
    }

    #[tokio::test]
    async fn nested_src_x_rs_needs_src_certified() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/x.rs"), "x\n").unwrap();
        let root_body = valid_root_notes();
        let dir_body = valid_dir_notes();
        // Only _root → still blocked for src/x.rs
        let script = Scripted::new(vec![
            ok(&notes_update("_root", &root_body)),
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "src/x.rs", "content": "blocked\n"}),
            )),
            ok(&notes_update("src", &dir_body)),
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "src/x.rs", "content": "ok\n"}),
            )),
            ok(&turn("run_command", serde_json::json!({"command": "true"}))),
            ok(&notes_update("src", &dir_body)),
            ok(&finish("done")),
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(session_contains(&session, crate::notes::NOTES_MUT_GATE_MARK));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/x.rs")).unwrap(),
            "ok\n"
        );
    }

    #[tokio::test]
    async fn repo_notes_false_edit_succeeds_and_prompt_lacks_pointer() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "x\n").unwrap();
        let script = Scripted::new(vec![
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "a.rs", "content": "y\n"}),
            )),
            ok(&turn("run_command", serde_json::json!({"command": "true"}))),
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        assert!(
            !session.messages()[0]
                .content
                .contains(prompt::REPO_NOTES_SYSTEM_POINTER),
            "flag false: no SYSTEM notes pointer"
        );
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "y\n"
        );
        assert!(!session_contains(&session, crate::notes::NOTES_MUT_GATE_MARK));
        assert!(!session_contains(&session, crate::notes::NOTES_STALE_MARK));
    }

    #[tokio::test]
    async fn mut_gate_reject_never_calls_approver() {
        // NonInteractiveApprover denies all mutations — if gate runs first we get
        // NOTES_MUT_GATE_MARK, never "mutating tools are unavailable".
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "x\n").unwrap();
        let script = Scripted::new(vec![
            ok(&turn(
                "write_file",
                serde_json::json!({"path": "a.rs", "content": "y\n"}),
            )),
            ok(&finish("d")),
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = agent
            .run(
                &mut session,
                "x",
                &mut crate::agent::approval::NonInteractiveApprover,
                &mut |_| {},
            )
            .await
            .unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert!(session_contains(&session, crate::notes::NOTES_MUT_GATE_MARK));
        assert!(
            !session_contains(&session, "mutating tools are unavailable"),
            "approver must not run on mut-gate reject"
        );
    }

    #[tokio::test]
    async fn edit_under_notes_dir_is_banned() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn(
                "write_file",
                serde_json::json!({
                    "path": ".loco/notes/_root.md",
                    "content": "hijack"
                }),
            )),
            ok(&finish("d")),
        ]);
        // Flag off still bans notes path (option A) without mut-gate
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session_contains(&session, "update_repo_notes"));
        assert!(!dir.path().join(".loco/notes/_root.md").exists());
    }

    #[tokio::test]
    async fn start_scan_records_notes_bytes_max_zero_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        let script = Scripted::new(vec![ok(&finish("d"))]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let extras: Vec<_> = std::fs::read_to_string(&tpath)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .filter(|v| v["kind"] == crate::notes::NOTES_BYTES_MAX_KIND)
            .collect();
        assert!(
            extras.iter().any(|e| e["content"] == "0"),
            "empty scan records 0: {extras:?}"
        );
    }

    #[tokio::test]
    async fn notes_flag_on_system_prompt_has_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![]);
        let agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let hist = agent.initial_history();
        assert!(
            hist[0].content.contains(prompt::REPO_NOTES_SYSTEM_POINTER),
            "flag true: SYSTEM pointer present"
        );
    }

    #[tokio::test]
    async fn notes_schema_two_rejects_inject_thrifty_correction_once() {
        // Oversized dir notes twice → NOTES_SCHEMA_CORRECTION on 2nd; third still fails schema
        // without a second copy of the correction.
        let dir = tempfile::tempdir().unwrap();
        let mut fat = String::from("## role\ncli\n\n## entrypoints\n");
        while fat.len() < 850 {
            fat.push_str("- X — long entrypoint line that pads the body over the dir cap\n");
        }
        let script = Scripted::new(vec![
            ok(&notes_update("src", &fat)),
            ok(&notes_update("src", &fat)),
            ok(&notes_update("src", &fat)),
            ok(&finish("done")),
        ]);
        let mut agent = make_notes_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let corr = crate::tools::update_repo_notes::NOTES_SCHEMA_CORRECTION;
        let joined: String = session
            .messages()
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n---\n");
        let n = joined.matches(corr).count();
        assert_eq!(n, 1, "correction once per run, got {n}: …");
        assert!(
            joined.contains(crate::tools::update_repo_notes::NOTES_SCHEMA_REJECT_PREFIX),
            "schema rejects present"
        );
    }
}
