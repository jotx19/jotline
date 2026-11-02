#![allow(dead_code)]

use anyhow::{anyhow, Result};
use jotline_core::config::ModelRouting;
use jotline_core::llm::{ChatMessage, LlmClient};
use jotline_core::orchestrator::{Orchestrator, OrchestratorConfig};
use jotline_core::tools::{ToolConfig, ToolExecutor};
use std::collections::VecDeque;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// Replays scripted replies and records every request it receives.
#[derive(Clone, Default)]
pub struct StubLlm {
    replies: Arc<Mutex<VecDeque<String>>>,
    calls: Arc<Mutex<Vec<Vec<ChatMessage>>>>,
    error: Option<String>,
}

impl StubLlm {
    pub fn new<const N: usize>(replies: [&str; N]) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.iter().map(|r| r.to_string()).collect())),
            ..Self::default()
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            error: Some(message.to_string()),
            ..Self::default()
        }
    }

    /// Every request sent so far, oldest first.
    pub fn calls(&self) -> Vec<Vec<ChatMessage>> {
        self.calls.lock().unwrap().clone()
    }
}

impl LlmClient for StubLlm {
    fn chat(&self, _model: &str, messages: &[ChatMessage], _temperature: f32) -> Result<String> {
        self.calls.lock().unwrap().push(messages.to_vec());
        if let Some(error) = &self.error {
            return Err(anyhow!(error.clone()));
        }
        Ok(self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| r#"{"type":"final","message":"stub exhausted"}"#.to_string()))
    }
}

pub fn build_orchestrator(tmp: &TempDir, llm: StubLlm) -> Orchestrator {
    build_orchestrator_with(tmp, llm, |_| {})
}

pub fn build_orchestrator_with(
    tmp: &TempDir,
    llm: StubLlm,
    configure: impl FnOnce(&mut OrchestratorConfig),
) -> Orchestrator {
    let tools = ToolExecutor::new(ToolConfig::for_workspace(tmp.path()));
    let mut config = OrchestratorConfig::new(ModelRouting::single("qwen3:8b"));
    config.max_steps = 6;
    configure(&mut config);
    Orchestrator::new(Box::new(llm), tools, config)
}

pub fn run_checked(args: &[&str], cwd: &Path) {
    let output = Command::new(args[0])
        .args(&args[1..])
        .current_dir(cwd)
        .output()
        .expect("failed to run command");
    assert!(
        output.status.success(),
        "command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn init_repo(dir: &Path) {
    run_checked(&["git", "init", "-q", "-b", "main"], dir);
    run_checked(&["git", "config", "user.email", "test@example.com"], dir);
    run_checked(&["git", "config", "user.name", "Test User"], dir);
    run_checked(&["git", "config", "commit.gpgsign", "false"], dir);
}
