//! Git tools. Every command is limited to the workspace directory with a `-- .` pathspec, so a
//! workspace inside a larger repository only sees its own changes.

use super::process::{format_output, run_process, ProcessResult};
use super::workspace::Workspace;
use super::{clip, fail, ToolResult};
use serde_json::json;
use std::path::{Component, Path};
use std::process::Command;

fn git(ws: &Workspace) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(ws.root());
    cmd
}

fn run(
    cmd: &mut Command,
    stdin: Option<&str>,
    timeout: u64,
) -> Result<ProcessResult, super::Failure> {
    run_process(cmd, stdin, timeout).or_else(|err| fail(format!("failed to run git: {err}")))
}

fn require_repo(ws: &Workspace, timeout: u64) -> Result<(), super::Failure> {
    let mut cmd = git(ws);
    cmd.args(["rev-parse", "--is-inside-work-tree"]);
    match run_process(&mut cmd, None, timeout) {
        Ok(result) if result.exit_code == 0 && result.stdout.trim() == "true" => Ok(()),
        _ => fail("workspace is not inside a git repository"),
    }
}

pub(crate) fn status(ws: &Workspace, timeout: u64) -> ToolResult {
    require_repo(ws, timeout)?;
    let mut cmd = git(ws);
    cmd.args(["status", "--short", "--branch", "--", "."]);
    Ok(format_output(&run(&mut cmd, None, timeout)?))
}

pub(crate) fn diff(
    ws: &Workspace,
    path: Option<&str>,
    staged: bool,
    unified: usize,
    timeout: u64,
) -> ToolResult {
    require_repo(ws, timeout)?;
    let mut cmd = git(ws);
    cmd.args(["diff", "--no-color"])
        .arg(format!("--unified={}", unified.min(20)));
    if staged {
        cmd.arg("--staged");
    }
    cmd.arg("--");
    match path {
        Some(path) => {
            let resolved = ws.resolve(path)?;
            cmd.arg(ws.relative(&resolved));
        }
        None => {
            cmd.arg(".");
        }
    }
    let result = run(&mut cmd, None, timeout)?;
    if result.exit_code == 0 && result.stdout.trim().is_empty() {
        return Ok("NO_DIFF: no changes in the selected scope".to_string());
    }
    Ok(format_output(&result))
}

pub(crate) fn log(ws: &Workspace, count: usize, timeout: u64) -> ToolResult {
    require_repo(ws, timeout)?;
    let mut cmd = git(ws);
    cmd.args(["log", "--oneline", "--decorate", "-n"])
        .arg(count.clamp(1, 100).to_string())
        .args(["--", "."]);
    Ok(format_output(&run(&mut cmd, None, timeout)?))
}

pub(crate) fn commit_plan(ws: &Workspace, timeout: u64) -> ToolResult {
    require_repo(ws, timeout)?;
    let mut cmd = git(ws);
    cmd.args(["status", "--porcelain=1", "--", "."]);
    let result = run(&mut cmd, None, timeout)?;
    if result.exit_code != 0 {
        return fail(format_output(&result));
    }

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    for line in result.stdout.lines() {
        let mut chars = line.chars();
        let (Some(left), Some(right)) = (chars.next(), chars.next()) else {
            continue;
        };
        let path = chars.skip(1).collect::<String>();
        if path.is_empty() {
            continue;
        }
        if left == '?' && right == '?' {
            untracked.push(path);
            continue;
        }
        if left != ' ' {
            staged.push(path.clone());
        }
        if right != ' ' {
            unstaged.push(path);
        }
    }

    let mut changed = staged
        .iter()
        .chain(&unstaged)
        .chain(&untracked)
        .cloned()
        .collect::<Vec<_>>();
    changed.sort();
    changed.dedup();

    Ok(json!({
        "staged": staged,
        "unstaged": unstaged,
        "untracked": untracked,
        "changed_paths": changed,
        "suggested_commit_message": suggest_commit_message(&changed),
    })
    .to_string())
}

/// A Conventional Commit style message from the changed paths.
pub(crate) fn suggest_commit_message(changed: &[String]) -> String {
    if changed.is_empty() {
        return "chore: no pending changes".to_string();
    }
    let all = |pred: fn(&str) -> bool| changed.iter().all(|path| pred(path));
    let is_docs = |path: &str| path.ends_with(".md") || path.starts_with("docs/");
    let is_test = |path: &str| {
        path.starts_with("tests/")
            || path.contains("/tests/")
            || path.contains("_test.")
            || path.contains(".test.")
    };
    let is_ci = |path: &str| path.starts_with(".github/");

    let kind = if all(is_docs) {
        "docs"
    } else if all(is_test) {
        "test"
    } else if all(is_ci) {
        "ci"
    } else if changed.iter().any(|path| path.contains("src/")) {
        "feat"
    } else {
        "chore"
    };

    let mut scopes = changed
        .iter()
        .map(|path| path.split('/').next().unwrap_or(path).to_string())
        .collect::<Vec<_>>();
    scopes.sort();
    scopes.dedup();
    let mut scope_summary = scopes
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if scopes.len() > 3 {
        scope_summary.push_str(", ...");
    }
    format!(
        "{kind}: update {} file{} ({scope_summary})",
        changed.len(),
        if changed.len() == 1 { "" } else { "s" }
    )
}

