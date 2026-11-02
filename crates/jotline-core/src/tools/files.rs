use super::workspace::{is_ignored_dir, Workspace};
use super::{clip, fail, ToolResult};
use regex::RegexBuilder;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_TREE_LINES: usize = 1_500;
const MAX_READ_LINES: usize = 1_000;
const MAX_SEARCH_FILE_BYTES: u64 = 1_000_000;
const MAX_MATCH_LINE_CHARS: usize = 240;

pub(crate) fn list_files(ws: &Workspace, path: &str, max_depth: usize) -> ToolResult {
    let root = ws.existing(path)?;
    let mut lines = Vec::new();
    walk_tree(ws, &root, max_depth.min(8), &mut lines);
    if lines.len() >= MAX_TREE_LINES {
        lines.truncate(MAX_TREE_LINES);
        lines.push("... output truncated; list a subdirectory ...".to_string());
    }
    Ok(lines.join("\n"))
}

fn walk_tree(ws: &Workspace, node: &Path, depth_left: usize, lines: &mut Vec<String>) {
    if lines.len() >= MAX_TREE_LINES {
        return;
    }
    let rel = ws.display(node);
    if node.is_file() {
        lines.push(rel);
        return;
    }
    let label = if rel == "." {
        ".".to_string()
    } else {
        format!("{rel}/")
    };
    if depth_left == 0 {
        lines.push(label);
        return;
    }
    let mut children = match fs::read_dir(node) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => {
            lines.push(format!("{label} [unreadable]"));
            return;
        }
    };
    lines.push(label);
    children.sort_by_key(|entry| {
        (
            entry.path().is_file(),
            entry.file_name().to_string_lossy().to_lowercase(),
        )
    });
    for child in children {
        if lines.len() >= MAX_TREE_LINES {
            break;
        }
        let path = child.path();
        let name = child.file_name().to_string_lossy().to_string();
        if path.is_dir() && is_ignored_dir(&name) {
            lines.push(format!("{}/ [skipped]", ws.display(&path)));
            continue;
        }
        walk_tree(ws, &path, depth_left - 1, lines);
    }
}

/// Every file under `root`, skipping ignored directories, in sorted order.
fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir.is_file() {
            files.push(dir);
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !is_ignored_dir(&entry.file_name().to_string_lossy()) {
                    stack.push(path);
                }
            } else if kind.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Glob match supporting `*` (any run of characters) and `?` (one character).
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    let (mut p, mut t) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

fn name_matches(ws: &Workspace, path: &Path, pattern: &str) -> bool {
    if pattern.contains('/') {
        glob_match(pattern, &ws.display(path))
    } else {
        path.file_name()
            .map(|name| glob_match(pattern, &name.to_string_lossy()))
            .unwrap_or(false)
    }
}

pub(crate) fn find_files(
    ws: &Workspace,
    pattern: &str,
    path: &str,
    max_results: usize,
) -> ToolResult {
    let root = ws.existing(path)?;
    let limit = max_results.clamp(1, 1_000);
    let matches = walk_files(&root)
        .into_iter()
        .filter(|file| name_matches(ws, file, pattern))
        .map(|file| ws.display(&file))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(format!(
            "NO_MATCHES: no files match '{pattern}' under {path}"
        ));
    }
    let total = matches.len();
    let mut out = matches.into_iter().take(limit).collect::<Vec<_>>();
    if total > limit {
        out.push(format!(
            "... {} more; narrow the pattern ...",
            total - limit
        ));
    }
    Ok(out.join("\n"))
}

pub(crate) struct SearchQuery<'a> {
    pub pattern: &'a str,
    pub path: &'a str,
    pub glob: Option<&'a str>,
    pub case_insensitive: bool,
    pub max_results: usize,
}

pub(crate) fn search_files(ws: &Workspace, query: &SearchQuery<'_>) -> ToolResult {
    if query.pattern.is_empty() {
        return fail("pattern cannot be empty");
    }
    let root = ws.existing(query.path)?;
    let build = |source: &str| {
        RegexBuilder::new(source)
            .case_insensitive(query.case_insensitive)
            .size_limit(1 << 20)
            .build()
    };
    let (regex, literal) = match build(query.pattern) {
        Ok(regex) => (regex, false),
        Err(_) => match build(&regex::escape(query.pattern)) {
            Ok(regex) => (regex, true),
            Err(err) => return fail(format!("invalid pattern: {err}")),
        },
    };

    let limit = query.max_results.clamp(1, 500);
    let mut matches = Vec::new();
    let mut truncated = false;
    'files: for file in walk_files(&root) {
        if let Some(glob) = query.glob {
            if !name_matches(ws, &file, glob) {
                continue;
            }
        }
        let Some(text) = read_text_for_search(&file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if regex.is_match(line) {
                if matches.len() >= limit {
                    truncated = true;
                    break 'files;
                }
                let shown = clip(line.trim(), MAX_MATCH_LINE_CHARS, "line truncated");
                matches.push(format!("{}:{}: {}", ws.display(&file), index + 1, shown));
            }
        }
    }

    if matches.is_empty() {
        return Ok(format!(
            "NO_MATCHES: '{}' under {}",
            query.pattern, query.path
        ));
    }
    let mut out = String::new();
    if literal {
        out.push_str("NOTE: pattern was not a valid regex; searched it literally\n");
    }
    out.push_str(&matches.join("\n"));
    if truncated {
        out.push_str(&format!(
            "\n... stopped at {limit} matches; narrow path, glob, or pattern ..."
        ));
    }
    Ok(out)
}

