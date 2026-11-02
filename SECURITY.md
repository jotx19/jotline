# Security

## Threat model

jotline executes actions chosen by a language model. Treat the model, and any file content or command output it reads, as untrusted: a file in the workspace can contain instructions that try to make the model run harmful commands (prompt injection).

jotline enforces these boundaries in code, independent of the model:

- File tools cannot read, write, list, or search outside the workspace root. Absolute paths, `..` components, and symlinks that resolve outside the root are rejected.
- `apply_patch` rejects patches whose headers name paths outside the workspace.
- Git tools are limited to the workspace with a pathspec, even inside a larger repository.
- Processes are killed with their whole process group after `--shell-timeout`.
- With `--ask-permissions`, `run_shell`, plugin tools, and `git_commit` run only after you approve each call.

jotline does **not** sandbox commands. In the default auto mode, `run_shell` runs any command the model chooses with your user's permissions, and that command can read or change anything your user can. Use `--ask-permissions`, a container, or a virtual machine when working in repositories you do not trust. Plugins run the command in their JSON spec; only install plugin files you have read.

jotline sends your prompts, the rules, and file contents the model reads to the configured `--endpoint`. With a local Ollama server that data stays on your machine; with a remote endpoint it is sent to that server.

## Reporting a vulnerability

Report vulnerabilities privately with **Report a vulnerability** in the [Security tab](https://github.com/jotx19/jotline/security). If private reporting is unavailable, [open an issue](https://github.com/jotx19/jotline/issues/new) asking for a private contact and leave out the details. Do not discuss vulnerabilities in an issue's JotPod chat room either. Do not put exploit code, credentials, or personal data in public issues.

Include the affected version or commit, your operating system, the provider and model, a minimal reproduction (workspace layout and the model reply or tool call that triggers it), and the impact you expect. A boundary bypass that works with a scripted model reply is as serious as one that needs a real model.

Security fixes target the current `main` branch and the next release. Older releases have no promised backport schedule.
