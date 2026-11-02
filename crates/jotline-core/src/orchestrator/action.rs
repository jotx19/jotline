//! The typed JSON action protocol between the model and the orchestrator.
//!
//! Small local models produce JSON with stray prose, markdown fences, `<think>` blocks, quoted
//! numbers, and alternate key names. Parsing is lenient about *form* (extraction, aliases,
//! scalar coercion) but strict about *meaning*: an unknown action type or a missing required
//! field is a [`ParseError`] whose message is sent back to the model as a correction.

use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::fmt;

/// Action type names accepted in the `type` field.
pub const ACTION_TYPES: [&str; 5] = ["plan", "tool", "parallel_tools", "delegate", "final"];

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Plan(PlanAction),
    Tool(ToolAction),
    ParallelTools(ParallelAction),
    Delegate(DelegateAction),
    Final(FinalAction),
}

impl Action {
    pub fn type_name(&self) -> &'static str {
        match self {
            Action::Plan(_) => "plan",
            Action::Tool(_) => "tool",
            Action::ParallelTools(_) => "parallel_tools",
            Action::Delegate(_) => "delegate",
            Action::Final(_) => "final",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlanAction {
    /// Intent name chosen by the model; overrides the classifier when it parses.
    #[serde(default, deserialize_with = "opt_string")]
    pub intent: Option<String>,
    #[serde(default, deserialize_with = "opt_bool")]
    pub needs_plan: Option<bool>,
    #[serde(default, deserialize_with = "string")]
    pub summary: String,
    #[serde(default, deserialize_with = "string")]
    pub reason: String,
    #[serde(default, deserialize_with = "string_list")]
    pub steps: Vec<String>,
    /// Files the model expects to touch.
    #[serde(default, deserialize_with = "string_list")]
    pub files: Vec<String>,
    #[serde(default, deserialize_with = "string_list")]
    pub focus: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToolAction {
    #[serde(alias = "name", alias = "tool_name", deserialize_with = "string")]
    pub tool: String,
    #[serde(default, alias = "arguments", alias = "parameters")]
    pub args: Value,
    #[serde(default, deserialize_with = "string")]
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ParallelAction {
    #[serde(alias = "tools", alias = "tool_calls")]
    pub calls: Vec<ParallelCall>,
    #[serde(default, deserialize_with = "opt_u32")]
    pub max_workers: Option<u32>,
    #[serde(default, deserialize_with = "string")]
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ParallelCall {
    #[serde(alias = "name", deserialize_with = "string")]
    pub tool: String,
    #[serde(default, alias = "arguments")]
    pub args: Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DelegateAction {
    #[serde(alias = "task", deserialize_with = "string")]
    pub prompt: String,
    #[serde(default, alias = "agent", deserialize_with = "opt_string")]
    pub agent_name: Option<String>,
    #[serde(default, deserialize_with = "opt_u32")]
    pub max_steps: Option<u32>,
    #[serde(default, deserialize_with = "string")]
    pub reason: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FinalAction {
    #[serde(
        default,
        alias = "answer",
        alias = "content",
        deserialize_with = "string"
    )]
    pub message: String,
    #[serde(default, deserialize_with = "string")]
    pub summary: String,
    #[serde(default, deserialize_with = "string_list")]
    pub completed: Vec<String>,
    #[serde(default, deserialize_with = "string_list")]
    pub verified: Vec<String>,
    #[serde(default, deserialize_with = "string_list")]
    pub missed: Vec<String>,
    #[serde(default, deserialize_with = "string_list")]
    pub next_steps: Vec<String>,
}

/// Why a model reply was not a usable action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseError {
    /// No JSON object could be found in the reply.
    NoJson,
    /// A JSON object was found but its `type` is missing and could not be inferred.
    MissingType,
    UnknownType(String),
    /// The type is known but a field has the wrong shape or a required field is missing.
    Invalid {
        action_type: String,
        message: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NoJson => f.write_str(
                "Your reply contained no JSON object. Reply with exactly one JSON object and nothing else.",
            ),
            ParseError::MissingType => write!(
                f,
                "Your JSON object has no \"type\" field. Set \"type\" to one of: {}.",
                ACTION_TYPES.join(", ")
            ),
            ParseError::UnknownType(kind) => write!(
                f,
                "Unknown action type \"{kind}\". Use one of: {}.",
                ACTION_TYPES.join(", ")
            ),
            ParseError::Invalid {
                action_type,
                message,
            } => write!(
                f,
                "Your \"{action_type}\" action is malformed: {message}. Fix it and reply again."
            ),
        }
    }
}

/// Parses one model reply into an action, returning the canonical JSON object as well so it
/// can be stored in the conversation instead of the raw reply.
pub fn parse_action(text: &str) -> Result<(Action, Value), ParseError> {
    let mut object = extract_json_object(text).ok_or(ParseError::NoJson)?;
    let map = object.as_object_mut().ok_or(ParseError::NoJson)?;

    let kind = match map.get("type").and_then(Value::as_str) {
        Some(kind) => normalize_type(kind),
        None => infer_type(map).ok_or(ParseError::MissingType)?.to_string(),
    };
    if !ACTION_TYPES.contains(&kind.as_str()) {
        return Err(ParseError::UnknownType(kind));
    }
    map.insert("type".to_string(), Value::String(kind.clone()));

    let action =
        serde_json::from_value::<Action>(object.clone()).map_err(|err| ParseError::Invalid {
            action_type: kind.clone(),
            message: err.to_string(),
        })?;
    validate(&action).map_err(|message| ParseError::Invalid {
        action_type: kind,
        message,
    })?;
    Ok((action, object))
}

fn normalize_type(kind: &str) -> String {
    let lowered = kind.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match lowered.as_str() {
        "tool_call" | "call" | "function" => "tool".to_string(),
        "parallel" | "parallel_tool" | "tools" => "parallel_tools".to_string(),
        "final_answer" | "answer" | "done" | "finish" => "final".to_string(),
        "delegation" | "subtask" => "delegate".to_string(),
        _ => lowered,
    }
}

/// Infers a missing `type` from distinctive fields.
fn infer_type(map: &serde_json::Map<String, Value>) -> Option<&'static str> {
    if map.contains_key("tool") {
        Some("tool")
    } else if map.contains_key("calls") {
        Some("parallel_tools")
    } else if map.contains_key("steps") {
        Some("plan")
    } else if map.contains_key("message") || map.contains_key("completed") {
        Some("final")
    } else {
        None
    }
}

fn validate(action: &Action) -> Result<(), String> {
    match action {
        Action::Tool(tool) if tool.tool.trim().is_empty() => {
            Err("\"tool\" must name a tool".into())
        }
        Action::Tool(tool) if !(tool.args.is_object() || tool.args.is_null()) => {
            Err("\"args\" must be a JSON object".into())
        }
        Action::ParallelTools(parallel) if parallel.calls.is_empty() => {
            Err("\"calls\" must contain at least one {tool, args}".into())
        }
        Action::Delegate(delegate) if delegate.prompt.trim().is_empty() => {
            Err("\"prompt\" must describe the subtask".into())
        }
        _ => Ok(()),
    }
}

/// Finds the JSON object in a reply: the whole reply, a fenced code block, or the first
/// embedded object that has a `type` field (falling back to the first object at all).
/// `<think>...</think>` reasoning blocks are ignored.
pub fn extract_json_object(text: &str) -> Option<Value> {
    let cleaned = strip_think_blocks(text);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(value) = parse_object_prefix(trimmed) {
        return Some(value);
    }

    for block in fenced_blocks(trimmed) {
        if let Some(value) = parse_object_prefix(block.trim()) {
            return Some(value);
        }
    }

    let mut first_object = None;
    for (index, _) in trimmed.match_indices('{') {
        if let Some(value) = parse_object_prefix(&trimmed[index..]) {
            if value.get("type").is_some() {
                return Some(value);
            }
            first_object.get_or_insert(value);
        }
    }
    first_object
}

/// Parses a JSON object at the start of `candidate`, ignoring anything after it.
fn parse_object_prefix(candidate: &str) -> Option<Value> {
    let mut stream = serde_json::Deserializer::from_str(candidate).into_iter::<Value>();
    match stream.next() {
        Some(Ok(value)) if value.is_object() => Some(value),
        _ => None,
    }
}

fn fenced_blocks(text: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let body = &after[body_start..];
        match body.find("```") {
            Some(end) => {
                blocks.push(&body[..end]);
                rest = &body[end + 3..];
            }
            None => {
                blocks.push(body);
                break;
            }
        }
    }
    blocks
}

/// Removes `<think>...</think>` sections (and an unterminated trailing one).
pub fn strip_think_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn scalar_to_string(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.trim().to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        other => Some(other.to_string()),
    }
}

fn string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(scalar_to_string(Value::deserialize(deserializer)?).unwrap_or_default())
}

fn opt_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(scalar_to_string(Value::deserialize(deserializer)?).filter(|text| !text.is_empty()))
}

fn opt_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Bool(flag) => Some(flag),
        Value::String(text) if text.eq_ignore_ascii_case("true") => Some(true),
        Value::String(text) if text.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    })
}

fn opt_u32<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => number.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    })
}

/// Accepts a list of strings, a single string, or a list containing objects such as
/// `{"step": "..."}`; empty entries are dropped.
fn string_list<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let items = match Value::deserialize(deserializer)? {
        Value::Array(items) => items,
        Value::Null => Vec::new(),
        single => vec![single],
    };
    Ok(items
        .into_iter()
        .filter_map(|item| match item {
            Value::Object(map) => {
                let text = map
                    .values()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" - ");
                Some(text)
            }
            other => scalar_to_string(other),
        })
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect())
}
