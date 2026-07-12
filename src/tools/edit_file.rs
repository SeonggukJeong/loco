use serde::Deserialize;

use super::diff::render_diff;
use super::eol::{dominant_crlf, normalize_eol, restore_eol};
use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

pub struct EditFile;

#[derive(Deserialize)]
struct Args {
    path: String,
    search: String,
    replace: String,
}

#[derive(Debug, PartialEq)]
enum MatchMode {
    Exact,
    IgnoreTrailingWs,
    IndentShift(String),
}

impl MatchMode {
    fn describe(&self) -> String {
        match self {
            MatchMode::Exact => "exact".to_string(),
            MatchMode::IgnoreTrailingWs => "ignoring trailing whitespace".to_string(),
            MatchMode::IndentShift(i) => format!("indent-shifted by {} chars", i.len()),
        }
    }
}

fn parse(args: &serde_json::Value) -> Result<Args, ToolError> {
    let a: Args = serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))?;
    if a.search.is_empty() {
        return Err(ToolError::BadArgs("`search` must not be empty".to_string()));
    }
    Ok(a)
}

/// apply_edit 결과 — run()이 성공 컨텍스트를 렌더링하는 데 필요한 위치 정보까지 담는다
struct EditOutcome {
    new_text: String,
    mode: MatchMode,
    /// 새 텍스트 기준 치환 시작 줄(0-기준) — 성공 컨텍스트 렌더링용 (첫 매치)
    start_line: usize,
    /// 치환으로 들어간 줄 수 (최소 1)
    replaced_lines: usize,
    occurrences: usize,
}

/// 매칭 사다리 (스펙 §4). text/search/replace는 이미 \n 정규화된 상태.
/// Err 문자열은 모델에게 가는 영어 메시지
fn apply_edit(text: &str, search: &str, replace: &str) -> Result<EditOutcome, String> {
    // 1단계: 정확 일치
    let exact_positions: Vec<usize> = text.match_indices(search).map(|(i, _)| i).collect();
    match exact_positions.len() {
        1 => {
            let start_line = text[..exact_positions[0]].matches('\n').count();
            let replaced_lines = replace.split('\n').count().max(1);
            return Ok(EditOutcome {
                new_text: text.replacen(search, replace, 1),
                mode: MatchMode::Exact,
                start_line,
                replaced_lines,
                occurrences: 1,
            });
        }
        n if n >= 2 => {
            return Err(format!(
                "search block matches {n} locations (exact match); add surrounding lines to make it unique"
            ));
        }
        _ => {}
    }

    let t_lines: Vec<&str> = text.split('\n').collect();
    let mut s_lines: Vec<&str> = search.split('\n').collect();
    while s_lines.last() == Some(&"") {
        s_lines.pop(); // search 끝의 빈 줄은 매칭에서 제외
    }
    let window = s_lines.len();
    if window == 0 || t_lines.len() < window {
        return Err(not_found_message(text, &s_lines));
    }

    // 2단계: 후행 공백 무시
    let stage2: Vec<usize> = (0..=t_lines.len() - window)
        .filter(|&i| {
            t_lines[i..i + window]
                .iter()
                .zip(&s_lines)
                .all(|(w, s)| w.trim_end() == s.trim_end())
        })
        .collect();
    match stage2.len() {
        1 => {
            let i = stage2[0];
            let replacement = replace_lines(replace, "");
            let replaced_lines = replacement.len().max(1);
            let new_text = splice(&t_lines, i, window, &replacement);
            return Ok(EditOutcome {
                new_text,
                mode: MatchMode::IgnoreTrailingWs,
                start_line: i,
                replaced_lines,
                occurrences: 1,
            });
        }
        n if n >= 2 => {
            return Err(format!(
                "search block matches {n} locations (ignoring trailing whitespace); add surrounding lines to make it unique"
            ));
        }
        _ => {}
    }

    // 3단계: 균일 들여쓰기 시프트
    let stage3: Vec<(usize, String)> = (0..=t_lines.len() - window)
        .filter_map(|i| indent_of_match(&t_lines[i..i + window], &s_lines).map(|ind| (i, ind)))
        .collect();
    match stage3.len() {
        1 => {
            let (i, indent) = &stage3[0];
            let replacement = replace_lines(replace, indent);
            let replaced_lines = replacement.len().max(1);
            let new_text = splice(&t_lines, *i, window, &replacement);
            Ok(EditOutcome {
                new_text,
                mode: MatchMode::IndentShift(indent.clone()),
                start_line: *i,
                replaced_lines,
                occurrences: 1,
            })
        }
        n if n >= 2 => Err(format!(
            "search block matches {n} locations (with indent shift); add surrounding lines to make it unique"
        )),
        _ => Err(not_found_message(text, &s_lines)),
    }
}

