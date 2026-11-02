# Configuration

jotline is configured with command-line flags. Flags with an environment variable can be set once in your shell profile. Run `jotline --help` for the same list.

## Model

| Flag | Environment | Default | Meaning |
|---|---|---|---|
| `--provider` | `JOTLINE_PROVIDER` | `ollama` | `ollama` (native `/api/chat`) or `openai` (any `/v1/chat/completions` server). |
| `--model` | `JOTLINE_MODEL` | `qwen3:8b` | Model for every role unless overridden. |
| `--planner-model` | `JOTLINE_PLANNER_MODEL` | `--model` | Model for the first call of each top-level task, which produces the plan. |
| `--coder-model` | `JOTLINE_CODER_MODEL` | `--model` | Model for every later call: tools, edits, and the handoff. |
| `--endpoint` | `JOTLINE_ENDPOINT` | `http://127.0.0.1:11434/api/chat` (ollama), `http://127.0.0.1:8000/v1/chat/completions` (openai) | Chat endpoint URL. |
| `--api-key` | `OPENAI_API_KEY` | none | Bearer token for OpenAI-compatible servers. |
| `--temperature` | | `0.1` | Sampling temperature. Low values make JSON output more reliable. |
| `--request-timeout` | | `600` | Seconds allowed for one model request. |

## Task loop

| Flag | Default | Meaning |
|---|---|---|
| `--max-steps` | `24` | Model calls allowed per task. |
| `--max-history` | `80` | Most recent messages sent per call. Lower it for models with small context windows; the current request is always kept. |
| `--max-delegation-depth` | `2` | Nesting limit for sub-agents; `0` disables delegation. |

## Workspace and safety

| Flag | Default | Meaning |
|---|---|---|
| `--workspace`, `-w` | current directory | Root that tools cannot leave. Change it during a session with `/scope use <path>`. |
| `--ask-permissions` | off | Ask before `run_shell`, plugins, and `git_commit`. Without it jotline runs them immediately (auto mode). |
| `--shell-timeout` | `120` | Seconds before a shell, git, or plugin process and its children are killed. |

## State and rules

| Flag | Default | Meaning |
|---|---|---|
| `--session-file` | none | Load this session at start (if it exists) and save after every turn. Relative to the workspace. |
| `--fresh-session` | off | Ignore an existing session file; it is still overwritten as you work. |
| `--memory-file` | `.jotline/memory.json` | Persistent memory used by the memory tools. |
| `--plugins-dir` | `.jotline/plugins` | Directory of plugin JSON files. |
| `--no-builtin-rules` | off | Skip the built-in rules. |
| `--no-project-rules` | off | Skip `JOTLINE.md`, `AGENTS.md`, and `.jotline/rules/*.md`. |

## Output

| Flag | Meaning |
|---|---|
| `--prompt`, `-p` | Run one task and exit. The exit status is 0 when the task ended with a `final` handoff and 1 otherwise (model error, step limit). |
| `--quiet`, `-q` | Hide intent, plan, and tool events; show only the spinner and the result. |
| `NO_COLOR=1` | Disable colors. Colors and the spinner are also off when stdout is not a terminal. |

## The `.jotline` directory

jotline keeps per-workspace state in `.jotline/` inside the workspace:

```text
.jotline/
  memory.json      persistent memory (memory_* tools)
  session.json     a conventional --session-file location
  plugins/*.json   plugin tools
  rules/*.md       project rules; user.md receives /rules add and rules_add
```

Add `.jotline/` to your project's `.gitignore` unless you want to share plugins or rules (see [rules](RULES.md#committing-rules)).
