//! The `jotline` command: a terminal front end for `jotline-core`.

mod cli;
mod repl;
mod stdio;
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

    // In --stdio mode the reader thread must exist before the tools, because approvals and
    // interrupts arrive on stdin while a task runs.
    let emitter = Arc::new(stdio::Emitter::new());
    let mut stdio_inputs = cli.stdio.then(|| stdio::spawn_reader(Arc::clone(&emitter)));
    let approver: Option<Arc<dyn jotline_core::tools::Approver>> = match stdio_inputs.as_mut() {
        _ if runtime.auto_mode => None,
        Some(inputs) => Some(Arc::new(stdio::StdioApprover::new(
            Arc::clone(&emitter),
            inputs,
        ))),
        None => Some(Arc::new(ui::TerminalApprover)),
    };

    let paths = runtime.state_paths(&workspace);
    let tools = ToolExecutor::new(ToolConfig {
        workspace_root: workspace.clone(),
        shell_timeout_seconds: cli.shell_timeout.max(1),
        memory_file: paths.memory_file,
        plugins_dir: paths.plugins_dir,
        rules_dir: paths.rules_dir,
        approver,
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
    let reporter: Arc<dyn jotline_core::reporter::Reporter> = if cli.stdio {
        Arc::new(stdio::StdioReporter::new(Arc::clone(&emitter)))
    } else {
        Arc::new(ui::TerminalReporter::new(!cli.quiet()))
    };
    let mut orchestrator = Orchestrator::new(llm, tools, config)
        .with_reporter(reporter)
        .with_rule_options(RuleOptions {
            builtin: !cli.no_builtin_rules,
            project: !cli.no_project_rules,
        });

    if let Some(inputs) = stdio_inputs {
        orchestrator = orchestrator.with_cancel_flag(Arc::clone(&inputs.cancel));
        if let Some(path) = runtime.session_file.as_deref() {
            if path.exists() && !cli.fresh_session {
                if let Err(err) = orchestrator.load_session(path) {
                    emitter.emit(serde_json::json!({"event": "error", "message": format!("could not load session: {err:#}")}));
                }
            }
        }
        let info = stdio::SessionInfo {
            provider: runtime.provider.as_str(),
            auto_mode: runtime.auto_mode,
            session_file: runtime.session_file.as_deref(),
        };
        stdio::serve(&mut orchestrator, inputs, &emitter, &info)?;
        return Ok(ExitCode::SUCCESS);
    }

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
