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
        AgentOutcome::RepetitionStop => {
            eprintln!("(같은 툴 호출 반복으로 조기 종료 — 요청을 바꿔 다시 시도하세요)");
            Ok(ExitCode::from(2))
        }
    }
}
