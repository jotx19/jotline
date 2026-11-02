## What changes for a user or for the model?

Describe the behavior. For prompt, playbook, rule, or tool changes, include a small before/after example of what the model sees or does.

## How was it checked?

- New or changed test, and evidence it fails without this change:
- `cargo test --workspace --locked`:
- `cargo clippy --workspace --all-targets --locked -- -D warnings`:
- `cargo fmt --all --check` and `python3 tools/check_docs.py`:
- Manual run with a real model (model name and task), if prompts or tools changed:
- System prompt size before/after, if prompts, rules, or tool specs changed:

## What remains unsupported or uncertain?

State remaining limitations, unverified platforms, and follow-up work. Mark breaking changes with `!` in the title and describe the migration here.
