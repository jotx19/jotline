mod support;

use jotline_core::tools::{Approver, ToolConfig, ToolExecutor, ToolStatus};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use support::{init_repo, run_checked};
use tempfile::TempDir;

struct Harness {
    _tmp: TempDir,
    workspace: PathBuf,
    executor: ToolExecutor,
}

impl Harness {
    /// A git repository with one committed file and an `echo` plugin.
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().canonicalize().unwrap();
        init_repo(&workspace);
        fs::write(workspace.join("hello.txt"), "one\n").unwrap();
        run_checked(&["git", "add", "hello.txt"], &workspace);
        run_checked(&["git", "commit", "-q", "-m", "init"], &workspace);

        let plugins_dir = workspace.join(".jotline").join("plugins");
        fs::create_dir_all(&plugins_dir).unwrap();
        let plugin = json!({
            "name": "echo",
            "description": "Echo input value",
            "command": ["python3", "-c", "import json,sys; p=json.load(sys.stdin); print('ECHO:' + str(p.get('value', '')))"],
            "args_schema": {"value": "text to echo"}
        });
        fs::write(plugins_dir.join("echo.json"), plugin.to_string()).unwrap();

        let executor = ToolExecutor::new(ToolConfig::for_workspace(&workspace));
        Self {
            _tmp: tmp,
            workspace,
            executor,
        }
    }

    fn run(&mut self, tool: &str, args: serde_json::Value) -> jotline_core::tools::ToolOutput {
        self.executor.execute(tool, &args)
    }
}

#[test]
fn read_and_write_block_workspace_escape() {
    let mut h = Harness::new();
    for (tool, args) in [
        ("read_file", json!({"path": "../secret.txt"})),
        (
            "write_file",
            json!({"path": "../secret.txt", "content": "x"}),
        ),
        (
            "edit_file",
            json!({"path": "/etc/hosts", "old": "a", "new": "b"}),
        ),
        ("list_files", json!({"path": ".."})),
    ] {
        let output = h.run(tool, args);
        assert_eq!(output.status, ToolStatus::Error, "{tool}");
        assert!(
            output.text.contains("path escapes workspace root"),
            "{tool}: {}",
            output.text
        );
        assert!(!output.wrote_files);
    }
}

#[test]
fn edit_file_requires_a_unique_match() {
    let mut h = Harness::new();
    fs::write(h.workspace.join("a.rs"), "let x = 1;\nlet y = 1;\n").unwrap();

    let ambiguous = h.run(
        "edit_file",
        json!({"path": "a.rs", "old": "= 1;", "new": "= 2;"}),
    );
    assert_eq!(ambiguous.status, ToolStatus::Error);
    assert!(ambiguous.text.contains("matches 2 times"));

    let missing = h.run(
        "edit_file",
        json!({"path": "a.rs", "old": "let z", "new": "x"}),
    );
    assert!(missing.text.contains("not found"));

    let unique = h.run(
        "edit_file",
        json!({"path": "a.rs", "old": "let y = 1;", "new": "let y = 2;"}),
    );
    assert!(unique.is_ok(), "{}", unique.text);
    assert!(unique.wrote_files);
    assert_eq!(
        fs::read_to_string(h.workspace.join("a.rs")).unwrap(),
        "let x = 1;\nlet y = 2;\n"
    );

    let all = h.run(
        "edit_file",
        json!({"path": "a.rs", "old": "let", "new": "const", "replace_all": "true"}),
    );
    assert!(all.text.contains("replacements=2"));
}

#[test]
fn write_file_creates_parents_and_reports_kind() {
    let mut h = Harness::new();
    let created = h.run(
        "write_file",
        json!({"path": "src/new/mod.rs", "content": "pub fn a() {}\n"}),
    );
    assert!(created.text.contains("created"), "{}", created.text);
    let overwritten = h.run(
        "write_file",
        json!({"path": "src/new/mod.rs", "content": "x"}),
    );
    assert!(overwritten.text.contains("overwritten"));
}

