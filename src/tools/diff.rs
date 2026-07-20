//! 확인 게이트용 diff 렌더링 (스펙 §4 — edit 적용 전 similar로 diff 표시)

use similar::ChangeTag;

pub const MAX_DIFF_LINES: usize = 120;

/// 모델 채널 전용 상한. MAX_DIFF_LINES(120)는 승인 게이트용이며 그 값을 매 편집
/// 턴에 붙이면 B-2가 고치려는 컨텍스트 문제를 스스로 악화시킨다 (스펙 §3-5-2)
pub const MODEL_DIFF_MAX_LINES: usize = 15;

/// 모델에게 되돌릴 편집 diff. 헤더는 **필수**다 — EDIT_STRATEGY_CORRECTION과
/// SR_CORRECTION이 막힌 모델을 write_file 전면 재작성으로 유도하는데, 헤더가
/// 없으면 상한에 걸린 큰 diff가 "몇 줄이 사라졌는지"조차 전하지 못한다
pub fn render_diff_for_model(old: &str, new: &str) -> String {
    let diff = similar::TextDiff::from_lines(old, new);
    // (is_delete, rendered) — 원래 순서를 유지한다. 접두 문자열로 헤더를 거르면
    // `--flag` 삭제가 `---flag`가 되어 파일 헤더로 오인된다
    let mut lines: Vec<(bool, String)> = Vec::new();
    let mut hunks = 0usize;
    for hunk in diff.unified_diff().context_radius(1).iter_hunks() {
        hunks += 1;
        // 헝크 헤더를 보존한다. 없으면 멀리 떨어진 두 편집이 **연속으로 보이고**,
        // 모델이 도구 결과 본문을 다음 `search` 인자에 그대로 복사하는 습성이
        // 있다는 점은 이 프로젝트가 반복적으로 확인해 온 사실이다 — 경계를 걸친
        // search는 0-match가 되어 S/R 루프(M9·M10·M12가 장치를 세 겹 쌓은 그 실패)의
        // 새 입구가 된다. `@@` 줄은 본문이 아니라 헤더라 "줄번호는 헤더에만" 규율과
        // 정합한다
        lines.push((false, hunk.header().to_string()));
        for change in hunk.iter_changes() {
            let v = change.value();
            let v = v.strip_suffix('\n').unwrap_or(v);
            match change.tag() {
                ChangeTag::Delete => lines.push((true, format!("-{v}"))),
                ChangeTag::Insert => lines.push((false, format!("+{v}"))),
                ChangeTag::Equal => lines.push((false, format!(" {v}"))),
            }
        }
    }
    let removed = lines.iter().filter(|(d, _)| *d).count();
    // 헤더 줄("@@ ...")은 '+'로 시작하지 않으므로 added에 안 걸린다
    let added = lines.iter().filter(|(d, l)| !d && l.starts_with('+')).count();

    if lines.len() <= MODEL_DIFF_MAX_LINES {
        let body: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
        return format!("-{removed} lines, +{added} lines\n{}", body.join("\n"));
    }
    // 절단 경로에서는 `@@` 헤더를 **버린다**. 남기면 삭제 줄이 전부 앞으로 모이면서
    // 헤더가 자기 헝크의 삭제를 잃은 채 뒤에 붙어 "이 헝크에는 삭제가 없다"는
    // **거짓 귀속**을 만들고, 삭제가 많으면 헤더가 아예 하나도 안 남는다.
    // 구조 정보는 요약 헤더의 "in K hunks"가 대신 나르며, 그 한 줄은 절단돼도
    // **항상 살아남는다**
    let header = format!("-{removed} lines, +{added} lines in {hunks} hunks");
    // KEEP = MAX-1 이라야 header + KEEP + "[diff truncated]" = MAX+1 로 맞는다
    const KEEP: usize = MODEL_DIFF_MAX_LINES - 1;
    let body: Vec<&(bool, String)> = lines.iter().filter(|(_, l)| !l.starts_with("@@")).collect();
    // 삭제 줄을 우선 보존한다 — 조용한 삭제가 A-3의 주 표적이고, 추가 줄은
    // 모델이 방금 자기가 쓴 내용이라 신호 가치가 낮다
    let mut kept: Vec<&str> = body.iter().filter(|(d, _)| *d).map(|(_, l)| l.as_str()).take(KEEP).collect();
    for (d, l) in &body {
        if kept.len() >= KEEP {
            break;
        }
        if !*d {
            kept.push(l);
        }
    }
    format!("{header}\n{}\n[diff truncated]", kept.join("\n"))
}

