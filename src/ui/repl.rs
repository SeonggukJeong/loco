use std::cell::RefCell;
use std::io::Write;

use regex::Regex;
use rustyline::error::ReadlineError;

use crate::agent::approval::{Approver, AutoApprover, compile_patterns};
use crate::agent::{Agent, AgentEvent, AgentOutcome, PARSE_ATTEMPTS};
use crate::config::Config;
use crate::llm::client::OpenAiClient;
use crate::llm::types::{ChatMessage, ChatRequest};
use crate::session::{Session, Transcript};
use crate::tools::{Registry, ToolCtx};
use crate::ui::gate::TtyApprover;
use crate::ui::status::{render_event, Spinner};

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
    println!("입력한 내용은 에이전트가 툴로 조사하고, 확인을 거쳐 수정·실행까지 수행합니다.");
    println!("명령어:");
    println!("  /chat <메시지>  에이전트 없이 모델과 바로 대화 (스트리밍)");
    println!("  /clear          에이전트·채팅 히스토리 초기화");
    println!("  /config         현재 설정 표시");
    println!("  /quit           종료");
    println!("실행 중 Ctrl+C 는 진행 중인 요청을 취소합니다.");
    println!("파일 수정·명령 실행은 미리보기 후 y/N 확인을 거칩니다 (--auto로 자동 승인).");
    println!("사용 가능한 툴: read_file, list_files, grep, write_file, edit_file, run_command");
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
    auto: bool,
) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let mut ctx = ToolCtx::new(root.clone());
    ctx.command_timeout = std::time::Duration::from_secs(config.command_timeout_secs);
    let cancel = ctx.cancel.clone();
    let deny = compile_patterns(&config.auto_deny_patterns)?; // 셋업에서 1회 컴파일
    let mut agent = Agent::new(client, Registry::guided(), ctx, model.to_string(), config);
    // 셋업: 트랜스크립트 실패는 경고 후 비활성 (기록이 에이전트를 못 죽인다)
    let transcript = Transcript::create_under(&root).unwrap_or_else(|e| {
        println!("(세션 기록을 열지 못했습니다: {e} — 기록 없이 진행)");
        Transcript::disabled()
    });
    let mut session = Session::new(agent.initial_history(), transcript);
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
            Input::Unknown(cmd) if cmd == "chat" => println!("사용법: /chat <메시지>"),
            Input::Unknown(cmd) => println!("알 수 없는 명령: /{cmd} — /help 참고"),
            Input::Clear => {
                // 에이전트/채팅 히스토리는 분리 운영 — 둘 다 초기화. 세션 기록도 새 파일로 (스펙 §7)
                session = Session::new(
                    agent.initial_history(),
                    Transcript::create_under(&root).unwrap_or_else(|_| Transcript::disabled()),
                );
                chat_history.truncate(1);
                println!("(히스토리 초기화 — 새 세션 파일)");
            }
            Input::Chat(text) => {
                run_chat_turn(client, config, model, &mut chat_history, &mut session, text).await;
            }
            Input::Agent(text) => {
                run_agent_turn(&mut agent, &mut session, config, &text, &cancel, auto, &deny)
                    .await;
            }
        }
    }
    println!("안녕히 가세요.");
    Ok(())
}

/// M1 스트리밍 채팅 경로 (/chat). Ctrl+C로 취소 가능. 성공한 턴만 세션 기록에 남긴다
async fn run_chat_turn(
    client: &OpenAiClient,
    config: &Config,
    model: &str,
    history: &mut Vec<ChatMessage>,
    session: &mut Session,
    text: String,
) {
    history.push(ChatMessage::user(text.clone()));
    let req = ChatRequest {
        model: model.to_string(),
        messages: history.clone(),
        temperature: config.temperature,
        max_tokens: Some(config.max_output_tokens as u32),
        stream: true,
        response_format: None,
        seed: None,
    };
    let mut on_delta = |delta: &str| {
        print!("{delta}");
        let _ = std::io::stdout().flush();
    };
    let result = tokio::select! {
        r = client.chat_stream(&req, &mut on_delta) => r,
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
            session.record_extra("user", &text);
            session.record_extra("assistant", &full);
            history.push(ChatMessage::assistant(full));
        }
        Err(e) => {
            history.pop();
            println!("\n오류: {e}");
        }
    }
}

/// 에이전트 턴. Ctrl+C·에러·파싱 실패 시 세션을 요청 이전 상태로 되돌린다
async fn run_agent_turn(
    agent: &mut Agent<&OpenAiClient>,
    session: &mut Session,
    config: &Config,
    text: &str,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    auto: bool,
    deny: &[Regex],
) {
    // 턴 시작 시 초기화 — 이전 턴에서 취소됐어도 이번 턴은 새로 시작
    cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    let snap = session.snapshot();
    let spinner = RefCell::new(Spinner::start("생각 중"));
    let mut on_event = |ev: AgentEvent<'_>| {
        spinner.borrow_mut().stop();
        render_event(&ev, false);
        *spinner.borrow_mut() = Spinner::start("생각 중");
    };
    let mut tty;
    let mut auto_approver;
    let approver: &mut dyn Approver = if auto {
        auto_approver = AutoApprover::from_compiled(deny.to_vec());
        &mut auto_approver
    } else {
        tty = TtyApprover { spinner: &spinner, deny };
        &mut tty
    };
    let result = tokio::select! {
        r = agent.run(session, text, approver, &mut on_event) => Some(r),
        _ = tokio::signal::ctrl_c() => {
            // 장기 실행 툴(run_command)이 폴링해 자진 중단하도록 신호만 세운다.
            // 이미 spawn_blocking에 들어간 동기 작업 자체를 강제로 끊지는 않는다
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            None
        }
    };
    // on_event는 &RefCell만 캡처해 Copy — drop()은 clippy::dropping_copy_types에 걸리고
    // 애초에 불필요하다 (NLL이 차용을 끝낸다)
    spinner.borrow_mut().stop();

    match result {
        None => {
            session.rollback(snap);
            println!("\n(중단됨 — 이번 요청은 히스토리에서 제거)");
        }
        Some(Ok(AgentOutcome::Finished(summary))) => println!("\n{summary}"),
        Some(Ok(AgentOutcome::MaxTurns)) => println!(
            "(최대 턴 {}회에 도달했습니다 — 작업을 더 작게 나눠 다시 시도하세요)",
            config.max_turns
        ),
        Some(Ok(AgentOutcome::ParseFailed(raw))) => {
            session.rollback(snap);
            println!("(모델 응답을 {PARSE_ATTEMPTS}회 파싱하지 못했습니다. 마지막 원문:)\n{raw}");
        }
        Some(Ok(AgentOutcome::RepetitionStop)) => {
            println!("(같은 툴 호출을 반복해 조기 종료했습니다 — 요청을 바꿔보세요)");
        }
        Some(Err(e)) => {
            session.rollback(snap);
            println!("오류: {e}");
        }
    }
}

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
