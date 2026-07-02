use std::io::Write;

use rustyline::error::ReadlineError;

use crate::config::Config;
use crate::llm::client::OpenAiClient;
use crate::llm::types::{ChatMessage, ChatRequest};

pub const SYSTEM_PROMPT: &str = "You are loco, a concise coding assistant running on a local model. \
Answer briefly and accurately. Reply in the user's language.";

#[derive(Debug, PartialEq)]
pub enum Input {
    Chat(String),
    Help,
    Clear,
    Config,
    Quit,
    Unknown(String),
}

pub fn parse_input(line: &str) -> Input {
    let line = line.trim();
    if let Some(cmd) = line.strip_prefix('/') {
        match cmd.trim() {
            "help" => Input::Help,
            "clear" => Input::Clear,
            "config" => Input::Config,
            "quit" | "exit" => Input::Quit,
            other => Input::Unknown(other.to_string()),
        }
    } else {
        Input::Chat(line.to_string())
    }
}

fn print_help() {
    println!("명령어: /help 도움말, /clear 히스토리 초기화, /config 설정 표시, /quit 종료");
    println!("그 외 입력은 모델에게 전달됩니다.");
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
    let mut rl = rustyline::DefaultEditor::new()?;
    let mut history: Vec<ChatMessage> = vec![ChatMessage::system(SYSTEM_PROMPT)];
    println!("loco — 로컬 모델 코딩 어시스턴트 (모델: {model}, /help 참고)");

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
            Input::Clear => {
                history.truncate(1); // 시스템 프롬프트만 남김
                println!("(히스토리 초기화)");
            }
            Input::Unknown(cmd) => println!("알 수 없는 명령: /{cmd} — /help 참고"),
            Input::Chat(text) => {
                history.push(ChatMessage::user(text));
                let req = ChatRequest {
                    model: model.to_string(),
                    messages: history.clone(),
                    temperature: config.temperature,
                    max_tokens: Some(config.max_output_tokens as u32),
                    stream: true,
                };
                let result = client
                    .chat_stream(&req, &mut |delta| {
                        print!("{delta}");
                        let _ = std::io::stdout().flush();
                    })
                    .await;
                match result {
                    Ok(full) => {
                        if full.is_empty() {
                            // 빈 응답은 히스토리를 오염시키므로 사용자 턴을 되돌린다 (에러 처리와 동일 패턴)
                            history.pop();
                            println!("(빈 응답 — 히스토리에 남기지 않음)");
                        } else {
                            println!();
                            history.push(ChatMessage::assistant(full));
                        }
                    }
                    Err(e) => {
                        // 실패한 사용자 턴은 히스토리에서 제거 (재입력 가능하게)
                        history.pop();
                        println!("\n오류: {e}");
                    }
                }
            }
        }
    }
    println!("안녕히 가세요.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_chat() {
        assert_eq!(parse_input("안녕"), Input::Chat("안녕".to_string()));
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
