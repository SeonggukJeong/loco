use std::cell::RefCell;
use std::process::ExitCode;

use clap::Parser;

use loco::agent::approval::{Approver, AutoApprover, NonInteractiveApprover};
use loco::agent::bounded::{run_bounded, Stopped};
use loco::agent::{Agent, AgentEvent, AgentOutcome, PARSE_ATTEMPTS};
use loco::config::Config;
use loco::llm::client::{resolve_model, OpenAiClient};
use loco::session::{Session, Transcript};
use loco::tools::{Registry, ToolCtx};
use loco::ui::repl::run_repl;
use loco::ui::status::{render_event, Spinner};

#[derive(Parser)]
#[command(name = "loco", version, about = "폐쇄망 소형모델 코딩 CLI")]
struct Cli {
    /// 단발 실행 프롬프트 (비대화형 에이전트 — 최종 답변만 stdout)
    #[arg(short, long)]
    prompt: Option<String>,
    /// 확인 게이트 전부 자동 승인 (auto_deny_patterns 차단은 유지)
    #[arg(long)]
    auto: bool,
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
        Some(prompt) => run_oneshot(&client, &config, &model, &prompt, cli.auto).await,
        None => {
            run_repl(&client, &config, &model, cli.auto).await?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// -p 출력 계약 (스펙 §7): 최종 답변만 stdout, 진행 표시는 전부 stderr.
/// 스피너는 stdout과 stderr 둘 다 TTY가 아니면 Spinner::start 내부에서 자동으로 꺼진다
async fn run_oneshot(
    client: &OpenAiClient,
    config: &Config,
    model: &str,
    prompt: &str,
    auto: bool,
) -> anyhow::Result<ExitCode> {
    let root = std::env::current_dir()?;
    let mut ctx = ToolCtx::new(root.clone());
    ctx.command_timeout = std::time::Duration::from_secs(config.command_timeout_secs);
    let cancel = ctx.cancel.clone();
    let mut agent = Agent::new(client, Registry::guided(), ctx, model.to_string(), config);
    let transcript = Transcript::create_under(&root).unwrap_or_else(|e| {
        eprintln!("(세션 기록을 열지 못했습니다: {e} — 기록 없이 진행)");
        Transcript::disabled()
    });
    let mut session = Session::new(agent.initial_history(), transcript);
    let spinner = RefCell::new(Spinner::start("생각 중"));
    let mut on_event = |ev: AgentEvent<'_>| {
        spinner.borrow_mut().stop();
        render_event(&ev, true);
        *spinner.borrow_mut() = Spinner::start("생각 중");
    };
    // -p: --auto 없으면 게이트를 띄우지 않고 거부 (스펙 §7 — 비대화형)
    let mut auto_approver;
    let mut non_interactive = NonInteractiveApprover;
    let approver: &mut dyn Approver = if auto {
        auto_approver = AutoApprover::new(&config.auto_deny_patterns)?;
        &mut auto_approver
    } else {
        &mut non_interactive
    };
    // Ctrl+C: cancel 플래그 → 유예 대기 (자식 프로세스 그룹 정리 후 종료 — 백로그 ①).
    // -p는 실행 전체가 하나의 select! 창이라 ctrl_c()를 그대로 interrupt로 쓴다
    let bounded = run_bounded(
        agent.run(&mut session, prompt, approver, &mut on_event),
        &cancel,
        None,
        std::time::Duration::from_secs(5),
        async {
            let _ = tokio::signal::ctrl_c().await;
        },
    )
    .await;
    spinner.borrow_mut().stop();
    let outcome = match bounded {
        Ok(r) => r?,
        Err(Stopped::Interrupted) | Err(Stopped::TimedOut) => {
            // limit이 None이므로 실제로는 Interrupted만 온다
            eprintln!("(중단됨 — 실행 중이던 명령까지 정리했습니다)");
            return Ok(ExitCode::from(2));
        }
    };

    match outcome {
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
        AgentOutcome::Cancelled => {
            eprintln!("(중단됨)");
            Ok(ExitCode::from(2))
        }
    }
}
