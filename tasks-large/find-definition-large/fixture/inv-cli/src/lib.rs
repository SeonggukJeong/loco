//! inv-cli: 재고/물류 워크스페이스의 커맨드라인 진입점.
//!
//! 서브커맨드 문자열을 파싱해 각 `commands::*` 모듈로 라우팅한다. 실제
//! 출력 로직은 `commands` 하위 모듈에 있고, 이 모듈은 인자 파싱과 라우팅만
//! 담당한다 — `main.rs`가 아니라 여기 두는 이유는 테스트에서 실제
//! 프로세스를 띄우지 않고도 라우팅 로직만 단위 테스트하기 위해서다.

pub mod commands;
pub mod util;

/// 파싱된 서브커맨드.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 정산 보고서 출력. `legacy`가 true면 옛 출력 포맷 경로를 탄다.
    Report { legacy: bool },
    /// 재고 현황 출력. `sku_filter`가 있으면 해당 SKU만 표시한다.
    Inventory { sku_filter: Option<String> },
    /// CSV/설정 텍스트를 파싱해 결과 요약을 출력한다.
    Ingest { text: String },
    /// 특정 SKU에 재고 이동(delta)을 적용한다.
    Movement { sku: String, delta: i64 },
    /// 사용법 출력.
    Help,
}

/// 커맨드 파싱/실행 중 발생하는 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    UnknownCommand(String),
    MissingArgument { command: &'static str, name: &'static str },
    InvalidNumber { command: &'static str, value: String },
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::UnknownCommand(cmd) => write!(f, "알 수 없는 커맨드: '{cmd}'"),
            CliError::MissingArgument { command, name } => {
                write!(f, "'{command}' 커맨드에 필요한 인자가 없습니다: {name}")
            }
            CliError::InvalidNumber { command, value } => {
                write!(f, "'{command}' 커맨드의 숫자 인자를 해석할 수 없습니다: '{value}'")
            }
        }
    }
}

impl std::error::Error for CliError {}

/// 이 커맨드라인 도구가 인식하는 서브커맨드 이름 목록.
pub const KNOWN_COMMANDS: [&str; 5] = ["report", "inventory", "ingest", "movement", "help"];

/// 인자 문자열 목록을 커맨드로 파싱한다. `args[0]`이 서브커맨드 이름이고,
/// 나머지가 해당 서브커맨드의 옵션/위치 인자다.
pub fn parse_args(args: &[String]) -> Result<Command, CliError> {
    let Some(first) = args.first() else {
        return Ok(Command::Help);
    };

    match first.as_str() {
        "report" => Ok(Command::Report { legacy: args.iter().any(|a| a == "--legacy") }),
        "inventory" => Ok(Command::Inventory { sku_filter: extract_flag_value(&args[1..], "--sku") }),
        "ingest" => {
            let text = args.get(1).ok_or(CliError::MissingArgument { command: "ingest", name: "text" })?;
            Ok(Command::Ingest { text: text.clone() })
        }
        "movement" => {
            let sku = args.get(1).ok_or(CliError::MissingArgument { command: "movement", name: "sku" })?;
            let delta_str = args.get(2).ok_or(CliError::MissingArgument { command: "movement", name: "delta" })?;
            let delta = delta_str
                .parse::<i64>()
                .map_err(|_| CliError::InvalidNumber { command: "movement", value: delta_str.clone() })?;
            Ok(Command::Movement { sku: sku.clone(), delta })
        }
        "help" | "--help" | "-h" => Ok(Command::Help),
        other => Err(CliError::UnknownCommand(other.to_string())),
    }
}

/// `--flag=value` 형태의 인자에서 값을 뽑아낸다(없으면 `None`).
fn extract_flag_value(args: &[String], flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    args.iter().find_map(|a| a.strip_prefix(prefix.as_str()).map(|v| v.to_string()))
}

