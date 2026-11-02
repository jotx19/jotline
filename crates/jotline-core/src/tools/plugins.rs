//! External command tools declared as JSON files in `.jotline/plugins/`.
//!
//! A plugin named `echo` becomes the tool `plugin.echo`. jotline runs its `command` in the
//! workspace root, writes the call's `args` object to stdin as JSON, and returns the output.

use super::process::{format_output, run_process};
use super::spec::{ArgSpec, Effect, ToolSpec};
use super::{fail, ToolResult};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PluginSpec {
    pub name: String,
    pub description: String,
    /// Program followed by its arguments; not run through a shell.
    pub command: Vec<String>,
    /// Argument name to description, shown to the model.
    #[serde(default)]
    pub args_schema: BTreeMap<String, String>,
}

impl PluginSpec {
    fn is_valid(&self) -> bool {
        let name_ok = !self.name.is_empty()
            && self.name.len() <= 64
            && self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        name_ok
            && !self.description.trim().is_empty()
            && !self.command.is_empty()
            && self.command.iter().all(|part| !part.trim().is_empty())
    }

    pub(crate) fn tool_spec(&self) -> ToolSpec {
        ToolSpec {
            name: format!("plugin.{}", self.name),
            description: self.description.clone(),
            args: self
                .args_schema
                .iter()
                .map(|(name, description)| ArgSpec {
                    name: name.clone(),
                    required: false,
                    description: description.clone(),
                })
                .collect(),
            effect: Effect::Execute,
        }
    }
}

/// Valid plugin specs in `dir`, keyed by name. Invalid or unreadable files are skipped.
pub(crate) fn load(dir: &Path) -> BTreeMap<String, PluginSpec> {
    let mut plugins = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return plugins;
    };
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(spec) = serde_json::from_str::<PluginSpec>(&text) else {
            continue;
        };
        if spec.is_valid() {
            plugins.insert(spec.name.clone(), spec);
        }
    }
    plugins
}

pub(crate) fn run(
    plugin: &PluginSpec,
    root: &Path,
    args: &Map<String, Value>,
    timeout: u64,
) -> ToolResult {
    let mut cmd = Command::new(&plugin.command[0]);
    cmd.args(&plugin.command[1..])
        .current_dir(root)
        .env("JOTLINE_WORKSPACE", root);
    let payload = Value::Object(args.clone()).to_string();
    match run_process(&mut cmd, Some(&payload), timeout) {
        Ok(result) => Ok(format!(
            "PLUGIN: {}\n{}",
            plugin.name,
            format_output(&result)
        )),
        Err(err) => fail(format!(
            "failed to start plugin '{}' ({}): {err}",
            plugin.name, plugin.command[0]
        )),
    }
}
