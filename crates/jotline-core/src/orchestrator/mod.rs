//! The orchestration loop ("operator") that turns model replies into tool calls.
//!
//! For each task the operator:
//!
//! 1. classifies the request into an [`Intent`] and attaches its playbook,
//! 2. requires a `plan` action before anything else,
//! 3. executes `tool`, `parallel_tools`, and `delegate` actions, feeding results back,
//! 4. applies the **verification gate**: for intents that change code, a `final` action sent
//!    after file changes with no command run since is answered once with a request to verify,
//! 5. returns the structured handoff from the `final` action.
//!
//! Invalid replies are answered with a specific correction instead of failing the task.

pub mod action;
mod prompt;
pub mod session;

#[cfg(test)]
mod tests;

pub use action::{parse_action, parse_action_with_tools, Action, ParseError, ToolShape};
pub use session::{is_valid_agent_name, DelegateRecord, ToolRecord};

use crate::config::{ModelRole, ModelRouting, StatePaths};
use crate::intent::{self, Intent};
use crate::llm::{ChatMessage, LlmClient};
use crate::reporter::{EventKind, NullReporter, Reporter};
use crate::rules::{RuleOptions, RuleSet};
use crate::tools::{ToolExecutor, ToolOutput, ToolStatus};
use action::{DelegateAction, FinalAction, ParallelAction, PlanAction, ToolAction};
use anyhow::{anyhow, Context, Result};
use serde_json::json;
use session::{AgentRecordOut, AgentSession, SessionFileIn, SessionFileOut, SESSION_VERSION};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The plan a model recorded for a task.
#[derive(Clone, Debug, Default)]
pub struct TaskPlan {
    pub intent: Option<Intent>,
    pub needs_plan: bool,
    pub summary: String,
    pub reason: String,
    pub steps: Vec<String>,
    pub files: Vec<String>,
    pub focus: Vec<String>,
}

impl TaskPlan {
    fn from_action(action: PlanAction) -> Self {
        let mut steps = action.steps;
        if steps.is_empty() && !action.reason.is_empty() {
            steps.push(action.reason.clone());
        }
        Self {
            intent: action.intent.as_deref().and_then(Intent::parse),
            needs_plan: action.needs_plan.unwrap_or(steps.len() > 1),
            summary: if action.summary.is_empty() {
                "Work the task".to_string()
            } else {
                action.summary
            },
            reason: action.reason,
            steps,
            files: action.files,
            focus: action.focus,
        }
    }

    fn as_feedback_block(&self) -> String {
        let mut lines = vec![
            format!("needs_plan: {}", self.needs_plan),
            format!("summary: {}", self.summary),
        ];
        if !self.reason.is_empty() {
            lines.push(format!("reason: {}", self.reason));
        }
        if !self.files.is_empty() {
            lines.push(format!("files: {}", self.files.join(", ")));
        }
        if !self.focus.is_empty() {
            lines.push(format!("focus: {}", self.focus.join(" | ")));
        }
        lines.push("steps:".to_string());
        for (index, step) in self.steps.iter().enumerate() {
            lines.push(format!("{}. {step}", index + 1));
        }
        lines.join("\n")
    }
}

/// The structured result of a `final` action.
#[derive(Clone, Debug, Default)]
pub struct TaskHandoff {
    pub message: String,
    pub summary: String,
    pub completed: Vec<String>,
    pub verified: Vec<String>,
    pub missed: Vec<String>,
    pub next_steps: Vec<String>,
}

impl TaskHandoff {
    fn from_action(action: FinalAction) -> Self {
        Self {
            message: action.message,
            summary: action.summary,
            completed: action.completed,
            verified: action.verified,
            missed: action.missed,
            next_steps: action.next_steps,
        }
    }

    /// The summary when present, otherwise the message.
    pub fn summary_or_message(&self) -> String {
        let summary = self.summary.trim();
        if summary.is_empty() {
            self.message.trim().to_string()
        } else {
            summary.to_string()
        }
    }