/// 모든 줄이 동일한 indent 접두로 매칭되면 그 indent를 반환 (후행 공백은 무시)
fn indent_of_match(window: &[&str], search: &[&str]) -> Option<String> {
    let (i, s0) = search.iter().enumerate().find(|(_, l)| !l.trim().is_empty())?;
    let w0 = window[i].trim_end();
    let s0 = s0.trim_end();
    let indent = w0.strip_suffix(s0)?;
    if !indent.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }
    let ok = window.iter().zip(search).all(|(w, s)| {
        let (w, s) = (w.trim_end(), s.trim_end());
        if s.is_empty() { w.is_empty() } else { w == format!("{indent}{s}") }
    });
    ok.then(|| indent.to_string())
}

/// replace를 줄 단위로 나누고 비어 있지 않은 줄에 indent를 접두한다
fn replace_lines(replace: &str, indent: &str) -> Vec<String> {
    let mut lines: Vec<&str> = replace.split('\n').collect();
    while lines.last() == Some(&"") {
        lines.pop();
    }
    lines
        .into_iter()
        .map(|l| if l.trim().is_empty() { String::new() } else { format!("{indent}{l}") })
        .collect()
}

fn splice(t_lines: &[&str], start: usize, window: usize, replacement: &[String]) -> String {
    let mut out: Vec<String> = t_lines[..start].iter().map(|s| s.to_string()).collect();
    out.extend(replacement.iter().cloned());
    out.extend(t_lines[start + window..].iter().map(|s| s.to_string()));
    out.join("\n")
}

fn not_found_message(text: &str, s_lines: &[&str]) -> String {
    let first = s_lines.first().map(|l| l.trim()).unwrap_or("");
    // let-chain: 단독 중첩 if는 clippy::collapsible_if가 -D warnings에서 거부한다 (edition 2024)
    if !first.is_empty()
        && let Some(i) = text.split('\n').position(|l| l.contains(first))
    {
        return format!(
            "search block not found. Line {} contains the first line of your block - \
             re-read the file and copy the exact text including whitespace",
            i + 1
        );
    }
    "search block not found - re-read the file and copy the exact text".to_string()
}

impl EditFile {
    /// 읽기 → 정규화 → 사다리 적용. (원본 본문, EditOutcome, 원본 CRLF 여부)
    fn dry_run(&self, args: &Args, ctx: &ToolCtx) -> Result<(String, EditOutcome, bool), ToolError> {
        let path = confine(&ctx.root, &args.path)?;
        let bytes = std::fs::read(&path)?;
        let raw = String::from_utf8(bytes).map_err(|_| ToolError::NotUtf8(args.path.clone()))?;
        let crlf = dominant_crlf(&raw);
        let text = normalize_eol(&raw);
        let search = normalize_eol(&args.search);
        let replace = normalize_eol(&args.replace);
        if search == replace {
            return Err(ToolError::EditFailed(
                "search and replace are identical - no change would be made".to_string(),
            ));
        }
        let outcome = apply_edit(&text, &search, &replace).map_err(ToolError::EditFailed)?;
        Ok((text, outcome, crlf))
    }
}

