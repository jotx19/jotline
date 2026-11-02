//! The `jotline` command: a terminal front end for `jotline-core`.

mod cli;
mod repl;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Provider};
use jotline_core::config::{resolve_against, ModelRouting, StatePaths};
use jotline_core::llm::{LlmClient, OllamaClient, OpenAiCompatibleClient};
use jotline_core::orchestrator::{Orchestrator, OrchestratorConfig};
use jotline_core::rules::RuleOptions;
use jotline_core::tools::{ToolConfig, ToolExecutor};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

/// Settings the REPL needs after startup.
pub struct Runtime {
    pub provider: Provider,
    pub auto_mode: bool,
    pub session_file: Option<PathBuf>,
    pub memory_file: Option<PathBuf>,
    pub plugins_dir: Option<PathBuf>,
}

impl Runtime {
    /// State paths for `workspace`, honoring --memory-file and --plugins-dir.
    pub fn state_paths(&self, workspace: &Path) -> StatePaths {
        let defaults = StatePaths::for_workspace(workspace);
        StatePaths {
            memory_file: resolve_against(
                workspace,
                self.memory_file.as_deref(),
                defaults.memory_file,
            ),
            plugins_dir: resolve_against(
                workspace,
                self.plugins_dir.as_deref(),
                defaults.plugins_dir,
            ),
            ..defaults
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            ui::print_error(&format!("{err:#}"));
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    let cwd = std::env::current_dir().context("failed to read the current directory")?;
    let workspace = resolve_against(&cwd, Some(&cli.workspace), cwd.clone())
        .canonicalize()
        .with_context(|| format!("workspace not found: {}", cli.workspace.display()))?;
    anyhow::ensure!(
        workspace.is_dir(),
        "workspace is not a directory: {}",
        workspace.display()
    );

    let runtime = Runtime {
        provider: cli.provider,
        auto_mode: cli.auto_mode(),
        session_file: cli
            .session_file
            .as_deref()
            .map(|path| resolve_against(&workspace, Some(path), path.to_path_buf())),
        memory_file: cli.memory_file.clone(),
        plugins_dir: cli.plugins_dir.clone(),
    };

    let paths = runtime.state_paths(&workspace);
    let tools = ToolExecutor::new(ToolConfig {
        workspace_root: workspace.clone(),
        shell_timeout_seconds: cli.shell_timeout.max(1),
        memory_file: paths.memory_file,
        plugins_dir: paths.plugins_dir,
        rules_dir: paths.rules_dir,
        approver: if runtime.auto_mode {
            None
        } else {
            Some(Arc::new(ui::TerminalApprover))
        },
    });

    let routing = ModelRouting {
        text_model: cli.base_model(),
        planner_model: cli.planner_model.clone(),
        coder_model: cli.coder_model.clone(),
    };
    let config = OrchestratorConfig {
        max_steps: cli.max_steps.max(1),
        temperature: cli.temperature,
        max_delegation_depth: cli.max_delegation_depth,
        routing,
        max_history_messages: cli.max_history.max(4),
    };

    let llm = build_llm(&cli)?;
    let mut orchestrator = Orchestrator::new(llm, tools, config)
        .with_reporter(Arc::new(ui::TerminalReporter::new(!cli.quiet())))
        .with_rule_options(RuleOptions {
            builtin: !cli.no_builtin_rules,
            project: !cli.no_project_rules,
        });

    if let Some(path) = runtime.session_file.as_deref() {
        if path.exists() && cli.fresh_session {
            ui::print_notice(&format!(
                "starting fresh; {} will be overwritten",
                path.display()
            ));
        } else if path.exists() {
            match orchestrator.load_session(path) {
                Ok(loaded) => ui::print_notice(&format!("loaded session {}", loaded.display())),
                Err(err) => ui::print_warning(&format!("could not load session: {err:#}")),
            }
        }
    }

    if let Some(prompt) = cli.prompt.as_deref() {
        let response = orchestrator.run_active(prompt);
        repl::render_response(&orchestrator, &response);
        if let Some(path) = runtime.session_file.as_deref() {
            orchestrator.save_session(path)?;
        }
        let failed = orchestrator
            .last_run_summary()
            .is_none_or(|summary| summary.handoff.is_none());
        return Ok(if failed {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        });
    }

    repl::run(&mut orchestrator, &runtime)?;
    Ok(ExitCode::SUCCESS)
}

fn build_llm(cli: &Cli) -> Result<Box<dyn LlmClient>> {
    let endpoint = cli
        .endpoint
        .clone()
        .unwrap_or_else(|| cli.provider.default_endpoint().to_string());
    Ok(match cli.provider {
        Provider::Ollama => Box::new(OllamaClient::new(endpoint, cli.request_timeout)?),
        Provider::Openai => Box::new(OpenAiCompatibleClient::new(
            endpoint,
            cli.api_key.clone(),
            cli.request_timeout,
        )?),
    })
}
