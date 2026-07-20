use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    pub stream: bool,
    /// 에이전트 턴의 json_schema 강제 (스펙 §4). None이면 필드 자체를 보내지 않는다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
    /// 평가 하네스의 재현성용 (스펙 §8). None이면 필드 자체를 보내지 않는다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// 응답의 message는 content가 null일 수 있어 요청용 ChatMessage와 분리
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    /// llama.cpp가 사고 토큰을 분리해 흘리는 필드 (M14 B-1). 이것을 안 읽으면
    /// content가 빈 턴에서 모델 출력을 통째로 버린다 — 파일럿 "빈 응답" 4건
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// 토큰 소비량. 서버가 reasoning_tokens를 분해해 주지 않으므로
/// completion_tokens는 추론분을 **포함한** 합산값이다 (M14 B-1, 라이브 실측)
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub completion_tokens: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

impl ChatResponse {
    /// 첫 번째 choice의 텍스트. 없으면 빈 문자열.
    pub fn text(&self) -> &str {
        self.choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .unwrap_or("")
    }

    /// 첫 번째 choice의 finish_reason ("stop", "length" 등)
    pub fn finish_reason(&self) -> Option<&str> {
        self.choices.first().and_then(|c| c.finish_reason.as_deref())
    }

    /// 첫 번째 choice의 추론 꼬리. 없으면 빈 문자열
    pub fn reasoning(&self) -> &str {
        self.choices
            .first()
            .and_then(|c| c.message.reasoning_content.as_deref())
            .unwrap_or("")
    }

    /// 출력 토큰 소비량. content가 빈 턴에서는 곧 추론 소비량이고,
    /// 그 외에는 합산값이라 분리되지 않는다
    pub fn completion_tokens(&self) -> Option<u32> {
        self.usage.as_ref().and_then(|u| u.completion_tokens)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Delta {
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamChoice {
    #[serde(default)]
    pub delta: Delta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub choices: Vec<StreamChoice>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_expected_fields() {
        let req = ChatRequest {
            model: "gemma-4b".into(),
            messages: vec![ChatMessage::system("be brief"), ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
            response_format: None,
            seed: None,
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["model"], "gemma-4b");
        assert_eq!(v["messages"][0]["role"], "system");
        assert_eq!(v["messages"][1]["content"], "hi");
        assert_eq!(v["stream"], false);
        assert!(v.get("max_tokens").is_none(), "None이면 필드 생략");
    }

    #[test]
    fn request_serializes_max_tokens_when_set() {
        let req = ChatRequest {
            model: "gemma-4b".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: Some(2048),
            stream: false,
            response_format: None,
            seed: None,
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["max_tokens"], 2048, "Some이면 값이 그대로 직렬화되어야 함");
    }

    #[test]
    fn response_format_is_omitted_when_none() {
        let req = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
            response_format: None,
            seed: None,
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert!(v.get("response_format").is_none());
    }

    #[test]
    fn response_format_serializes_when_set() {
        let req = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
            response_format: Some(serde_json::json!({"type": "json_schema"})),
            seed: None,
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["response_format"]["type"], "json_schema");
    }

    #[test]
    fn finish_reason_reads_first_choice() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": "x"}, "finish_reason": "length"}]}"#;
        let resp: ChatResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.finish_reason(), Some("length"));

        let none = r#"{"choices": []}"#;
        let resp: ChatResponse = serde_json::from_str(none).unwrap();
        assert_eq!(resp.finish_reason(), None);
    }

    #[test]
    fn response_deserializes_openai_shape() {
        let body = r#"{
            "id": "chatcmpl-1", "object": "chat.completion", "created": 1,
            "model": "gemma-4b",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "안녕하세요"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13}
        }"#;
        let resp: ChatResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.text(), "안녕하세요");
    }

    #[test]
    fn response_with_null_content_is_ok() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": null}}]}"#;
        let resp: ChatResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.text(), "");
    }

    #[test]
    fn stream_chunk_deserializes() {
        let body = r#"{"choices": [{"delta": {"content": "안"}, "finish_reason": null}]}"#;
        let chunk: StreamChunk = serde_json::from_str(body).unwrap();
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("안"));

        let done = r#"{"choices": [{"delta": {}, "finish_reason": "stop"}]}"#;
        let chunk: StreamChunk = serde_json::from_str(done).unwrap();
        assert_eq!(chunk.choices[0].delta.content, None);
        assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn reasoning_content_is_parsed_from_a_non_streaming_response() {
        // llama.cpp 실측 페이로드 형태 — content는 빈 채 reasoning_content만 온다
        let raw = r#"{
          "choices": [{
            "message": {"role":"assistant","content":"","reasoning_content":"Let me think about the file layout"},
            "finish_reason": "length"
          }],
          "usage": {"completion_tokens": 40, "prompt_tokens": 23, "total_tokens": 63}
        }"#;
        let r: ChatResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.text(), "");
        assert_eq!(r.reasoning(), "Let me think about the file layout");
        assert_eq!(r.completion_tokens(), Some(40));
        assert_eq!(r.finish_reason(), Some("length"));
    }

    #[test]
    fn a_response_without_the_new_fields_still_parses() {
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":"hi"}}]}"#;
        let r: ChatResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.text(), "hi");
        assert_eq!(r.reasoning(), "");
        assert_eq!(r.completion_tokens(), None);
    }

    #[test]
    fn seed_is_omitted_when_none_and_serialized_when_set() {
        let mut req = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
            response_format: None,
            seed: None,
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert!(v.get("seed").is_none(), "None이면 필드 생략 (기존 경로 무영향)");
        req.seed = Some(42);
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["seed"], 42);
    }
}
