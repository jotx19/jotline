# The `--stdio` protocol

`jotline --stdio` runs the agent without a terminal UI and talks to a front end over standard input and output. The TypeScript terminal UI in [packages/tui/](../packages/tui/) uses it, and any editor plugin or GUI can too. Every line on stdin is one JSON request, and every line on stdout is one JSON event. stdout carries nothing else; diagnostics go to stderr.

```bash
jotline --stdio --model qwen2.5-coder:7b -w ./project
```

All other flags (`--model`, `--workspace`, `--ask-permissions`, `--session-file`, rule flags, limits) work as usual. `--stdio` cannot be combined with `--prompt`. The process exits when stdin closes or an `exit` request arrives.

The protocol version is `1`, reported in the `ready` event. Fields may be added to events in later releases without changing the version; removing or renaming a field increments it.

## Requests (stdin)

| `type` | Fields | Effect |
|---|---|---|
| `prompt` | `text` | Run a task on the active agent. Requests received while a task runs are queued. |
| `approval` | `id`, `approved` | Answer an `approval_request` with the same `id`. |
| `interrupt` | none | Stop the running task before its next model call, and deny a pending approval. A model request or tool call already in progress finishes first. |
| `command` | `name`, optional `arg` | `reset` (clear the active agent's conversation), `rules`, `rules_add` (`arg` is the rule), `reload_rules`, `tools`, or `status`. |
| `exit` | none | Stop reading requests and exit after the current task. |

```json
{"type":"prompt","text":"the test in test_calc.py fails, fix the bug in calc.py"}
{"type":"approval","id":1,"approved":true}
{"type":"command","name":"rules_add","arg":"Always run pytest -q to verify"}
```

## Events (stdout)

Every event has an `event` field naming its type.

| `event` | Fields | When |
|---|---|---|
| `ready` | `protocol`, `version`, `workspace`, `provider`, `model`, `planner_model`, `coder_model`, `auto_mode`, `agent`, `rules`, `tools` | Once, at startup. `rules` entries have `origin` (`builtin`/`project`), `name`, `chars`, `truncated`. |
| `run_started` | `agent`, `prompt` | A `prompt` request starts. |
| `intent` | `agent`, `intent`, `read_only` | The request was classified. |
| `thinking` / `thinking_done` | `phase`, `message` / `ok` | A model call starts / returns. |
| `plan` | `agent`, `plan` (`intent`, `needs_plan`, `summary`, `reason`, `steps`, `files`) | The model recorded a plan. Models may send more than one plan per task. |
| `tool_started` | `agent`, `id`, `tool`, `args` | A tool call or `parallel_tools` batch starts. `id` is unique per process. |
| `tool_finished` | `agent`, `id`, `tool`, `status` (`ok`/`error`/`denied`), `text`, `wrote_files`, `executed`, `changes` | The call with the same `id` finished. `text` is exactly what the model sees. |
| `notice` | `kind` (`correction`/`delegate`/`delegate_result`), `agent`, `message` | The orchestrator corrected the model, or a sub-agent started or returned. |
| `approval_request` | `id`, `request` | Only with `--ask-permissions`. The task waits until an `approval` with this `id` arrives. |
| `run_finished` | `agent`, `message`, `intent`, `steps`, `files_changed`, `verified`, `handoff` | The task ended. `handoff` (`message`, `summary`, `completed`, `verified`, `missed`, `next_steps`) is `null` when it ended without a final answer (model error, step limit, interrupt). |
| `rules`, `tools`, `status`, `reset` | see below | Replies to `command` requests. `rules` has `rules` and, after `rules_add`, `added` (the file path). |
| `error` | `message` | A request could not be handled, or the session could not be saved. |

### File changes

`changes` in `tool_finished` lists every file the call modified, in write order, so a front end can show diffs without reading the disk:

```json
{"path":"calc.py","created":false,"before":"def add(a, b):\n    return a - b\n","after":"def add(a, b):\n    return a + b\n","too_large":false}
```

`path` is workspace-relative. `before` is `null` for created files. When either version exceeds 256 KiB, `too_large` is `true` and both contents are `null`. Calls that fail or only read files have an empty list. Rewriting a file with identical content is not reported.

## Example session

```text
→ {"type":"prompt","text":"fix the bug in calc.py"}
← {"event":"run_started","agent":"main","prompt":"fix the bug in calc.py"}
← {"event":"intent","agent":"main","intent":"bugfix","read_only":false}
← {"event":"thinking","phase":"plan","message":"main · fix the bug in calc.py"}
← {"event":"thinking_done","ok":true}
← {"event":"plan","agent":"main","plan":{"intent":null,"needs_plan":true,"summary":"fix add","reason":"","steps":["edit","check"],"files":[]}}
← {"event":"tool_started","agent":"main","id":1,"tool":"edit_file","args":{"path":"calc.py","old":"a - b","new":"a + b"}}
← {"event":"tool_finished","agent":"main","id":1,"tool":"edit_file","status":"ok","text":"EDITED: calc.py (replacements=1)","wrote_files":true,"executed":false,"changes":[...]}
← {"event":"run_finished","agent":"main","message":"Fixed add.","intent":"bugfix","steps":4,"files_changed":true,"verified":true,"handoff":{...}}
```

The protocol is tested end-to-end in `crates/jotline/tests/cli.rs` (`stdio_*` tests) against a fake model server.
