//! Persistent key/value memory stored as a JSON object in `.jotline/memory.json`.
//!
//! Read-modify-write cycles hold a lock shared by every executor clone, so concurrent
//! `memory_set` calls inside `parallel_tools` cannot lose each other's writes.

use super::{fail, ToolResult};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

fn lock(mutex: &Mutex<()>) -> MutexGuard<'_, ()> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A missing file is empty memory. A file that is not a JSON object is an error, so it is never
/// silently overwritten.
fn load(file: &Path) -> Result<Map<String, Value>, super::Failure> {
    if !file.exists() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(file)
        .or_else(|err| fail(format!("failed reading memory file: {err}")))?;
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => fail(format!(
            "memory file {} is not a JSON object; fix or delete it",
            file.display()
        )),
    }
}

fn save(file: &Path, data: &Map<String, Value>) -> Result<(), super::Failure> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .or_else(|err| fail(format!("failed creating memory directory: {err}")))?;
    }
    let text = serde_json::to_string_pretty(data)
        .or_else(|err| fail(format!("failed serializing memory: {err}")))?;
    fs::write(file, text).or_else(|err| fail(format!("failed writing memory file: {err}")))
}

pub(crate) fn get(file: &Path, mutex: &Mutex<()>, key: Option<&str>) -> ToolResult {
    let _guard = lock(mutex);
    let data = load(file)?;
    match key {
        Some(key) => match data.get(key) {
            Some(value) => Ok(json!({ "key": key, "value": value }).to_string()),
            None => Ok(format!("NOT_FOUND: memory key '{key}'")),
        },
        None => Ok(Value::Object(data).to_string()),
    }
}

pub(crate) fn set(file: &Path, mutex: &Mutex<()>, key: &str, value: Value) -> ToolResult {
    if key.trim().is_empty() {
        return fail("memory key cannot be empty");
    }
    let _guard = lock(mutex);
    let mut data = load(file)?;
    data.insert(key.to_string(), value);
    save(file, &data)?;
    Ok(format!("MEMORY_SET: {key}"))
}

pub(crate) fn delete(file: &Path, mutex: &Mutex<()>, key: &str) -> ToolResult {
    let _guard = lock(mutex);
    let mut data = load(file)?;
    if data.remove(key).is_none() {
        return Ok(format!("NOT_FOUND: memory key '{key}'"));
    }
    save(file, &data)?;
    Ok(format!("MEMORY_DELETED: {key}"))
}

pub(crate) fn search(file: &Path, mutex: &Mutex<()>, query: &str, limit: usize) -> ToolResult {
    if query.trim().is_empty() {
        return fail("query cannot be empty");
    }
    let _guard = lock(mutex);
    let needle = query.to_lowercase();
    let matches = load(file)?
        .into_iter()
        .filter(|(key, value)| format!("{key}\n{value}").to_lowercase().contains(&needle))
        .take(limit.clamp(1, 100))
        .map(|(key, value)| json!({ "key": key, "value": value }))
        .collect::<Vec<_>>();
    Ok(Value::Array(matches).to_string())
}
