# Tools

The model acts on the workspace only through these tools. Paths are relative to the workspace root; absolute paths, `..`, and symlinks that leave the root are rejected with `ERROR: path escapes workspace root`. List them at runtime with `/tools`.

Arguments marked `*` are required. Numbers and booleans may be sent as JSON values or as strings (`"10"`, `"true"`).

## Reading and searching

| Tool | Arguments | Result |
|---|---|---|
| `list_files` | `path` (default `.`), `max_depth` (0-8, default 3) | Tree of paths; directories end in `/`. `.git`, `target`, `node_modules`, `__pycache__`, `.venv`, `venv`, `.jotline`, `.idea`, `.next`, `dist`, and `.mypy_cache` are shown as `[skipped]`. Stops at 1,500 lines. |
| `find_files` | `pattern*` glob with `*` and `?`, `path`, `max_results` (default 200) | Matching paths, one per line, or `NO_MATCHES:`. A pattern containing `/` matches the relative path; otherwise the file name. |
| `search_files` | `pattern*` regex, `path`, `glob`, `case_insensitive`, `max_results` (default 100, max 500) | `path:line: text` lines, or `NO_MATCHES:`. An invalid regex is searched literally with a `NOTE:`. Skips ignored directories, files over 1 MB, and binary files. |
| `read_file` | `path*`, `start_line` (default 1), `end_line` (default 250) | `FILE: <path> (lines a-b of n)` followed by numbered lines, at most 1,000 per call, and a hint with the next `start_line` when more remain. |
| `python_symbol_overview` | `path*` (`.py`) | JSON list of top-level functions and classes with line ranges, including decorators. |

## Editing

| Tool | Arguments | Result |
|---|---|---|
| `edit_file` | `path*`, `old*`, `new*`, `replace_all` | `EDITED: <path> (replacements=n)`. Fails when `old` matches more than once without `replace_all`, telling the model to add context. When `old` is not found, the error includes the current lines around the best match for its first line (or the whole file, up to 60 lines) so the model can copy the exact text. An `old` that only matches after turning literal `\n` into line breaks is accepted and reported. This is the preferred edit tool. |
| `write_file` | `path*`, `content*` | `WROTE: <path> (n bytes, created\|overwritten)`. Creates parent directories. |
| `structured_patch` | `operations*`: `replace_text {path, old, new, count?}`, `replace_lines {path, start_line, end_line, new_text}`, `append_text {path, text}` | One `op#i` line per operation. Fails only if every operation fails. |
| `apply_patch` | `patch*` unified diff | `PATCH APPLIED` and short git status. Requires a git repository; rejects any header path that would leave the workspace. Paths are relative to the workspace, even when it is nested in a larger repository. |
| `replace_python_symbol` | `path*`, `symbol*`, `symbol_type` (`function`/`class`), `code*` | Replaces the symbol's line range, decorators included. Lists the available symbols when the name is not found. |

## Running commands

| Tool | Arguments | Result |
|---|---|---|
| `run_shell` | `command*` | `EXIT_CODE`, `STDOUT`, `STDERR` (each cut at 12,000 characters). Runs `bash -lc` in the workspace root. Killed with its whole process group after `--shell-timeout` seconds. Counts as verification for the verification gate. Asks first with `--ask-permissions`. |

## Git

All git tools are limited to the workspace with a `-- .` pathspec.

| Tool | Arguments | Result |
|---|---|---|
| `git_status` | none | Branch and short status. |
| `git_diff` | `path`, `staged`, `unified` (0-20) | Diff, or `NO_DIFF:`. |
| `git_log` | `count` (1-100, default 10) | One-line log. |
| `git_commit_plan` | none | JSON with staged, unstaged, untracked, changed paths, and a suggested Conventional Commit message. |
| `git_commit` | `message*`, `add_all` | Commit output. Asks first with `--ask-permissions`. |

## Memory and rules

| Tool | Arguments | Result |
|---|---|---|
| `memory_get` | `key` | One entry, or all memory when `key` is omitted. |
| `memory_set` | `key*`, `value*` (any JSON) | `MEMORY_SET:` |
| `memory_delete` | `key*` | `MEMORY_DELETED:` or `NOT_FOUND:` |
| `memory_search` | `query*`, `limit` | Matching entries (case-insensitive over keys and values). |
| `rules_add` | `rule*` (one sentence, at most 500 characters) | `RULE_ADDED:`; appends to `.jotline/rules/user.md` and reloads rules. See [rules](RULES.md). |

Memory lives in `.jotline/memory.json` (override with `--memory-file`). A memory file that is not a JSON object is reported as an error and never overwritten.

## Orchestration

| Tool | Arguments | Result |
|---|---|---|
| `parallel_tools` | `calls*` `[{tool, args}]`, `max_workers` (1-8, default 4) | JSON `parallel_results` with `index`, `tool`, `args`, `ok`, `result` for each call, in call order. Runs sequentially when approvals are enabled. Cannot nest itself or `reload_plugins`. |
| `reload_plugins` | none | Names of loaded plugins. |

## Result prefixes

Results start with a stable prefix so the model and tests can tell outcomes apart: `ERROR:` (invalid call or failed operation), `DENIED:` (approval refused), `WROTE:`, `EDITED:`, `REPLACED:`, `APPENDED:`, `PATCH APPLIED`, `NO_MATCHES:`, `NO_DIFF:`, `NOT_FOUND:`, `MEMORY_SET:`, `MEMORY_DELETED:`, `RULE_ADDED:`, `EXIT_CODE:`, and `PLUGIN:`.

## Plugins

A plugin is a JSON file in `.jotline/plugins/` (override with `--plugins-dir`) that turns an external command into a tool named `plugin.<name>`:

```json
{
  "name": "lint",
  "description": "Run the project linter on one file and report problems",
  "command": ["python3", "scripts/lint_one.py"],
  "args_schema": { "path": "file to lint, relative to the workspace" }
}
```

`name` must be 1-64 characters of letters, digits, `_`, or `-`. `command` is executed directly, not through a shell, in the workspace root with `JOTLINE_WORKSPACE` set. The call's `args` object is written to the command's stdin as JSON, and its exit code and output are returned like `run_shell`. Invalid plugin files are skipped. Plugins count as verification for the verification gate and ask first with `--ask-permissions`. Reload with `/plugins` after editing. A working example is in [examples/plugins/](../examples/plugins/).
