use std::path::Path;

use crate::tools::list_files::walk_entries;

/// 트리 주입 상한 (스펙 §6 "상한 있음"). 8K 컨텍스트 예산을 고려해 보수적으로
const TREE_MAX_ENTRIES: usize = 100;
const TREE_DEPTH: usize = 3;

/// 에이전트 시스템 프롬프트 (영어 — 소형 모델의 지시 이행률, 스펙 §4).
/// 매 턴 JSON 하나, 답변 채널은 finish.summary, few-shot 1개 포함
pub fn system_prompt(tool_docs: &str, root: &Path) -> String {
    let tree = project_tree(root);
    format!(
        "You are loco, a coding agent working inside the user's project directory. \
You interact with the project ONLY by calling tools.\n\
\n\
Respond with exactly ONE JSON object per turn and nothing else:\n\
{{\"thought\": \"<one short sentence of reasoning, in English>\", \"action\": {{\"tool\": \"<name>\", \"args\": {{...}}}}}}\n\
\n\
Rules:\n\
- One tool call per turn.\n\
- Never repeat a tool call that already returned a result - reuse that result. As soon as you have enough information, call `finish`.\n\
- To change an existing file, prefer `edit_file` with a small unique search block. Use `write_file` only for new files or full rewrites.\n\
- File paths are relative to the project root. Explore with list_files or grep before reading whole files.\n\
- When you know the answer (or cannot proceed), call `finish`. Its `summary` is the ONLY text shown to the user - put the complete answer there, written in the user's language.\n\
\n\
Tools:\n\
{tool_docs}\n\
- finish(summary): End the task and give `summary` to the user as the final answer.\n\
\n\
Example turn:\n\
{{\"thought\": \"I need to find where the config is loaded.\", \"action\": {{\"tool\": \"grep\", \"args\": {{\"pattern\": \"fn load\", \"path\": \"src\"}}}}}}\n\
\n\
Project files (partial, gitignore respected):\n\
{tree}"
    )
}

/// 프롬프트 주입용 파일 목록. list_files의 워커를 재사용한다 (DRY)
pub fn project_tree(root: &Path) -> String {
    let entries = walk_entries(root, root, Some(TREE_DEPTH), TREE_MAX_ENTRIES + 1);
    if entries.is_empty() {
        return "(no files)".to_string();
    }
    let truncated = entries.len() > TREE_MAX_ENTRIES;
    let mut out: Vec<String> = entries.into_iter().take(TREE_MAX_ENTRIES).collect();
    if truncated {
        out.push("[tree truncated]".to_string());
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_states_protocol_and_finish_channel() {
        let dir = tempfile::tempdir().unwrap();
        let p = system_prompt("- read_file(path): Read a file.", dir.path());
        assert!(p.contains("\"thought\""), "프로토콜 형태 명시");
        assert!(p.contains("- read_file(path)"), "툴 목록 주입");
        assert!(p.contains("finish"), "답변 채널 명시 (스펙 §4)");
        assert!(p.contains("summary"), "summary가 사용자에게 가는 유일한 채널");
        assert!(p.contains("Example"), "few-shot 예시 (스펙 §4)");
        assert!(p.is_ascii(), "시스템 프롬프트는 영어 (스펙 §4)");
    }

    #[test]
    fn tree_lists_files_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/junk.o"), "").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/target\n").unwrap();
        let tree = project_tree(dir.path());
        assert!(tree.contains("src/main.rs"), "{tree}");
        assert!(!tree.contains("junk.o"), "{tree}");
    }

    #[test]
    fn tree_is_capped_at_100_entries() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..110 {
            std::fs::write(dir.path().join(format!("f{i:03}.txt")), "").unwrap();
        }
        let tree = project_tree(dir.path());
        assert_eq!(tree.lines().count(), 101, "100항목 + 절삭 표시\n{tree}");
        assert_eq!(tree.lines().last().unwrap(), "[tree truncated]");
    }

    #[test]
    fn empty_project_says_no_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(project_tree(dir.path()), "(no files)");
    }
}
