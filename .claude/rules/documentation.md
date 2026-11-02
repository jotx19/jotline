# Documentation rules

- Keep each prose paragraph on one source line; do not hard-wrap Markdown. Keep structural line breaks for headings, lists, tables, and code blocks.
- Write for a user or contributor who has never seen the development conversation: what the feature does, how to use it, a runnable example, and its limitations.
- Describe the resulting behavior, never the process that produced it. No session summaries, agent handoffs, or "as requested" narration.
- Use repository-relative links and copy-pasteable commands. Never record absolute paths from your machine, home directories, or temporary directories.
- Scope claims to evidence. Say "tested on Linux and macOS in CI", not "works everywhere". Do not claim a check passed unless it ran on the documented revision.
- When behavior changes, update every place that describes it: README, the guide in `docs/`, CLI `--help` text, and the tool or command tables. Run `python3 tools/check_docs.py` to catch broken relative links.
- Built-in rule files in `crates/jotline-core/rules/` are sent to the model on every request. Keep them short, imperative, and free of examples that cost tokens without changing behavior.
