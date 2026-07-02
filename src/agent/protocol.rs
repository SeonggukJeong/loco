use serde::Deserialize;

/// 매 턴 모델이 출력해야 하는 구조 (스펙 §4)
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelTurn {
    pub thought: String,
    pub action: Action,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Action {
    pub tool: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

/// 모델 출력에서 턴 하나를 파싱한다. Err는 모델에 되먹일 영어 피드백 (스펙 §9).
/// 사다리: 그대로 → 펜스 제거 → 첫 JSON 오브젝트 스캔.
/// json_schema 강제가 꺼진 폴백 모드에서도 이 사다리가 동작해야 한다 (스펙 §4)
pub fn parse_turn(text: &str) -> Result<ModelTurn, String> {
    const FORMAT_HINT: &str = r#"Reply with exactly one JSON object: {"thought": "...", "action": {"tool": "...", "args": {...}}}"#;
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("Your reply was empty. {FORMAT_HINT}"));
    }
    if let Ok(turn) = serde_json::from_str::<ModelTurn>(text) {
        return Ok(turn);
    }
    // let 체인 (edition 2024) — 중첩 if let은 clippy::collapsible_if에 걸린다
    if let Some(inner) = strip_fence(text)
        && let Ok(turn) = serde_json::from_str::<ModelTurn>(inner)
    {
        return Ok(turn);
    }
    if let Some(obj) = first_json_object(text) {
        return match serde_json::from_str::<ModelTurn>(obj) {
            Ok(turn) => Ok(turn),
            Err(e) => Err(format!("Your reply was not a valid turn ({e}). {FORMAT_HINT}")),
        };
    }
    Err(format!("Your reply contained no JSON object. {FORMAT_HINT}"))
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