    /// The text returned to callers: the message when present, otherwise the summary.
    pub fn message_or_summary(&self) -> String {
        let message = self.message.trim();
        if message.is_empty() {
            self.summary.trim().to_string()
        } else {
            message.to_string()
        }
    }
}

/// What happened in the most recent top-level task.
#[derive(Clone, Debug, Default)]
pub struct RunSummary {
    pub agent_name: String,
    pub user_prompt: String,
    pub intent: Option<Intent>,
    pub plan: Option<TaskPlan>,
    pub handoff: Option<TaskHandoff>,
    /// Model calls used by the top-level agent.
    pub steps_used: u32,
    pub files_changed: bool,
    /// True when a command ran after the last file change.
    pub verified: bool,
}

/// A read-only copy of one agent's state.
#[derive(Clone, Debug)]
pub struct AgentSnapshot {
    pub name: String,
    pub messages: Vec<ChatMessage>,
    pub tool_history: Vec<ToolRecord>,
    pub delegate_history: Vec<DelegateRecord>,
}

/// Loop limits and model settings.
#[derive(Clone, Debug)]
pub struct OrchestratorConfig {
    /// Maximum model calls for one top-level task.
    pub max_steps: u32,
    pub temperature: f32,
    /// How many levels of `delegate` may nest; 0 disables delegation.
    pub max_delegation_depth: u32,
    pub routing: ModelRouting,
    /// Most recent non-system messages sent per model call; older ones stay in the session
    /// but are not sent. The current task's request is always kept.
    pub max_history_messages: usize,
}

impl OrchestratorConfig {
    pub fn new(routing: ModelRouting) -> Self {
        Self {
            max_steps: 24,
            temperature: 0.1,
            max_delegation_depth: 2,
            routing,
            max_history_messages: 80,
        }
    }
}

/// Outcome of one agent run, used to merge delegate effects into the parent.
struct RunOutcome {
    message: String,
    wrote_files: bool,
    verified: bool,
}

/// Mutable state of one agent's task.
struct RunState {
    intent: Intent,
    has_plan: bool,
    files_changed: bool,
    verified: bool,
    verify_reminded: bool,
    request_index: usize,
    /// Consecutive replies that failed to parse; from 2 on, corrections include examples.
    invalid_streak: u32,
    /// `tool args` signatures of calls that failed, to detect identical retries.
    failed_calls: std::collections::HashSet<String>,
}

impl RunState {
    fn record(&mut self, wrote_files: bool, executed: bool) {
        if wrote_files {
            self.files_changed = true;
            self.verified = false;
        }
        if executed && self.files_changed {
            self.verified = true;
        }
    }
}

pub struct Orchestrator {
    llm: Box<dyn LlmClient>,
    tools: ToolExecutor,
    config: OrchestratorConfig,
    reporter: Arc<dyn Reporter>,
    rule_options: RuleOptions,
    rules: RuleSet,
    agents: BTreeMap<String, AgentSession>,
    active_agent: String,
    delegate_counter: u64,
    last_run_summary: Option<RunSummary>,
}

impl Orchestrator {
    /// Creates an orchestrator with a `main` agent, loading built-in and project rules for the
    /// executor's workspace.
    pub fn new(llm: Box<dyn LlmClient>, tools: ToolExecutor, config: OrchestratorConfig) -> Self {
        let rule_options = RuleOptions::default();
        let rules = RuleSet::load(tools.workspace_root(), tools.rules_dir(), rule_options);
        let mut this = Self {
            llm,
            tools,
            config: OrchestratorConfig {
                max_steps: config.max_steps.max(1),
                ..config
            },
            reporter: Arc::new(NullReporter),
            rule_options,
            rules,
            agents: BTreeMap::new(),
            active_agent: "main".to_string(),
            delegate_counter: 0,
            last_run_summary: None,
        };
        this.ensure_agent("main");
        this
    }

