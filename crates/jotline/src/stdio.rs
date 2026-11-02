//! `jotline --stdio`: a newline-delimited JSON protocol for interactive front ends such as the
//! TypeScript terminal UI in `packages/tui`.
//!
//! Every line on stdin is one JSON request; every line on stdout is one JSON event. stdout
//! carries nothing else. The protocol is documented in `docs/PROTOCOL.md`.

use anyhow::Result;
use jotline_core::config::ModelRole;
use jotline_core::intent::Intent;
use jotline_core::orchestrator::{Orchestrator, RunSummary, TaskPlan};
use jotline_core::reporter::{EventKind, Reporter};
use jotline_core::rules::{self, RuleOrigin};
use jotline_core::tools::{Approver, ToolOutput, ToolStatus};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// Version of the event and request shapes; bumped on incompatible changes.
pub const PROTOCOL_VERSION: u32 = 1;

/// Writes events to stdout, one JSON object per line.
pub struct Emitter {
    out: Mutex<io::Stdout>,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            out: Mutex::new(io::stdout()),
        }
    }

    pub fn emit(&self, event: Value) {
        if let Ok(mut out) = self.out.lock() {
            let _ = writeln!(out, "{event}");
            let _ = out.flush();
        }
    }
}

/// A request that the main loop handles between tasks.
pub enum Request {
    Prompt(String),
    Command { name: String, arg: String },
}

enum ApprovalMessage {
    Answer { id: u64, approved: bool },
    Interrupt,
}

/// Channels fed by the stdin reader thread.
pub struct Inputs {
    pub requests: mpsc::Receiver<Request>,
    approvals: mpsc::Receiver<ApprovalMessage>,
    pub cancel: Arc<AtomicBool>,
}

/// Starts the thread that reads stdin. `interrupt` requests set the cancel flag immediately,
/// even while a task is running, and also deny a pending approval.
pub fn spawn_reader(emitter: Arc<Emitter>) -> Inputs {
    let (request_tx, requests) = mpsc::channel();
    let (approval_tx, approvals) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let reader_cancel = Arc::clone(&cancel);

    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let message: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(err) => {
                    emitter.emit(json!({"event": "error", "message": format!("invalid request JSON: {err}")}));
                    continue;
                }
            };
            let text = |key: &str| {
                message
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            match message.get("type").and_then(Value::as_str) {
                Some("prompt") => {
                    let _ = request_tx.send(Request::Prompt(text("text")));
                }
                Some("command") => {
                    let _ = request_tx.send(Request::Command {
                        name: text("name"),
                        arg: text("arg"),
                    });
                }
                Some("approval") => {
                    let _ = approval_tx.send(ApprovalMessage::Answer {
                        id: message.get("id").and_then(Value::as_u64).unwrap_or(0),
                        approved: message
                            .get("approved")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    });
                }
                Some("interrupt") => {
                    reader_cancel.store(true, Ordering::SeqCst);
                    let _ = approval_tx.send(ApprovalMessage::Interrupt);
                }
                Some("exit") => break,
                other => emitter.emit(json!({
                    "event": "error",
                    "message": format!("unknown request type {other:?}; expected prompt, command, approval, interrupt, or exit"),
                })),
            }
        }
    });

    Inputs {
        requests,
        approvals,
        cancel,
    }
}

/// Asks the front end to approve a side-effecting call and waits for the answer.
pub struct StdioApprover {
    emitter: Arc<Emitter>,
    answers: Mutex<mpsc::Receiver<ApprovalMessage>>,
    next_id: AtomicU64,
}

impl StdioApprover {
    /// Takes the approval channel out of `inputs`; call before using `inputs.requests`.
    pub fn new(emitter: Arc<Emitter>, inputs: &mut Inputs) -> Self {
        let (_, empty) = mpsc::channel();
        let answers = std::mem::replace(&mut inputs.approvals, empty);
        Self {
            emitter,
            answers: Mutex::new(answers),
            next_id: AtomicU64::new(1),
        }
    }
}

