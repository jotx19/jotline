use crate::STATE_DIR;
use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// The job a model call performs. Each role can be routed to a different model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelRole {
    /// The first call of a top-level task, which must produce the plan.
    Planner,
    /// Every later call: tool use, editing, review, and the final handoff.
    Coder,
}

impl fmt::Display for ModelRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Planner => f.write_str("planner"),
            Self::Coder => f.write_str("coder"),
        }
    }
}

/// Model names per role. Unset roles fall back to `text_model`.
#[derive(Clone, Debug, Serialize)]
pub struct ModelRouting {
    pub text_model: String,
    pub planner_model: Option<String>,
    pub coder_model: Option<String>,
}

impl ModelRouting {
    /// Routing that sends every role to one model.
    pub fn single(model: impl Into<String>) -> Self {
        Self {
            text_model: model.into(),
            planner_model: None,
            coder_model: None,
        }
    }

    /// The model name to call for `role`, after applying the `text_model` fallback.
    pub fn model_for(&self, role: ModelRole) -> &str {
        let routed = match role {
            ModelRole::Planner => self.planner_model.as_deref(),
            ModelRole::Coder => self.coder_model.as_deref(),
        };
        routed.unwrap_or(self.text_model.as_str())
    }

    pub fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "text_model": self.text_model,
            "planner_model": self.model_for(ModelRole::Planner),
            "coder_model": self.model_for(ModelRole::Coder),
        })
    }
}

/// Default locations of jotline state inside a workspace.
#[derive(Clone, Debug)]
pub struct StatePaths {
    pub memory_file: PathBuf,
    pub plugins_dir: PathBuf,
    pub rules_dir: PathBuf,
    pub session_file: PathBuf,
}

impl StatePaths {
    /// State paths under `<workspace>/.jotline/`.
    pub fn for_workspace(workspace: &Path) -> Self {
        let base = workspace.join(STATE_DIR);
        Self {
            memory_file: base.join("memory.json"),
            plugins_dir: base.join("plugins"),
            rules_dir: base.join("rules"),
            session_file: base.join("session.json"),
        }
    }
}

/// Resolves `value` against `base` when it is relative; returns `fallback` when it is absent.
pub fn resolve_against(base: &Path, value: Option<&Path>, fallback: PathBuf) -> PathBuf {
    match value {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => base.join(path),
        None => fallback,
    }
}