    pub fn with_reporter(mut self, reporter: Arc<dyn Reporter>) -> Self {
        self.reporter = reporter;
        self
    }

    /// Selects which rule layers load, reloading rules and system prompts.
    pub fn with_rule_options(mut self, options: RuleOptions) -> Self {
        self.rule_options = options;
        self.reload_rules();
        self
    }

    pub fn active_agent(&self) -> &str {
        &self.active_agent
    }

    pub fn routing(&self) -> &ModelRouting {
        &self.config.routing
    }

    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    pub fn workspace_root(&self) -> &Path {
        self.tools.workspace_root()
    }

    pub fn tools(&self) -> &ToolExecutor {
        &self.tools
    }

    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    pub fn list_agents(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    pub fn has_agent(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }

    pub fn create_agent(&mut self, name: &str) -> Result<()> {
        let name = normalize_agent_name(name)?;
        self.ensure_agent(&name);
        Ok(())
    }

    pub fn set_active_agent(&mut self, name: &str) -> Result<()> {
        let name = normalize_agent_name(name)?;
        self.ensure_agent(&name);
        self.active_agent = name;
        Ok(())
    }

    pub fn reset_active(&mut self) -> Result<()> {
        let active = self.active_agent.clone();
        self.reset_agent(&active)
    }

    /// Clears an agent's conversation and history, creating the agent if needed.
    pub fn reset_agent(&mut self, name: &str) -> Result<()> {
        let name = normalize_agent_name(name)?;
        let fresh = self.new_session(&name);
        self.agents.insert(name, fresh);
        Ok(())
    }

    /// Runs a task on the active agent and returns the text for the user.
    pub fn run_active(&mut self, user_prompt: &str) -> String {
        let active = self.active_agent.clone();
        self.run_top_level(&active, user_prompt)
    }

    /// Runs a task on a named agent (created if needed) without changing the active agent.
    pub fn run_with_agent(&mut self, name: &str, user_prompt: &str) -> Result<String> {
        let name = normalize_agent_name(name)?;
        self.ensure_agent(&name);
        Ok(self.run_top_level(&name, user_prompt))
    }

    pub fn agent_snapshot(&self, name: &str) -> Option<AgentSnapshot> {
        self.agents.get(name).map(|session| AgentSnapshot {
            name: name.to_string(),
            messages: session.messages.clone(),
            tool_history: session.tool_history.clone(),
            delegate_history: session.delegate_history.clone(),
        })
    }

    pub fn last_run_summary(&self) -> Option<&RunSummary> {
        self.last_run_summary.as_ref()
    }

    /// Reloads plugins and refreshes system prompts; returns the tool's report.
    pub fn reload_plugins(&mut self) -> String {
        let output = self.tools.execute("reload_plugins", &json!({}));
        self.refresh_system_prompts();
        output.text
    }

    /// Re-reads rule files from disk and refreshes every agent's system prompt.
    pub fn reload_rules(&mut self) {
        self.rules = RuleSet::load(
            self.tools.workspace_root(),
            self.tools.rules_dir(),
            self.rule_options,
        );
        self.refresh_system_prompts();
    }

    /// Moves to another workspace, reloading plugins and rules.
    pub fn set_workspace(&mut self, root: &Path, paths: StatePaths) -> Result<PathBuf> {
        let root = self
            .tools
            .set_workspace(root, paths)
            .map_err(|err| anyhow!(err))?;
        self.reload_rules();
        Ok(root)
    }

    pub fn save_session<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let target = path.as_ref().to_path_buf();
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed creating directory {}", parent.display()))?;
        }
        let file = SessionFileOut {
            version: SESSION_VERSION,
            active_agent: &self.active_agent,
            delegate_counter: self.delegate_counter,
            agents: self
                .agents
                .iter()
                .map(|(name, session)| {
                    (
                        name.as_str(),
                        AgentRecordOut {
                            messages: session.messages.iter().skip(1).collect(),
                            tool_history: &session.tool_history,
                            delegate_history: &session.delegate_history,
                        },
                    )
                })
                .collect(),
        };
        let text = serde_json::to_string_pretty(&file).context("failed serializing session")?;
        fs::write(&target, text)
            .with_context(|| format!("failed writing session {}", target.display()))?;
        Ok(target)
    }

    /// Replaces all agents with those in the session file. Invalid agent names and malformed
    /// entries are skipped; a file with no agents yields a fresh `main` agent.
    pub fn load_session<P: AsRef<Path>>(&mut self, path: P) -> Result<PathBuf> {
        let source = path.as_ref().to_path_buf();
        let text = fs::read_to_string(&source)
            .with_context(|| format!("failed reading session {}", source.display()))?;
        let mut file: SessionFileIn = serde_json::from_str(&text)
            .with_context(|| format!("invalid session file {}", source.display()))?;

        let mut agents = BTreeMap::new();
        for (name, record) in file.take_agents() {
            if !is_valid_agent_name(&name) {
                continue;
            }
            let (messages, tool_history, delegate_history) = record.into_parts();
            let mut session = self.new_session(&name);
            session.messages.extend(messages);
            session.tool_history = tool_history;
            session.delegate_history = delegate_history;
            agents.insert(name, session);
        }
        if agents.is_empty() {
            agents.insert("main".to_string(), self.new_session("main"));
        }
        self.agents = agents;

        self.active_agent = file
            .active_agent
            .filter(|name| self.agents.contains_key(name))
            .or_else(|| self.agents.contains_key("main").then(|| "main".to_string()))
            .or_else(|| self.agents.keys().next().cloned())
            .unwrap_or_else(|| "main".to_string());
        self.delegate_counter = file.delegate_counter;
        Ok(source)
    }

    fn run_top_level(&mut self, agent: &str, user_prompt: &str) -> String {
        self.last_run_summary = Some(RunSummary {
            agent_name: agent.to_string(),
            user_prompt: user_prompt.trim().to_string(),
            ..RunSummary::default()
        });
        self.run_agent(agent, user_prompt, 0, self.config.max_steps)
            .message
    }

    fn run_agent(&mut self, agent: &str, request: &str, depth: u32, max_steps: u32) -> RunOutcome {
        self.ensure_agent(agent);
        let intent = intent::classify(request);
        if depth == 0 {
            self.reporter.intent(agent, intent);
            self.update_summary(|summary| summary.intent = Some(intent));
        }

        self.push_message(
            agent,
            ChatMessage::user(prompt::task_brief(request, intent)),
        );
        let mut state = RunState {
            intent,
            has_plan: false,
            files_changed: false,
            verified: false,
            verify_reminded: false,
            invalid_streak: 0,
            failed_calls: std::collections::HashSet::new(),
            request_index: self.message_count(agent) - 1,
        };

        let step_limit = max_steps.max(1);
        for step in 0..step_limit {
            if depth == 0 {
                self.update_summary(|summary| summary.steps_used = step + 1);
            }
            let steps_left = step_limit - step - 1;
            let role = if step == 0 && depth == 0 {
                ModelRole::Planner
            } else {
                ModelRole::Coder
            };
            let model = self.config.routing.model_for(role).to_string();
            let messages = self
                .agents
                .get(agent)
                .map(|session| {
                    session::context_window(
                        &session.messages,
                        self.config.max_history_messages,
                        Some(state.request_index),
                    )
                })
                .unwrap_or_default();

            let phase = if !state.has_plan {
                "plan"
            } else if depth > 0 {
                "delegate"
            } else {
                "work"
            };
            self.reporter
                .thinking_started(phase, &format!("{agent} · {}", preview(request, 64)));
            let reply = match self.llm.chat(&model, &messages, self.config.temperature) {
                Ok(reply) => {
                    self.reporter.thinking_finished(true);
                    reply
                }
                Err(err) => {
                    self.reporter.thinking_finished(false);
                    return RunOutcome {
                        message: format!("Model call failed ({model}): {err:#}"),
                        wrote_files: state.files_changed,
                        verified: state.verified,
                    };
                }
            };

            let shapes = self
                .tools
                .list_specs()
                .iter()
                .map(ToolShape::from)
                .collect::<Vec<_>>();
            let (action, canonical) = match parse_action_with_tools(&reply, &shapes) {
                Ok(parsed) => {
                    state.invalid_streak = 0;
                    parsed
                }
                Err(err) => {
                    state.invalid_streak += 1;
                    self.push_message(agent, ChatMessage::assistant(reply));
                    let mut correction = err.to_string();
                    if !state.has_plan {
                        correction.push_str("\n\n");
                        correction.push_str(prompt::PLAN_REQUIRED);
                    }
                    if state.invalid_streak >= 2 {
                        correction.push_str("\n\n");
                        correction.push_str(prompt::VALID_EXAMPLES);
                    }
                    self.correct(agent, &correction);
                    continue;
                }
            };
            self.push_message(agent, ChatMessage::assistant(canonical.to_string()));

            if !state.has_plan && !matches!(action, Action::Plan(_)) {
                self.correct(agent, prompt::PLAN_REQUIRED);
                continue;
            }

            match action {
                Action::Plan(plan) => self.handle_plan(agent, depth, plan, &mut state),
                Action::Final(final_action) => {
                    let handoff = TaskHandoff::from_action(final_action);
                    if handoff.message_or_summary().is_empty() {
                        self.correct(agent, prompt::FINAL_EMPTY);
                        continue;
                    }
                    if state.intent.requires_verification()
                        && state.files_changed
                        && !state.verified
                        && !state.verify_reminded
                    {
                        state.verify_reminded = true;
                        self.correct(agent, prompt::VERIFY_REQUIRED);
                        continue;
                    }
                    let message = handoff.message_or_summary();
                    if depth == 0 {
                        let (files_changed, verified) = (state.files_changed, state.verified);
                        self.update_summary(|summary| {
                            summary.handoff = Some(handoff);
                            summary.files_changed = files_changed;
                            summary.verified = verified;
                        });
                    }
                    return RunOutcome {
                        message,
                        wrote_files: state.files_changed,
                        verified: state.verified,
                    };
                }
                Action::Tool(tool) => {
                    let name = tool.tool.clone();
                    let signature = format!("{name} {}", tool.args);
                    let output = self.handle_tool(agent, tool);
                    state.record(output.wrote_files, output.executed);
                    let mut feedback = prompt::tool_result(&name, &output.text, steps_left);
                    if !output.is_ok() && !state.failed_calls.insert(signature) {
                        feedback.push_str("\n\n");
                        feedback.push_str(prompt::REPEATED_FAILURE);
                    }
                    self.push_message(agent, ChatMessage::user(feedback));
                }
                Action::ParallelTools(parallel) => {
                    let output = self.handle_parallel(agent, parallel);
                    state.record(output.wrote_files, output.executed);
                    self.push_message(
                        agent,
                        ChatMessage::user(prompt::tool_result(
                            "parallel_tools",
                            &output.text,
                            steps_left,
                        )),
                    );
                }
                Action::Delegate(delegate) => {
                    let (child, outcome) = self.handle_delegate(agent, delegate, depth, step_limit);
                    if outcome.wrote_files {
                        state.files_changed = true;
                        state.verified = outcome.verified;
                    } else if outcome.verified {
                        state.verified = true;
                    }
                    self.push_message(
                        agent,
                        ChatMessage::user(prompt::delegate_result(
                            &child,
                            &outcome.message,
                            steps_left,
                        )),
                    );
                }
            }
        }

        if depth == 0 {
            let (files_changed, verified) = (state.files_changed, state.verified);
            self.update_summary(|summary| {
                summary.files_changed = files_changed;
                summary.verified = verified;
            });
        }
        RunOutcome {
            message: prompt::STEP_LIMIT.to_string(),
            wrote_files: state.files_changed,
            verified: state.verified,
        }
    }

    fn handle_plan(&mut self, agent: &str, depth: u32, action: PlanAction, state: &mut RunState) {
        let plan = TaskPlan::from_action(action);
        let mut intent_changed = false;
        if let Some(chosen) = plan.intent {
            intent_changed = chosen != state.intent;
            state.intent = chosen;
        }
        state.has_plan = true;
        self.reporter.plan(agent, &plan);
        if depth == 0 {
            let intent = state.intent;
            let recorded = plan.clone();
            self.update_summary(|summary| {
                summary.intent = Some(intent);
                summary.plan = Some(recorded);
            });
        }
        self.push_message(
            agent,
            ChatMessage::user(prompt::plan_ack(&plan, state.intent, intent_changed)),
        );
    }

    fn handle_tool(&mut self, agent: &str, action: ToolAction) -> ToolOutput {
        let args = if action.args.is_object() {
            action.args
        } else {
            json!({})
        };
        let name = action.tool.trim().to_string();
        self.reporter.event(
            EventKind::ToolCall,
            agent,
            &format!("{name} {}", preview(&args.to_string(), 160)),
        );
        let output = self.tools.execute(&name, &args);
        self.report_output(agent, &output);
        self.push_tool_record(
            agent,
            ToolRecord {
                tool: name.clone(),
                args,
                ok: output.is_ok(),
                result: output.text.clone(),
            },
        );
        if name == "rules_add" && output.is_ok() {
            self.reload_rules();
        }
        output
    }

    fn handle_parallel(&mut self, agent: &str, action: ParallelAction) -> ToolOutput {
        let calls = action
            .calls
            .iter()
            .map(|call| {
                json!({
                    "tool": call.tool,
                    "args": if call.args.is_object() { call.args.clone() } else { json!({}) },
                })
            })
            .collect::<Vec<_>>();
        let args = json!({
            "calls": calls,
            "max_workers": action.max_workers.unwrap_or(4),
        });
        let names = action
            .calls
            .iter()
            .map(|call| call.tool.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        self.reporter
            .event(EventKind::ToolCall, agent, &format!("parallel [{names}]"));
        let output = self.tools.execute("parallel_tools", &args);
        self.report_output(agent, &output);
        self.push_tool_record(
            agent,
            ToolRecord {
                tool: "parallel_tools".to_string(),
                args,
                ok: output.is_ok(),
                result: output.text.clone(),
            },
        );
        if action.calls.iter().any(|call| call.tool == "rules_add") {
            self.reload_rules();
        }
        output
    }

    fn handle_delegate(
        &mut self,
        parent: &str,
        action: DelegateAction,
        depth: u32,
        parent_steps: u32,
    ) -> (String, RunOutcome) {
        let refused = |message: String| {
            (
                String::from("-"),
                RunOutcome {
                    message,
                    wrote_files: false,
                    verified: false,
                },
            )
        };
        if depth >= self.config.max_delegation_depth {
            return refused(format!(
                "ERROR: delegation depth limit ({}) reached. Do the work yourself.",
                self.config.max_delegation_depth
            ));
        }
        let child = match action.agent_name.as_deref() {
            Some(name) => match normalize_agent_name(name) {
                Ok(name) if name == parent => {
                    return refused("ERROR: an agent cannot delegate to itself".to_string())
                }
                Ok(name) => name,
                Err(err) => return refused(format!("ERROR: invalid agent_name: {err}")),
            },
            None => self.next_delegate_name(depth + 1),
        };
        let child_steps = action
            .max_steps
            .unwrap_or((parent_steps / 2).max(2))
            .clamp(1, parent_steps.max(1));

        self.ensure_agent(&child);
        self.reporter.event(
            EventKind::ToolCall,
            parent,
            &format!(
                "delegate → {child} (steps={child_steps}): {}",
                preview(&action.prompt, 120)
            ),
        );
        let outcome = self.run_agent(&child, &action.prompt, depth + 1, child_steps);
        self.reporter.event(
            EventKind::ToolResult,
            parent,
            &format!("{child} returned: {}", preview(&outcome.message, 200)),
        );
        if let Some(session) = self.agents.get_mut(parent) {
            session.delegate_history.push(DelegateRecord {
                agent_name: child.clone(),
                prompt: action.prompt,
                max_steps: child_steps,
                result: outcome.message.clone(),
            });
        }
        (child, outcome)
    }

    fn report_output(&self, agent: &str, output: &ToolOutput) {
        let kind = match output.status {
            ToolStatus::Ok => EventKind::ToolResult,
            ToolStatus::Error | ToolStatus::Denied => EventKind::ToolError,
        };
        self.reporter
            .event(kind, agent, &preview(&output.text, 240));
    }

    fn correct(&mut self, agent: &str, message: &str) {
        self.reporter
            .event(EventKind::Correction, agent, &preview(message, 160));
        self.push_message(agent, ChatMessage::user(message));
    }

    fn update_summary(&mut self, update: impl FnOnce(&mut RunSummary)) {
        if let Some(summary) = self.last_run_summary.as_mut() {
            update(summary);
        }
    }

    fn ensure_agent(&mut self, name: &str) {
        if !self.agents.contains_key(name) {
            let session = self.new_session(name);
            self.agents.insert(name.to_string(), session);
        }
    }

    fn new_session(&self, name: &str) -> AgentSession {
        AgentSession {
            messages: vec![ChatMessage::system(self.system_prompt_for(name))],
            ..AgentSession::default()
        }
    }

    fn system_prompt_for(&self, agent: &str) -> String {
        let root = self.tools.workspace_root().display().to_string();
        let specs = self.tools.render_specs_for_prompt();
        prompt::system_prompt(&prompt::PromptContext {
            workspace_root: &root,
            agent_name: agent,
            max_delegation_depth: self.config.max_delegation_depth,
            rules: &self.rules,
            tool_specs: &specs,
        })
    }

    fn refresh_system_prompts(&mut self) {
        let prompts = self
            .agents
            .keys()
            .map(|name| (name.clone(), self.system_prompt_for(name)))
            .collect::<Vec<_>>();
        for (name, system) in prompts {
            if let Some(session) = self.agents.get_mut(&name) {
                match session.messages.first_mut() {
                    Some(first) => *first = ChatMessage::system(system),
                    None => session.messages.push(ChatMessage::system(system)),
                }
            }
        }
    }

    fn next_delegate_name(&mut self, depth: u32) -> String {
        loop {
            self.delegate_counter += 1;
            let candidate = format!("delegate_{depth}_{}", self.delegate_counter);
            if !self.agents.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    fn push_message(&mut self, agent: &str, message: ChatMessage) {
        if let Some(session) = self.agents.get_mut(agent) {
            session.messages.push(message);
        }
    }

    fn message_count(&self, agent: &str) -> usize {
        self.agents
            .get(agent)
            .map(|session| session.messages.len())
            .unwrap_or(0)
    }

    fn push_tool_record(&mut self, agent: &str, record: ToolRecord) {
        if let Some(session) = self.agents.get_mut(agent) {
            session.tool_history.push(record);
        }
    }
}

/// Collapses whitespace and cuts `value` to `max_chars` characters.
pub fn preview(value: &str, max_chars: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let mut cut = collapsed.chars().take(max_chars).collect::<String>();
    cut.push_str("...");
    cut
}

fn normalize_agent_name(name: &str) -> Result<String> {
    let value = name.trim();
    if !is_valid_agent_name(value) {
        return Err(anyhow!(
            "agent name must be 1-64 characters of A-Z, a-z, 0-9, '_' or '-'"
        ));
    }
    Ok(value.to_string())
}
