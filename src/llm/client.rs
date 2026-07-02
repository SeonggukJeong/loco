use std::time::Duration;

use super::types::{ChatRequest, ChatResponse};

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error(
        "서버에 연결할 수 없습니다 ({base_url}).\nLM Studio(또는 사용 중인 서버)가 켜져 있고 주소/포트가 맞는지 확인하세요.\n원인: {source}"
    )]
    Connect {
        base_url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("API 오류 (HTTP {status}): {body}")]
    Api { status: u16, body: String },
    #[error("응답 파싱 실패: {0}")]
    Parse(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

/// 총 시도 횟수 (초기 1회 + 재시도 2회). 스펙 §9의 "3회"는 총 시도 기준.
const MAX_ATTEMPTS: u32 = 3;

pub struct OpenAiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl OpenAiClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            // 사내망의 http_proxy/HTTP_PROXY env var가 localhost LLM 트래픽을
            // 프록시로 보내는 것을 차단
            http: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("HTTP 클라이언트 생성 실패"),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    fn post(&self, url: &str) -> reqwest::RequestBuilder {
        let mut rb = self.http.post(url);
        if let Some(key) = &self.api_key {
            rb = rb.bearer_auth(key);
        }
        rb
    }

    /// 연결 실패/5xx는 지수 백오프(200ms, 400ms)로 총 MAX_ATTEMPTS회 시도
    async fn send_with_retry(
        &self,
        req: &ChatRequest,
    ) -> Result<reqwest::Response, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut last_err: Option<LlmError> = None;
        for attempt in 0..MAX_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(200 * 2u64.pow(attempt - 1))).await;
            }
            match self.post(&url).json(req).send().await {
                Err(e) if e.is_connect() => {
                    last_err = Some(LlmError::Connect {
                        base_url: self.base_url.clone(),
                        source: e,
                    });
                }
                Err(e) => return Err(LlmError::Http(e)),
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_server_error() {
                        let body = resp.text().await.unwrap_or_default();
                        last_err = Some(LlmError::Api { status: status.as_u16(), body });
                    } else if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        return Err(LlmError::Api { status: status.as_u16(), body });
                    } else {
                        return Ok(resp);
                    }
                }
            }
        }
        Err(last_err.expect("루프가 최소 1회 실행됨"))
    }

    pub async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let resp = self.send_with_retry(req).await?;
        let body = resp.text().await?;
        serde_json::from_str(&body).map_err(|e| LlmError::Parse(format!("{e}: {body}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::ChatMessage;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_request() -> ChatRequest {
        ChatRequest {
            model: "gemma-4b".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
        }
    }

    fn ok_body() -> serde_json::Value {
        serde_json::json!({
            "choices": [{
                "message": {"role": "assistant", "content": "hello"},
                "finish_reason": "stop"
            }]
        })
    }

    #[tokio::test]
    async fn chat_posts_to_chat_completions() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_partial_json(serde_json::json!({
                "model": "gemma-4b",
                "messages": [{"role": "user", "content": "hi"}]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1/", server.uri()), None);
        let resp = client.chat(&sample_request()).await.unwrap();
        assert_eq!(resp.text(), "hello");
    }

    #[tokio::test]
    async fn chat_sends_bearer_token_when_configured() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), Some("sk-test".into()));
        client.chat(&sample_request()).await.unwrap();
    }

    #[tokio::test]
    async fn chat_retries_on_500_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        let resp = client.chat(&sample_request()).await.unwrap();
        assert_eq!(resp.text(), "hello");
    }

    #[tokio::test]
    async fn chat_gives_actionable_error_when_server_down() {
        // 포트를 잡았다가 놓아서 "아무도 리슨하지 않는 주소"를 확보
        let addr = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap()
        };
        let client = OpenAiClient::new(&format!("http://{addr}/v1"), None);
        let err = client.chat(&sample_request()).await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&addr.to_string()), "주소가 포함돼야 함: {msg}");
        assert!(msg.contains("LM Studio"), "실행 가능한 안내 포함: {msg}");
    }

    #[tokio::test]
    async fn chat_reports_4xx_without_retry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(404).set_body_string("model not found"))
            .expect(1)
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        let err = client.chat(&sample_request()).await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 404, .. }));
    }
}
