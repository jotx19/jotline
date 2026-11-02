//! Workspace-scoped tools the model can call.
//!
//! Every path argument is resolved by [`workspace::Workspace`], which rejects absolute paths,
//! `..` components, and symlinks that leave the workspace root. Tools return a
//! [`ToolOutput`] whose text is sent back to the model verbatim.

mod args;
mod files;
mod git;
mod memory;
mod plugins;
mod process;
mod python;
mod spec;
mod workspace;

#[cfg(test)]
mod tests;

pub use spec::{ArgSpec, Effect, ToolSpec};
pub use workspace::Workspace;

use crate::config::StatePaths;
use args::Args;
use plugins::PluginSpec;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

/// Decides whether a side-effecting tool call may run. Without an approver, every call runs
/// (auto mode).
pub trait Approver: Send + Sync {
    /// `request` is a human-readable description of the action, such as the shell command.
    fn approve(&self, request: &str) -> bool;
}

/// How a tool call ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolStatus {
    Ok,
    /// Invalid arguments, a failed operation, or an unknown tool.
    Error,
    /// The approver rejected the call.
    Denied,
}

/// One file modified by a tool call, for front ends that show diffs.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct FileChange {
    /// Workspace-relative path with `/` separators.
    pub path: String,
    /// True when the file did not exist before the call.
    pub created: bool,
    /// Previous contents; `None` when the file was created, or when `too_large` is set.
    pub before: Option<String>,
    /// New contents; `None` when `too_large` is set.
    pub after: Option<String>,
    /// True when either version exceeded 256 KiB, so contents were not recorded.
    pub too_large: bool,
}

/// The result of one tool call (or one `parallel_tools` batch).
#[derive(Clone, Debug)]
pub struct ToolOutput {
    pub status: ToolStatus,
    /// Text returned to the model. Errors start with `ERROR:` and denials with `DENIED:`.
    pub text: String,
    /// True when at least one successful call modified workspace files.
    pub wrote_files: bool,
    /// True when at least one successful call executed a command (a verification attempt).
    pub executed: bool,
    /// Files this call changed, in the order they were written. Empty for read-only calls.
    pub changes: Vec<FileChange>,
}

impl ToolOutput {
    pub fn is_ok(&self) -> bool {
        self.status == ToolStatus::Ok
    }
}

/// Internal failure type; rendered with an `ERROR:` or `DENIED:` prefix.
#[derive(Debug)]
pub(crate) enum Failure {
    Invalid(String),
    Denied(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Invalid(message) => write!(f, "ERROR: {message}"),
            Failure::Denied(message) => write!(f, "DENIED: {message}"),
        }
    }
}

pub(crate) type ToolResult = Result<String, Failure>;

pub(crate) fn fail<T>(message: impl Into<String>) -> Result<T, Failure> {
    Err(Failure::Invalid(message.into()))
}

/// Construction parameters for [`ToolExecutor`].
#[derive(Clone)]
pub struct ToolConfig {
    pub workspace_root: PathBuf,
    /// Wall-clock limit for one shell, git, or plugin process.
    pub shell_timeout_seconds: u64,
    pub memory_file: PathBuf,
    pub plugins_dir: PathBuf,
    pub rules_dir: PathBuf,
    /// `None` runs every call without asking (auto mode).
    pub approver: Option<Arc<dyn Approver>>,
}

impl ToolConfig {
    /// Defaults: state under `<workspace>/.jotline/`, a 45 second process timeout, auto mode.
    pub fn for_workspace(workspace_root: impl Into<PathBuf>) -> Self {
        let workspace_root = workspace_root.into();
        let paths = StatePaths::for_workspace(&workspace_root);
        Self {
            workspace_root,
            shell_timeout_seconds: 45,
            memory_file: paths.memory_file,
            plugins_dir: paths.plugins_dir,
            rules_dir: paths.rules_dir,
            approver: None,
        }
    }
}

/// Executes tool calls inside one workspace. Cheap to clone; clones share the memory-file lock.
#[derive(Clone)]
pub struct ToolExecutor {
    workspace: Workspace,
    shell_timeout_seconds: u64,
    memory_file: PathBuf,
    plugins_dir: PathBuf,
    rules_dir: PathBuf,
    plugins: BTreeMap<String, PluginSpec>,
    approver: Option<Arc<dyn Approver>>,
    memory_lock: Arc<Mutex<()>>,
}