#[test]
fn search_and_find_skip_ignored_directories() {
    let mut h = Harness::new();
    fs::create_dir_all(h.workspace.join("src")).unwrap();
    fs::create_dir_all(h.workspace.join("node_modules/pkg")).unwrap();
    fs::write(
        h.workspace.join("src/lib.rs"),
        "fn needle() {}\nfn other() {}\n",
    )
    .unwrap();
    fs::write(h.workspace.join("node_modules/pkg/index.js"), "needle\n").unwrap();
    fs::write(h.workspace.join("src/data.bin"), b"needle\0binary").unwrap();

    let found = h.run("search_files", json!({"pattern": "fn needle"}));
    assert!(found.is_ok());
    assert_eq!(found.text, "src/lib.rs:1: fn needle() {}");

    let insensitive = h.run(
        "search_files",
        json!({"pattern": "NEEDLE", "case_insensitive": true, "glob": "*.rs"}),
    );
    assert!(insensitive.text.contains("src/lib.rs:1"));

    let literal = h.run("search_files", json!({"pattern": "needle("}));
    assert!(literal.text.contains("searched it literally"));

    let none = h.run("search_files", json!({"pattern": "absent_symbol"}));
    assert!(none.text.starts_with("NO_MATCHES"));

    let files = h.run("find_files", json!({"pattern": "*.rs"}));
    assert_eq!(files.text, "src/lib.rs");
    let by_path = h.run("find_files", json!({"pattern": "src/*.bin"}));
    assert_eq!(by_path.text, "src/data.bin");

    let tree = h.run("list_files", json!({"max_depth": 1}));
    assert!(tree.text.contains("node_modules/ [skipped]"));
    assert!(tree.text.contains(".git/ [skipped]"));
}

#[test]
fn read_file_reports_ranges_and_continuation() {
    let mut h = Harness::new();
    let body = (1..=10)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(h.workspace.join("f.txt"), body).unwrap();
    let out = h.run(
        "read_file",
        json!({"path": "f.txt", "start_line": "3", "end_line": 4}),
    );
    assert!(out.text.starts_with("FILE: f.txt (lines 3-4 of 10)"));
    assert!(out.text.contains("    3: line 3"));
    assert!(!out.text.contains("line 5"));
    assert!(out.text.contains("read from start_line=5"));

    let bad = h.run("read_file", json!({"path": "f.txt", "start_line": "first"}));
    assert!(bad.text.contains("must be a non-negative integer"));
}

#[test]
fn apply_patch_rejects_unsafe_path() {
    let mut h = Harness::new();
    let patch = "diff --git a/../evil.txt b/../evil.txt\n--- a/../evil.txt\n+++ b/../evil.txt\n@@ -0,0 +1 @@\n+pwnd\n";
    let out = h.run("apply_patch", json!({"patch": patch}));
    assert!(out.text.contains("unsafe patch path"));
}

#[test]
fn git_tools_and_patch_workflow() {
    let mut h = Harness::new();
    let patch = "diff --git a/hello.txt b/hello.txt\n--- a/hello.txt\n+++ b/hello.txt\n@@ -1 +1 @@\n-one\n+two\n";
    let applied = h.run("apply_patch", json!({"patch": patch}));
    assert!(applied.text.contains("PATCH APPLIED"), "{}", applied.text);
    assert!(applied.wrote_files);
    assert_eq!(
        fs::read_to_string(h.workspace.join("hello.txt")).unwrap(),
        "two\n"
    );

    assert!(h
        .run("git_diff", json!({"path": "hello.txt"}))
        .text
        .contains("+two"));
    assert!(h.run("git_status", json!({})).text.contains("hello.txt"));
    let plan = h.run("git_commit_plan", json!({}));
    assert!(plan.text.contains("\"suggested_commit_message\""));

    let commit = h.run(
        "git_commit",
        json!({"message": "test: update hello", "add_all": true}),
    );
    assert!(commit.text.contains("EXIT_CODE: 0"), "{}", commit.text);
    assert!(h
        .run("git_log", json!({"count": 1}))
        .text
        .contains("test: update hello"));
    assert!(h.run("git_diff", json!({})).text.starts_with("NO_DIFF"));
}

#[test]
fn git_tools_respect_nested_workspace_scope() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().canonicalize().unwrap().join("repo");
    let workspace = repo.join("app");
    fs::create_dir_all(&workspace).unwrap();
    init_repo(&repo);
    fs::write(repo.join("root.txt"), "root\n").unwrap();
    fs::write(workspace.join("local.txt"), "local\n").unwrap();
    run_checked(&["git", "add", "."], &repo);
    run_checked(&["git", "commit", "-q", "-m", "init"], &repo);
    fs::write(repo.join("root.txt"), "root changed\n").unwrap();

    let mut executor = ToolExecutor::new(ToolConfig::for_workspace(&workspace));

    // Patch paths are relative to the nested workspace, not the repository root.
    let patch = "diff --git a/local.txt b/local.txt\n--- a/local.txt\n+++ b/local.txt\n@@ -1 +1 @@\n-local\n+local changed\n";
    let applied = executor.execute("apply_patch", &json!({"patch": patch}));
    assert!(applied.text.contains("PATCH APPLIED"), "{}", applied.text);
    assert_eq!(
        fs::read_to_string(workspace.join("local.txt")).unwrap(),
        "local changed\n"
    );

    let status = executor.execute("git_status", &json!({}));
    assert!(status.text.contains("local.txt"));
    assert!(!status.text.contains("root.txt"));
    let diff = executor.execute("git_diff", &json!({}));
    assert!(diff.text.contains("local changed"));
    assert!(!diff.text.contains("root changed"));
}

