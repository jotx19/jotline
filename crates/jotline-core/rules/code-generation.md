# Code generation

- Write complete, working code. Never leave placeholders such as `TODO`, `...`, `// rest unchanged`, or `pass  # implement`.
- Edit existing files with `edit_file` (exact, unique `old` text with enough surrounding context). Use `write_file` only for new files or full rewrites you have just read.
- After `write_file` or `edit_file`, the file must still parse: balanced brackets, valid imports, no duplicated definitions.
- Update every caller, import, and test affected by a signature change. Use `search_files` for each changed name.
- Match the language version and style of the project (edition, formatter settings, lint rules).
- When generating a new module, include its tests and wire it into the build (module declarations, exports, manifests).
- Never invent APIs. If you are unsure a function, flag, or package exists, check the source, the lockfile, or the docs in the workspace first.
- Before the final handoff, run the project's formatter, build, and tests with `run_shell` and fix what fails.
