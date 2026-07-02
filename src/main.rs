use std::io::Write;

use clap::Parser;

use loco::config::Config;
use loco::llm::client::{resolve_model, OpenAiClient};
use loco::llm::types::{ChatMessage, ChatRequest};
use loco::ui::repl::{run_repl, CHAT_SYSTEM_PROMPT};

#[derive(Parser)]
#[command(name = "loco", version, about = "폐쇄망 소형모델 코딩 CLI")]
struct Cli {
    /// 단발 실행 프롬프트 (비대화형)
    #[arg(short, long)]
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ring을 프로세스 기본 TLS 프로바이더로 설치 (aws-lc-sys 제거 — Windows 오프라인 빌드 대응).
    // 테스트는 이 설치 없이도 동작한다: 그래프에 프로바이더가 ring 하나뿐이면 rustls가 자동 선택.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("rustls crypto provider 설치 실패");

    let cli = Cli::parse();
    let config = Config::load_default()?;
    let client = OpenAiClient::new(&config.base_url, config.api_key.clone());
    let model = resolve_model(&client, &config).await?;

    match cli.prompt {
        Some(prompt) => {
            let req = ChatRequest {
                model,
                messages: vec![ChatMessage::system(CHAT_SYSTEM_PROMPT), ChatMessage::user(prompt)],
                temperature: config.temperature,
                max_tokens: Some(config.max_output_tokens as u32),
                stream: true,
                response_format: None,
            };
            client
                .chat_stream(&req, &mut |delta| {
                    print!("{delta}");
                    let _ = std::io::stdout().flush();
                })
                .await?;
            println!();
        }
        None => run_repl(&client, &config, &model).await?,
    }
    Ok(())
}