impl ToolExecutor {
    pub fn new(config: ToolConfig) -> Self {
        let mut executor = Self {
            workspace: Workspace::new(&config.workspace_root),
            shell_timeout_seconds: config.shell_timeout_seconds.max(1),
            memory_file: config.memory_file,
            plugins_dir: config.plugins_dir,
            rules_dir: config.rules_dir,
            plugins: BTreeMap::new(),
            approver: config.approver,
            memory_lock: Arc::new(Mutex::new(())),
        };
        executor.plugins = plugins::load(&executor.plugins_dir);
        executor
    }

    pub fn workspace_root(&self) -> &Path {
        self.workspace.root()
    }

    pub fn rules_dir(&self) -> &Path {
        &self.rules_dir
    }

    pub fn auto_mode(&self) -> bool {
        self.approver.is_none()
    }

    /// Moves the executor to another workspace and reloads its plugins. Fails when `root` is
    /// not an existing directory.
    pub fn set_workspace(&mut self, root: &Path, paths: StatePaths) -> Result<PathBuf, String> {
        let canonical = root
            .canonicalize()
            .map_err(|err| format!("failed to resolve workspace {}: {err}", root.display()))?;
        if !canonical.is_dir() {
            return Err(format!(
                "workspace is not a directory: {}",
                canonical.display()
            ));
        }
        self.workspace = Workspace::new(&canonical);
        self.memory_file = paths.memory_file;
        self.plugins_dir = paths.plugins_dir;
        self.rules_dir = paths.rules_dir;
        self.plugins = plugins::load(&self.plugins_dir);
        Ok(canonical)
    }

    /// Built-in tools followed by loaded plugins, in a stable order.
    pub fn list_specs(&self) -> Vec<ToolSpec> {
        let mut specs = spec::builtin_specs();
        for plugin in self.plugins.values() {
            specs.push(plugin.tool_spec());
        }
        specs
    }

    pub fn spec(&self, name: &str) -> Option<ToolSpec> {
        self.list_specs().into_iter().find(|spec| spec.name == name)
    }

    /// The tool catalogue in the compact text form used in the system prompt.
    pub fn render_specs_for_prompt(&self) -> String {
        spec::render(&self.list_specs())
    }

    /// Names of loaded plugins without the `plugin.` prefix.
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Runs one tool. `args` must be a JSON object; unknown tools return an error output.
    pub fn execute(&mut self, tool_name: &str, args: &Value) -> ToolOutput {
        if tool_name == "reload_plugins" {
            self.plugins = plugins::load(&self.plugins_dir);
            return ToolOutput {
                status: ToolStatus::Ok,
                text: json!({ "loaded_plugins": self.plugin_names() }).to_string(),
                wrote_files: false,
                executed: false,
                changes: Vec::new(),
            };
        }
        let mut output = if tool_name == "parallel_tools" {
            self.parallel_tools(args)
        } else {
            self.execute_single(tool_name, args)
        };
        output.changes = self.workspace.take_changes();
        output
    }

    fn execute_single(&self, tool_name: &str, args: &Value) -> ToolOutput {
        let effect = self
            .spec(tool_name)
            .map(|spec| spec.effect)
            .unwrap_or(Effect::Read);
        let result = match Args::new(args) {
            Ok(args) => self.dispatch(tool_name, &args),
            Err(failure) => Err(failure),
        };
        match result {
            Ok(text) => ToolOutput {
                status: ToolStatus::Ok,
                text,
                wrote_files: effect == Effect::Write,
                executed: effect == Effect::Execute,
                changes: Vec::new(),
            },
            Err(failure) => ToolOutput {
                status: match failure {
                    Failure::Invalid(_) => ToolStatus::Error,
                    Failure::Denied(_) => ToolStatus::Denied,
                },
                text: failure.to_string(),
                wrote_files: false,
                executed: false,
                changes: Vec::new(),
            },
        }
    }