pub(crate) fn commit(ws: &Workspace, message: &str, add_all: bool, timeout: u64) -> ToolResult {
    if message.trim().is_empty() {
        return fail("commit message cannot be empty");
    }
    require_repo(ws, timeout)?;
    if add_all {
        let mut add = git(ws);
        add.args(["add", "-A", "--", "."]);
        let result = run(&mut add, None, timeout)?;
        if result.exit_code != 0 {
            return fail(format!("git add failed\n{}", format_output(&result)));
        }
    }
    let mut cmd = git(ws);
    cmd.args(["commit", "-m", message, "--", "."]);
    let result = run(&mut cmd, None, timeout)?;
    if result.exit_code != 0 {
        return fail(format!("git commit failed\n{}", format_output(&result)));
    }
    Ok(format_output(&result))
}

pub(crate) fn apply_patch(ws: &Workspace, patch: &str, timeout: u64) -> ToolResult {
    if patch.trim().is_empty() {
        return fail("patch is empty");
    }
    require_repo(ws, timeout)?;
    if let Some(bad) = find_unsafe_patch_path(ws, patch) {
        return fail(format!("unsafe patch path '{bad}'"));
    }

    // git apply reads patch paths relative to the repository root. Patch paths are
    // workspace-relative, so a workspace nested inside a repository needs its prefix prepended.
    let mut prefix_cmd = git(ws);
    prefix_cmd.args(["rev-parse", "--show-prefix"]);
    let prefix = run(&mut prefix_cmd, None, timeout)?
        .stdout
        .trim()
        .to_string();

    let mut cmd = git(ws);
    cmd.args(["apply", "--whitespace=nowarn"]);
    if !prefix.is_empty() {
        cmd.arg(format!("--directory={}", prefix.trim_end_matches('/')));
    }
    cmd.arg("-");
    let targets = patch_paths(patch)
        .into_iter()
        .filter_map(|path| ws.resolve(&path).ok())
        .map(|resolved| {
            let before = ws.snapshot(&resolved);
            (resolved, before)
        })
        .collect::<Vec<_>>();
    let result = run(&mut cmd, Some(patch), timeout)?;
    if result.exit_code != 0 {
        return fail(format!("git apply failed\n{}", format_output(&result)));
    }
    for (resolved, before) in targets {
        if resolved.exists() || before.is_some() {
            ws.record_change(&resolved, before);
        }
    }

    let mut status_cmd = git(ws);
    status_cmd.args(["status", "--short", "--", "."]);
    let status = run(&mut status_cmd, None, timeout)?;
    let summary = status.stdout.trim();
    Ok(format!(
        "PATCH APPLIED\nGIT_STATUS:\n{}",
        if summary.is_empty() {
            "<clean>".to_string()
        } else {
            clip(summary, 4_000, "status truncated")
        }
    ))
}

/// The first path in `patch` that would leave the workspace, if any.
pub(crate) fn find_unsafe_patch_path(ws: &Workspace, patch: &str) -> Option<String> {
    patch_paths(patch)
        .into_iter()
        .find(|path| is_unsafe_patch_path(ws, path))
}

/// Every distinct path named in a patch's headers, without `a/` and `b/` prefixes.
fn patch_paths(patch: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            candidates.extend(rest.split_whitespace().take(2).map(str::to_string));
        } else if let Some(rest) = line
            .strip_prefix("--- ")
            .or_else(|| line.strip_prefix("+++ "))
        {
            candidates.push(rest.split('\t').next().unwrap_or(rest).trim().to_string());
        } else if let Some(rest) = line
            .strip_prefix("rename from ")
            .or_else(|| line.strip_prefix("rename to "))
            .or_else(|| line.strip_prefix("copy from "))
            .or_else(|| line.strip_prefix("copy to "))
        {
            candidates.push(rest.trim().to_string());
        }
    }

    let mut paths = Vec::new();
    for path in candidates
        .into_iter()
        .filter_map(|raw| normalize_patch_path(&raw))
    {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

fn normalize_patch_path(raw: &str) -> Option<String> {
    let value = raw.trim().trim_matches('"').trim_matches('\'');
    if value.is_empty() || value == "/dev/null" {
        return None;
    }
    let value = value
        .strip_prefix("a/")
        .or_else(|| value.strip_prefix("b/"))
        .unwrap_or(value);
    Some(value.to_string())
}

fn is_unsafe_patch_path(ws: &Workspace, value: &str) -> bool {
    if value.contains('\\') || value.starts_with('/') {
        return true;
    }
    let path = Path::new(value);
    if path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return true;
    }
    if let Some(Component::Normal(first)) = path.components().next() {
        if first.to_string_lossy().contains(':') {
            return true;
        }
    }
    ws.resolve(value).is_err()
}
