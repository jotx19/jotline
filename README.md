<div align="center">

# jotline

**A local-first coding agent for open models.**

Plan, edit, and verify code from your terminal with Ollama or any OpenAI-compatible server. No cloud account, no telemetry.

[![CI](https://github.com/jotx19/jotline/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jotx19/jotline/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/jotx19/jotline?sort=semver&display_name=tag&label=release)](https://github.com/jotx19/jotline/releases/latest)
[![Release workflow](https://github.com/jotx19/jotline/actions/workflows/release.yml/badge.svg)](https://github.com/jotx19/jotline/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/github/license/jotx19/jotline?color=blue)](LICENSE)
[![MSRV 1.85](https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust)](Cargo.toml)
[![Platforms](https://img.shields.io/badge/platform-linux%20%7C%20macOS-lightgrey)](#install)
[![Works with Ollama](https://img.shields.io/badge/works%20with-Ollama-black)](https://ollama.com)
[![Chat on JotPod](https://img.shields.io/badge/chat-JotPod-5865F2)](https://jotpod.jotx.space)

[Install](#install) · [Quickstart](#quickstart) · [Interactive UI](#interactive-ui) · [Usage](#usage) · [Rules](#project-rules) · [Docs](#documentation) · [Contributing](#contributing)

</div>

> [!NOTE]
> **jotline is source-only for now.** Build it locally with the [steps below](#install); it takes about two minutes. One-command installs through **npm** and **Homebrew** are coming soon.

---

jotline is a coding agent built for **small local models**: 7-8B parameters, short context windows, and imperfect JSON. Instead of trusting the model to behave, the orchestrator does the disciplined parts in Rust: it classifies every request, hands the model a short playbook for that kind of task, insists on a plan first, corrects malformed replies with precise messages, keeps every file operation inside your workspace, and refuses to accept "done" after a code change until a build or test command has actually run.

```text
╭───────────────────────────────────────────────────────────────╮
│ ✻ jotline 2.0.0                                               │
│   ~/jotline-playground                                        │
│   qwen2.5-coder:7b · ollama · auto-approve                    │
│   rules: 4 built-in, 1 project · 23 tools                     │
╰───────────────────────────────────────────────────────────────╯

> the test in test_calc.py fails, fix the bug in calc.py

  ✻ intent bugfix

● Plan · Find why test_add fails and fix add() in calc.py
  ⎿  3 steps · 1. Run the test to see the error

● Bash(python3 -c "import test_calc; test_calc.test_add()") 82ms
  ⎿  exit 1 · 5 lines of output

● Update(calc.py) 2ms
  ⎿  Updated calc.py with 1 addition and 1 removal
     1   def add(a, b):
     2 -     return a - b
     2 +     return a + b

● Bash(python3 -c "import test_calc; test_calc.test_add()") 59ms
  ⎿  exit 0 · 1 line of output

● Fixed add() in calc.py: it subtracted instead of adding.
    verified
    ✓ test_calc.test_add -> ok
  1 file changed (+1 −1) · 5 steps · bugfix
╭───────────────────────────────────────────────────────────────╮
│ > Describe a task, or type / for commands                     │
╰───────────────────────────────────────────────────────────────╯
  /help · ↑↓ select · ctrl+o expand all      auto · qwen2.5-coder:7b
```

Every tool call is a live row you can select and expand, every file change is shown as a diff, and the agent must run a check before it claims a fix. See [Interactive UI](#interactive-ui).

## Table of contents

- [Features](#features)
- [Install](#install)
- [Quickstart](#quickstart)
- [Usage](#usage)
- [Interactive UI](#interactive-ui)
- [Configuration](#configuration)
- [Project rules](#project-rules)
- [Tools](#tools)
- [How it works](#how-it-works)
- [Project layout](#project-layout)
- [Development](#development)
- [CI and releases](#ci-and-releases)
- [Support and discussion](#support-and-discussion)
- [Security](#security)
- [Limitations](#limitations)
- [Contributing](#contributing)
- [License](#license)

## Features

| | |
|---|---|
| 🧭 **Intent routing** | Every request is classified as `feature`, `bugfix`, `refactor`, `test`, `review`, `docs`, `question`, `ops`, or `general`, and gets a matching step-by-step playbook (for a bug fix: reproduce, find the root cause, minimal fix, regression test, run tests). The model may override the intent in its plan. `/intent <request>` shows the scoring. |
| 🧩 **Typed action protocol** | The model answers with one JSON action: `plan`, `tool`, `parallel_tools`, `delegate`, or `final`. Replies wrapped in prose, code fences, or `<think>` blocks still parse; unknown types and missing fields get a specific correction instead of a crash. |
| ✅ **Verification gate** | For intents that change code, a final answer after file edits with no command run since is sent back once with a request to build or test. The handoff lists what was verified and warns when nothing was. |
| 📜 **Markdown rules** | Built-in coding-principle, code-generation, testing, and security rules, plus your `JOTLINE.md`, `AGENTS.md`, and `.jotline/rules/*.md`. Say "from now on, always ..." and the agent saves the rule for you. |
| 🛠 **Code-oriented tools** | Regex `search_files`, glob `find_files`, ranged `read_file`, unique-match `edit_file`, `write_file`, `structured_patch`, `apply_patch`, Python symbol edits, `run_shell`, scoped git tools, memory, parallel batches, and JSON plugins. |
| 🔒 **Workspace boundary** | Every path is checked against the workspace root, symlinks included. Git tools only see the workspace, even inside a larger repository. Optional per-call approvals. |
| 🧵 **Sub-agents and sessions** | Named agents, bounded delegation for self-contained subtasks, and resumable session files. |
| 🪶 **Small and portable** | One ~5 MB binary. Runtime dependencies: `bash`, `git`, and a model server. |

## Install

jotline is distributed as source code for now. The steps below clone the repository, build the agent and the terminal UI, and run them on a small demo project so you can watch the agent fix a real bug.

> One-command installs are on the way: **npm** (the terminal UI) and **Homebrew** (the agent and the UI together). Until they ship, build from source.

### 1. Prerequisites

| Tool | Version | Install | Check |
|---|---|---|---|
| Rust | 1.85 or newer | [rustup.rs](https://rustup.rs) | `cargo --version` |
| Node.js | 18.18 or newer | [nodejs.org](https://nodejs.org) or `brew install node` | `node --version` |
| Ollama | any recent | [ollama.com](https://ollama.com) or `brew install ollama` | `ollama --version` |
| git, bash, python3 | any | preinstalled on macOS and most Linux | `git --version` |

jotline runs on macOS and Linux. On Windows, use WSL.

### 2. Clone and build

```bash
git clone https://github.com/jotx19/jotline.git
cd jotline

# the agent (Rust)
cargo build --release -p jotline

# the interactive terminal UI (TypeScript)
cd packages/tui
npm ci
npm run build
cd ../..
```

The agent binary is now at `target/release/jotline` and the UI at `packages/tui/dist/src/cli.js`.

### 3. Get a model

```bash
ollama pull qwen2.5-coder:7b     # about 4.7 GB; see "Choosing a model" for others
```

### 4. Try it on a demo project

This creates a tiny Python project with a bug, then starts jotline in it:

```bash
JOTLINE_REPO="$PWD"
mkdir -p ~/jotline-playground && cd ~/jotline-playground
git init -q
printf 'def add(a, b):\n    return a - b\n' > calc.py
printf 'from calc import add\n\ndef test_add():\n    assert add(2, 3) == 5\n' > test_calc.py
printf -- '- Verify with `python3 -c "import test_calc; test_calc.test_add(); print(\x27ok\x27)"`.\n' > JOTLINE.md

JOTLINE_BIN="$JOTLINE_REPO/target/release/jotline" \
  node "$JOTLINE_REPO/packages/tui/dist/src/cli.js" --model qwen2.5-coder:7b
```

Type this and press enter:

```text
the test in test_calc.py fails, fix the bug in calc.py
```

Watch the plan, the tool calls, and the diff. Press ↑ to select a row and enter to expand it; press ctrl+c to quit. Then check the result:

```bash
cat calc.py                                                   # should say: return a + b
python3 -c "import test_calc; test_calc.test_add(); print('ok')"
```

To run it again, rerun the three `printf` lines to put the bug back.

### 5. Install the commands (optional)

To run `jotline` and `jotline-tui` from any folder:

```bash
cd ~/path/to/jotline                         # your clone
cargo install --path crates/jotline --locked  # installs `jotline` into ~/.cargo/bin
cd packages/tui && npm link                   # installs `jotline-tui`
```

```bash
cd ~/any/project
jotline-tui --model qwen2.5-coder:7b          # interactive UI
jotline --model qwen2.5-coder:7b              # classic terminal
jotline -p "explain this project"             # one-shot, for scripts
```

After pulling new changes, rebuild with `cargo install --path crates/jotline --locked --force` and `npm run build` in `packages/tui`.

### Troubleshooting

| You see | Fix |
|---|---|
| `could not find the jotline binary` | Build step 2 did not finish, or `JOTLINE_BIN` points to the wrong path. |
| `model '…' is not installed in Ollama` | Run `ollama list` and pass an installed tag with `--model`. |
| `failed request to http://127.0.0.1:11434` | Start Ollama: open the app or run `ollama serve`. |
| `command not found: jotline` | Run step 5, and make sure `~/.cargo/bin` is on your `PATH`. |
| The agent retries a lot or gets stuck | Small models struggle with longer tasks. Try `ollama pull qwen3:8b` and `--model qwen3:8b`, or a smaller request. |

## Quickstart

After [installing](#install):

```bash
ollama pull qwen2.5-coder:7b
cd path/to/your/project
jotline-tui --model qwen2.5-coder:7b              # interactive UI
jotline -p "explain how configuration is loaded"  # one-shot in the classic terminal
```

If the model is not installed, jotline prints the exact `ollama pull` command to run.

### Choosing a model

| Hardware | Suggested starting models |
|---|---|
| 8 GB RAM, no GPU | `qwen3:4b`, `qwen2.5-coder:3b` |
| 16 GB RAM or 8 GB VRAM | `qwen3:8b`, `qwen2.5-coder:7b` |
| 24 GB+ VRAM | `qwen3:32b`, `qwen2.5-coder:32b`, `devstral` |

Use a larger model for planning and a faster one for edits:

```bash
jotline --planner-model qwen3:32b --coder-model qwen2.5-coder:7b
```

### OpenAI-compatible servers

```bash
jotline --provider openai \
  --endpoint http://127.0.0.1:8000/v1/chat/completions \
  --model Qwen/Qwen2.5-Coder-7B-Instruct

export OPENAI_API_KEY=...   # only if the server requires a key
```

## Usage

Describe the task in plain language. jotline shows the detected intent, the model's plan, every tool call, and a final handoff with completed work, verification, and anything missed.

### Example requests

```text
› where is the retry logic for HTTP calls?                     # question: read-only
› add a --json flag to the export command                      # feature: edit, test, verify
› the parser panics on empty input, fix it                     # bugfix: regression test first
› split tools.rs into smaller modules without changing behavior # refactor: tests before and after
› review my staged changes                                     # review: findings, no edits
› from now on, always run `npm test -- --run` to verify        # saved as a project rule
```

### Slash commands

| Command | Effect |
|---|---|
| `/help` | List commands. |
| `/status` | Scope, models, rules, plugins, and the last task: intent, steps, files changed, verified. |
| `/plan` | Show the last recorded plan. |
| `/intent <request>` | Show how a request would be classified, with scores and its playbook. |
| `/rules` | List loaded rule sources. |
| `/rules add <rule>` | Save a rule to `.jotline/rules/user.md`. |
| `/rules show <name>` | Print one rule source, e.g. `/rules show JOTLINE.md`. |
| `/rules reload` | Re-read rule files from disk. |
| `/tools` | List available tools, including plugins. |
| `/scope`, `/scope use <path>` | Show or change the workspace boundary. |
| `/agents` | List agents. |
| `/agent new\|use\|reset <name>` | Create, switch to, or clear a named agent. |
| `/agent run <name> <prompt>` | Run a task on another agent. |
| `/reset` | Clear the active agent's conversation. |
| `/save [path]`, `/load [path]` | Save or load a session. |
| `/plugins` | Reload plugin tools. |
| `/models` | Show model routing. |
| `/exit` | Quit (Ctrl-D also works). |

### Common invocations

```bash
jotline                                         # interactive in the current directory
jotline -w ./services/api                       # narrow the workspace boundary
jotline -p "add input validation to signup" -q  # one-shot, quiet output
jotline --ask-permissions                       # confirm shell commands, plugins, commits
jotline --session-file .jotline/session.json    # resume the conversation across runs
jotline --no-project-rules                      # ignore JOTLINE.md / AGENTS.md / .jotline/rules
jotline --max-history 30                        # tighter context for small-context models
```

With `-p`, the exit status is `0` when the task finished with a handoff and `1` otherwise, so jotline can run in scripts.

## Interactive UI

`jotline-tui` is a full-screen-style terminal UI written in TypeScript with Ink. Instead of a scrolling log, every tool call is a live row (`Update(calc.py)`, `Bash(pytest -q)`) with its result, every file change shows a colored diff, and you can expand any item.

```text
● Update(calc.py) 0.3s
  ⎿  Updated calc.py with 1 addition and 1 removal
     1   def add(a, b):
     2 -     return a - b
     2 +     return a + b

● Bash(pytest -q) 1.2s
  ⎿  exit 0 · 3 lines of output
```

Build it with the [install steps](#install), then run `jotline-tui` in any project. Every argument is passed to the agent, so `--model`, `-w`, `--ask-permissions`, and the other [flags](#configuration) work the same way.

| Key | Action |
|---|---|
| ↑ / ↓ | select a tool call or plan |
| enter / space | expand or collapse it |
| ctrl+o | expand or collapse everything in the current turn |
| esc | interrupt the agent |
| `y` / `n` | answer an approval prompt (with `--ask-permissions`) |
| `/` | list commands; tab completes |
| ctrl+c | quit |

More detail, including every slash command, is in [packages/tui/README.md](packages/tui/README.md). The UI talks to `jotline --stdio`, a JSON-lines protocol any editor or GUI can use; see [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Configuration

Common flags and environment variables. The complete reference is [docs/CONFIGURATION.md](docs/CONFIGURATION.md), and `jotline --help` prints it too.

| Flag | Environment | Default | Meaning |
|---|---|---|---|
| `--provider` | `JOTLINE_PROVIDER` | `ollama` | `ollama` or `openai` (OpenAI-compatible). |
| `--model` | `JOTLINE_MODEL` | `qwen3:8b` | Model for every role. |
| `--planner-model` | `JOTLINE_PLANNER_MODEL` | `--model` | Model for the planning call. |
| `--coder-model` | `JOTLINE_CODER_MODEL` | `--model` | Model for tool use and edits. |
| `--endpoint` | `JOTLINE_ENDPOINT` | provider default | Chat endpoint URL. |
| `--api-key` | `OPENAI_API_KEY` | none | Bearer token for OpenAI-compatible servers. |
| `-w`, `--workspace` | | `.` | Boundary tools cannot leave. |
| `--ask-permissions` | | off | Ask before shell commands, plugins, and commits. |
| `--max-steps` | | `24` | Model calls per task. |
| `--max-history` | | `80` | Recent messages sent per call. |
| `--shell-timeout` | | `120` | Seconds before a command and its children are killed. |
| `-p`, `--prompt` | | | Run one task and exit. |
| `-q`, `--quiet` | | off | Hide intent, plan, and tool events. |

jotline keeps per-project state in `.jotline/` (memory, sessions, plugins, rules). Add it to your project's `.gitignore` unless you want to share rules or plugins.

## Project rules

Rules are Markdown instructions added to every request. They teach jotline your project's conventions. Create `JOTLINE.md` in your project root:

```markdown
# Project rules

- Python 3.12, formatted with ruff. Verify with `ruff check . && pytest -q`.
- Use pathlib, never os.path.
- Public functions need type hints and a one-line docstring.
- Do not edit files under migrations/.
```

| Order | Source | Use it for |
|---|---|---|
| 1 | Built-in rules ([crates/jotline-core/rules/](crates/jotline-core/rules/)) | General coding, code generation, testing, and security principles. |
| 2 | `JOTLINE.md` | Your main project rules. |
| 3 | `AGENTS.md` | Agent guidance your project already shares with other tools. |
| 4 | `.jotline/rules/*.md` | Extra rule files by topic; `user.md` collects rules added with `/rules add` or by the agent. |

Later sources win on conflict. Add a rule in three ways: say "from now on ..." to the agent, run `/rules add <rule>`, or edit the files and run `/rules reload`. Ready-to-copy examples for Python, Rust, and TypeScript are in [examples/](examples/). Limits and tips are in [docs/RULES.md](docs/RULES.md).

## Tools

| Group | Tools |
|---|---|
| Read and search | `list_files`, `find_files`, `search_files`, `read_file`, `python_symbol_overview` |
| Edit | `edit_file`, `write_file`, `structured_patch`, `apply_patch`, `replace_python_symbol` |
| Run | `run_shell` |
| Git | `git_status`, `git_diff`, `git_log`, `git_commit_plan`, `git_commit` |
| Memory and rules | `memory_get`, `memory_set`, `memory_delete`, `memory_search`, `rules_add` |
| Orchestration | `parallel_tools`, `reload_plugins` |
| Plugins | `plugin.<name>` from `.jotline/plugins/*.json` |

Arguments, limits, and result formats for every tool, plus how to write a plugin, are in [docs/TOOLS.md](docs/TOOLS.md).

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

1. **Classify** the request into an intent with a deterministic keyword scorer, at no model cost.
2. **Brief** the model with the request, the intent, and its numbered playbook.
3. **Plan** first. Any other action before a plan is rejected and the model is told why.
4. **Act** through tools. Results go back to the model, and the orchestrator tracks whether files changed and whether a command ran afterwards.
5. **Verify** before accepting the final answer on code-changing tasks.
6. **Hand off** a structured summary: completed, verified, missed, next steps.

The full design is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): the action protocol, parsing leniency, context trimming, the workspace boundary, delegation, and the session format.

## Project layout

```text
jotline/
├── crates/
│   ├── jotline/            # CLI binary: flags, REPL, slash commands, terminal UI, approvals
│   └── jotline-core/       # engine library: orchestrator, intent, rules, tools, model clients
│       └── rules/          # built-in rules compiled into the binary
├── packages/
│   └── tui/                # TypeScript terminal UI (Ink) on top of `jotline --stdio`
├── docs/                   # user and contributor guides
├── examples/               # sample JOTLINE.md, rule files, and a plugin
├── tools/                  # maintainer scripts used by CI
├── .github/                # CI, PR title, and release workflows; issue and PR templates
├── .claude/ .codex/        # agent rules and reviewer agents for contributors
├── AGENTS.md CLAUDE.md     # guidance for coding agents working on this repository
└── Cargo.toml              # virtual workspace
```

See [docs/REPOSITORY_LAYOUT.md](docs/REPOSITORY_LAYOUT.md) for where new code belongs.

## Development

```bash
git clone https://github.com/jotx19/jotline.git && cd jotline

cargo build                                                    # build both crates
cargo run -p jotline -- --help                                 # run the CLI from source
cargo run -p jotline -- -w /path/to/another/project            # try it on a project

cargo test --workspace --locked                                # all tests (no model needed)
cargo test -p jotline-core intent                              # tests matching "intent"
cargo clippy --workspace --all-targets --locked -- -D warnings # lints
cargo fmt --all                                                # format
cargo doc --workspace --no-deps --open                         # API docs
python3 tools/check_docs.py                                    # broken relative links in Markdown
(cd packages/tui && npm ci && npm test)                        # terminal UI typecheck + tests
```

Tests never call a real model: the orchestrator is driven by a scripted stub, and the binary is tested end-to-end against a fake Ollama server. The [developer guide](docs/DEVELOPMENT.md) maps the codebase and walks through adding a tool.

## CI and releases

| Workflow | Runs on | Checks |
|---|---|---|
| [CI](.github/workflows/ci.yml) | every push to `main` and every pull request | `cargo fmt --check`, clippy with `-D warnings`, `cargo doc` with `-D warnings`, the docs link check, tests on Ubuntu and macOS, a release-build smoke test, and a build with the minimum supported Rust version. The single `CI required` job passes only if all of them pass. |
| [PR title](.github/workflows/pr-title.yml) | pull request opened, edited, or updated | The title follows Conventional Commits, e.g. `feat(tools): add search_files`. |
| [Release](.github/workflows/release.yml) | manual run, and merge of the release PR | release-please opens a release PR with the version bump and changelog. Merging it tags `vX.Y.Z`, creates the GitHub release, and uploads binaries for four targets. |

Versions follow [Semantic Versioning](https://semver.org) and are chosen from pull request titles: `fix:` → patch, `feat:` → minor, `!` → major. See [CHANGELOG.md](CHANGELOG.md) and [docs/RELEASING.md](docs/RELEASING.md).

## Support and discussion

- **Bugs and feature requests:** open an [issue](https://github.com/jotx19/jotline/issues/new/choose) using the forms.
- **Live discussion:** every issue gets its own chat room on [JotPod](https://jotpod.jotx.space), a GitHub App that opens a private, real-time chat pod per issue for the repository's collaborators. Join the pod linked on an issue to work through it together with threaded replies; it becomes read-only when the issue closes. Record decisions and reproduction steps on the issue itself so they stay searchable.
- **Questions about usage:** check the [docs](#documentation) first, then open an issue.

## Security

jotline can run shell commands chosen by a model. It keeps file tools inside the workspace and kills timed-out processes with their children, but it does not sandbox commands. In repositories you do not trust, use `--ask-permissions` or run jotline in a container. Report vulnerabilities privately through the [Security tab](https://github.com/jotx19/jotline/security), not in issues or JotPod rooms. Details are in [SECURITY.md](SECURITY.md).

## Limitations

- Quality depends on the model. 7-8B models handle focused edits well; large multi-file features need a bigger model or smaller steps.
- Tool calls use a JSON protocol in plain chat, not native function calling. Any chat model works, but very small models may need several corrections.
- Responses are not streamed; the spinner shows elapsed time while the model thinks.
- Only Python has symbol-aware editing; other languages use text edits and patches.
- Windows is supported only through WSL.

## Documentation

| Guide | Contents |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | The task loop, action protocol, verification gate, context handling, sessions. |
| [Configuration](docs/CONFIGURATION.md) | Every flag, environment variable, and the `.jotline/` directory. |
| [Rules](docs/RULES.md) | Rule sources, adding rules, writing good rules, limits. |
| [Tools](docs/TOOLS.md) | Every tool's arguments and results, and plugins. |
| [Protocol](docs/PROTOCOL.md) | The `--stdio` JSON-lines protocol for front ends. |
| [Terminal UI](packages/tui/README.md) | Install, keys, and commands for `jotline-tui`. |
| [Development](docs/DEVELOPMENT.md) | Setup, commands, codebase map, adding a tool, testing without a model. |
| [Quality](docs/QUALITY.md) | How changes are specified, tested, reviewed, and documented. |
| [Releasing](docs/RELEASING.md) | Versioning, release steps, and one-time repository setup. |
| [Repository layout](docs/REPOSITORY_LAYOUT.md) | Where code, docs, and tooling belong. |

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) first. In short: open an issue for anything non-trivial, keep each pull request focused on one behavior, add a test that fails without your change, run the checks under [Development](#development), and give the pull request a Conventional Commit title. If you use a coding agent, point it at [AGENTS.md](AGENTS.md).

## License

MIT © [jotx19](https://github.com/jotx19). See [LICENSE](LICENSE).