pub fn render_diff(old: &str, new: &str) -> String {
    let text = similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(2)
        .to_string();
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_DIFF_LINES {
        let mut s = lines[..MAX_DIFF_LINES].join("\n");
        s.push_str("\n[diff truncated]");
        return s;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_changed_lines_with_signs() {
        let d = render_diff("a\nb\nc\n", "a\nB\nc\n");
        assert!(d.contains("-b"), "{d}");
        assert!(d.contains("+B"), "{d}");
    }

    #[test]
    fn long_diff_is_truncated() {
        let old = String::new();
        let new: String = (0..500).map(|i| format!("line{i}\n")).collect();
        let d = render_diff(&old, &new);
        assert!(d.lines().count() <= MAX_DIFF_LINES + 1);
        assert!(d.ends_with("[diff truncated]"));
    }

    #[test]
    fn model_diff_is_capped_and_always_carries_a_count_header() {
        let old: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let new: String = (0..100).filter(|i| *i < 40 || *i >= 61).map(|i| format!("line {i}\n")).collect();
        let d = render_diff_for_model(&old, &new);
        assert!(d.lines().count() <= MODEL_DIFF_MAX_LINES + 1, "상한 초과:\n{d}");
        assert!(d.starts_with("-21 lines, +0 lines"), "헤더가 없다:\n{d}");
    }

    #[test]
    fn model_diff_keeps_deleted_lines_when_truncating() {
        // 조용한 삭제가 주 표적 — 잘릴 때 삭제 줄이 먼저 남아야 한다
        let old: String = (0..60).map(|i| format!("keep {i}\n")).collect();
        let mut new = String::new();
        for i in 0..60 {
            if i == 30 { continue; }            // 삭제 1줄
            new.push_str(&format!("keep {i}\n"));
            if i % 5 == 0 { new.push_str("ADDED\n"); }   // 추가 다수
        }
        let d = render_diff_for_model(&old, &new);
        assert!(d.contains("-keep 30"), "삭제 줄이 절단으로 사라졌다:\n{d}");
    }

    #[test]
    fn model_diff_of_a_tiny_edit_is_smaller_than_the_cap() {
        let d = render_diff_for_model("a\nb\nc\n", "a\nB\nc\n");
        assert!(d.contains("-b") && d.contains("+B"), "{d}");
        assert!(d.lines().count() < MODEL_DIFF_MAX_LINES, "{d}");
    }

    #[test]
    fn a_deleted_line_starting_with_dashes_is_still_counted_and_shown() {
        // unified diff에서 `--flag` 삭제는 `---flag`가 된다. 접두 문자열로 파일 헤더를
        // 거르는 구현은 이 줄을 버려 "-0 lines"를 렌더한다 — A-3의 목적을 정반대로
        // 위반하고, 대상 레포가 전부 CLI 도구라 `--flag`는 흔하다
        let d = render_diff_for_model("keep\n--flag\nkeep2\n", "keep\nkeep2\n");
        assert!(d.starts_with("-1 lines, +0 lines"), "삭제가 안 세어졌다:\n{d}");
        assert!(d.contains("--flag"), "삭제된 줄이 사라졌다:\n{d}");
    }

    #[test]
    fn distant_edits_keep_their_hunk_boundary() {
        // 헤더가 없으면 line 6 다음에 line 149가 와서 모델이 인접한 것으로 읽는다
        let old: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let new: String = (0..200)
            .map(|i| match i {
                5 => "LINE5\n".to_string(),
                150 => "LINE150\n".to_string(),
                _ => format!("line {i}\n"),
            })
            .collect();
        let d = render_diff_for_model(&old, &new);
        // ⚠ 헤더 형식이 `@@ -5,3 +5,3 @@`라 **한 줄에 `@@`가 두 번** 들어간다.
        // matches("@@")로 세면 헝크 2개가 4가 된다 — 줄 단위로 셀 것(형식 변화에도 둔감)
        assert_eq!(d.lines().filter(|l| l.starts_with("@@")).count(), 2, "헝크 경계가 사라졌다:\n{d}");
        assert!(d.starts_with("-2 lines, +2 lines"), "{d}");
    }

    #[test]
    fn a_truncated_diff_reports_hunk_count_instead_of_false_boundaries() {
        // 절단 시 `@@`를 남기면 삭제 줄이 앞으로 모이면서 헤더가 엉뚱한 헝크를
        // 설명하게 된다. 대신 요약 헤더가 구간 수를 나른다
        let old: String = (0..120).map(|i| format!("line {i}\n")).collect();
        let new: String = (0..120)
            .filter(|i| !(10..26).contains(i) && !(80..96).contains(i))
            .map(|i| format!("line {i}\n"))
            .collect();
        let d = render_diff_for_model(&old, &new);
        assert!(d.starts_with("-32 lines, +0 lines in 2 hunks"), "구간 수가 없다:\n{d}");
        assert!(!d.contains("@@"), "절단인데 헝크 헤더가 남았다:\n{d}");
        assert!(d.lines().count() <= MODEL_DIFF_MAX_LINES + 1, "{d}");
    }

    #[test]
    fn an_added_line_starting_with_pluses_is_still_counted() {
        let d = render_diff_for_model("keep\n", "keep\n++x\n");
        assert!(d.starts_with("-0 lines, +1 lines"), "추가가 안 세어졌다:\n{d}");
        assert!(d.contains("++x"), "추가된 줄이 본문에서 사라졌다:\n{d}");
    }

    /// A-3 리뷰 블로커 핀: 삭제된 테스트
    /// `success_reports_post_edit_context_with_line_numbers_in_header_only`가 지키던
    /// 두 성질 중 "줄번호는 헤더에만" — CLAUDE.md·M5 스펙 §6.1·본 파일 상단 주석이
    /// 근거로 삼는 그 규칙 — 을 새 렌더러에 다시 고정한다. `@@` 헝크 헤더와 절단
    /// 표식을 뺀 모든 본문 줄은 `-`/`+`/공백 중 정확히 하나로 시작해야 한다;
    /// 숫자 접두(줄번호)가 붙으면 모델이 그 문자열을 다음 `search`에 그대로
    /// 복사해 0-match S/R 루프의 새 입구가 된다.
    fn assert_body_lines_have_no_stray_prefix(d: &str) {
        for line in d.lines().skip(1) {
            // 1행은 요약 헤더(`-N lines, +M lines[...]`)라 본문이 아니다
            if line.starts_with("@@") || line == "[diff truncated]" {
                continue;
            }
            let ok = line.starts_with('-') || line.starts_with('+') || line.starts_with(' ');
            assert!(ok, "본문 줄이 -/+/공백 외 접두를 가진다 (줄번호 유출 의심): {line:?}\n전체 출력:\n{d}");
        }
    }

    #[test]
    fn model_diff_body_lines_never_carry_a_line_number_prefix_non_truncated() {
        let d = render_diff_for_model(
            "pub const A: u8 = 1;\npub const B: u8 = 2;\npub const C: u8 = 3;\n",
            "pub const A: u8 = 1;\n",
        );
        assert!(
            d.lines().count() <= MODEL_DIFF_MAX_LINES + 1,
            "이 테스트는 비절단 경로를 검증해야 한다:\n{d}"
        );
        assert_body_lines_have_no_stray_prefix(&d);
    }

    #[test]
    fn model_diff_body_lines_never_carry_a_line_number_prefix_truncated() {
        let old: String = (0..120).map(|i| format!("line {i}\n")).collect();
        let new: String = (0..120)
            .filter(|i| !(10..26).contains(i) && !(80..96).contains(i))
            .map(|i| format!("line {i}\n"))
            .collect();
        let d = render_diff_for_model(&old, &new);
        assert!(d.ends_with("[diff truncated]"), "이 테스트는 절단 경로를 검증해야 한다:\n{d}");
        assert_body_lines_have_no_stray_prefix(&d);
    }
}
