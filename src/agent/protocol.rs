const FORMAT_HINT: &str = r#"Reply with exactly one JSON object: {"thought": "...", "action": {"tool": "...", "args": {...}}}"#;

/// 매 턴 모델이 출력해야 하는 구조 (스펙 §4)
#[derive(Debug, Clone, PartialEq)]
pub struct ModelTurn {
    pub thought: String,
    pub action: Action,
    /// salvage 정규화(M5 §5.1)가 적용됐는지 — 루프가 툴 결과에 교정 노트를 붙인다
    pub salvaged: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub tool: String,
    pub args: serde_json::Value,
}

/// 모델 출력에서 턴 하나를 파싱한다. Err는 모델에 되먹일 영어 피드백 (스펙 §9).
/// 사다리: 그대로 → 펜스 제거 → 첫 JSON 오브젝트 스캔.
/// json_schema 강제가 꺼진 폴백 모드에서도 이 사다리가 동작해야 한다 (스펙 §4)
pub fn parse_turn(text: &str) -> Result<ModelTurn, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("Your reply was empty. {FORMAT_HINT}"));
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text)
        && v.is_object()
    {
        return build_turn(v);
    }
    // 전체가 JSON이어도 오브젝트가 아니면(배열 등) 사다리 계속 — 내부 오브젝트 salvage 기회 보존
    // let 체인 (edition 2024) — 중첩 if let은 clippy::collapsible_if에 걸린다
    if let Some(inner) = strip_fence(text)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(inner)
        && v.is_object()
    {
        return build_turn(v);
    }
    // 펜스 안쪽도 마찬가지 — 오브젝트가 아니면 다음 사다리 단계(첫 오브젝트 스캔)로 폴스루
    if let Some(obj) = first_json_object(text) {
        return match serde_json::from_str::<serde_json::Value>(obj) {
            Ok(v) => build_turn(v),
            Err(e) => Err(format!("Your reply was not a valid turn ({e}). {FORMAT_HINT}")),
        };
    }
    Err(format!("Your reply contained no JSON object. {FORMAT_HINT}"))
}

/// Value → ModelTurn + salvage 정규화 (M5 §5.1). serde_json 기본 Map은 BTreeMap이라
/// 순회가 키 이름 오름차순 — 병합이 결정론적이고 나중 키(args_2 등)가 이긴다.
/// 스칼라 승격은 부재 시 삽입만(기존 args 보호), 오브젝트 병합은 덮어쓴다(최신 의도)
fn build_turn(v: serde_json::Value) -> Result<ModelTurn, String> {
    let serde_json::Value::Object(mut top) = v else {
        return Err(format!("Your reply was not a valid turn (not a JSON object). {FORMAT_HINT}"));
    };
    let thought = match top.remove("thought") {
        Some(serde_json::Value::String(s)) => s,
        _ => return Err(format!("Your reply was not a valid turn (missing field `thought`). {FORMAT_HINT}")),
    };
    let serde_json::Value::Object(mut act) = top.remove("action").unwrap_or(serde_json::Value::Null) else {
        return Err(format!("Your reply was not a valid turn (missing field `action`). {FORMAT_HINT}"));
    };
    let tool = match act.remove("tool") {
        Some(serde_json::Value::String(s)) => s,
        _ => return Err(format!("Your reply was not a valid turn (missing field `tool` in action). {FORMAT_HINT}")),
    };
    let (mut map, args_was_object) = match act.remove("args") {
        Some(serde_json::Value::Object(m)) => (m, true),
        Some(serde_json::Value::Null) | None => (serde_json::Map::new(), false),
        // 비오브젝트 args는 salvage 불가 — 그대로 전달, 툴 쪽 BadArgs가 처리
        Some(other) => return Ok(ModelTurn { thought, action: Action { tool, args: other }, salvaged: false }),
    };
    let mut salvaged = false;
    for (k, val) in act {
        salvaged |= merge_entry(&mut map, k, val);
    }
    for (k, val) in top {
        if k == "tool" || k.starts_with("args") {
            continue; // 예약어 — 플랫 턴 변형과의 혼동 방지 (M5 §5.1 범위 제외)
        }
        salvaged |= merge_entry(&mut map, k, val);
    }
    let args = if map.is_empty() && !args_was_object {
        serde_json::Value::Null // 기존 계약: args 부재 → Null
    } else {
        serde_json::Value::Object(map)
    };
    Ok(ModelTurn { thought, action: Action { tool, args }, salvaged })
}

