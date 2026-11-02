# Contributing agent guidance

These instructions apply to every coding agent working in this repository (Claude Code, Codex, jotline itself, and others). Claude Code loads them through [CLAUDE.md](CLAUDE.md); topic rules live in [.claude/rules/](.claude/rules/).

## Repository layout

Before creating or moving a crate or directory, read and follow the [repository layout rules](docs/REPOSITORY_LAYOUT.md). Keep the root `Cargo.toml` a virtual workspace. The engine library is `crates/jotline-core/`; the terminal binary is `crates/jotline/`; the TypeScript terminal UI is `packages/tui/`. The engine never prints to the terminal or reads stdin; it reports through the `Reporter` trait and asks for approval through the `Approver` trait.

## Project scope

jotline is a local-first coding agent for open models. It must work with small local models (7-8B parameters, 8k-32k context) over Ollama and OpenAI-compatible APIs, with no network access other than the configured model endpoint. Keep it a single static binary with no runtime dependencies beyond `bash` and `git`. Hosted-only features, telemetry, and background services are out of scope.

Every design choice is judged by its effect on small models: fewer prompt tokens, shorter and more explicit instructions, deterministic orchestration in Rust rather than hoping the model behaves, and precise error messages the model can act on.

## Before changing code

Read the working-tree status and the relevant source, tests, and documentation before editing. Historical reports and earlier messages are context, not proof that the current checkout works. Choose one bounded change with an observable acceptance criterion. When a prerequisite is blocked, record the exact blocker and continue independent work.

## Implementation rules

- Follow [.claude/rules/rust.md](.claude/rules/rust.md) for Rust style, error handling, and typing.
- The workspace boundary is a security property. Every tool path goes through `Workspace::resolve`; never join user or model input onto a path directly. Changes to `tools/workspace.rs`, `tools/git.rs` patch checks, `tools/process.rs`, or approval logic need the `security-review` agent.
- Prompt text (system prompt, playbooks, corrections, built-in rules) is product code. Keep it short and imperative, measure its size when you grow it, and update the tests that assert on it.
- Tool results start with a stable machine-readable prefix (`ERROR:`, `DENIED:`, `WROTE:`, `EDITED:`, `NO_MATCHES:`, ...). Do not change a prefix without updating [docs/TOOLS.md](docs/TOOLS.md) and the tests.
- Keep source files focused. Unit tests live in a sibling `tests.rs` module (`#[cfg(test)] mod tests;`); integration tests live in each crate's `tests/` directory; shared test helpers live in `tests/support/`.
- Session files are a compatibility contract. Bump `SESSION_VERSION` and keep loading older versions when the format changes.

## Testing

Follow [.claude/rules/testing.md](.claude/rules/testing.md) and [docs/QUALITY.md](docs/QUALITY.md). For a bug fix, first add a test that fails without the fix. Orchestrator behavior is tested with the scripted `StubLlm` in `crates/jotline-core/tests/support/`; binary behavior is tested end-to-end against the fake Ollama server in `crates/jotline/tests/cli.rs`. Tests never call a real model.

Run before handing off:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 tools/check_docs.py
(cd packages/tui && npm ci && npm test)
```

Changes to `--stdio` events or requests must update [docs/PROTOCOL.md](docs/PROTOCOL.md), the types in `packages/tui/src/protocol.ts`, and the `stdio_*` tests in `crates/jotline/tests/cli.rs` together.

## Documentation

Follow [.claude/rules/documentation.md](.claude/rules/documentation.md). Keep each prose paragraph on one source line. Document lasting behavior for users and contributors who have not read any development conversation; exclude session narration, agent handoffs, and task history. When behavior changes, reconcile the README, the relevant guide in `docs/`, and `CHANGELOG.md` (release tooling maintains released sections; describe unreleased user-visible changes in the PR body).

## Reviews

Delegate independent review of substantial changes to the project agents in [.claude/agents/](.claude/agents/) (Codex equivalents in [.codex/agents/](.codex/agents/)):

| Change | Reviewers |
|---|---|
| Orchestrator, intent, action parsing, prompts | `code-review`, `test-review` |
| Tools, workspace boundary, process execution, approvals | `code-review`, `security-review`, `test-review` |
| Hot paths (search, file walking, context building) | `performance-review` |
| Documentation only | `docs-review` |
| CI and release automation | `test-review` (including failure paths) |

Give each reviewer the changed files and a bounded question. Reviewers report findings; the coordinating agent owns edits and test execution. Reviewer approval supplements executed tests and never replaces them.

## Commits and handoff

Follow [.claude/rules/commits.md](.claude/rules/commits.md): Conventional Commit messages, small scoped commits, no generated files or secrets. At handoff, state what changed, which checks actually ran and their results, and what remains unverified. Do not describe local success as CI success. Pushing, tagging, releasing, and publishing require explicit authorization from a maintainer.
