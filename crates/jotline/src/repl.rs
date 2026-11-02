//! The interactive loop and its slash commands.

use crate::ui::{self, Tone};
use crate::Runtime;
use anyhow::{anyhow, Context, Result};
use jotline_core::intent;
use jotline_core::orchestrator::Orchestrator;
use jotline_core::rules::{self, RuleOrigin};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Eq, PartialEq)]
enum Control {
    Continue,
    Exit,
}

pub fn run(orchestrator: &mut Orchestrator, runtime: &Runtime) -> Result<()> {
    ui::print_welcome(orchestrator, runtime);

    loop {
        print!(
            "{}",
            ui::prompt(
                orchestrator.active_agent(),
                orchestrator.workspace_root(),
                &orchestrator.routing().text_model,
                runtime.auto_mode
            )
        );
        ui::flush_stdout();

        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            println!();
            return Ok(());
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        if input.starts_with('/') {
            match handle_command(input, orchestrator, runtime) {
                Ok(Control::Exit) => return Ok(()),
                Ok(Control::Continue) => {}
                Err(err) => ui::print_warning(&format!("{err:#}")),
            }
        } else {
            let response = orchestrator.run_active(input);
            render_response(orchestrator, &response);
        }
        autosave(orchestrator, runtime);
    }
}

fn handle_command(
    input: &str,
    orchestrator: &mut Orchestrator,
    runtime: &Runtime,
) -> Result<Control> {
    let (command, rest) = input.split_once(' ').unwrap_or((input, ""));
    let rest = rest.trim();

    match command {
        "/help" | "/?" => ui::print_help(),
        "/exit" | "/quit" => return Ok(Control::Exit),
        "/status" => ui::print_status(orchestrator, runtime),
        "/plan" => match orchestrator
            .last_run_summary()
            .and_then(|s| s.plan.as_ref())
        {
            Some(plan) => ui::print_plan("last task", plan),
            None => ui::print_warning("no plan recorded yet"),
        },
        "/intent" => {
            if rest.is_empty() {
                ui::print_warning("usage: /intent <request>");
            } else {
                ui::print_intent_scores(rest, intent::classify(rest), &intent::scores(rest));
            }
        }
        "/rules" => handle_rules(rest, orchestrator)?,
        "/scope" => handle_scope(rest, orchestrator, runtime)?,
        "/models" | "/routing" => {
            let routing = serde_json::to_string_pretty(&orchestrator.routing().as_json())?;
            ui::print_panel("Routing", Tone::Info, &[routing]);
        }
        "/tools" => ui::print_tools(&orchestrator.tools().list_specs()),
        "/reload_plugins" | "/plugins" => {
            let result = orchestrator.reload_plugins();
            ui::print_notice(&result);
        }
        "/agents" => ui::print_agents(orchestrator.active_agent(), &orchestrator.list_agents()),
        "/agent" => handle_agent(rest, orchestrator)?,
        "/reset" => {
            orchestrator.reset_active()?;
            ui::print_notice(&format!("reset agent '{}'", orchestrator.active_agent()));
        }
        "/save" => {
            let target = path_or_session(rest, runtime)?;
            let saved = orchestrator.save_session(target)?;
            ui::print_notice(&format!("session saved to {}", saved.display()));
        }
        "/load" => {
            let target = path_or_session(rest, runtime)?;
            let loaded = orchestrator.load_session(target)?;
            ui::print_notice(&format!("session loaded from {}", loaded.display()));
        }
        other => ui::print_warning(&format!("unknown command {other}; type /help")),
    }
    Ok(Control::Continue)
}

