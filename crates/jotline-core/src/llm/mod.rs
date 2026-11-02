//! Chat-completion clients for local and OpenAI-compatible model servers.

mod ollama;
mod openai;

pub use ollama::OllamaClient;
pub use openai::OpenAiCompatibleClient;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Sender of a chat message, serialized as the lowercase role names both APIs expect.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

/// A blocking chat-completion backend. Implementations return the assistant text only.
pub trait LlmClient: Send + Sync {
    fn chat(&self, model: &str, messages: &[ChatMessage], temperature: f32) -> Result<String>;
}

fn http_client(timeout_seconds: u64) -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_seconds.max(1)))
        .build()
        .context("failed to build HTTP client")
}

/// Shortens a response body so error messages stay readable.
fn clip_body(body: &str) -> String {
    const LIMIT: usize = 600;
    if body.chars().count() <= LIMIT {
        return body.to_string();
    }
    let mut clipped = body.chars().take(LIMIT).collect::<String>();
    clipped.push_str("...");
    clipped
}