    fn dispatch(&self, tool_name: &str, args: &Args<'_>) -> ToolResult {
        if let Some(plugin_name) = tool_name.strip_prefix("plugin.") {
            return self.run_plugin(plugin_name, args);
        }

        match tool_name {
            "list_files" => files::list_files(
                &self.workspace,
                args.opt_str("path")?.unwrap_or("."),
                args.usize_or("max_depth", 3)?,
            ),
            "read_file" => files::read_file(
                &self.workspace,
                args.req_str("path")?,
                args.usize_or("start_line", 1)?,
                args.usize_or("end_line", 250)?,
            ),
            "find_files" => files::find_files(
                &self.workspace,
                args.req_str("pattern")?,
                args.opt_str("path")?.unwrap_or("."),
                args.usize_or("max_results", 200)?,
            ),
            "search_files" => files::search_files(
                &self.workspace,
                &files::SearchQuery {
                    pattern: args.req_str("pattern")?,
                    path: args.opt_str("path")?.unwrap_or("."),
                    glob: args.opt_str("glob")?,
                    case_insensitive: args.bool_or("case_insensitive", false)?,
                    max_results: args.usize_or("max_results", 100)?,
                },
            ),
            "write_file" => files::write_file(
                &self.workspace,
                args.req_str("path")?,
                args.opt_str("content")?.unwrap_or(""),
            ),
            "edit_file" => files::edit_file(
                &self.workspace,
                args.req_str("path")?,
                args.req_str("old")?,
                args.opt_str("new")?.unwrap_or(""),
                args.bool_or("replace_all", false)?,
            ),
            "structured_patch" => files::structured_patch(&self.workspace, args.get("operations")),
            "python_symbol_overview" => {
                python::symbol_overview(&self.workspace, args.req_str("path")?)
            }
            "replace_python_symbol" => python::replace_symbol(
                &self.workspace,
                args.req_str("path")?,
                args.req_str("symbol")?,
                args.opt_str("symbol_type")?,
                args.req_str("code")?,
            ),
            "run_shell" => {
                let command = args.req_str("command")?;
                self.confirm(&format!("run shell command\n$ {command}"))?;
                process::run_shell(self.workspace.root(), command, self.shell_timeout_seconds)
            }
            "git_status" => git::status(&self.workspace, self.shell_timeout_seconds),
            "git_diff" => git::diff(
                &self.workspace,
                args.opt_str("path")?,
                args.bool_or("staged", false)?,
                args.usize_or("unified", 3)?,
                self.shell_timeout_seconds,
            ),
            "git_log" => git::log(
                &self.workspace,
                args.usize_or("count", 10)?,
                self.shell_timeout_seconds,
            ),
            "git_commit_plan" => git::commit_plan(&self.workspace, self.shell_timeout_seconds),
            "git_commit" => {
                let message = args.req_str("message")?;
                self.confirm(&format!("create git commit\nmessage: {message}"))?;
                git::commit(
                    &self.workspace,
                    message,
                    args.bool_or("add_all", false)?,
                    self.shell_timeout_seconds,
                )
            }
            "apply_patch" => git::apply_patch(
                &self.workspace,
                args.req_str("patch")?,
                self.shell_timeout_seconds,
            ),
            "memory_get" => memory::get(&self.memory_file, &self.memory_lock, args.opt_str("key")?),
            "memory_set" => memory::set(
                &self.memory_file,
                &self.memory_lock,
                args.req_str("key")?,
                args.get("value").cloned().unwrap_or(Value::Null),
            ),
            "memory_delete" => {
                memory::delete(&self.memory_file, &self.memory_lock, args.req_str("key")?)
            }
            "memory_search" => memory::search(
                &self.memory_file,
                &self.memory_lock,
                args.req_str("query")?,
                args.usize_or("limit", 10)?,
            ),
            "rules_add" => {
                let rule = args.req_str("rule")?;
                match crate::rules::add_user_rule(&self.rules_dir, rule) {
                    Ok(path) => Ok(format!(
                        "RULE_ADDED: {} (applies to every following task)",
                        self.workspace.display(&path)
                    )),
                    Err(err) => fail(format!("failed to add rule: {err}")),
                }
            }
            "parallel_tools" => fail("parallel_tools cannot be nested"),
            "reload_plugins" => fail("reload_plugins cannot run inside parallel_tools"),
            other => fail(format!(
                "unknown tool '{other}'. Use one of the tools listed in the system prompt."
            )),
        }
    }

