use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;

use super::sse::SseParser;
use super::types::{ChatRequest, ChatResponse, StreamChunk};
use crate::config::Config;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error(
        "서버에 연결할 수 없습니다 ({base_url}).\nllama-server(또는 LM Studio 등 사용 중인 서버)가 켜져 있고 주소/포트가 맞는지 확인하세요.\n원인: {source}"
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
    #[error("HTTP 요청 실패: {0}")]
    Http(#[from] reqwest::Error),
}

/// serde_json 파싱 실패를 원문과 함께 LlmError::Parse로 감싼다 (M1 중복 제거)
fn parse_json<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, LlmError> {
    serde_json::from_str(body).map_err(|e| LlmError::Parse(format!("{e}: {body}")))
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

    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        let mut rb = self.http.get(url);
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
        parse_json(&body)
    }

    /// 스트리밍 응답. 델타마다 on_delta를 호출하고 전체 텍스트를 반환한다.
    pub async fn chat_stream(
        &self,
        req: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError> {
        let resp = self.send_with_retry(req).await?;
        let mut stream = resp.bytes_stream();
        let mut parser = SseParser::new();
        let mut full = String::new();
        'outer: while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            for payload in parser.feed(&chunk) {
                if payload == "[DONE]" {
                    break 'outer;
                }
                let parsed: StreamChunk = parse_json(&payload)?;
                if let Some(content) = parsed
                    .choices
                    .first()
                    .and_then(|c| c.delta.content.as_deref())
                {
                    on_delta(content);
                    full.push_str(content);
                }
            }
        }
        Ok(full)
    }

    pub async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        #[derive(Deserialize)]
        struct ModelList {
            data: Vec<ModelEntry>,
        }
        #[derive(Deserialize)]
        struct ModelEntry {
            id: String,
        }

        let url = format!("{}/models", self.base_url);
        let resp = self.get(&url).send().await.map_err(|e| {
            if e.is_connect() {
                LlmError::Connect { base_url: self.base_url.clone(), source: e }
            } else {
                LlmError::Http(e)
            }
        })?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api { status: status.as_u16(), body });
        }
        let body = resp.text().await?;
        let list: ModelList = parse_json(&body)?;
        Ok(list.data.into_iter().map(|m| m.id).collect())
    }
}

impl crate::llm::LlmClient for OpenAiClient {
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        OpenAiClient::chat(self, req).await
    }
}

/// 설정에 모델이 지정돼 있으면 그대로 쓰고, 없으면 서버의 첫 모델을 쓴다.
pub async fn resolve_model(client: &OpenAiClient, config: &Config) -> anyhow::Result<String> {
    if !config.model.is_empty() {
        return Ok(config.model.clone());
    }
    let models = client.list_models().await?;
    models.into_iter().next().ok_or_else(|| {
        anyhow::anyhow!(
            "서버에 로드된 모델이 없습니다. llama-server를 모델과 함께 기동하거나(scripts/serve.sh), 설정 파일에 model을 지정하세요."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::ChatMessage;
    use std::sync::Once;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static INIT: Once = Once::new();

    fn init_crypto() {
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    fn sample_request() -> ChatRequest {
        ChatRequest {
            model: "gemma-4b".into(),
            messages: vec![ChatMessage::user("hi")],
            temperature: 0.1,
            max_tokens: None,
            stream: false,
            response_format: None,
            seed: None,
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
        init_crypto();
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
        init_crypto();
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
        init_crypto();
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
        init_crypto();
        // 포트를 잡았다가 놓아서 "아무도 리슨하지 않는 주소"를 확보
        let addr = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap()
        };
        let client = OpenAiClient::new(&format!("http://{addr}/v1"), None);
        let err = client.chat(&sample_request()).await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&addr.to_string()), "주소가 포함돼야 함: {msg}");
        assert!(msg.contains("llama-server"), "실행 가능한 안내 포함: {msg}");
    }

    #[tokio::test]
    async fn chat_reports_4xx_without_retry() {
        init_crypto();
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

    #[tokio::test]
    async fn chat_stream_accumulates_deltas_in_order() {
        init_crypto();
        let server = MockServer::start().await;
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"안녕\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"하세요\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_partial_json(serde_json::json!({"stream": true})))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        let mut req = sample_request();
        req.stream = true;

        let mut seen = Vec::new();
        let full = client
            .chat_stream(&req, &mut |d| seen.push(d.to_string()))
            .await
            .unwrap();
        assert_eq!(seen, vec!["안녕".to_string(), "하세요".to_string()]);
        assert_eq!(full, "안녕하세요");
    }

    #[tokio::test]
    async fn list_models_parses_ids() {
        init_crypto();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"id": "gemma-4b", "object": "model"}, {"id": "qwen-4b", "object": "model"}]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        let models = client.list_models().await.unwrap();
        assert_eq!(models, vec!["gemma-4b".to_string(), "qwen-4b".to_string()]);
    }

    #[tokio::test]
    async fn resolve_model_prefers_config_value() {
        init_crypto();
        // 이 주소는 실제로 다이얼되지 않는다 — config에 모델이 있으면
        // 네트워크를 아예 안 탄다는 것을 검증하는 테스트라 하드코딩해도 안전
        let client = OpenAiClient::new("http://127.0.0.1:9/v1", None);
        let config = crate::config::Config {
            model: "my-model".into(),
            ..Default::default()
        };
        let m = resolve_model(&client, &config).await.unwrap();
        assert_eq!(m, "my-model");
    }

    #[tokio::test]
    async fn resolve_model_falls_back_to_first_server_model() {
        init_crypto();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "loaded-model"}]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        let config = crate::config::Config::default();
        let m = resolve_model(&client, &config).await.unwrap();
        assert_eq!(m, "loaded-model");
    }

    #[tokio::test]
    async fn resolve_model_errors_when_server_has_none() {
        init_crypto();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"data": []})),
            )
            .mount(&server)
            .await;

        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        let config = crate::config::Config::default();
        let err = resolve_model(&client, &config).await.unwrap_err();
        assert!(err.to_string().contains("모델"));
    }

    #[tokio::test]
    async fn http_error_message_is_korean() {
        // 잘못된 URL → 연결 이전의 reqwest 빌더 에러 → LlmError::Http 경로
        let client = OpenAiClient::new("not-a-url", None);
        let err = client.chat(&sample_request()).await.unwrap_err();
        assert!(err.to_string().starts_with("HTTP 요청 실패"), "{err}");
    }

    async fn call_via_trait<C: crate::llm::LlmClient>(c: &C, req: &ChatRequest) -> String {
        c.chat(req).await.unwrap().text().to_string()
    }

    #[tokio::test]
    async fn openai_client_implements_llm_client_trait() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .mount(&server)
            .await;
        let client = OpenAiClient::new(&format!("{}/v1", server.uri()), None);
        assert_eq!(call_via_trait(&client, &sample_request()).await, "hello");
    }
}