fn handle_rules(rest: &str, orchestrator: &mut Orchestrator) -> Result<()> {
    let (sub, arg) = rest.split_once(' ').unwrap_or((rest, ""));
    match sub {
        "" | "list" => {
            let lines = orchestrator
                .rules()
                .sources()
                .iter()
                .map(|source| {
                    let origin = match source.origin {
                        RuleOrigin::Builtin => "built-in",
                        RuleOrigin::Project => "project ",
                    };
                    format!(
                        "{origin}  {}  ({} chars{})",
                        source.name,
                        source.content.chars().count(),
                        if source.truncated { ", truncated" } else { "" }
                    )
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                ui::print_warning("no rules loaded");
            } else {
                ui::print_panel("Rules", Tone::Info, &lines);
            }
        }
        "add" => {
            let path = rules::add_user_rule(orchestrator.tools().rules_dir(), arg)?;
            orchestrator.reload_rules();
            ui::print_notice(&format!("rule added to {}", path.display()));
        }
        "show" => {
            let wanted = arg.trim();
            let source = orchestrator
                .rules()
                .sources()
                .iter()
                .find(|source| source.name == wanted)
                .ok_or_else(|| anyhow!("no rule source named '{wanted}'; see /rules"))?;
            ui::print_panel(
                &source.name,
                Tone::Info,
                std::slice::from_ref(&source.content),
            );
        }
        "reload" => {
            orchestrator.reload_rules();
            ui::print_notice(&format!(
                "reloaded {} rule sources",
                orchestrator.rules().sources().len()
            ));
        }
        _ => ui::print_warning("usage: /rules [list | add <rule> | show <name> | reload]"),
    }
    Ok(())
}

fn handle_scope(rest: &str, orchestrator: &mut Orchestrator, runtime: &Runtime) -> Result<()> {
    if rest.is_empty() {
        ui::print_scope(orchestrator.workspace_root());
        return Ok(());
    }
    let Some(raw) = rest.strip_prefix("use ") else {
        ui::print_warning("usage: /scope or /scope use <path>");
        return Ok(());
    };
    let current = orchestrator.workspace_root().to_path_buf();
    let expanded = expand_home(raw.trim());
    let target = if expanded.is_absolute() {
        expanded
    } else {
        current.join(expanded)
    };
    let target = target
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", target.display()))?;
    let paths = runtime.state_paths(&target);
    let root = orchestrator.set_workspace(&target, paths)?;
    ui::print_scope(&root);
    Ok(())
}

fn handle_agent(rest: &str, orchestrator: &mut Orchestrator) -> Result<()> {
    let (sub, arg) = rest.split_once(' ').unwrap_or((rest, ""));
    let arg = arg.trim();
    match sub {
        "" | "list" => ui::print_agents(orchestrator.active_agent(), &orchestrator.list_agents()),
        "new" => {
            orchestrator.create_agent(arg)?;
            ui::print_notice(&format!("created agent '{arg}'"));
        }
        "use" => {
            orchestrator.set_active_agent(arg)?;
            ui::print_notice(&format!("active agent: {}", orchestrator.active_agent()));
        }
        "reset" => {
            orchestrator.reset_agent(arg)?;
            ui::print_notice(&format!("reset agent '{arg}'"));
        }
        "run" => {
            let (name, prompt) = arg.split_once(' ').unwrap_or((arg, ""));
            if name.is_empty() || prompt.trim().is_empty() {
                ui::print_warning("usage: /agent run <name> <prompt>");
                return Ok(());
            }
            let response = orchestrator.run_with_agent(name, prompt.trim())?;
            render_response(orchestrator, &response);
        }
        _ => ui::print_warning(
            "usage: /agent [list | new <name> | use <name> | reset <name> | run <name> <prompt>]",
        ),
    }
    Ok(())
}

/// Prints the structured handoff when the last task produced one, otherwise the raw message.
pub fn render_response(orchestrator: &Orchestrator, response: &str) {
    match orchestrator.last_run_summary() {
        Some(summary) if summary.handoff.is_some() => ui::print_handoff(summary),
        _ => ui::print_assistant_message(orchestrator.active_agent(), response),
    }
}

fn autosave(orchestrator: &Orchestrator, runtime: &Runtime) {
    if let Some(path) = runtime.session_file.as_deref() {
        if let Err(err) = orchestrator.save_session(path) {
            ui::print_warning(&format!("failed to save session: {err:#}"));
        }
    }
}

fn path_or_session(rest: &str, runtime: &Runtime) -> Result<PathBuf> {
    if !rest.is_empty() {
        return Ok(expand_home(rest));
    }
    runtime
        .session_file
        .clone()
        .ok_or_else(|| anyhow!("no path given and no --session-file set"))
}

fn expand_home(raw: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match (raw, home) {
        ("~", Some(home)) => home,
        (path, Some(home)) if path.starts_with("~/") => home.join(&path[2..]),
        (path, _) => Path::new(path).to_path_buf(),
    }
}
