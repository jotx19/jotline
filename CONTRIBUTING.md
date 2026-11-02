# Contribute to jotline

Thanks for helping. Start with the [developer guide](docs/DEVELOPMENT.md) for setup and the codebase map, and follow the [quality process](docs/QUALITY.md) for every change. If you use a coding agent, point it at [AGENTS.md](AGENTS.md); Claude Code loads it automatically through [CLAUDE.md](CLAUDE.md).

## Before you start

For anything larger than a small fix, open an issue first describing the problem and the proposed behavior, so we can agree on the approach before you write code. jotline's design goal is to work well on small local models, so proposals that add prompt tokens, tools, or model steps should explain the effect on a 7-8B model.

## Making a change

Keep each pull request focused on one behavior. Add a test that fails without your change, keep existing tests passing, and update documentation for anything a user can see. Run the full check list before pushing:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 tools/check_docs.py
```

Fill in every section of the pull request template, including the commands you ran and anything that remains unverified. The default branch requires the `CI required` check and resolved review conversations before merging. Maintainers merge accepted contributions; opening a pull request does not grant write access.

## Pull request titles

Use `type(optional-scope)!: description`. The scope and breaking-change marker (`!`) are optional. Allowed types are `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, and `revert`. Use a short scope such as `core`, `cli`, `tools`, `intent`, `rules`, `llm`, or `deps`; spaces are not allowed inside the scope.

Examples: `feat(tools): add search_files`, `fix(core): keep the request in the context window`, and `feat!: rename the session file format`. The PR title check runs whenever a pull request is opened, edited, or updated, including pull requests from forks.

Pull requests are squash-merged with the PR title as the commit title, and [release-please](docs/RELEASING.md) uses those titles to choose versions and write the changelog. Explain breaking changes and migration steps in the PR body.

## Discussing an issue

Each issue has a chat pod on [JotPod](https://jotpod.jotx.space) for real-time discussion between collaborators. Use it to work through a problem quickly, then record the outcome (decision, reproduction, next step) as a comment on the issue so it stays searchable after the pod closes.

## Reporting bugs

Use the bug report form in the [issue tracker](https://github.com/jotx19/jotline/issues). Include `jotline --version`, the provider and model, your operating system, a minimal workspace and request that reproduces the problem, and the relevant output. Remove private code, secrets, and machine-specific paths. Report security problems privately as described in [SECURITY.md](SECURITY.md).

## Conduct

Be respectful and keep discussion focused on the work. Critique ideas and code, not people. Do not share anyone's private information. Maintainers may remove comments or contributions that do not follow these expectations.
