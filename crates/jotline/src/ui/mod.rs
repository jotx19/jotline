//! Terminal rendering: panels, events, and status views.

mod term;
mod terminal;

pub use term::Tone;
pub use terminal::{TerminalApprover, TerminalReporter};

use crate::Runtime;
use jotline_core::intent::Intent;
use jotline_core::orchestrator::{Orchestrator, RunSummary, TaskPlan};
use jotline_core::tools::ToolSpec;
use std::io::{self, Write};
use std::path::Path;
use term::{clear_live_line, display_width, glyph, paint, panel_width, wrap_text};

pub fn flush_stdout() {
    let _ = io::stdout().flush();
}

pub fn print_panel(title: &str, tone: Tone, lines: &[String]) {
    clear_live_line();
    let inner = panel_width().saturating_sub(4);
    let title_text = format!(" {title} ");
    let fill = "─".repeat(
        (inner + 2)
            .saturating_sub(display_width(&title_text))
            .max(2),
    );
    println!("{}", paint(&format!("╭{title_text}{fill}╮"), tone, true));
    for line in lines.iter().flat_map(|line| wrap_text(line, inner)) {
        let pad = inner.saturating_sub(display_width(&line));
        println!(
            "{} {line}{} {}",
            paint("│", tone, true),
            " ".repeat(pad),
            paint("│", tone, true)
        );
    }
    println!(
        "{}",
        paint(&format!("╰{}╯", "─".repeat(inner + 2)), tone, true)
    );
}

pub fn print_event(label: &str, tone: Tone, message: &str) {
    clear_live_line();
    let prefix_plain = format!("{} {label}", glyph(tone));
    let prefix = format!(
        "{} {}",
        paint(glyph(tone), tone, true),
        paint(label, tone, true)
    );
    let width = panel_width()
        .saturating_sub(display_width(&prefix_plain) + 1)
        .max(24);
    for (index, line) in wrap_text(message, width).iter().enumerate() {
        if index == 0 {
            println!("{prefix} {}", paint(line, Tone::Muted, false));
        } else {
            println!(
                "{} {}",
                " ".repeat(display_width(&prefix_plain)),
                paint(line, Tone::Muted, false)
            );
        }
    }
}

pub fn print_notice(message: &str) {
    print_event("info", Tone::Info, message);
}

pub fn print_warning(message: &str) {
    print_event("warn", Tone::Warning, message);
}

pub fn print_error(message: &str) {
    print_event("error", Tone::Danger, message);
}

fn workspace_name(workspace: &Path) -> String {
    workspace
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("/")
        .to_string()
}

pub fn prompt(agent: &str, workspace: &Path, model: &str, auto_mode: bool) -> String {
    let label = format!(
        "✦ {agent}  {}  {}  {}",
        workspace_name(workspace),
        term::compact(model, 22),
        if auto_mode { "auto" } else { "ask" }
    );
    if term::supports_color() {
        format!("{} › ", paint(&label, Tone::Accent, true))
    } else {
        format!("{label} > ")
    }
}

pub fn print_welcome(orchestrator: &Orchestrator, runtime: &Runtime) {
    let routing = orchestrator.routing();
    let rules = orchestrator.rules().sources();
    let project_rules = rules
        .iter()
        .filter(|s| s.origin == jotline_core::rules::RuleOrigin::Project)
        .count();
    print_panel(
        &format!("jotline {}", env!("CARGO_PKG_VERSION")),
        Tone::Accent,
        &[
            format!("scope     {}", orchestrator.workspace_root().display()),
            format!(
                "models    {} via {} (planner {}, coder {})",
                routing.text_model,
                runtime.provider.as_str(),
                routing.model_for(jotline_core::config::ModelRole::Planner),
                routing.model_for(jotline_core::config::ModelRole::Coder),
            ),
            format!(
                "rules     {} built-in, {project_rules} project   (/rules to inspect, /rules add <rule>)",
                rules.len() - project_rules
            ),
            format!(
                "approvals {}   session {}",
                if runtime.auto_mode { "auto" } else { "ask" },
                runtime
                    .session_file
                    .as_deref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "off".to_string())
            ),
            String::new(),
            "Describe a task, or type /help for commands.".to_string(),
        ],
    );
}

pub fn print_help() {
    let commands = [
        ("/help", "show this help"),
        ("/status", "scope, models, rules, and last task"),
        ("/plan", "show the last plan"),
        (
            "/intent <request>",
            "show how a request would be classified",
        ),
        ("/rules", "list loaded rule files"),
        ("/rules add <rule>", "save a rule to .jotline/rules/user.md"),
        ("/rules show <name>", "print one rule source"),
        ("/rules reload", "re-read rule files from disk"),
        ("/tools", "list available tools"),
        (
            "/scope [use <path>]",
            "show or change the workspace boundary",
        ),
        ("/agents", "list agents"),
        ("/agent new|use|reset <name>", "manage named agents"),
        ("/agent run <name> <prompt>", "run a task on another agent"),
        ("/reset", "clear the active agent's conversation"),
        ("/save [path] /load [path]", "save or load the session"),
        ("/plugins", "reload plugin tools"),
        ("/models", "show model routing"),
        ("/exit", "quit (Ctrl-D also works)"),
    ];
    let lines = commands
        .iter()
        .map(|(command, help)| format!("{command:<30}{help}"))
        .collect::<Vec<_>>();
    print_panel("Commands", Tone::Info, &lines);
}

