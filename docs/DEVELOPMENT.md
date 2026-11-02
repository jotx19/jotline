# Developer guide

## Setup

Install Rust with [rustup](https://rustup.rs). The minimum supported Rust version is in the root `Cargo.toml` (`rust-version`); development uses current stable. You also need `git`, `bash`, and `python3` (used by a plugin test and the documentation checker).

```bash
git clone <your fork URL> jotline
cd jotline
cargo build
cargo test --workspace
```

## Everyday commands

```bash
cargo run -p jotline -- --help                 # run the CLI from source
cargo run -p jotline -- -w /path/to/project    # try it on another project
cargo test --workspace --locked                # all tests
cargo test -p jotline-core intent              # tests whose name contains "intent"
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all
cargo doc --workspace --no-deps                # API docs for jotline-core
python3 tools/check_docs.py                    # broken relative links in Markdown
```

CI runs exactly these checks; see [.github/workflows/ci.yml](../.github/workflows/ci.yml).

## Cargo workspace

| Package | Path | Kind |
|---|---|---|
| `jotline-core` | `crates/jotline-core/` | library (`jotline_core`) |
| `jotline` | `crates/jotline/` | binary (`jotline`) |

Plain `cargo build` and `cargo test` in the root cover both packages. Select one with `-p`.

## Where things live

| To change | Edit |
|---|---|
| How requests are classified, or a playbook | `crates/jotline-core/src/intent/mod.rs` and its `tests.rs` |
| The JSON protocol or parsing leniency | `crates/jotline-core/src/orchestrator/action.rs` |
| System prompt, task brief, corrections | `crates/jotline-core/src/orchestrator/prompt.rs` |
| The loop, verification gate, delegation | `crates/jotline-core/src/orchestrator/mod.rs` |
| Built-in rules | `crates/jotline-core/rules/*.md` |
| A tool's behavior | `crates/jotline-core/src/tools/<area>.rs`; its description in `tools/spec.rs` |
| A CLI flag | `crates/jotline/src/cli.rs`, then [configuration](CONFIGURATION.md) |
| A slash command | `crates/jotline/src/repl.rs` and `ui::print_help`, then the README table |

## Adding a tool

1. Implement it in the right module under `crates/jotline-core/src/tools/`, returning `ToolResult` (`Ok(text)` or `fail("message that tells the model how to fix the call")`). Resolve every path with `Workspace::resolve`, `existing`, or `existing_file`.
2. Add its `ToolSpec` to `builtin_specs` in `tools/spec.rs` with the right `Effect`. The effect drives the verification gate: `Write` for anything that modifies workspace files, `Execute` for commands.
3. Dispatch it in `ToolExecutor::dispatch`, reading arguments with `Args` (`req_str`, `opt_str`, `usize_or`, `bool_or`).
4. Add integration tests in `crates/jotline-core/tests/tools.rs`, including a malformed-argument case and, for path arguments, an escape attempt.
5. Document it in [tools](TOOLS.md). Every tool adds prompt tokens to every request, so prefer extending an existing tool when the behavior fits.

## Testing the orchestrator without a model

`crates/jotline-core/tests/support/mod.rs` provides `StubLlm`, which replays scripted replies and records every request. A typical test scripts a plan, some tool calls, and a final answer, then asserts on the result, the tool history, and what the model was told:

```rust
let llm = StubLlm::new([
    r#"{"type":"plan","summary":"do it","steps":["act"]}"#,
    r#"{"type":"tool","tool":"write_file","args":{"path":"a.txt","content":"x"}}"#,
    r#"{"type":"final","message":"done"}"#,
]);
let mut orchestrator = build_orchestrator(&tmp, llm.clone());
orchestrator.run_active("implement it");
assert!(llm.calls()[3].last().unwrap().content.contains("have not run any verification"));
```

The binary is tested end-to-end in `crates/jotline/tests/cli.rs` against a fake Ollama HTTP server bound to a random local port.

## Trying it with a real model

```bash
ollama pull qwen3:8b
cargo run -p jotline -- -w /tmp/scratch-project
```

When you change prompts, playbooks, or built-in rules, try the same few tasks (a question, a small feature, a bug fix with a failing test) before and after with the same model, and describe the difference in the pull request. Prompt changes are behavior changes.
