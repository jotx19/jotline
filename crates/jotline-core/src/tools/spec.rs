use serde::Serialize;

/// What a tool does to the outside world. The orchestrator uses it to track whether files
/// changed and whether a verification command ran afterwards.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// Reads the workspace only.
    Read,
    /// Modifies workspace files.
    Write,
    /// Executes a process (shell command or plugin); counts as a verification attempt.
    Execute,
    /// Changes jotline or repository state without editing source files (memory, rules, commits).
    State,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArgSpec {
    pub name: String,
    pub required: bool,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub args: Vec<ArgSpec>,
    pub effect: Effect,
}

fn req(name: &str, description: &str) -> ArgSpec {
    ArgSpec {
        name: name.to_string(),
        required: true,
        description: description.to_string(),
    }
}

fn opt(name: &str, description: &str) -> ArgSpec {
    ArgSpec {
        name: name.to_string(),
        required: false,
        description: description.to_string(),
    }
}

fn tool(name: &str, effect: Effect, description: &str, args: Vec<ArgSpec>) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        args,
        effect,
    }
}

pub(crate) fn builtin_specs() -> Vec<ToolSpec> {
    use Effect::{Execute, Read, State, Write};
    vec![
        tool(
            "list_files",
            Read,
            "List files and directories as a tree. Skips .git, target, node_modules and similar.",
            vec![
                opt("path", "directory relative to the workspace (default '.')"),
                opt("max_depth", "depth limit, 0-8 (default 3)"),
            ],
        ),
        tool(
            "find_files",
            Read,
            "Find files whose name (or relative path, if the pattern has '/') matches a glob with * and ?.",
            vec![
                req("pattern", "glob such as '*.rs' or 'src/*/mod.rs'"),
                opt("path", "directory to search (default '.')"),
                opt("max_results", "default 200"),
            ],
        ),
        tool(
            "search_files",
            Read,
            "Search file contents with a regular expression. Returns path:line: text.",
            vec![
                req("pattern", "regex (invalid regex is searched literally)"),
                opt("path", "file or directory to search (default '.')"),
                opt("glob", "only files whose name matches, e.g. '*.py'"),
                opt("case_insensitive", "boolean (default false)"),
                opt("max_results", "default 100"),
            ],
        ),
        tool(
            "read_file",
            Read,
            "Read a text file with line numbers.",
            vec![
                req("path", "file relative to the workspace"),
                opt("start_line", "1-based first line (default 1)"),
                opt("end_line", "1-based last line, inclusive (default 250)"),
            ],
        ),
        tool(
            "write_file",
            Write,
            "Create a file or overwrite it completely. Prefer edit_file for existing files.",
            vec![
                req("path", "file relative to the workspace"),
                req("content", "the complete file content"),
            ],
        ),
        tool(
            "edit_file",
            Write,
            "Replace exact text in a file. 'old' must match exactly once unless replace_all is true.",
            vec![
                req("path", "file relative to the workspace"),
                req("old", "exact existing text, including indentation; add context to make it unique"),
                req("new", "replacement text"),
                opt("replace_all", "boolean: replace every occurrence (default false)"),
            ],
        ),
        tool(
            "structured_patch",
            Write,
            "Apply several edit operations in order.",
            vec![req(
                "operations",
                "list of {type:'replace_text',path,old,new,count?} | {type:'replace_lines',path,start_line,end_line,new_text} | {type:'append_text',path,text}",
            )],
        ),
        tool(
            "apply_patch",
            Write,
            "Apply a unified diff with git apply (workspace must be a git repository).",
            vec![req("patch", "unified diff text with a/ and b/ paths")],
        ),
        tool(
            "python_symbol_overview",
            Read,
            "List top-level Python functions and classes with line ranges.",
            vec![req("path", ".py file relative to the workspace")],
        ),
        tool(
            "replace_python_symbol",
            Write,
            "Replace one top-level Python function or class by name.",
            vec![
                req("path", ".py file relative to the workspace"),
                req("symbol", "function or class name"),
                opt("symbol_type", "'function' or 'class'"),
                req("code", "the complete replacement definition"),
            ],
        ),
        tool(
            "run_shell",
            Execute,
            "Run a bash command in the workspace root and return exit code, stdout, and stderr. Use it to build, test, lint, and format.",
            vec![req("command", "shell command")],
        ),
        tool(
            "git_status",
            Read,
            "Show branch and short status for the workspace.",
            vec![],
        ),
        tool(
            "git_diff",
            Read,
            "Show the diff for the workspace or one path.",
            vec![
                opt("path", "path relative to the workspace"),
                opt("staged", "boolean (default false)"),
                opt("unified", "context lines, 0-20 (default 3)"),
            ],
        ),
        tool(
            "git_log",
            Read,
            "Show recent commits.",
            vec![opt("count", "1-100 (default 10)")],
        ),
        tool(
            "git_commit_plan",
            Read,
            "Summarize pending changes and suggest a Conventional Commit message.",
            vec![],
        ),
        tool(
            "git_commit",
            State,
            "Create a commit of workspace changes.",
            vec![
                req("message", "commit message"),
                opt("add_all", "boolean: stage all workspace changes first (default false)"),
            ],
        ),
        tool(
            "memory_get",
            Read,
            "Read persistent memory; omit key to read everything.",
            vec![opt("key", "memory key")],
        ),
        tool(
            "memory_set",
            State,
            "Store a persistent key/value note for later sessions.",
            vec![req("key", "memory key"), req("value", "any JSON value")],
        ),
        tool(
            "memory_delete",
            State,
            "Delete a persistent memory key.",
            vec![req("key", "memory key")],
        ),
        tool(
            "memory_search",
            Read,
            "Search persistent memory keys and values (case-insensitive).",
            vec![req("query", "search text"), opt("limit", "1-100 (default 10)")],
        ),
        tool(
            "rules_add",
            State,
            "Save a coding rule the user wants followed from now on (e.g. 'always use pytest'). Appends to .jotline/rules/user.md.",
            vec![req("rule", "one imperative sentence, at most 500 characters")],
        ),
        tool(
            "parallel_tools",
            Read,
            "Run independent tool calls concurrently, e.g. several reads or searches.",
            vec![
                req("calls", "list of {tool, args}"),
                opt("max_workers", "1-8 (default 4)"),
            ],
        ),
        tool(
            "reload_plugins",
            Read,
            "Reload plugin tools from .jotline/plugins.",
            vec![],
        ),
    ]
}

/// Renders specs as compact text; far fewer tokens than JSON, which matters for small context
/// windows. Required arguments are marked with `*`.
pub(crate) fn render(specs: &[ToolSpec]) -> String {
    let mut out = String::new();
    for spec in specs {
        out.push_str(&format!("- {}: {}\n", spec.name, spec.description));
        for arg in &spec.args {
            let marker = if arg.required { "*" } else { "" };
            out.push_str(&format!("    {}{marker}: {}\n", arg.name, arg.description));
        }
    }
    out
}