    fn run_plugin(&self, plugin_name: &str, args: &Args<'_>) -> ToolResult {
        let Some(plugin) = self.plugins.get(plugin_name) else {
            return fail(format!("plugin tool not found: plugin.{plugin_name}"));
        };
        self.confirm(&format!(
            "run plugin '{plugin_name}'\ncommand: {}",
            plugin.command.join(" ")
        ))?;
        plugins::run(
            plugin,
            self.workspace.root(),
            args.raw(),
            self.shell_timeout_seconds,
        )
    }

    fn confirm(&self, request: &str) -> Result<(), Failure> {
        match &self.approver {
            None => Ok(()),
            Some(approver) if approver.approve(request) => Ok(()),
            Some(_) => Err(Failure::Denied(format!(
                "not approved by user: {}",
                request.lines().next().unwrap_or(request)
            ))),
        }
    }

    fn parallel_tools(&self, args: &Value) -> ToolOutput {
        let invalid = |message: String| ToolOutput {
            status: ToolStatus::Error,
            text: format!("ERROR: {message}"),
            wrote_files: false,
            executed: false,
            changes: Vec::new(),
        };

        let Some(calls) = args.get("calls").and_then(Value::as_array) else {
            return invalid("calls must be a non-empty list of {tool, args}".to_string());
        };
        if calls.is_empty() {
            return invalid("calls must be a non-empty list of {tool, args}".to_string());
        }

        let mut normalized = Vec::with_capacity(calls.len());
        for (index, call) in calls.iter().enumerate() {
            let Some(tool) = call.get("tool").and_then(Value::as_str) else {
                return invalid(format!("calls[{index}].tool must be a string"));
            };
            if matches!(tool, "parallel_tools" | "reload_plugins") {
                return invalid(format!("{tool} cannot run inside parallel_tools"));
            }
            let call_args = call.get("args").cloned().unwrap_or_else(|| json!({}));
            if !call_args.is_object() {
                return invalid(format!("calls[{index}].args must be an object"));
            }
            normalized.push((tool.to_string(), call_args));
        }

        let workers = args
            .get("max_workers")
            .and_then(args::as_usize)
            .unwrap_or(4)
            .clamp(1, 8);

        // Approval prompts read stdin, so they must not run concurrently.
        let outputs: Vec<ToolOutput> = if self.approver.is_some() || workers == 1 {
            normalized
                .iter()
                .map(|(tool, call_args)| self.execute_single(tool, call_args))
                .collect()
        } else {
            let mut outputs = Vec::with_capacity(normalized.len());
            for chunk in normalized.chunks(workers) {
                let handles = chunk
                    .iter()
                    .cloned()
                    .map(|(tool, call_args)| {
                        let executor = self.clone();
                        thread::spawn(move || executor.execute_single(&tool, &call_args))
                    })
                    .collect::<Vec<_>>();
                for handle in handles {
                    outputs.push(handle.join().unwrap_or_else(|_| ToolOutput {
                        status: ToolStatus::Error,
                        text: "ERROR: parallel call panicked".to_string(),
                        wrote_files: false,
                        executed: false,
                        changes: Vec::new(),
                    }));
                }
            }
            outputs
        };

        let results = normalized
            .iter()
            .zip(&outputs)
            .enumerate()
            .map(|(index, ((tool, call_args), output))| {
                json!({
                    "index": index,
                    "tool": tool,
                    "args": call_args,
                    "ok": output.is_ok(),
                    "result": output.text,
                })
            })
            .collect::<Vec<_>>();

        ToolOutput {
            status: if outputs.iter().all(ToolOutput::is_ok) {
                ToolStatus::Ok
            } else {
                ToolStatus::Error
            },
            text: json!({ "parallel_results": results }).to_string(),
            wrote_files: outputs.iter().any(|o| o.is_ok() && o.wrote_files),
            executed: outputs.iter().any(|o| o.is_ok() && o.executed),
            changes: Vec::new(),
        }
    }
}

/// Cuts `text` to at most `max_chars` characters (never inside a UTF-8 sequence), appending
/// `note` when anything was removed.
pub(crate) fn clip(text: &str, max_chars: usize, note: &str) -> String {
    match text.char_indices().nth(max_chars) {
        None => text.to_string(),
        Some((byte_index, _)) => format!("{}\n... {note} ...", &text[..byte_index]),
    }
}