/// 편집 후 변경 부위 ±3줄 (M5 §6.1). 줄번호는 헤더에만 — 본문에 접두를 붙이면
/// 모델이 다음 search에 복사한다
fn render_context(new_text: &str, start_line: usize, replaced_lines: usize) -> String {
    let mut lines: Vec<&str> = new_text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop(); // 후행 개행의 빈 꼬리 줄은 컨텍스트·줄 범위에서 제외
    }
    let from = start_line.saturating_sub(3);
    let to = (start_line + replaced_lines + 3).min(lines.len());
    format!(
        "Context after edit (lines {}-{}):\n{}\nVerify this is what you intended.",
        from + 1,
        to,
        lines[from..to].join("\n")
    )
}

impl Tool for EditFile {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn doc(&self) -> &'static str {
        "edit_file(path, search, replace): Replace one occurrence of `search` with `replace` in an existing file. `search` must match exactly one location; include a few surrounding lines to make it unique."
    }

    fn is_mutating(&self) -> bool {
        true
    }

    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let (old, outcome, _crlf) = self.dry_run(&args, ctx)?;
        Ok(format!("edit_file {} ({})\n{}", args.path, outcome.mode.describe(), render_diff(&old, &outcome.new_text)))
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let (_old, outcome, crlf) = self.dry_run(&args, ctx)?;
        let path = confine(&ctx.root, &args.path)?;
        std::fs::write(&path, restore_eol(&outcome.new_text, crlf))?;
        // occurrences 분기를 지금부터 사용 — Task 11 전까지는 항상 1이지만, 안 읽는
        // private 필드는 dead_code로 -D warnings 게이트를 깨뜨린다
        let head = if outcome.occurrences > 1 {
            format!(
                "Edited {} (replaced {} occurrences, matched {})",
                args.path,
                outcome.occurrences,
                outcome.mode.describe()
            )
        } else {
            format!("Edited {} (matched {})", args.path, outcome.mode.describe())
        };
        Ok(format!("{head}\n{}", render_context(&outcome.new_text, outcome.start_line, outcome.replaced_lines)))
    }
}

#[cfg(test)]
mod tests {
    use crate::tools::{Tool, ToolCtx, ToolError};
    use super::EditFile;