impl Approver for StdioApprover {
    fn approve(&self, request: &str) -> bool {
        let Ok(answers) = self.answers.lock() else {
            return false;
        };
        // Discard answers and interrupts left over from earlier requests.
        while answers.try_recv().is_ok() {}
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.emitter
            .emit(json!({"event": "approval_request", "id": id, "request": request}));
        loop {
            match answers.recv() {
                Ok(ApprovalMessage::Answer {
                    id: answered,
                    approved,
                }) if answered == id => return approved,
                Ok(ApprovalMessage::Answer { .. }) => continue,
                Ok(ApprovalMessage::Interrupt) | Err(_) => return false,
            }
        }
    }
}

/// Forwards orchestrator progress as events.
pub struct StdioReporter {
    emitter: Arc<Emitter>,
}

impl StdioReporter {
    pub fn new(emitter: Arc<Emitter>) -> Self {
        Self { emitter }
    }
}

fn plan_json(plan: &TaskPlan) -> Value {
    json!({
        "intent": plan.intent.map(Intent::as_str),
        "needs_plan": plan.needs_plan,
        "summary": plan.summary,
        "reason": plan.reason,
        "steps": plan.steps,
        "files": plan.files,
    })
}

impl Reporter for StdioReporter {
    fn intent(&self, agent: &str, intent: Intent) {
        self.emitter.emit(json!({
            "event": "intent",
            "agent": agent,
            "intent": intent.as_str(),
            "read_only": intent.is_read_only(),
        }));
    }

    fn plan(&self, agent: &str, plan: &TaskPlan) {
        self.emitter
            .emit(json!({"event": "plan", "agent": agent, "plan": plan_json(plan)}));
    }

    fn thinking_started(&self, label: &str, message: &str) {
        self.emitter
            .emit(json!({"event": "thinking", "phase": label, "message": message}));
    }

    fn thinking_finished(&self, ok: bool) {
        self.emitter
            .emit(json!({"event": "thinking_done", "ok": ok}));
    }

    fn event(&self, kind: EventKind, agent: &str, message: &str) {
        // Tool calls and results are reported through tool_started/tool_finished; only
        // corrections and delegation messages are useful as notices.
        let kind = match kind {
            EventKind::Correction => "correction",
            EventKind::ToolCall if message.starts_with("delegate") => "delegate",
            EventKind::ToolResult if message.contains(" returned: ") => "delegate_result",
            _ => return,
        };
        self.emitter
            .emit(json!({"event": "notice", "kind": kind, "agent": agent, "message": message}));
    }

    fn tool_started(&self, agent: &str, call_id: u64, tool: &str, args: &Value) {
        self.emitter.emit(json!({
            "event": "tool_started",
            "agent": agent,
            "id": call_id,
            "tool": tool,
            "args": args,
        }));
    }

    fn tool_finished(&self, agent: &str, call_id: u64, tool: &str, output: &ToolOutput) {
        self.emitter.emit(json!({
            "event": "tool_finished",
            "agent": agent,
            "id": call_id,
            "tool": tool,
            "status": match output.status {
                ToolStatus::Ok => "ok",
                ToolStatus::Error => "error",
                ToolStatus::Denied => "denied",
            },
            "text": output.text,
            "wrote_files": output.wrote_files,
            "executed": output.executed,
            "changes": output.changes,
        }));
    }
}

fn summary_json(summary: Option<&RunSummary>, message: &str) -> Value {
    let Some(summary) = summary else {
        return json!({"message": message});
    };
    json!({
        "agent": summary.agent_name,
        "message": message,
        "intent": summary.intent.map(Intent::as_str),
        "steps": summary.steps_used,
        "files_changed": summary.files_changed,
        "verified": summary.verified,
        "handoff": summary.handoff.as_ref().map(|handoff| json!({
            "message": handoff.message,
            "summary": handoff.summary,
            "completed": handoff.completed,
            "verified": handoff.verified,
            "missed": handoff.missed,
            "next_steps": handoff.next_steps,
        })),
    })
}

fn rules_json(orchestrator: &Orchestrator) -> Value {
    Value::Array(
        orchestrator
            .rules()
            .sources()
            .iter()
            .map(|source| {
                json!({
                    "origin": match source.origin {
                        RuleOrigin::Builtin => "builtin",
                        RuleOrigin::Project => "project",
                    },
                    "name": source.name,
                    "chars": source.content.chars().count(),
                    "truncated": source.truncated,
                })
            })
            .collect(),
    )
}

