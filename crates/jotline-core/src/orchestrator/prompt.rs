//! System prompt and feedback messages.

use super::TaskPlan;
use crate::intent::Intent;
use crate::rules::RuleSet;

pub(crate) struct PromptContext<'a> {
    pub workspace_root: &'a str,
    pub agent_name: &'a str,
    pub max_delegation_depth: u32,
    pub rules: &'a RuleSet,
    pub tool_specs: &'a str,
}

pub(crate) fn system_prompt(ctx: &PromptContext<'_>) -> String {
    let intents = Intent::ALL
        .iter()
        .map(|intent| intent.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let rules = ctx.rules.render_for_prompt();
    let rules_section = if rules.is_empty() {
        String::new()
    } else {
        format!("\n# Rules\n{rules}")
    };

    format!(
        r#"You are jotline, a careful senior software engineer working in a terminal. You act only through the JSON protocol below.

# Workspace
root: {root}
agent: {agent}
Stay inside this directory. Tool paths are relative to it. Do not assume parent or sibling directories matter unless the user asks.

# Protocol
Reply with exactly ONE JSON object and nothing else: no markdown fences, no prose before or after.
{{"type":"plan","intent":"<intent>","needs_plan":true,"summary":"...","reason":"...","steps":["..."],"files":["paths you expect to touch"]}}
{{"type":"tool","tool":"<tool name>","args":{{...}},"reason":"..."}}
{{"type":"parallel_tools","calls":[{{"tool":"<tool name>","args":{{...}}}}],"reason":"..."}}
{{"type":"delegate","prompt":"self-contained subtask","agent_name":"optional","max_steps":8,"reason":"..."}}
{{"type":"final","message":"answer for the user","summary":"one line","completed":["..."],"verified":["command -> result"],"missed":["..."],"next_steps":["..."]}}

# Workflow
1. Your first reply for every task is a plan. intent is one of: {intents}. Use needs_plan=true for multi-step or multi-file work; still give 1-3 concrete steps for simple tasks.
2. Investigate before editing: search_files and find_files to locate code, then read_file the relevant ranges. Batch independent reads with parallel_tools.
3. Change code with edit_file (existing files) or write_file (new files). Write complete code.
4. Verify with run_shell: build, tests, linter. Fix failures and re-run.
5. Finish with final. List only checks you actually ran in "verified"; put anything unfinished or unverified in "missed".
If a tool returns ERROR, read the message and adapt; never repeat an identical failing call. Delegate only self-contained subtasks (depth limit {depth}).
When the user asks you to always or never do something from now on, save it with rules_add.
{rules_section}
# Tools
(* marks a required argument)
{tools}"#,
        root = ctx.workspace_root,
        agent = ctx.agent_name,
        intents = intents,
        depth = ctx.max_delegation_depth,
        rules_section = rules_section,
        tools = ctx.tool_specs,
    )
}

/// The user turn for a new task: the request followed by the intent playbook.
pub(crate) fn task_brief(request: &str, intent: Intent) -> String {
    format!(
        "{request}\n\n---\nTask brief from jotline\nintent: {intent}{}\nplaybook:\n{}\nStart with a plan action.",
        if intent.is_read_only() {
            " (read-only: do not modify files)"
        } else {
            ""
        },
        numbered(intent.playbook()),
    )
}

pub(crate) fn plan_ack(plan: &TaskPlan, intent: Intent, intent_changed: bool) -> String {
    let mut text = format!(
        "Plan recorded (intent: {intent}).\n{}",
        plan.as_feedback_block()
    );
    if intent_changed {
        text.push_str(&format!(
            "\n\nPlaybook for {intent}:\n{}",
            numbered(intent.playbook())
        ));
    }
    text.push_str(
        "\n\nExecute the plan now. Reply with a tool, parallel_tools, delegate, or final action.",
    );
    text
}

pub(crate) fn tool_result(tool: &str, result: &str, steps_left: u32) -> String {
    format!(
        "Result of {tool}:\n{result}\n\n{}",
        continue_hint(steps_left)
    )
}

pub(crate) fn delegate_result(agent: &str, result: &str, steps_left: u32) -> String {
    format!(
        "Delegate '{agent}' finished:\n{result}\n\n{}",
        continue_hint(steps_left)
    )
}

fn continue_hint(steps_left: u32) -> String {
    if steps_left <= 2 {
        format!(
            "Only {steps_left} step(s) left. Finish now with a final action that reports what is done and what is missed."
        )
    } else {
        "Continue with the next action as one JSON object.".to_string()
    }
}

pub(crate) const PLAN_REQUIRED: &str =
    "Start with a plan: reply with {\"type\":\"plan\", \"intent\":..., \"summary\":..., \"steps\":[...]} before using tools, delegating, or finishing.";

pub(crate) const VALID_EXAMPLES: &str = r#"Copy one of these shapes exactly, replacing the values:
{"type":"tool","tool":"read_file","args":{"path":"calc.py"}}
{"type":"tool","tool":"edit_file","args":{"path":"calc.py","old":"exact old text","new":"new text"}}
{"type":"tool","tool":"run_shell","args":{"command":"python3 -m pytest -q"}}
{"type":"final","message":"what you did","verified":["command -> result"]}"#;

pub(crate) const REPEATED_FAILURE: &str = "You already made this exact call and it failed. Do not repeat it. Use the error message above (it may show the current file text) or read_file, then change the arguments.";

pub(crate) const FINAL_EMPTY: &str =
    "A final action needs a non-empty \"message\" or \"summary\" for the user.";

pub(crate) const VERIFY_REQUIRED: &str = "You changed files but have not run any verification since the last change. Run the project's build, tests, or linter with run_shell and fix any failures. If verification is impossible here, say why in final.missed. Then send final again.";

pub(crate) const STEP_LIMIT: &str = "Stopped: reached the step limit before the task finished. Retry with a narrower request or a higher --max-steps.";

fn numbered(lines: &[&str]) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{}. {line}", index + 1))
        .collect::<Vec<_>>()
        .join("\n")
}
