//! M12 §2-1 — libtest 요약 파서. 하네스가 이미 아는 검증 실질(몇 개 돌았고
//! 몇 개 실패했는지)을 exit code 대신 접지하기 위한 순수 함수.
//! 보수 폴백: 요약 줄이 없으면 None — 모든 소비처는 None에서 기존 동작을 유지한다.

/// verification 렌더·노트가 인용하는 실패 테스트명 상한
pub const MAX_FAILED_NAMES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSummary {
    /// passed + failed (ignored 미포함 — §2-1)
    pub ran: usize,
    pub passed: usize,
    pub failed: usize,
    /// 수집 순서, MAX_FAILED_NAMES개까지
    pub failed_names: Vec<String>,
    pub filtered_out: usize,
}

/// run_command 결과 body에서 libtest 요약을 합산한다. 요약 줄이 하나도 없으면 None.
/// 줄 시작 앵커 — 임의 출력이 문구를 줄 중간에 품는 오탐을 막는다 (§2-1)
pub fn parse_test_summary(body: &str) -> Option<TestSummary> {
    let mut s = TestSummary { ran: 0, passed: 0, failed: 0, failed_names: Vec::new(), filtered_out: 0 };
    let mut saw_summary = false;
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("test result: ") {
            saw_summary = true;
            s.passed += count_field(rest, "passed");
            s.failed += count_field(rest, "failed");
            s.filtered_out += count_field(rest, "filtered out");
        } else if let Some(name) = line.strip_prefix("test ").and_then(|r| r.strip_suffix(" ... FAILED"))
            && s.failed_names.len() < MAX_FAILED_NAMES
        {
            s.failed_names.push(name.to_string());
        }
    }
    if !saw_summary {
        return None;
    }
    s.ran = s.passed + s.failed;
    Some(s)
}

/// `ok. 1 passed; 2 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s`에서
/// 라벨 앞 숫자를 뽑는다. 없으면 0 (문구 드리프트에 대한 보수 폴백).
/// **첫 필드는 상태 접두(`ok. ` / `FAILED. `)를 함께 갖는다** — 라벨을 떼고 남은
/// 문자열을 통째로 파싱하면 `passed`가 항상 0이 된다(플랜 리뷰 실측). 마지막
/// 공백 토큰만 취해야 한다
fn count_field(rest: &str, label: &str) -> usize {
    rest.split(';')
        .find_map(|part| {
            let n = part.trim().strip_suffix(label)?;
            n.trim().rsplit(char::is_whitespace).next()?.parse::<usize>().ok()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_single_failing_section() {
        let body = "exit code: 101\n\
running 2 tests\n\
test tests::a ... ok\n\
test tests::b ... FAILED\n\
\n\
failures:\n\
    tests::b\n\
\n\
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
        let s = parse_test_summary(body).expect("요약 줄이 있으면 Some");
        assert_eq!((s.ran, s.passed, s.failed, s.filtered_out), (2, 1, 1, 0));
        assert_eq!(s.failed_names, vec!["tests::b".to_string()]);
    }

    #[test]
    fn sums_every_section_in_a_workspace_run() {
        let body = "exit code: 101\n\
running 1 test\n\
test alpha ... ok\n\
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\
\n\
running 2 tests\n\
test beta ... FAILED\n\
test gamma ... FAILED\n\
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n";
        let s = parse_test_summary(body).unwrap();
        assert_eq!((s.ran, s.passed, s.failed, s.filtered_out), (3, 1, 2, 3));
        assert_eq!(s.failed_names, vec!["beta".to_string(), "gamma".to_string()]);
    }

    #[test]
    fn filter_matching_nothing_is_zero_ran_with_filtered_out() {
        let body = "exit code: 0\n\
running 0 tests\n\
\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s\n";
        let s = parse_test_summary(body).unwrap();
        assert_eq!((s.ran, s.filtered_out), (0, 13));
    }

    #[test]
    fn ignored_tests_do_not_count_as_ran() {
        // ignored만 있는 섹션은 ran=0 — §2-2 노트·렌더 규칙 3의 문안이 그 경우에도 정직하다
        let body = "test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        let s = parse_test_summary(body).unwrap();
        assert_eq!(s.ran, 0);
        assert_eq!(s.passed, 0);
    }

    #[test]
    fn no_summary_line_is_none() {
        assert!(parse_test_summary("exit code: 0\nhello world\n").is_none());
        assert!(parse_test_summary("").is_none());
    }

    #[test]
    fn summary_line_must_start_the_line() {
        // 임의 출력(cat한 로그 등)이 요약 문구를 줄 중간에 품는 오탐 봉쇄
        let body = "exit code: 0\necho 'test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'\n";
        assert!(parse_test_summary(body).is_none());
    }

    #[test]
    fn count_field_strips_the_status_prefix_on_the_first_field() {
        // 첫 필드는 `ok. ` / `FAILED. ` 접두를 함께 갖는다 — 회귀 방어선
        assert_eq!(count_field("ok. 27 passed; 0 failed; 0 ignored; 0 measured; 291 filtered out", "passed"), 27);
        assert_eq!(count_field("FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out", "passed"), 1);
        assert_eq!(count_field("FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out", "failed"), 2);
    }

    #[test]
    fn failed_names_are_capped() {
        let mut body = String::from("running 9 tests\n");
        for i in 0..9 {
            body.push_str(&format!("test t{i} ... FAILED\n"));
        }
        body.push_str("test result: FAILED. 0 passed; 9 failed; 0 ignored; 0 measured; 0 filtered out\n");
        let s = parse_test_summary(&body).unwrap();
        assert_eq!(s.failed, 9);
        assert_eq!(s.failed_names.len(), MAX_FAILED_NAMES);
    }
}
