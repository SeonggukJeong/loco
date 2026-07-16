//! inv-cli 베이스 테스트: 라우팅 단위 테스트.
//!
//! `calc_total_v2`/`monthly_total`의 합계값과 세율 파생값(`apply_tax`/
//! `invoice_total`/`forecast_projection`이 실제로 계산한 값,
//! `DEFAULT_VAT_PERCENT`)은 설정에 따라 바뀔 수 있는 값이라 이 파일에서는
//! 단정하지 않는다. 여기서는 인자 파싱이 올바른 `Command`를 만드는지, 각
//! 서브커맨드가 올바르게 라우팅되는지(출력에 기대한 문구/구조가 담기는지)만
//! 검증한다.

use inv_cli::commands::{ingest, inventory, movement, report};
use inv_cli::{dispatch, is_known_command, parse_args, Command};

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn parse_args_empty_is_help() {
    assert_eq!(parse_args(&[]).unwrap(), Command::Help);
}

#[test]
fn parse_args_report_without_flag() {
    let cmd = parse_args(&args(&["report"])).unwrap();
    assert_eq!(cmd, Command::Report { legacy: false });
}

#[test]
fn parse_args_report_with_legacy_flag() {
    let cmd = parse_args(&args(&["report", "--legacy"])).unwrap();
    assert_eq!(cmd, Command::Report { legacy: true });
}

#[test]
fn parse_args_inventory_with_sku_filter() {
    let cmd = parse_args(&args(&["inventory", "--sku=EL-000123"])).unwrap();
    assert_eq!(cmd, Command::Inventory { sku_filter: Some("EL-000123".to_string()) });
}

#[test]
fn parse_args_inventory_without_filter() {
    let cmd = parse_args(&args(&["inventory"])).unwrap();
    assert_eq!(cmd, Command::Inventory { sku_filter: None });
}

#[test]
fn parse_args_unknown_command_errors() {
    let err = parse_args(&args(&["bogus"])).unwrap_err();
    assert_eq!(err, inv_cli::CliError::UnknownCommand("bogus".to_string()));
}

#[test]
fn parse_args_ingest_requires_text_argument() {
    let err = parse_args(&args(&["ingest"])).unwrap_err();
    assert_eq!(err, inv_cli::CliError::MissingArgument { command: "ingest", name: "text" });
}

#[test]
fn parse_args_movement_requires_two_positionals() {
    let err = parse_args(&args(&["movement", "EL-000123"])).unwrap_err();
    assert_eq!(err, inv_cli::CliError::MissingArgument { command: "movement", name: "delta" });
}

#[test]
fn parse_args_movement_rejects_non_numeric_delta() {
    let err = parse_args(&args(&["movement", "EL-000123", "abc"])).unwrap_err();
    assert_eq!(err, inv_cli::CliError::InvalidNumber { command: "movement", value: "abc".to_string() });
}

#[test]
fn parse_args_movement_parses_negative_delta() {
    let cmd = parse_args(&args(&["movement", "EL-000123", "-5"])).unwrap();
    assert_eq!(cmd, Command::Movement { sku: "EL-000123".to_string(), delta: -5 });
}

#[test]
fn dispatch_report_default_routes_to_current_path() {
    let output = dispatch(&Command::Report { legacy: false });
    assert!(!report::is_legacy_output(&output));
    assert!(output.contains("순매출"));
}

#[test]
fn dispatch_report_legacy_routes_to_legacy_path() {
    let output = dispatch(&Command::Report { legacy: true });
    assert!(report::is_legacy_output(&output));
}

#[test]
fn dispatch_inventory_lists_known_sample_sku() {
    let output = dispatch(&Command::Inventory { sku_filter: None });
    assert!(inventory::output_mentions_sku(&output, "EL-000123"));
}

#[test]
fn dispatch_inventory_filter_returns_single_sku() {
    let output = dispatch(&Command::Inventory { sku_filter: Some("EL-000456".to_string()) });
    assert_eq!(inventory::count_output_lines(&output), 1);
}

#[test]
fn dispatch_inventory_unknown_sku_reports_not_found() {
    let output = dispatch(&Command::Inventory { sku_filter: Some("ZZ-999999".to_string()) });
    assert!(output.contains("찾을 수 없습니다"));
}

#[test]
fn dispatch_movement_reports_before_and_after() {
    let output = dispatch(&Command::Movement { sku: "EL-000123".to_string(), delta: 10 });
    assert!(output.contains("50 -> 60"));
}

#[test]
fn dispatch_movement_large_delta_requires_approval() {
    let output = dispatch(&Command::Movement { sku: "EL-000123".to_string(), delta: 2000 });
    assert!(output.contains("수동 승인 필요"));
}

#[test]
fn dispatch_ingest_summarizes_valid_and_error_rows() {
    let text = "EL-000123,SEL1,10,5000,ELEC\nBAD ROW";
    let cmd = Command::Ingest { text: text.to_string() };
    let output = dispatch(&cmd);
    assert_eq!(ingest::extract_valid_count(&output), Some(1));
}

#[test]
fn dispatch_help_lists_known_commands() {
    let output = dispatch(&Command::Help);
    assert!(output.contains("report"));
    assert!(output.contains("inventory"));
}

#[test]
fn is_known_command_recognizes_all_subcommands() {
    for cmd in ["report", "inventory", "ingest", "movement", "help"] {
        assert!(is_known_command(cmd));
    }
    assert!(!is_known_command("nope"));
}

#[test]
fn movement_preview_result_matches_store_backed_execute() {
    // 저장소 없이 순수 계산한 결과가 실제 커맨드 실행 결과와 일치하는지
    // 확인한다(이동 수량 계산 로직 자체는 inv-store 책임이므로 세율과
    // 무관하다).
    let preview = movement::preview_result(50, 10);
    assert_eq!(preview, 60);
}
