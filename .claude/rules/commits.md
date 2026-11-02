# Commit and pull request rules

- Commit messages and PR titles use Conventional Commits: `type(optional-scope)!: description`. Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`. Useful scopes: `core`, `cli`, `tools`, `intent`, `rules`, `llm`, `deps`.
- PRs are squash-merged and the PR title becomes the commit on `main`, which release-please reads to pick the next version and write the changelog. A `!` or a `BREAKING CHANGE:` footer triggers a breaking release; explain the migration in the PR body.
- Keep commits small and focused on one behavior. Do not mix refactors with behavior changes.
- Never commit secrets, `.env` files, `.jotline/` state, `target/`, or editor files. Check `git status` before committing.
- Fill in every section of the pull request template, including the exact commands you ran.
- Do not push, tag, publish, or create releases without explicit maintainer authorization.