/// k/val을 args 맵에 병합. 오브젝트는 엔트리 덮어쓰기(최신 의도 우선), 스칼라는
/// 부재 시에만 삽입(기존 args 보호). 반환: 실제로 뭔가 넣었는지
fn merge_entry(map: &mut serde_json::Map<String, serde_json::Value>, k: String, val: serde_json::Value) -> bool {
    match val {
        serde_json::Value::Object(inner) => {
            let mut any = false;
            for (ik, iv) in inner {
                map.insert(ik, iv);
                any = true;
            }
            any
        }
        other => {
            if map.contains_key(&k) {
                false
            } else {
                map.insert(k, other);
                true
            }
        }
    }
}

/// ```json ... ``` 펜스 내부를 꺼낸다
fn strip_fence(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("```")?;
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    let end = rest.rfind("```")?;
    Some(rest[..end].trim())
}

/// 문자열/이스케이프를 인지하며 첫 번째 균형 잡힌 {...}를 찾는다.
/// 인덱스는 전부 ASCII 바이트(`{`, `}`, `"`, `\`) 위치라 char 경계 안전.
fn first_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, &b) in text.as_bytes().iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// response_format: json_schema — 의도적으로 얕은 스키마 (스펙 §4).
/// tool만 enum으로 제약하고 args는 자유 오브젝트로 둔다. 툴별 인자 검증은
/// 앱 쪽(serde)에서 하고 위반 시 에러를 되먹인다. strict 플래그는 쓰지 않는다
/// (백엔드가 additionalProperties: false를 요구해 자유 args와 충돌할 수 있음)
pub fn response_format(tool_names: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": "agent_turn",
            "schema": {
                "type": "object",
                "properties": {
                    "thought": {"type": "string"},
                    "action": {
                        "type": "object",
                        "properties": {
                            "tool": {"type": "string", "enum": tool_names},
                            "args": {"type": "object"}
                        },
                        "required": ["tool", "args"]
                    }
                },
                "required": ["thought", "action"]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_json_turn() {
        let t = parse_turn(r#"{"thought": "look", "action": {"tool": "read_file", "args": {"path": "a.rs"}}}"#)
            .unwrap();
        assert_eq!(t.thought, "look");
        assert_eq!(t.action.tool, "read_file");
        assert_eq!(t.action.args["path"], "a.rs");
    }

    #[test]
    fn missing_args_defaults_to_null() {
        let t = parse_turn(r#"{"thought": "done", "action": {"tool": "finish"}}"#).unwrap();
        assert!(t.action.args.is_null());
    }

    #[test]
    fn parses_a_fenced_turn() {
        let text = "```json\n{\"thought\": \"t\", \"action\": {\"tool\": \"finish\", \"args\": {}}}\n```";
        assert_eq!(parse_turn(text).unwrap().action.tool, "finish");
    }

    #[test]
    fn extracts_object_from_surrounding_prose() {
        let text = "Sure! Here is my move: {\"thought\": \"t\", \"action\": {\"tool\": \"finish\", \"args\": {}}} hope that helps";
        assert_eq!(parse_turn(text).unwrap().action.tool, "finish");
    }

    #[test]
    fn handles_braces_and_escapes_inside_strings() {
        let text = r#"{"thought": "regex like {\"x\": 1}", "action": {"tool": "grep", "args": {"pattern": "fn \\w+ \\{"}}}"#;
        let t = parse_turn(text).unwrap();
        assert_eq!(t.action.tool, "grep");
        assert_eq!(t.action.args["pattern"], "fn \\w+ \\{");
        // 산문에 싸여 있으면 스캐너 경로를 타는데, 문자열 안 중괄호에 속지 않아야 한다
        let wrapped = format!("Here is my move: {text} — done");
        assert_eq!(parse_turn(&wrapped).unwrap().action.tool, "grep");
    }

    #[test]
    fn garbage_and_empty_are_errors_with_format_hint() {
        for bad in ["no json here", "", "   "] {
            let err = parse_turn(bad).unwrap_err();
            assert!(err.contains("JSON object"), "{err}");
            assert!(err.contains("thought"), "형식 힌트 포함: {err}");
        }
    }

    #[test]
    fn valid_json_with_wrong_shape_is_an_error() {
        let err = parse_turn(r#"{"answer": 42}"#).unwrap_err();
        assert!(err.contains("thought"), "{err}");
    }

    #[test]
    fn salvages_action_level_scalar_fields_into_args() {
        // qwen fix-compile-error 실측: 인자를 action 레벨에 둠
        let t = parse_turn(r#"{"thought": "build", "action": {"args": {}, "tool": "run_command", "command": "cargo build"}}"#).unwrap();
        assert_eq!(t.action.args["command"], "cargo build");
        assert!(t.salvaged);
    }

    #[test]
    fn salvages_args_2_object_overwriting_stale_args() {
        // gemma add-function 실측: args에 grep 잔재, 진짜 인자는 args_2
        let t = parse_turn(r#"{"thought": "edit", "action": {"args": {"pattern": "median", "path": "src"}, "tool": "edit_file", "args_2": {"search": "todo!()", "replace": "42", "path": "src/lib.rs"}}}"#).unwrap();
        assert_eq!(t.action.args["search"], "todo!()");
        assert_eq!(t.action.args["path"], "src/lib.rs", "args_2(키 이름 뒤 순서)가 stale args를 덮는다");
        assert_eq!(t.action.args["pattern"], "median", "잔재 키는 남아도 무해 — 툴이 무시");
        assert!(t.salvaged);
    }

    #[test]
    fn salvages_finish_summary_from_args_2() {
        let t = parse_turn(r#"{"thought": "done", "action": {"tool": "finish", "args_2": {"summary": "답"}}}"#).unwrap();
        assert_eq!(t.action.args["summary"], "답");
        assert!(t.salvaged);
    }

    #[test]
    fn top_level_unknown_scalar_is_promoted_but_reserved_names_are_not() {
        let t = parse_turn(r#"{"thought": "run", "action": {"tool": "run_command"}, "command": "ls", "args": {"junk": 1}}"#).unwrap();
        assert_eq!(t.action.args["command"], "ls");
        assert!(t.action.args.get("junk").is_none(), "최상위 tool/args*는 예약어 — 승격 금지 (플랫 턴은 범위 제외)");
    }

    #[test]
    fn scalar_promotion_does_not_overwrite_existing_args() {
        let t = parse_turn(r#"{"thought": "r", "action": {"tool": "read_file", "args": {"path": "good.rs"}, "path": "junk"}}"#).unwrap();
        assert_eq!(t.action.args["path"], "good.rs", "스칼라 승격은 부재 시 삽입만");
    }

    #[test]
    fn non_object_args_pass_through_without_salvage() {
        let t = parse_turn(r#"{"thought": "x", "action": {"tool": "grep", "args": "fn main"}}"#).unwrap();
        assert_eq!(t.action.args, serde_json::json!("fn main"));
        assert!(!t.salvaged);
    }

    #[test]
    fn clean_turns_are_not_marked_salvaged() {
        let t = parse_turn(r#"{"thought": "look", "action": {"tool": "read_file", "args": {"path": "a.rs"}}}"#).unwrap();
        assert!(!t.salvaged);
    }

    #[test]
    fn top_level_array_falls_through_to_inner_object() {
        // 전체 텍스트가 유효 JSON(배열)이어도 내부 턴 오브젝트를 건진다 — 사다리 폴스루 회귀 방지
        let t = parse_turn(r#"[{"thought": "x", "action": {"tool": "list_files", "args": {}}}]"#).unwrap();
        assert_eq!(t.action.tool, "list_files");
        assert!(!t.salvaged);
    }

    #[test]
    fn schema_is_shallow_with_tool_enum() {
        let v = response_format(&["read_file", "list_files", "grep", "finish"]);
        assert_eq!(v["type"], "json_schema");
        let schema = &v["json_schema"]["schema"];
        assert_eq!(schema["required"], serde_json::json!(["thought", "action"]));
        assert_eq!(
            schema["properties"]["action"]["properties"]["tool"]["enum"],
            serde_json::json!(["read_file", "list_files", "grep", "finish"])
        );
        // args는 자유 오브젝트 — 깊은 oneOf 유니온 금지 (스펙 §4)
        assert_eq!(
            schema["properties"]["action"]["properties"]["args"],
            serde_json::json!({"type": "object"})
        );
    }
}
