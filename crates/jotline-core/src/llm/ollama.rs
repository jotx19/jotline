use super::{clip_body, http_client, ChatMessage, LlmClient};
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

/// Client for Ollama's `/api/chat` endpoint, for example `http://127.0.0.1:11434/api/chat`.
#[derive(Clone)]
pub struct OllamaClient {
    endpoint: String,
    http: Client,
}

impl OllamaClient {
    /// `timeout_seconds` bounds one whole request; local models under load can need minutes.
    pub fn new(endpoint: String, timeout_seconds: u64) -> Result<Self> {
        Ok(Self {
            endpoint,
            http: http_client(timeout_seconds)?,
        })
    }
}

#[derive(Serialize)]
struct Payload<'a> {
    model: &'a str,
    stream: bool,
    messages: &'a [ChatMessage],
    options: Options,
}

#[derive(Serialize)]
struct Options {
    temperature: f32,
}

#[derive(Deserialize)]
struct Response {
    message: Option<ResponseMessage>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

impl LlmClient for OllamaClient {
    fn chat(&self, model: &str, messages: &[ChatMessage], temperature: f32) -> Result<String> {
        let payload = Payload {
            model,
            stream: false,
            messages,
            options: Options { temperature },
        };

        let resp = self
            .http
            .post(&self.endpoint)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .json(&payload)
            .send()
            .with_context(|| {
                format!(
                    "failed request to {} (is `ollama serve` running?)",
                    self.endpoint
                )
            })?;

        let status = resp.status();
        let body = resp.text().context("failed to read response body")?;
        if status == StatusCode::NOT_FOUND && body.contains("not found") {
            return Err(anyhow!(
                "model '{model}' is not installed in Ollama; run `ollama pull {model}` or pick an installed tag from `ollama list`"
            ));
        }
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
            .message
            .map(|message| message.content)
            .ok_or_else(|| anyhow!("unexpected Ollama response shape: {}", clip_body(&body)))
    }
}