#[test]
fn memory_lifecycle() {
    let mut h = Harness::new();
    assert!(h
        .run(
            "memory_set",
            json!({"key": "goal", "value": "build jotline"})
        )
        .text
        .contains("MEMORY_SET"));
    assert!(h
        .run("memory_get", json!({"key": "goal"}))
        .text
        .contains("build jotline"));
    assert!(h
        .run("memory_search", json!({"query": "JOTLINE"}))
        .text
        .contains("goal"));
    assert!(h
        .run("memory_delete", json!({"key": "goal"}))
        .text
        .contains("MEMORY_DELETED"));
    assert!(h
        .run("memory_get", json!({"key": "goal"}))
        .text
        .contains("NOT_FOUND"));
}

#[test]
fn corrupt_memory_file_is_not_overwritten() {
    let mut h = Harness::new();
    let memory = h.workspace.join(".jotline/memory.json");
    fs::write(&memory, "[1, 2]").unwrap();
    let out = h.run("memory_set", json!({"key": "k", "value": 1}));
    assert_eq!(out.status, ToolStatus::Error);
    assert_eq!(fs::read_to_string(&memory).unwrap(), "[1, 2]");
}

#[test]
fn structured_patch_and_python_symbol_edits() {
    let mut h = Harness::new();
    let source = h.workspace.join("sample.py");
    fs::write(
        &source,
        "def greet():\n    return 'hi'\n\nclass A:\n    pass\n",
    )
    .unwrap();

    let overview = h.run("python_symbol_overview", json!({"path": "sample.py"}));
    assert!(overview.text.contains("\"name\":\"greet\""));
    assert!(overview.text.contains("\"name\":\"A\""));

    let replaced = h.run(
        "replace_python_symbol",
        json!({"path": "sample.py", "symbol": "greet", "symbol_type": "function", "code": "def greet():\n    return 'hello'"}),
    );
    assert!(replaced.is_ok(), "{}", replaced.text);
    let updated = fs::read_to_string(&source).unwrap();
    assert_eq!(
        updated,
        "def greet():\n    return 'hello'\n\nclass A:\n    pass\n"
    );

    let missing = h.run(
        "replace_python_symbol",
        json!({"path": "sample.py", "symbol": "nope", "code": "x"}),
    );
    assert!(missing.text.contains("top-level symbols: [greet, A]"));

    let patched = h.run(
        "structured_patch",
        json!({"operations": [
            {"type": "replace_text", "path": "sample.py", "old": "class A", "new": "class B"},
            {"type": "append_text", "path": "sample.py", "text": "\n# end\n"},
            {"type": "replace_lines", "path": "sample.py", "start_line": "1", "end_line": 1, "new_text": "def greet() -> str:"}
        ]}),
    );
    assert!(patched.is_ok(), "{}", patched.text);
    let updated = fs::read_to_string(&source).unwrap();
    assert!(updated.starts_with("def greet() -> str:\n"));
    assert!(updated.contains("class B"));
    assert!(updated.ends_with("# end\n"));
}

#[test]
fn plugin_and_parallel_tools() {
    let mut h = Harness::new();
    let names = h
        .executor
        .list_specs()
        .into_iter()
        .map(|s| s.name)
        .collect::<Vec<_>>();
    assert!(names.contains(&"plugin.echo".to_string()));

    let plugin = h.run("plugin.echo", json!({"value": "ok"}));
    assert!(plugin.text.contains("ECHO:ok"), "{}", plugin.text);
    assert!(plugin.executed);

    let parallel = h.run(
        "parallel_tools",
        json!({"calls": [
            {"tool": "read_file", "args": {"path": "hello.txt"}},
            {"tool": "memory_set", "args": {"key": "a", "value": 1}},
            {"tool": "memory_set", "args": {"key": "b", "value": 2}},
            {"tool": "write_file", "args": {"path": "p.txt", "content": "x"}}
        ], "max_workers": 4}),
    );
    assert!(parallel.is_ok(), "{}", parallel.text);
    assert!(parallel.wrote_files);
    let parsed: serde_json::Value = serde_json::from_str(&parallel.text).unwrap();
    let results = parsed["parallel_results"].as_array().unwrap();
    assert_eq!(results.len(), 4);
    assert_eq!(results[0]["tool"], "read_file");
    // Concurrent memory writes share a lock, so neither is lost.
    let memory = h.run("memory_get", json!({}));
    assert!(
        memory.text.contains("\"a\"") && memory.text.contains("\"b\""),
        "{}",
        memory.text
    );

    let nested = h.run(
        "parallel_tools",
        json!({"calls": [{"tool": "parallel_tools", "args": {}}]}),
    );
    assert_eq!(nested.status, ToolStatus::Error);
}