    fn setup(content: &str) -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), content).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        (dir, ctx)
    }

    fn edit(ctx: &ToolCtx, search: &str, replace: &str) -> Result<String, ToolError> {
        EditFile.run(&serde_json::json!({"path": "f.rs", "search": search, "replace": replace}), ctx)
    }

    #[test]
    fn exact_match_replaces_once_and_reports_mode() {
        let (dir, ctx) = setup("fn a() {}\nfn b() {}\n");
        let out = edit(&ctx, "fn a() {}", "fn a() { todo!() }").unwrap();
        assert!(out.contains("exact"), "{out}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert_eq!(t, "fn a() { todo!() }\nfn b() {}\n");
    }

    #[test]
    fn trailing_whitespace_is_ignored_at_stage_two() {
        let (dir, ctx) = setup("let x = 1;   \nlet y = 2;\n");
        let out = edit(&ctx, "let x = 1;\nlet y = 2;", "let x = 9;\nlet y = 2;").unwrap();
        assert!(out.contains("trailing"), "적용 모드 보고: {out}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert!(t.contains("let x = 9;"));
    }

    #[test]
    fn uniform_indent_shift_matches_and_reindents_replacement() {
        let (dir, ctx) = setup("fn outer() {\n    if x {\n        do_it();\n    }\n}\n");
        // search는 들여쓰기 없이 — 4칸 시프트로 매칭돼야 함
        let out = edit(&ctx, "if x {\n    do_it();\n}", "if x {\n    do_other();\n}").unwrap();
        assert!(out.contains("indent"), "{out}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert!(t.contains("        do_other();"), "치환문에 시프트 재적용:\n{t}");
    }

    #[test]
    fn two_exact_matches_is_an_immediate_ambiguity_error() {
        let (_d, ctx) = setup("dup();\ndup();\n");
        let err = edit(&ctx, "dup();", "x();").unwrap_err();
        assert!(matches!(err, ToolError::EditFailed(_)));
        assert!(err.to_string().contains("2 locations"), "{err}");
    }

    #[test]
    fn crlf_file_stays_crlf_after_edit() {
        let (dir, ctx) = setup("a\r\nb\r\nc\r\n");
        edit(&ctx, "b", "B").unwrap(); // search는 \n 세계에서 옴 (스펙 §4 매칭 규칙)
        let t = std::fs::read(dir.path().join("f.rs")).unwrap();
        assert_eq!(String::from_utf8(t).unwrap(), "a\r\nB\r\nc\r\n");
    }

    #[test]
    fn not_found_reports_near_miss_line() {
        let (_d, ctx) = setup("alpha\nbeta\ngamma\n");
        let err = edit(&ctx, "beta\nDELTA", "x").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "{msg}");
        assert!(msg.contains("Line 2"), "첫 줄 근접 위치 안내: {msg}");
    }

    #[test]
    fn preview_is_a_dry_run_diff_without_writing() {
        let (dir, ctx) = setup("keep\nold\n");
        let p = EditFile
            .preview(&serde_json::json!({"path": "f.rs", "search": "old", "replace": "new"}), &ctx)
            .unwrap();
        assert!(p.contains("-old") && p.contains("+new"), "{p}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert_eq!(t, "keep\nold\n", "preview는 쓰지 않는다");
    }

    #[test]
    fn empty_search_is_bad_args() {
        let (_d, ctx) = setup("x\n");
        assert!(edit(&ctx, "", "y").is_err());
    }

    #[test]
    fn identical_search_and_replace_is_an_error() {
        let (_d, ctx) = setup("fn a() {}\n");
        let err = edit(&ctx, "fn a() {}", "fn a() {}").unwrap_err();
        assert!(err.to_string().contains("identical"), "{err}");
    }

    #[test]
    fn success_reports_post_edit_context_with_line_numbers_in_header_only() {
        let (_d, ctx) = setup("l1\nl2\nl3\nl4\nOLD\nl6\nl7\nl8\nl9\n");
        let out = edit(&ctx, "OLD", "NEW").unwrap();
        assert!(out.contains("Context after edit (lines 2-8):"), "{out}");
        assert!(out.contains("NEW"), "{out}");
        assert!(out.contains("l2\nl3\nl4\nNEW\nl6\nl7\nl8"), "±3줄 원문 — 줄번호 접두 금지: {out}");
        assert!(out.contains("Verify this is what you intended"), "{out}");
    }

    #[test]
    fn context_clamps_at_file_boundaries() {
        let (_d, ctx) = setup("OLD\nl2\n");
        let out = edit(&ctx, "OLD", "NEW").unwrap();
        assert!(out.contains("Context after edit (lines 1-2):"), "{out}");
    }

    #[test]
    fn stage_two_deletion_keeps_replaced_lines_at_least_one() {
        // 2단계(후행 공백 무시) 매칭에서 replace가 빈 문자열이면 replace_lines()가
        // 빈 Vec을 반환 — replaced_lines가 0이 될 수 있다. 필드 계약(최소 1, 문서
        // 주석 참조)을 지키려면 stage 1처럼 .max(1)로 보정되어야 한다.
        let outcome = super::apply_edit("a;  \nb;\nc\n", "a;\nb;", "").unwrap();
        assert_eq!(outcome.replaced_lines, 1, "삭제(빈 치환)도 replaced_lines는 최소 1이어야 함");
        assert_eq!(outcome.new_text, "c\n");
    }
}
