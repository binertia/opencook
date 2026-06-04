//! Mock LLM provider server using wiremock.

use wiremock::{
    matchers::{header_exists, method, path},
    Mock, MockServer, ResponseTemplate,
};

/// A mock OpenAI-compatible provider server.
pub struct MockProvider {
    pub server: MockServer,
}

impl MockProvider {
    /// Start a new mock provider server.
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    /// Base URL of the mock server.
    pub fn base_url(&self) -> String {
        self.server.uri()
    }

    /// Mock a non-streaming chat completion response.
    pub async fn mock_chat_completion(&self, model: &str, response_text: &str) {
        let body = serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1715000000,
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": response_text
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header_exists("authorization"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&self.server)
            .await;
    }

    /// Mock a streaming chat completion response (SSE).
    pub async fn mock_chat_completion_stream(&self, model: &str, chunks: &[&str]) {
        let mut events = String::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let data = serde_json::json!({
                "id": "chatcmpl-test",
                "object": "chat.completion.chunk",
                "created": 1715000000,
                "model": model,
                "choices": [
                    {
                        "index": 0,
                        "delta": {
                            "role": if i == 0 { Some("assistant") } else { None },
                            "content": chunk
                        },
                        "finish_reason": null
                    }
                ]
            });
            events.push_str(&format!("data: {}\n\n", data));
        }
        events.push_str("data: [DONE]\n\n");

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header_exists("authorization"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(events),
            )
            .mount(&self.server)
            .await;
    }

    /// Mock an error response.
    pub async fn mock_error(&self, status: u16, error_msg: &str) {
        let body = serde_json::json!({
            "error": {
                "message": error_msg,
                "type": "invalid_request_error"
            }
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&self.server)
            .await;
    }
}
