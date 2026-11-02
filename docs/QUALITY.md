# Quality process

Every change to jotline follows the same sequence. The goal is that a reviewer can trust a merged change without re-deriving it.

## 1. Define the behavior

State what changes for a user or for the model, in one or two sentences, before writing code. Name the acceptance criterion: a test that passes, a command whose output changes, or a measured prompt size.

## 2. Demonstrate the problem

For a bug fix or new behavior, first write a test that fails on the current code. For orchestrator behavior, script the model with `StubLlm` and assert on the outcome *and* on the messages the model received. For tools, assert on the exact result prefix and on the file system. For a behavior-preserving refactor, run the existing suite first and record that it passes.

## 3. Implement the smallest change

Follow [AGENTS.md](../AGENTS.md) and [.claude/rules/rust.md](../.claude/rules/rust.md). Keep refactors and behavior changes in separate commits.

## 4. Verify

Run the full check list and keep the exact output for the pull request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 tools/check_docs.py
```

For prompt, playbook, or built-in rule changes, also try representative tasks with a real local model and describe the before/after behavior. Report the system prompt size change; you can print it from a test with `orchestrator.agent_snapshot("main").unwrap().messages[0].content.len()`.

## 5. Review

Ask the project reviewers in [.claude/agents/](../.claude/agents/) as listed in the Reviews table in [AGENTS.md](../AGENTS.md#reviews). Resolve concrete findings before claiming the change is verified. Reviewers supplement executed tests; their approval alone does not establish correctness.

## 6. Document

Update the README, the relevant guide in `docs/`, and `--help` text for any user-visible change. Describe user-visible changes in the pull request body so the release notes are accurate.

## Writing doc comments

Public items in `jotline-core` are an API. A doc comment must tell a caller something the name and type do not: valid values and defaults, what `None` or an empty value means, units (characters, bytes, lines, seconds), what is validated and what fails, and how the item relates to other items. Delete comments that restate the signature.

```rust
// Noise: restates the name.
/// The maximum steps.
pub max_steps: u32,

// Useful: says what counts and what happens.
/// Maximum model calls for one top-level task; the loop returns a step-limit message after
/// the last call. Values below 1 are raised to 1.
pub max_steps: u32,
```

## Definition of done

- The new or changed behavior has a test that fails without the change.
- All checks above pass locally, and CI passes on the pull request.
- Documentation matches the behavior.
- The pull request template is complete, including what remains unverified.
