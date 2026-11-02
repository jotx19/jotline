use super::{clip_body, http_client, ChatMessage, LlmClient};
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

/// Client for any `/v1/chat/completions` server: vLLM, llama.cpp, LM Studio, LocalAI, and
/// hosted OpenAI-compatible APIs.
#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    endpoint: String,
    api_key: Option<String>,
    http: Client,
}

impl OpenAiCompatibleClient {
    /// `api_key` is sent as a bearer token when present; local servers usually need none.
    pub fn new(endpoint: String, api_key: Option<String>, timeout_seconds: u64) -> Result<Self> {
        Ok(Self {
            endpoint,
            api_key,
            http: http_client(timeout_seconds)?,
        })
    }
}

#[derive(Serialize)]
struct Payload<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
}

#[derive(Deserialize)]
struct Response {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

impl LlmClient for OpenAiCompatibleClient {
    fn chat(&self, model: &str, messages: &[ChatMessage], temperature: f32) -> Result<String> {
        let payload = Payload {
            model,
            messages,
            temperature,
        };

        let mut request = self
            .http
            .post(&self.endpoint)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json");
        if let Some(api_key) = &self.api_key {
            request = request.header(AUTHORIZATION, format!("Bearer {api_key}"));
        }

        let resp = request
            .json(&payload)
            .send()
            .with_context(|| format!("failed request to {}", self.endpoint))?;

        let status = resp.status();
        let body = resp.text().context("failed to read response body")?;
        if !status.is_success() {
            return Err(anyhow!(
                "HTTP {status} from {}: {}",
                self.endpoint,
                clip_body(&body)
            ));
        }

        let parsed: Response = serde_json::from_str(&body).with_context(|| {
            format!("invalid JSON from {}: {}", self.endpoint, clip_body(&body))
        })?;
        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| {
                anyhow!(
                    "unexpected OpenAI-compatible response shape: {}",
                    clip_body(&body)
                )
            })
    }
}
