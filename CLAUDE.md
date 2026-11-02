# Claude Code instructions

Follow the shared agent guidance and the topic rules below. They apply to every change in this repository.

@AGENTS.md

## Topic rules

@.claude/rules/rust.md
@.claude/rules/testing.md
@.claude/rules/documentation.md
@.claude/rules/commits.md

## Project agents

Reviewer subagents live in `.claude/agents/`: `code-review`, `test-review`, `security-review`, `performance-review`, and `docs-review`. Use them as described in the Reviews section of AGENTS.md.

## Quick commands

```bash
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all
python3 tools/check_docs.py
cargo run -p jotline -- --help
```