/// 커맨드를 실행하고 사람이 읽는 출력 텍스트를 만든다.
pub fn dispatch(command: &Command) -> String {
    match command {
        Command::Report { legacy } => {
            let lines = commands::report::sample_lines();
            if *legacy {
                commands::report::execute_legacy(&lines)
            } else {
                commands::report::execute(&lines)
            }
        }
        Command::Inventory { sku_filter } => commands::inventory::execute(sku_filter.as_deref()),
        Command::Ingest { text } => commands::ingest::execute(text),
        Command::Movement { sku, delta } => commands::movement::execute(sku, *delta),
        Command::Help => help_text(),
    }
}

/// 사용법 텍스트를 만든다.
pub fn help_text() -> String {
    format!("사용법: inv-cli <커맨드> [인자...]\n지원 커맨드: {}", KNOWN_COMMANDS.join(", "))
}

/// 서브커맨드 이름이 이 도구가 인식하는 값인지 검사한다.
pub fn is_known_command(name: &str) -> bool {
    KNOWN_COMMANDS.contains(&name)
}

/// 인자 목록의 첫 토큰(서브커맨드 이름)만 미리 들여다본다(라우팅 전
/// 로깅/감사용, 파싱하지는 않는다).
pub fn peek_command_name(args: &[String]) -> Option<&str> {
    args.first().map(|s| s.as_str())
}

/// 명령행 전체를 사람이 읽는 한 줄로 합친다(디버그 로그/에러 메시지에
/// 원본 호출을 그대로 보여줄 때 쓴다).
pub fn reconstruct_command_line(args: &[String]) -> String {
    args.join(" ")
}

/// 서브커맨드 이름의 오타를 흔한 실수 목록 기준으로 교정 제안한다(완전한
/// 철자 교정기는 아니고, 자주 틀리는 몇 가지 패턴만 다룬다).
pub fn suggest_command(typo: &str) -> Option<&'static str> {
    match typo.to_ascii_lowercase().as_str() {
        "repot" | "reprot" | "rpeort" => Some("report"),
        "invetory" | "inventroy" | "inv" => Some("inventory"),
        "injest" | "digest" => Some("ingest"),
        "movment" | "mvoement" => Some("movement"),
        _ => None,
    }
}

/// 커맨드가 인자를 추가로 받는지(위치 인자가 필수인지) 판정한다. 라우팅
/// 전에 사용자에게 "이 커맨드는 인자가 더 필요합니다" 같은 사전 안내를
/// 줄 때 쓴다.
pub fn requires_extra_args(command_name: &str) -> bool {
    matches!(command_name, "ingest" | "movement")
}

/// 커맨드 이름이 읽기 전용(재고/저장소를 바꾸지 않는) 커맨드인지 판정한다.
pub fn is_read_only(command_name: &str) -> bool {
    matches!(command_name, "report" | "inventory" | "help")
}

/// 파싱된 커맨드를 사람이 읽는 이름으로 되돌린다(로그/감사 문자열용).
pub fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Report { .. } => "report",
        Command::Inventory { .. } => "inventory",
        Command::Ingest { .. } => "ingest",
        Command::Movement { .. } => "movement",
        Command::Help => "help",
    }
}

/// 인자 목록 중 값이 없는(플래그만 있고 값이 비어 있는) 플래그가 있는지
/// 검사한다(`--sku=` 처럼 값이 빈 경우 경고용).
pub fn has_empty_flag_value(args: &[String]) -> bool {
    args.iter().any(|a| a.ends_with('=') && a.starts_with("--"))
}

/// 인자 목록을 서브커맨드 이름과 나머지 인자로 나눈다(파싱 전 사전 분리
/// — 로깅에서 서브커맨드만 따로 남기고 싶을 때 쓴다).
pub fn split_command_and_rest(args: &[String]) -> (Option<&str>, &[String]) {
    match args.split_first() {
        Some((first, rest)) => (Some(first.as_str()), rest),
        None => (None, &[]),
    }
}
