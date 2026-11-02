# jotline

jotline is a small, local-first coding agent for open models. It runs in your terminal, talks to [Ollama](https://ollama.com) or any OpenAI-compatible server (vLLM, llama.cpp, LM Studio, LocalAI), and edits code inside one workspace directory through a typed tool protocol. No cloud account, telemetry, or background service is required.

jotline is written for models with small context windows and imperfect JSON. Instead of trusting the model to behave, the orchestrator classifies every request, gives the model a short playbook for that kind of task, insists on a plan first, corrects malformed replies with specific messages, and refuses to accept "done" after a code change until a verification command has run.

```text
╭ jotline 0.3.0 ────────────────────────────────────────────────────────────╮
│ scope     /home/me/projects/api                                           │
│ models    qwen3:8b via ollama (planner qwen3:8b, coder qwen3:8b)          │
│ rules     4 built-in, 2 project   (/rules to inspect, /rules add <rule>)  │
╰───────────────────────────────────────────────────────────────────────────╯
✦ main  api  qwen3:8b  auto › fix the off-by-one in pagination and add a test
✦ intent bugfix
```

## Features

- **Intent routing.** Each request is classified as `feature`, `bugfix`, `refactor`, `test`, `review`, `docs`, `question`, `ops`, or `general`. The intent selects a step-by-step playbook (for example: reproduce, find root cause, minimal fix, regression test, run tests) and decides whether verification is required. The model can override the intent in its plan. Try `/intent <request>` to see the classification.
- **Typed action protocol.** The model replies with one JSON action: `plan`, `tool`, `parallel_tools`, `delegate`, or `final`. Replies wrapped in prose, markdown fences, or `<think>` blocks are still understood; unknown types and missing fields get a precise correction.
- **Verification gate.** For intents that change code, a `final` answer after file edits with no command run since is sent back once with a request to build or test. The handoff shows what was verified and warns when nothing was.
- **Markdown rules.** Built-in coding principles, code-generation, testing, and security rules are compiled in. Your project rules in `JOTLINE.md`, `AGENTS.md`, and `.jotline/rules/*.md` are added on top and win on conflict. Say "from now on, always ..." and the agent saves the rule with `rules_add`; or run `/rules add <rule>` yourself. See [rules](docs/RULES.md).
- **Code-oriented tools.** `search_files` (regex), `find_files` (glob), `read_file`, unique-match `edit_file`, `write_file`, `structured_patch`, `apply_patch`, Python symbol edits, `run_shell`, scoped git tools, persistent memory, parallel batches, and JSON-declared plugins. See [tools](docs/TOOLS.md).
- **Workspace boundary.** Every path is checked against the workspace root, including symlinks. Git tools only see the workspace, even inside a larger repository.
- **Sub-agents.** Named agents and bounded delegation for self-contained subtasks.
- **Sessions.** Save and resume conversations per workspace.

## Install

Install a Rust toolchain (1.85 or newer) with [rustup](https://rustup.rs), then build from a checkout:

```bash
cargo install --git https://github.com/jotx19/jotline jotline --locked
# or, from a checkout:
cargo install --path crates/jotline --locked
```

The release workflow attaches prebuilt Linux and macOS binaries to each GitHub release.

jotline runs commands with `bash`, so it supports Linux and macOS. On Windows, use WSL.

## Quickstart with Ollama

```bash
ollama pull qwen3:8b          # any chat model works; coder models do best
cd path/to/your/project
jotline                       # interactive
jotline -p "explain how configuration is loaded"   # one-shot
```

Use another model with `--model` (or `JOTLINE_MODEL`). If the model is missing, jotline tells you the `ollama pull` command to run.

| Hardware | Suggested starting models |
|---|---|
| 8 GB RAM / no GPU | `qwen3:4b`, `qwen2.5-coder:3b` |
| 16 GB RAM or 8 GB VRAM | `qwen3:8b`, `qwen2.5-coder:7b` |
| 24 GB+ VRAM | `qwen3:32b`, `qwen2.5-coder:32b`, `devstral` |

Split planning and coding across models with `--planner-model` and `--coder-model`.

### OpenAI-compatible servers

```bash
jotline --provider openai \
  --endpoint http://127.0.0.1:8000/v1/chat/completions \
  --model Qwen/Qwen2.5-Coder-7B-Instruct
```

Set `OPENAI_API_KEY` when the server needs a key.

## Using jotline

Describe the task in plain language. jotline prints the detected intent, the model's plan, each tool call, and a final handoff with completed work, verification, and anything missed.

| Command | Effect |
|---|---|
| `/help` | list commands |
| `/status` | scope, models, rules, plugins, and the last task (intent, steps, files changed, verified) |
| `/plan` | the last recorded plan |
| `/intent <request>` | classification scores and the playbook a request would get |
| `/rules`, `/rules add <rule>`, `/rules show <name>`, `/rules reload` | inspect and extend rules |
| `/tools` | available tools, including plugins |
| `/scope`, `/scope use <path>` | show or change the workspace boundary |
| `/agents`, `/agent new\|use\|reset <name>`, `/agent run <name> <prompt>` | named agents |
| `/reset` | clear the active agent's conversation |
| `/save [path]`, `/load [path]` | save or load a session |
| `/plugins` | reload plugin tools |
| `/exit` | quit |

Every flag and environment variable is listed in [configuration](docs/CONFIGURATION.md). The most useful ones:

```bash
jotline --ask-permissions                      # confirm shell commands, plugins, commits
jotline --session-file .jotline/session.json   # resume across runs
jotline --workspace ./services/api             # narrow the boundary
jotline --no-project-rules                     # ignore JOTLINE.md / AGENTS.md / .jotline/rules
```

By default jotline runs tool calls **without asking** (auto mode). It cannot touch files outside the workspace, but `run_shell` executes arbitrary commands with your permissions. Use `--ask-permissions` in repositories you do not trust, and read [SECURITY.md](SECURITY.md).

## Project rules

Create `JOTLINE.md` in your project root:

```markdown
# Project rules

- Python 3.12, formatted with ruff; run `ruff check . && pytest -q` to verify.
- Use pathlib, never os.path.
- Public functions need type hints and a docstring.
```

jotline reads it at startup and after `/rules reload`. Larger rule sets belong in `.jotline/rules/*.md`; copy [examples/](examples/) to start. Details and limits are in [docs/RULES.md](docs/RULES.md).

## How it works

```text
request ─► classify intent ─► task brief + playbook ─► model
                                                        │
          ┌──────────── correction ◄── invalid JSON ◄───┤
          │                                             ▼
          └──► plan ─► tool / parallel_tools / delegate ─► results ─► model ...
                                                        │
                                   final ─► verification gate ─► handoff
```

The [architecture guide](docs/ARCHITECTURE.md) describes the loop, the action protocol, the verification gate, and how context is trimmed for small models.

## Limitations

- Quality depends on the model. 7-8B models handle focused edits well; large multi-file features need a larger model or smaller steps.
- Tool calling uses a JSON protocol in plain chat, not native function-calling APIs, so any chat model works but very small models may need several corrections.
- Responses are not streamed; the spinner shows elapsed time while the model thinks.
- `run_shell` needs `bash`; Windows is supported only through WSL.
- Only Python has symbol-aware editing; other languages use text edits and patches.

## Support and discussion

Report bugs and request features in the [issue tracker](https://github.com/jotx19/jotline/issues). Every issue gets its own chat room on [JotPod](https://jotpod.jotx.space), a GitHub App that opens a private, real-time chat pod per issue for the repository's collaborators. Use the pod linked on an issue to discuss it live, with threaded replies; it becomes read-only when the issue closes. Keep decisions and reproduction steps on the issue itself so they stay searchable.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Read [CONTRIBUTING.md](CONTRIBUTING.md), the [developer guide](docs/DEVELOPMENT.md), and the [quality process](docs/QUALITY.md) before opening a pull request. Agent-assisted contributions follow [AGENTS.md](AGENTS.md).

## License

MIT © [jotx19](https://github.com/jotx19). See [LICENSE](LICENSE).