/// Static facts about the session, sent once in the `ready` event.
pub struct SessionInfo<'a> {
    pub provider: &'a str,
    pub auto_mode: bool,
    pub session_file: Option<&'a Path>,
}

fn ready_json(orchestrator: &Orchestrator, info: &SessionInfo<'_>) -> Value {
    let routing = orchestrator.routing();
    json!({
        "event": "ready",
        "protocol": PROTOCOL_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "workspace": orchestrator.workspace_root().display().to_string(),
        "provider": info.provider,
        "model": routing.text_model,
        "planner_model": routing.model_for(ModelRole::Planner),
        "coder_model": routing.model_for(ModelRole::Coder),
        "auto_mode": info.auto_mode,
        "agent": orchestrator.active_agent(),
        "rules": rules_json(orchestrator),
        "tools": orchestrator.tools().list_specs().into_iter().map(|spec| spec.name).collect::<Vec<_>>(),
    })
}

/// Serves requests until stdin closes or an `exit` request arrives.
pub fn serve(
    orchestrator: &mut Orchestrator,
    inputs: Inputs,
    emitter: &Emitter,
    info: &SessionInfo<'_>,
) -> Result<()> {
    emitter.emit(ready_json(orchestrator, info));

    for request in inputs.requests {
        match request {
            Request::Prompt(text) if text.trim().is_empty() => {
                emitter.emit(json!({"event": "error", "message": "prompt text is empty"}));
            }
            Request::Prompt(text) => {
                inputs.cancel.store(false, Ordering::SeqCst);
                emitter.emit(json!({"event": "run_started", "agent": orchestrator.active_agent(), "prompt": text}));
                let message = orchestrator.run_active(&text);
                let mut event = summary_json(orchestrator.last_run_summary(), &message);
                event["event"] = json!("run_finished");
                emitter.emit(event);
            }
            Request::Command { name, arg } => handle_command(orchestrator, emitter, &name, &arg),
        }
        if let Some(path) = info.session_file {
            if let Err(err) = orchestrator.save_session(path) {
                emitter.emit(json!({"event": "error", "message": format!("failed to save session: {err:#}")}));
            }
        }
    }
    Ok(())
}

fn handle_command(orchestrator: &mut Orchestrator, emitter: &Emitter, name: &str, arg: &str) {
    let result: Result<Value> = match name {
        "reset" => orchestrator
            .reset_active()
            .map(|()| json!({"event": "reset", "agent": orchestrator.active_agent()})),
        "rules" => Ok(json!({"event": "rules", "rules": rules_json(orchestrator)})),
        "rules_add" => rules::add_user_rule(orchestrator.tools().rules_dir(), arg)
            .map_err(anyhow::Error::from)
            .map(|path| {
                orchestrator.reload_rules();
                json!({"event": "rules", "added": path.display().to_string(), "rules": rules_json(orchestrator)})
            }),
        "reload_rules" => {
            orchestrator.reload_rules();
            Ok(json!({"event": "rules", "rules": rules_json(orchestrator)}))
        }
        "tools" => Ok(json!({
            "event": "tools",
            "tools": orchestrator.tools().list_specs().into_iter().map(|spec| json!({"name": spec.name, "description": spec.description})).collect::<Vec<_>>(),
        })),
        "status" => Ok(json!({
            "event": "status",
            "agent": orchestrator.active_agent(),
            "workspace": orchestrator.workspace_root().display().to_string(),
            "model": orchestrator.routing().text_model,
            "last_run": orchestrator.last_run_summary().map(|summary| summary_json(Some(summary), &summary.handoff.as_ref().map(|h| h.message_or_summary()).unwrap_or_default())),
        })),
        other => Err(anyhow::anyhow!(
            "unknown command '{other}'; expected reset, rules, rules_add, reload_rules, tools, or status"
        )),
    };
    match result {
        Ok(event) => emitter.emit(event),
        Err(err) => emitter.emit(json!({"event": "error", "message": format!("{err:#}")})),
    }
}
