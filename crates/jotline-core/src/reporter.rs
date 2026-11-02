//! Progress reporting from the orchestrator to a front end.

use crate::intent::Intent;
use crate::orchestrator::TaskPlan;

/// What kind of progress an [`Reporter::event`] describes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    /// A tool, parallel batch, or delegation is starting.
    ToolCall,
    /// A tool finished successfully.
    ToolResult,
    /// A tool returned an error or was denied.
    ToolError,
    /// The orchestrator corrected the model (invalid JSON, missing plan, verification gate).
    Correction,
}

/// Receives orchestration progress. Every method has a no-op default, so a front end only
/// implements what it renders.
pub trait Reporter: Send + Sync {
    /// The request was classified; called once per top-level task.
    fn intent(&self, _agent: &str, _intent: Intent) {}

    /// The model recorded a plan.
    fn plan(&self, _agent: &str, _plan: &TaskPlan) {}

    /// A model call is starting. `label` is a short phase name such as `plan` or `work`.
    fn thinking_started(&self, _label: &str, _message: &str) {}

    /// The model call started by the last [`Reporter::thinking_started`] returned.
    fn thinking_finished(&self, _ok: bool) {}

    fn event(&self, _kind: EventKind, _agent: &str, _message: &str) {}
}

/// A reporter that discards everything; used by tests and one-shot library callers.
pub struct NullReporter;

impl Reporter for NullReporter {}