/// File text for searching; `None` for large, unreadable, or binary (NUL-containing) files.
fn read_text_for_search(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > MAX_SEARCH_FILE_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(path).ok()?.read_to_end(&mut bytes).ok()?;
    if bytes.iter().take(8_192).any(|byte| *byte == 0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_text(path: &Path, display: &str) -> Result<String, super::Failure> {
    match fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(err) => fail(format!("failed reading '{display}': {err}")),
    }
}

fn write_text(path: &Path, display: &str, text: &str) -> Result<(), super::Failure> {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return fail(format!("failed creating parent of '{display}': {err}"));
        }
    }
    fs::write(path, text.as_bytes())
        .or_else(|err| fail(format!("failed writing '{display}': {err}")))
}

pub(crate) fn read_file(
    ws: &Workspace,
    path: &str,
    start_line: usize,
    end_line: usize,
) -> ToolResult {
    let resolved = ws.existing_file(path)?;
    let text = read_text(&resolved, path)?;
    let lines = text.lines().collect::<Vec<_>>();

    let start = start_line.max(1);
    let end = end_line.max(start).min(start + MAX_READ_LINES - 1);
    let end_shown = end.min(lines.len());

    let mut out = format!(
        "FILE: {path} (lines {start}-{end_shown} of {})",
        lines.len()
    );
    if start > lines.len() {
        out.push_str("\n<no content in selected range>");
        return Ok(out);
    }
    for (index, line) in lines.iter().enumerate().take(end_shown).skip(start - 1) {
        out.push_str(&format!("\n{:5}: {line}", index + 1));
    }
    if end_shown < lines.len() {
        out.push_str(&format!(
            "\n... {} more lines; read from start_line={} ...",
            lines.len() - end_shown,
            end_shown + 1
        ));
    }
    Ok(out)
}

pub(crate) fn write_file(ws: &Workspace, path: &str, content: &str) -> ToolResult {
    let resolved = ws.resolve(path)?;
    if resolved.is_dir() {
        return fail(format!("'{path}' is a directory"));
    }
    let existed = resolved.exists();
    write_text(&resolved, path, content)?;
    Ok(format!(
        "WROTE: {path} ({} bytes, {})",
        content.len(),
        if existed { "overwritten" } else { "created" }
    ))
}

pub(crate) fn edit_file(
    ws: &Workspace,
    path: &str,
    old: &str,
    new: &str,
    replace_all: bool,
) -> ToolResult {
    if old.is_empty() {
        return fail("'old' cannot be empty; use write_file to create a file");
    }
    if old == new {
        return fail("'old' and 'new' are identical; nothing to change");
    }
    let resolved = ws.existing_file(path)?;
    let text = read_text(&resolved, path)?;

    // Small models often double-escape line breaks ("a\\nb" instead of "a\nb"). Accept that
    // only when the literal text is absent and the unescaped text is present.
    let (old, new, note) = if !text.contains(old) && old.contains("\\n") {
        let unescaped = unescape_breaks(old);
        if text.contains(&unescaped) {
            (
                unescaped,
                unescape_breaks(new),
                " (interpreted \\n in old/new as line breaks)",
            )
        } else {
            (old.to_string(), new.to_string(), "")
        }
    } else {
        (old.to_string(), new.to_string(), "")
    };

    let occurrences = text.matches(old.as_str()).count();
    match occurrences {
        0 => {
            return fail(format!(
                "'old' text not found in {path}. Copy 'old' exactly from the current file, including indentation.\n{}",
                current_text_hint(&text, &old)
            ))
        }
        1 => {}
        n if !replace_all => {
            return fail(format!(
                "'old' text matches {n} times in {path}. Include more surrounding lines to make it unique, or set replace_all=true."
            ))
        }
        _ => {}
    }
    let updated = text.replace(old.as_str(), &new);
    write_text(&resolved, path, &updated)?;
    Ok(format!("EDITED: {path} (replacements={occurrences}){note}"))
}

fn unescape_breaks(value: &str) -> String {
    value
        .replace("\\r\\n", "\n")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
}