pub fn print_status(orchestrator: &Orchestrator, runtime: &Runtime) {
    let routing = orchestrator.routing();
    let mut lines = vec![
        format!("agent     {}", orchestrator.active_agent()),
        format!("scope     {}", orchestrator.workspace_root().display()),
        format!("provider  {}", runtime.provider.as_str()),
        format!("model     {}", routing.text_model),
        format!(
            "approvals {}",
            if runtime.auto_mode { "auto" } else { "ask" }
        ),
        format!("rules     {} sources", orchestrator.rules().sources().len()),
        format!(
            "plugins   {}",
            match orchestrator.tools().plugin_names() {
                names if names.is_empty() => "none".to_string(),
                names => names.join(", "),
            }
        ),
    ];
    if let Some(summary) = orchestrator.last_run_summary() {
        lines.push(String::new());
        lines.push(format!("last task {}", summary.user_prompt));
        if let Some(intent) = summary.intent {
            lines.push(format!("intent    {intent}"));
        }
        lines.push(format!(
            "steps     {}   files changed {}   verified {}",
            summary.steps_used,
            yes_no(summary.files_changed),
            yes_no(summary.verified)
        ));
        if let Some(handoff) = summary.handoff.as_ref() {
            lines.push(format!("handoff   {}", handoff.summary_or_message()));
        }
    }
    print_panel("Status", Tone::Info, &lines);
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

pub fn print_scope(workspace: &Path) {
    print_panel(
        "Scope",
        Tone::Accent,
        &[
            format!("current {}", workspace.display()),
            "Tools cannot read or write outside this directory.".to_string(),
            "Change it with /scope use <path> or --workspace <path>.".to_string(),
        ],
    );
}

pub fn print_plan(agent: &str, plan: &TaskPlan) {
    let mut lines = vec![format!(
        "{agent} · {}{}",
        if plan.needs_plan {
            "detailed plan"
        } else {
            "direct task"
        },
        plan.intent
            .map(|intent| format!(" · {intent}"))
            .unwrap_or_default()
    )];
    lines.push(plan.summary.clone());
    if !plan.reason.is_empty() {
        lines.push(format!("why: {}", plan.reason));
    }
    if !plan.files.is_empty() {
        lines.push(format!("files: {}", plan.files.join(", ")));
    }
    for (index, step) in plan.steps.iter().enumerate() {
        lines.push(format!("{}. {step}", index + 1));
    }
    print_panel("Plan", Tone::Accent, &lines);
}

pub fn print_intent_scores(request: &str, chosen: Intent, scores: &[(Intent, u32)]) {
    let mut lines = vec![format!("request  {request}"), format!("intent   {chosen}")];
    let mut ranked = scores
        .iter()
        .filter(|(_, score)| *score > 0)
        .collect::<Vec<_>>();
    ranked.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    if !ranked.is_empty() {
        lines.push(format!(
            "scores   {}",
            ranked
                .iter()
                .map(|(intent, score)| format!("{intent}={score}"))
                .collect::<Vec<_>>()
                .join("  ")
        ));
    }
    lines.push(String::new());
    lines.push("playbook".to_string());
    for (index, step) in chosen.playbook().iter().enumerate() {
        lines.push(format!("{}. {step}", index + 1));
    }
    print_panel("Intent", Tone::Info, &lines);
}

pub fn print_tools(specs: &[ToolSpec]) {
    let lines = specs
        .iter()
        .map(|spec| format!("{:<24}{}", spec.name, spec.description))
        .collect::<Vec<_>>();
    print_panel("Tools", Tone::Info, &lines);
}

pub fn print_agents(active: &str, names: &[String]) {
    let lines = names
        .iter()
        .map(|name| {
            if name == active {
                format!("• {name}  (active)")
            } else {
                format!("• {name}")
            }
        })
        .collect::<Vec<_>>();
    print_panel("Agents", Tone::Info, &lines);
}

pub fn print_handoff(summary: &RunSummary) {
    let Some(handoff) = summary.handoff.as_ref() else {
        return;
    };
    let mut lines = Vec::new();
    if !handoff.message.trim().is_empty() {
        lines.extend(handoff.message.lines().map(str::to_string));
    } else {
        lines.push(handoff.summary.clone());
    }

    let mut section = |title: &str, items: &[String]| {
        if !items.is_empty() {
            lines.push(String::new());
            lines.push(title.to_string());
            lines.extend(items.iter().map(|item| format!("  • {item}")));
        }
    };
    section("completed", &handoff.completed);
    section("verified", &handoff.verified);
    section("missed", &handoff.missed);
    section("next steps", &handoff.next_steps);

    if summary.files_changed && !summary.verified {
        lines.push(String::new());
        lines
            .push("⚠ files changed without a verification command after the last edit".to_string());
    }

    let title = format!(
        "Done · {}{} · {} steps",
        summary.agent_name,
        summary
            .intent
            .map(|intent| format!(" · {intent}"))
            .unwrap_or_default(),
        summary.steps_used
    );
    print_panel(&title, Tone::Success, &lines);
}

pub fn print_assistant_message(agent: &str, message: &str) {
    let lines = message.lines().map(str::to_string).collect::<Vec<_>>();
    print_panel(agent, Tone::Warning, &lines);
}
