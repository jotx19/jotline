//! Agent conversation state and its JSON session file.
//!
//! The file stores only non-system messages; system prompts are rebuilt on load so rules and
//! tools always reflect the current workspace. Version 3 files from earlier releases load
//! unchanged (their system messages are dropped).

use crate::llm::{ChatMessage, ChatRole};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SESSION_VERSION: u32 = 4;

/// One tool call recorded in an agent's history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolRecord {
    pub tool: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default = "default_true")]
    pub ok: bool,
    #[serde(default)]
    pub result: String,
}

/// One delegated subtask recorded in the parent agent's history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DelegateRecord {
    pub agent_name: String,
    pub prompt: String,
    #[serde(default)]
    pub max_steps: u32,
    #[serde(default)]
    pub result: String,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AgentSession {
    /// `messages[0]` is always the system prompt.
    pub messages: Vec<ChatMessage>,
    pub tool_history: Vec<ToolRecord>,
    pub delegate_history: Vec<DelegateRecord>,
}

#[derive(Serialize)]
pub(crate) struct SessionFileOut<'a> {
    pub version: u32,
    pub active_agent: &'a str,
    pub delegate_counter: u64,
    pub agents: BTreeMap<&'a str, AgentRecordOut<'a>>,
}

#[derive(Serialize)]
pub(crate) struct AgentRecordOut<'a> {
    pub messages: Vec<&'a ChatMessage>,
    pub tool_history: &'a [ToolRecord],
    pub delegate_history: &'a [DelegateRecord],
}

/// Session file as read. Lists hold raw values so one malformed entry is skipped instead of
/// failing the whole load.
#[derive(Deserialize, Default)]
pub(crate) struct SessionFileIn {
    #[serde(default)]
    pub active_agent: Option<String>,
    #[serde(default)]
    pub delegate_counter: u64,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentRecordIn>,
    /// Single-agent files from before named agents existed.
    #[serde(default)]
    pub messages: Option<Vec<Value>>,
    #[serde(default)]
    pub tool_history: Vec<Value>,
    #[serde(default)]
    pub delegate_history: Vec<Value>,
}

#[derive(Deserialize, Default)]
pub(crate) struct AgentRecordIn {
    #[serde(default)]
    pub messages: Vec<Value>,
    #[serde(default)]
    pub tool_history: Vec<Value>,
    #[serde(default)]
    pub delegate_history: Vec<Value>,
}

impl AgentRecordIn {
    pub(crate) fn into_parts(self) -> (Vec<ChatMessage>, Vec<ToolRecord>, Vec<DelegateRecord>) {
        (
            restore_messages(self.messages),
            typed(self.tool_history),
            typed(self.delegate_history),
        )
    }
}

impl SessionFileIn {
    /// Agents in the file, converting the legacy single-agent layout to an agent named `main`.
    pub(crate) fn take_agents(&mut self) -> BTreeMap<String, AgentRecordIn> {
        let mut agents = std::mem::take(&mut self.agents);
        if agents.is_empty() {
            if let Some(messages) = self.messages.take() {
                agents.insert(
                    "main".to_string(),
                    AgentRecordIn {
                        messages,
                        tool_history: std::mem::take(&mut self.tool_history),
                        delegate_history: std::mem::take(&mut self.delegate_history),
                    },
                );
            }
        }
        agents
    }
}

fn restore_messages(raw: Vec<Value>) -> Vec<ChatMessage> {
    raw.into_iter()
        .filter_map(|value| serde_json::from_value::<ChatMessage>(value).ok())
        .filter(|message| message.role != ChatRole::System)
        .collect()
}

fn typed<T: for<'de> Deserialize<'de>>(raw: Vec<Value>) -> Vec<T> {
    raw.into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect()
}

/// Agent names are 1-64 ASCII letters, digits, `_`, or `-`.
pub fn is_valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// The messages to send for one model call: the system prompt plus at most `limit` of the most
/// recent messages. The window never starts with an assistant message, so the conversation
/// stays well-formed for strict chat templates. `pinned` is an index into `messages` (normally
/// the current task's request) that is kept even when it falls outside the window.
pub(crate) fn context_window(
    messages: &[ChatMessage],
    limit: usize,
    pinned: Option<usize>,
) -> Vec<ChatMessage> {
    let Some((system, rest)) = messages.split_first() else {
        return Vec::new();
    };
    let mut start = rest.len().saturating_sub(limit.max(2));
    while start < rest.len() && rest[start].role != ChatRole::User {
        start += 1;
    }
    let mut window = Vec::with_capacity(rest.len() - start + 2);
    window.push(system.clone());
    if let Some(index) = pinned.and_then(|index| index.checked_sub(1)) {
        if index < start {
            if let Some(message) = rest.get(index) {
                window.push(message.clone());
            }
        }
    }
    window.extend_from_slice(&rest[start..]);
    window
}