/// Shows the model the real text it should have copied: the lines around the best match for
/// the first line of `old`, the whole file when it is short, or a pointer to `read_file`.
pub(crate) fn current_text_hint(text: &str, old: &str) -> String {
    const SHORT_FILE_LINES: usize = 60;
    const CONTEXT: usize = 3;
    let lines = text.lines().collect::<Vec<_>>();
    let numbered = |start: usize, end: usize| {
        lines[start..end]
            .iter()
            .enumerate()
            .map(|(offset, line)| format!("{:5}: {line}", start + offset + 1))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let anchor = unescape_breaks(old)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string);
    if let Some(anchor) = anchor {
        if let Some(index) = lines
            .iter()
            .position(|line| line.trim() == anchor)
            .or_else(|| lines.iter().position(|line| line.contains(anchor.as_str())))
        {
            let old_lines = old.lines().count().max(1);
            let start = index.saturating_sub(CONTEXT);
            let end = (index + old_lines + CONTEXT).min(lines.len());
            return format!(
                "Current text near line {} (line numbers are not part of the file):\n{}",
                index + 1,
                numbered(start, end)
            );
        }
    }
    if lines.len() <= SHORT_FILE_LINES {
        return format!(
            "Current file content (line numbers are not part of the file):\n{}",
            numbered(0, lines.len())
        );
    }
    "Use read_file to see the current text before editing.".to_string()
}

fn replace_text(
    ws: &Workspace,
    path: &str,
    old: &str,
    new: &str,
    count: Option<usize>,
) -> ToolResult {
    if old.is_empty() {
        return fail("'old' cannot be empty");
    }
    let resolved = ws.existing_file(path)?;
    let text = read_text(&resolved, path)?;
    let occurrences = text.matches(old).count();
    if occurrences == 0 {
        return fail(format!("'old' text not found in {path}"));
    }
    let (updated, applied) = match count {
        Some(limit) => (text.replacen(old, new, limit), occurrences.min(limit)),
        None => (text.replace(old, new), occurrences),
    };
    write_text(&resolved, path, &updated)?;
    Ok(format!(
        "REPLACED: {path} occurrences={occurrences} applied={applied}"
    ))
}

pub(crate) fn replace_lines(
    ws: &Workspace,
    path: &str,
    start_line: usize,
    end_line: usize,
    new_text: &str,
) -> ToolResult {
    if start_line < 1 || end_line < start_line {
        return fail("invalid line range: need 1 <= start_line <= end_line");
    }
    let resolved = ws.existing_file(path)?;
    let text = read_text(&resolved, path)?;
    let had_trailing_newline = text.ends_with('\n');
    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    if start_line > lines.len() + 1 {
        return fail(format!(
            "start_line {start_line} is past the end of {path} ({} lines)",
            lines.len()
        ));
    }
    let end_index = end_line.min(lines.len());
    lines.splice(
        start_line - 1..end_index,
        new_text.lines().map(str::to_string),
    );
    let mut updated = lines.join("\n");
    if had_trailing_newline || text.is_empty() {
        updated.push('\n');
    }
    write_text(&resolved, path, &updated)?;
    Ok(format!("WROTE: {path} lines {start_line}-{end_line}"))
}

fn append_text(ws: &Workspace, path: &str, text: &str) -> ToolResult {
    let resolved = ws.resolve(path)?;
    if let Some(parent) = resolved.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return fail(format!("failed creating parent of '{path}': {err}"));
        }
    }
    let mut handle = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&resolved)
        .or_else(|err| fail(format!("failed opening '{path}': {err}")))?;
    handle
        .write_all(text.as_bytes())
        .or_else(|err| fail(format!("failed appending to '{path}': {err}")))?;
    Ok(format!("APPENDED: {path} ({} bytes)", text.len()))
}

pub(crate) fn structured_patch(ws: &Workspace, operations: Option<&Value>) -> ToolResult {
    let Some(list) = operations
        .and_then(Value::as_array)
        .filter(|l| !l.is_empty())
    else {
        return fail("operations must be a non-empty list");
    };

    let mut results = Vec::with_capacity(list.len());
    let mut failures = 0usize;
    for (index, op) in list.iter().enumerate() {
        let field = |key: &str| op.get(key).and_then(Value::as_str);
        let number = |key: &str| op.get(key).and_then(super::args::as_usize);
        let kind = field("type").unwrap_or("");
        let result = match kind {
            "replace_text" => match (field("path"), field("old")) {
                (Some(path), Some(old)) => {
                    replace_text(ws, path, old, field("new").unwrap_or(""), number("count"))
                }
                _ => fail("replace_text needs path and old"),
            },
            "replace_lines" => match (field("path"), number("start_line"), number("end_line")) {
                (Some(path), Some(start), Some(end)) => {
                    replace_lines(ws, path, start, end, field("new_text").unwrap_or(""))
                }
                _ => fail("replace_lines needs path, start_line, end_line"),
            },
            "append_text" => match field("path") {
                Some(path) => append_text(ws, path, field("text").unwrap_or("")),
                None => fail("append_text needs path"),
            },
            other => fail(format!("unknown operation type '{other}'")),
        };
        match result {
            Ok(text) => results.push(format!("op#{index} {kind}: {text}")),
            Err(failure) => {
                failures += 1;
                results.push(format!("op#{index} {kind}: {failure}"));
            }
        }
    }

    if failures == list.len() {
        return fail(format!("every operation failed\n{}", results.join("\n")));
    }
    Ok(results.join("\n"))
}
