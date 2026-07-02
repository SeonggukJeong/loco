use std::io::Write;

use clap::Parser;

use loco::config::Config;
use loco::llm::client::{resolve_model, OpenAiClient};
use loco::llm::types::{ChatMessage, ChatRequest};
use loco::ui::repl::{run_repl, SYSTEM_PROMPT};

#[derive(Parser)]
#[command(name = "loco", version, about = "폐쇄망 소형모델 코딩 CLI")]
struct Cli {
    /// 단발 실행 프롬프트 (비대화형)
    #[arg(short, long)]
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load_default()?;
    let client = OpenAiClient::new(&config.base_url, config.api_key.clone());
    let model = resolve_model(&client, &config).await?;

    match cli.prompt {
        Some(prompt) => {
            let req = ChatRequest {
                model,
                messages: vec![ChatMessage::system(SYSTEM_PROMPT), ChatMessage::user(prompt)],
                temperature: config.temperature,
                max_tokens: Some(config.max_output_tokens as u32),
                stream: true,
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
