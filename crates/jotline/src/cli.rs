use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum Provider {
    /// Ollama's native /api/chat endpoint.
    Ollama,
    /// Any OpenAI-compatible /v1/chat/completions server.
    Openai,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::Openai => "openai",
        }
    }

    pub fn default_endpoint(self) -> &'static str {
        match self {
            Provider::Ollama => "http://127.0.0.1:11434/api/chat",
            Provider::Openai => "http://127.0.0.1:8000/v1/chat/completions",
        }
    }
}

/// jotline: a small, local-first coding agent for open models.
#[derive(Parser, Debug)]
#[command(name = "jotline", version, about, long_about = None)]
pub struct Cli {
    /// Model backend.
    #[arg(long, value_enum, default_value = "ollama", env = "JOTLINE_PROVIDER")]
    pub provider: Provider,

    /// Model used for every role unless overridden below.
    #[arg(long, default_value = "qwen3:8b", env = "JOTLINE_MODEL")]
    pub model: String,

    /// Alias for --model (kept for compatibility).
    #[arg(long, hide = true)]
    pub text_model: Option<String>,

    /// Model for the first (planning) call of each task.
    #[arg(long, env = "JOTLINE_PLANNER_MODEL")]
    pub planner_model: Option<String>,

    /// Model for tool use, editing, and the final handoff.
    #[arg(long, env = "JOTLINE_CODER_MODEL")]
    pub coder_model: Option<String>,

    /// Chat endpoint URL; defaults to the provider's local default.
    #[arg(long, env = "JOTLINE_ENDPOINT")]
    pub endpoint: Option<String>,

    /// Bearer token for OpenAI-compatible servers.
    #[arg(long, env = "OPENAI_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    /// Workspace root; tools cannot read or write outside it.
    #[arg(long, short = 'w', default_value = ".")]
    pub workspace: PathBuf,

    /// Maximum model calls per task.
    #[arg(long, default_value_t = 24)]
    pub max_steps: u32,

    /// Most recent messages sent per model call (older history stays in the session).
    #[arg(long, default_value_t = 80)]
    pub max_history: usize,

    /// Timeout in seconds for one shell, git, or plugin process.
    #[arg(long, default_value_t = 120)]
    pub shell_timeout: u64,

    /// Timeout in seconds for one model request.
    #[arg(long, default_value_t = 600)]
    pub request_timeout: u64,

    #[arg(long, default_value_t = 0.1)]
    pub temperature: f32,

    /// Ask before shell commands, plugins, and commits (default: run without asking).
    #[arg(long)]
    pub ask_permissions: bool,

    /// Force auto mode on even if --ask-permissions is set.
    #[arg(long, alias = "auto-approve", hide = true)]
    pub auto_mode: bool,

    /// Maximum nesting of delegated sub-agents; 0 disables delegation.
    #[arg(long, default_value_t = 2)]
    pub max_delegation_depth: u32,

    /// Memory file (default: .jotline/memory.json in the workspace).
    #[arg(long)]
    pub memory_file: Option<PathBuf>,

    /// Plugin directory (default: .jotline/plugins in the workspace).
    #[arg(long)]
    pub plugins_dir: Option<PathBuf>,

    /// Session file to load at start and save after every turn.
    #[arg(long)]
    pub session_file: Option<PathBuf>,

    /// Ignore an existing --session-file and start fresh (it is still saved).
    #[arg(long)]
    pub fresh_session: bool,

    /// Do not load the built-in coding rules.
    #[arg(long)]
    pub no_builtin_rules: bool,

    /// Do not load JOTLINE.md, AGENTS.md, or .jotline/rules/*.md.
    #[arg(long)]
    pub no_project_rules: bool,

    /// Run one prompt non-interactively and exit.
    #[arg(long, short = 'p')]
    pub prompt: Option<String>,

    /// Hide tool and progress events.
    #[arg(long, short = 'q')]
    pub quiet: bool,

    /// Speak the JSON-lines protocol on stdin/stdout for a front end such as jotline-tui
    /// (see docs/PROTOCOL.md) instead of starting the interactive terminal.
    #[arg(long, conflicts_with = "prompt")]
    pub stdio: bool,

    /// Alias for --quiet.
    #[arg(long, hide = true)]
    pub no_tool_logs: bool,
}

impl Cli {
    pub fn base_model(&self) -> String {
        self.text_model
            .clone()
            .unwrap_or_else(|| self.model.clone())
    }

    pub fn auto_mode(&self) -> bool {
        self.auto_mode || !self.ask_permissions
    }

    pub fn quiet(&self) -> bool {
        self.quiet || self.no_tool_logs
    }
}
