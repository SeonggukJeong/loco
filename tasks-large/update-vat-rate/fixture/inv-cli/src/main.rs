//! inv-cli 실행 파일 진입점.
//!
//! 실제 인자 파싱/라우팅 로직은 전부 `inv_cli` 라이브러리 크레이트에
//! 있다(`lib.rs`) — 이 파일은 `std::env::args`를 모아 넘기고, 결과를
//! stdout/stderr에 내보내고, 종료 코드를 정하는 아주 얇은 어댑터다.

use std::process::ExitCode;

use inv_cli::{dispatch, parse_args, CliError};

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    run(&raw_args)
}

/// 실제 실행 로직. `main`에서 분리해 두면 인자 목록을 직접 넘겨 통합
/// 테스트를 작성하기 쉽다(프로세스를 새로 띄울 필요가 없다).
fn run(raw_args: &[String]) -> ExitCode {
    match parse_args(raw_args) {
        Ok(command) => {
            let output = dispatch(&command);
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}", format_cli_error(&err));
            ExitCode::FAILURE
        }
    }
}

/// CLI 오류를 stderr에 출력할 최종 메시지로 포맷한다.
fn format_cli_error(err: &CliError) -> String {
    format!("오류: {err}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_help_with_no_args_succeeds() {
        assert_eq!(run(&[]), ExitCode::SUCCESS);
    }

    #[test]
    fn run_unknown_command_fails() {
        let args = vec!["nope".to_string()];
        assert_eq!(run(&args), ExitCode::FAILURE);
    }

    #[test]
    fn format_cli_error_includes_prefix() {
        let err = CliError::UnknownCommand("xyz".to_string());
        assert!(format_cli_error(&err).starts_with("오류: "));
    }
}
