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

[Install](#install) · [Quickstart](#quickstart) · [Usage](#usage) · [Rules](#project-rules) · [Docs](#documentation) · [Contributing](#contributing)

</div>

---

jotline is a coding agent built for **small local models**: 7-8B parameters, short context windows, and imperfect JSON. Instead of trusting the model to behave, the orchestrator does the disciplined parts in Rust: it classifies every request, hands the model a short playbook for that kind of task, insists on a plan first, corrects malformed replies with precise messages, keeps every file operation inside your workspace, and refuses to accept "done" after a code change until a build or test command has actually run.

```text
╭ jotline 2.0.0 ────────────────────────────────────────────────────────────╮
│ scope     /home/me/projects/api                                           │
│ models    qwen3:8b via ollama (planner qwen3:8b, coder qwen3:8b)          │
│ rules     4 built-in, 2 project   (/rules to inspect, /rules add <rule>)  │
╰───────────────────────────────────────────────────────────────────────────╯
✦ main  api  qwen3:8b  auto › fix the off-by-one in pagination and add a test
✦ intent bugfix
╭ Plan ─────────────────────────────────────────────────────────────────────╮
│ main · detailed plan · bugfix                                             │
│ Fix page offset calculation and cover it with a regression test           │
│ 1. search_files for the offset computation                                │
│ 2. reproduce with a failing test in tests/test_pagination.py              │
│ 3. fix the root cause, run pytest                                         │
╰───────────────────────────────────────────────────────────────────────────╯
● tool main search_files {"pattern":"offset ="}
● tool main edit_file {"path":"api/pagination.py", ...}
● tool main run_shell {"command":"pytest -q tests/test_pagination.py"}
╭ Done · main · bugfix · 7 steps ───────────────────────────────────────────╮
│ Fixed the off-by-one: offset now uses (page - 1) * size.                  │
│ verified                                                                  │
│   • pytest -q tests/test_pagination.py -> 4 passed                        │
╰───────────────────────────────────────────────────────────────────────────╯
```

## Table of contents

- [Features](#features)
- [Install](#install)
- [Quickstart](#quickstart)
- [Usage](#usage)
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

### Prebuilt binaries

Each GitHub release from the release workflow includes archives for Linux (x86_64, aarch64) and macOS (Apple Silicon, Intel), with SHA-256 checksums. Download one from the [releases page](https://github.com/jotx19/jotline/releases/latest), or from a terminal:

```bash
VERSION=v2.0.0                      # pick a tag from the releases page
TARGET=aarch64-apple-darwin         # x86_64-apple-darwin | x86_64-unknown-linux-gnu | aarch64-unknown-linux-gnu
curl -LO "https://github.com/jotx19/jotline/releases/download/${VERSION}/jotline-${VERSION}-${TARGET}.tar.gz"
curl -LO "https://github.com/jotx19/jotline/releases/download/${VERSION}/jotline-${VERSION}-${TARGET}.tar.gz.sha256"
shasum -a 256 -c "jotline-${VERSION}-${TARGET}.tar.gz.sha256"
tar -xzf "jotline-${VERSION}-${TARGET}.tar.gz"
sudo mv "jotline-${VERSION}-${TARGET}/jotline" /usr/local/bin/
jotline --version
```

### From source

Requires Rust **1.85 or newer** ([rustup](https://rustup.rs)).

```bash
# latest main
cargo install --git https://github.com/jotx19/jotline jotline --locked

# or from a checkout
git clone https://github.com/jotx19/jotline.git
cd jotline
cargo install --path crates/jotline --locked
```

### Requirements

| Requirement | Why |
|---|---|
| Linux or macOS | `run_shell` uses `bash`. On Windows, use WSL. |
| `git` | git tools and `apply_patch`. Everything else works without it. |
| A model server | [Ollama](https://ollama.com) (default) or any OpenAI-compatible `/v1/chat/completions` server: vLLM, llama.cpp `llama-server`, LM Studio, LocalAI. |

## Quickstart

```bash
# 1. Get a model (any chat model works; coder models do best)
ollama pull qwen3:8b

# 2. Start jotline in your project
cd path/to/your/project
jotline

# 3. Or run one task and exit
jotline -p "explain how configuration is loaded"
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
| [Development](docs/DEVELOPMENT.md) | Setup, commands, codebase map, adding a tool, testing without a model. |
| [Quality](docs/QUALITY.md) | How changes are specified, tested, reviewed, and documented. |
| [Releasing](docs/RELEASING.md) | Versioning, release steps, and one-time repository setup. |
| [Repository layout](docs/REPOSITORY_LAYOUT.md) | Where code, docs, and tooling belong. |

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) first. In short: open an issue for anything non-trivial, keep each pull request focused on one behavior, add a test that fails without your change, run the checks under [Development](#development), and give the pull request a Conventional Commit title. If you use a coding agent, point it at [AGENTS.md](AGENTS.md).

## License

MIT © [jotx19](https://github.com/jotx19). See [LICENSE](LICENSE).
