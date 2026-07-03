//! 확인 게이트용 diff 렌더링 (스펙 §4 — edit 적용 전 similar로 diff 표시)

pub const MAX_DIFF_LINES: usize = 120;

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
}
