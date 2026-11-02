//! Symbol-aware edits for top-level Python functions and classes.

use super::files::replace_lines;
use super::workspace::Workspace;
use super::{fail, ToolResult};
use serde_json::json;
use std::fs;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PythonSymbol {
    pub name: String,
    pub kind: &'static str,
    pub start_line: usize,
    pub end_line: usize,
}

/// Top-level `def`, `async def`, and `class` definitions with 1-based inclusive line ranges.
/// A symbol's range starts at its decorators and ends before the next unindented line
/// (statement or comment), excluding trailing blank lines.
pub(crate) fn parse_symbols(source: &str) -> Vec<PythonSymbol> {
    let lines = source.lines().collect::<Vec<_>>();
    // Any unindented, non-blank line (including a column-0 comment) ends the previous symbol.
    let is_top_level = |line: &str| !line.trim().is_empty() && !line.starts_with([' ', '\t']);

    let mut symbols: Vec<PythonSymbol> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if !is_top_level(line) {
            index += 1;
            continue;
        }

        let decorator_start = index;
        let mut def_index = index;
        while def_index < lines.len() && lines[def_index].starts_with('@') {
            def_index += 1;
        }
        let Some(def_line) = lines.get(def_index) else {
            break;
        };

        let header = def_line.strip_prefix("async ").unwrap_or(def_line);
        let (kind, rest) = if let Some(rest) = header.strip_prefix("def ") {
            ("function", rest)
        } else if let Some(rest) = header.strip_prefix("class ") {
            ("class", rest)
        } else {
            index = def_index + 1;
            continue;
        };
        let name = rest
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .collect::<String>();

        let mut end = def_index + 1;
        while end < lines.len() && !is_top_level(lines[end]) {
            end += 1;
        }
        let mut last = end;
        while last > def_index + 1 && lines[last - 1].trim().is_empty() {
            last -= 1;
        }

        if !name.is_empty() {
            symbols.push(PythonSymbol {
                name,
                kind,
                start_line: decorator_start + 1,
                end_line: last.max(def_index + 1),
            });
        }
        index = end;
    }
    symbols
}

fn read_python(ws: &Workspace, path: &str) -> Result<String, super::Failure> {
    let resolved = ws.existing_file(path)?;
    if resolved.extension().and_then(|ext| ext.to_str()) != Some("py") {
        return fail("only .py files are supported");
    }
    fs::read_to_string(&resolved).or_else(|err| fail(format!("failed reading '{path}': {err}")))
}

pub(crate) fn symbol_overview(ws: &Workspace, path: &str) -> ToolResult {
    let source = read_python(ws, path)?;
    let symbols = parse_symbols(&source)
        .into_iter()
        .map(|symbol| {
            json!({
                "name": symbol.name,
                "type": symbol.kind,
                "start_line": symbol.start_line,
                "end_line": symbol.end_line,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "path": path, "symbols": symbols }).to_string())
}

pub(crate) fn replace_symbol(
    ws: &Workspace,
    path: &str,
    symbol: &str,
    symbol_type: Option<&str>,
    code: &str,
) -> ToolResult {
    let wanted = match symbol_type.map(|kind| kind.trim().to_ascii_lowercase()) {
        None => None,
        Some(kind) if kind == "function" || kind == "class" => Some(kind),
        Some(other) => {
            return fail(format!(
                "symbol_type must be 'function' or 'class', not '{other}'"
            ))
        }
    };
    let source = read_python(ws, path)?;
    let symbols = parse_symbols(&source);
    let target = symbols.iter().find(|candidate| {
        candidate.name == symbol && wanted.as_deref().is_none_or(|kind| kind == candidate.kind)
    });
    let Some(target) = target else {
        let known = symbols
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return fail(format!(
            "symbol '{symbol}' not found in {path}; top-level symbols: [{known}]"
        ));
    };
    replace_lines(ws, path, target.start_line, target.end_line, code)
}
