# Rules

Rules are Markdown instructions that jotline adds to every agent's system prompt. They are how you teach jotline your project's conventions: language version, formatter, test command, error-handling style, files it must not touch.

## Where rules come from

jotline loads rules in this order. Later rules take precedence when they conflict, and the prompt tells the model so.

| Order | Source | Purpose |
|---|---|---|
| 1 | Built-in rules | General coding principles, code generation, testing, and security rules compiled into the binary from [crates/jotline-core/rules/](../crates/jotline-core/rules/). |
| 2 | `JOTLINE.md` in the workspace root | Your main project rules for jotline. |
| 3 | `AGENTS.md` in the workspace root | Shared agent guidance, if your project already has it for other tools. |
| 4 | `.jotline/rules/*.md` | Additional rule files, loaded in file-name order. `user.md` holds rules added from the REPL or by the agent. |

Disable layers with `--no-builtin-rules` or `--no-project-rules`.

## Adding a rule

Three ways, from quickest to most structured:

1. **Ask.** Tell jotline "from now on, always run `npm test -- --run` to verify" or "never edit files in `migrations/`". The agent calls the `rules_add` tool, which appends the rule to `.jotline/rules/user.md` and reloads rules immediately.
2. **Command.** Run `/rules add Use pytest fixtures instead of setUp methods`. The rule is appended to `.jotline/rules/user.md` and applies from the next task.
3. **File.** Write `JOTLINE.md` or a file in `.jotline/rules/`, then run `/rules reload` (or restart).

Inspect what is loaded with `/rules`, and print one source with `/rules show JOTLINE.md` or `/rules show coding-principles`.

## Writing good rules

Rules are sent on every request, so they compete with your code for the model's context window. Short, imperative, checkable rules work best with small models.

```markdown
# Project rules

- Rust 2021, MSRV 1.82. Verify with `cargo test --workspace && cargo clippy -- -D warnings`.
- Errors: `thiserror` in library crates, `anyhow` in binaries. Never `unwrap()` outside tests.
- New HTTP handlers go in `src/routes/<resource>.rs` and are registered in `src/routes/mod.rs`.
- Do not modify files under `vendor/` or `generated/`.
```

- State the verification command. The verification gate asks the model to run "the project's build or tests"; telling it which command removes guessing.
- Prefer "do X" over "do not do Y" unless it is a hard boundary.
- One rule per line. Skip explanations the model does not need to act.
- Put language- or directory-specific rules in separate files such as `.jotline/rules/python.md` so they are easy to find and remove.

See [examples/](../examples/) for a sample `JOTLINE.md` and rule file.

## Limits

| Limit | Value | Behavior when exceeded |
|---|---|---|
| One project rule file | 12,000 characters | truncated; the prompt notes the truncation |
| All project rules together | 24,000 characters | later files are truncated or skipped |
| One rule added with `/rules add` or `rules_add` | 500 characters | rejected with an error |

Empty and unreadable files are skipped. Only files ending in `.md` are read from `.jotline/rules/`.

## Committing rules

`JOTLINE.md` and `AGENTS.md` are meant to be committed so the whole team shares them. `.jotline/` also holds per-user state (memory, sessions), so most projects add `.jotline/` to `.gitignore` and, if they want shared rule files there, re-include them with `!.jotline/rules/`.
