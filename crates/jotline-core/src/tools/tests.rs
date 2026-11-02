use super::files::glob_match;
use super::git::{find_unsafe_patch_path, suggest_commit_message};
use super::process::run_process;
use super::python::parse_symbols;
use super::workspace::Workspace;
use super::*;
use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn glob_supports_star_and_question_mark() {
    assert!(glob_match("*.rs", "main.rs"));
    assert!(glob_match("*", ""));
    assert!(glob_match("m?in.rs", "main.rs"));
    assert!(glob_match("src/*/mod.rs", "src/tools/mod.rs"));
    assert!(glob_match("*test*", "agent_test.rs"));
    assert!(!glob_match("*.rs", "main.rsx"));
    assert!(!glob_match("?.rs", "ab.rs"));
    assert!(!glob_match("a*b*c", "aXbX"));
}

#[test]
fn clip_never_splits_a_character() {
    let text = "é".repeat(10);
    let clipped = clip(&text, 3, "cut");
    assert!(clipped.starts_with("ééé\n"));
    assert!(clipped.ends_with("... cut ..."));
    assert_eq!(clip("short", 10, "cut"), "short");
}

#[test]
fn python_symbols_include_decorators_and_async() {
    let source = "import os\n\n@cache\n@other\ndef a():\n    return 1\n\n\nasync def b(x):\n    pass\n\nclass C:\n    def m(self):\n        pass\n# trailing comment\nVALUE = 1\n";
    let symbols = parse_symbols(source);
    let summary = symbols
        .iter()
        .map(|s| (s.name.as_str(), s.kind, s.start_line, s.end_line))
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        [
            ("a", "function", 3, 6),
            ("b", "function", 9, 10),
            ("C", "class", 12, 14),
        ]
    );
}

#[test]
fn commit_message_kind_follows_changed_paths() {
    assert_eq!(suggest_commit_message(&[]), "chore: no pending changes");
    assert!(suggest_commit_message(&["README.md".into(), "docs/a.md".into()]).starts_with("docs:"));
    assert!(suggest_commit_message(&["tests/x.rs".into()]).starts_with("test:"));
    assert!(suggest_commit_message(&[".github/workflows/ci.yml".into()]).starts_with("ci:"));
    assert!(suggest_commit_message(&["crates/a/src/lib.rs".into()]).starts_with("feat:"));
    assert_eq!(
        suggest_commit_message(&["Cargo.toml".into()]),
        "chore: update 1 file (Cargo.toml)"
    );
}

#[test]
fn unsafe_patch_paths_are_detected() {
    let tmp = TempDir::new().unwrap();
    let ws = Workspace::new(tmp.path());
    let safe = "diff --git a/x.txt b/x.txt\n--- a/x.txt\n+++ b/x.txt\n";
    assert_eq!(find_unsafe_patch_path(&ws, safe), None);
    let new_file = "diff --git a/n.txt b/n.txt\n--- /dev/null\n+++ b/n.txt\n";
    assert_eq!(find_unsafe_patch_path(&ws, new_file), None);
    for bad in [
        "--- a/../x\n",
        "+++ /etc/passwd\n",
        "rename to ../../x\n",
        "+++ b/C:\\x\n",
    ] {
        assert!(find_unsafe_patch_path(&ws, bad).is_some(), "{bad}");
    }
}

#[test]
fn workspace_rejects_escapes() {
    let tmp = TempDir::new().unwrap();
    let ws = Workspace::new(tmp.path());
    assert!(ws.resolve("../x").is_err());
    assert!(ws.resolve("/etc/passwd").is_err());
    assert!(ws.resolve("a/../../x").is_err());
    assert!(ws.resolve("new/dir/file.txt").is_ok());
}

#[cfg(unix)]
#[test]
fn workspace_rejects_symlink_escape() {
    let outside = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), tmp.path().join("link")).unwrap();
    let ws = Workspace::new(tmp.path());
    assert!(ws.resolve("link/secret.txt").is_err());
    assert!(ws.resolve("link").is_err());
}

#[cfg(unix)]
#[test]
fn large_output_does_not_deadlock() {
    // 1 MB of output is far beyond the OS pipe buffer (typically 64 KiB).
    let mut cmd = Command::new("bash");
    cmd.arg("-c")
        .arg("head -c 1000000 /dev/zero | tr '\\0' 'a'");
    let started = Instant::now();
    let result = run_process(&mut cmd, None, 20).unwrap();
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.len(), 1_000_000);
    assert!(started.elapsed().as_secs() < 15);
}

#[cfg(unix)]
#[test]
fn timeout_kills_process_group() {
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg("sleep 30 & sleep 30; echo never");
    let started = Instant::now();
    let result = run_process(&mut cmd, None, 1).unwrap();
    assert!(result.timed_out);
    assert_eq!(result.exit_code, -1);
    assert!(!result.stdout.contains("never"));
    assert!(
        started.elapsed().as_secs() < 10,
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn process_receives_stdin_payload() {
    if Command::new("cat").arg("--version").output().is_err() {
        return;
    }
    let mut cmd = Command::new("cat");
    let result = run_process(&mut cmd, Some("payload"), 10).unwrap();
    assert_eq!(result.stdout, "payload");
}