#[test]
fn unknown_tool_and_bad_args_are_errors() {
    let mut h = Harness::new();
    assert!(h
        .run("teleport", json!({}))
        .text
        .contains("unknown tool 'teleport'"));
    assert!(h
        .run("read_file", json!({}))
        .text
        .contains("missing required argument 'path'"));
    assert!(h
        .run("read_file", json!({"path": 5}))
        .text
        .contains("must be a string"));
    let not_object = h.executor.execute("read_file", &json!([1]));
    assert!(not_object.text.contains("args must be a JSON object"));
}

struct DenyAll;

impl Approver for DenyAll {
    fn approve(&self, _request: &str) -> bool {
        false
    }
}

#[test]
fn approver_can_deny_side_effects() {
    let tmp = TempDir::new().unwrap();
    let mut config = ToolConfig::for_workspace(tmp.path());
    config.approver = Some(Arc::new(DenyAll));
    let mut executor = ToolExecutor::new(config);
    assert!(!executor.auto_mode());

    let out = executor.execute("run_shell", &json!({"command": "touch should_not_exist"}));
    assert_eq!(out.status, ToolStatus::Denied);
    assert!(out.text.starts_with("DENIED:"));
    assert!(!out.executed);
    assert!(!tmp.path().join("should_not_exist").exists());
}

#[test]
fn rules_add_writes_user_rules() {
    let mut h = Harness::new();
    let out = h.run("rules_add", json!({"rule": "Prefer early returns"}));
    assert!(
        out.text.contains("RULE_ADDED: .jotline/rules/user.md"),
        "{}",
        out.text
    );
    let empty = h.run("rules_add", json!({"rule": "  "}));
    assert_eq!(empty.status, ToolStatus::Error);
}

#[test]
fn run_shell_reports_exit_code_and_counts_as_execution() {
    let mut h = Harness::new();
    let ok = h.run("run_shell", json!({"command": "echo hi"}));
    assert!(ok.text.starts_with("EXIT_CODE: 0\nSTDOUT:\nhi"));
    assert!(ok.executed);
    let failing = h.run("run_shell", json!({"command": "echo oops >&2; exit 3"}));
    assert!(failing.text.contains("EXIT_CODE: 3"));
    assert!(failing.text.contains("oops"));
}

#[test]
fn edit_file_not_found_shows_the_current_text() {
    let mut h = Harness::new();
    fs::write(
        h.workspace.join("calc.py"),
        "import math\n\ndef add(a, b):\n    return a - b\n",
    )
    .unwrap();
    // The model invented the body: the first line exists, the rest does not.
    let out = h.run(
        "edit_file",
        json!({"path": "calc.py", "old": "def add(a, b):\n    pass", "new": "x"}),
    );
    assert_eq!(out.status, ToolStatus::Error);
    assert!(
        out.text.contains("Current text near line 3"),
        "{}",
        out.text
    );
    assert!(out.text.contains("    4:     return a - b"), "{}", out.text);

    // Nothing matches at all in a short file: the whole file is shown.
    let out = h.run(
        "edit_file",
        json!({"path": "calc.py", "old": "class Missing", "new": "x"}),
    );
    assert!(out.text.contains("Current file content"), "{}", out.text);
    assert!(out.text.contains("    1: import math"));
}

#[test]
fn edit_file_accepts_double_escaped_line_breaks() {
    let mut h = Harness::new();
    fs::write(
        h.workspace.join("calc.py"),
        "def add(a, b):\n    return a - b\n",
    )
    .unwrap();
    // Literal backslash-n, as sent by some small models.
    let out = h.run(
        "edit_file",
        json!({"path": "calc.py", "old": "def add(a, b):\\n    return a - b", "new": "def add(a, b):\\n    return a + b"}),
    );
    assert!(out.is_ok(), "{}", out.text);
    assert!(out.text.contains("interpreted"));
    assert_eq!(
        fs::read_to_string(h.workspace.join("calc.py")).unwrap(),
        "def add(a, b):\n    return a + b\n"
    );

    // A file that really contains backslash-n is edited literally.
    fs::write(h.workspace.join("s.py"), "s = 'a\\nb'\n").unwrap();
    let out = h.run(
        "edit_file",
        json!({"path": "s.py", "old": "'a\\nb'", "new": "'c'"}),
    );
    assert!(out.is_ok(), "{}", out.text);
    assert!(!out.text.contains("interpreted"));
    assert_eq!(
        fs::read_to_string(h.workspace.join("s.py")).unwrap(),
        "s = 'c'\n"
    );
}
