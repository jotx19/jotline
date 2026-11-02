# Architecture

jotline is two crates in one Cargo workspace. `jotline-core` (`crates/jotline-core/`) is the engine: intent classification, the action protocol, the orchestration loop, rules, tools, and model clients. It has no terminal output. `jotline` (`crates/jotline/`) is the command-line front end: argument parsing, the REPL, rendering, and interactive approvals. The front end plugs into the engine through two traits: `Reporter` receives progress events and `Approver` decides whether a side-effecting tool call may run.

```text
crates/jotline            CLI: clap args, REPL, slash commands, panels, spinner, approvals
   │ Reporter / Approver
   ▼
crates/jotline-core
   orchestrator/          operator loop, typed actions, prompts, sessions
   intent/                request classifier and playbooks
   rules/                 built-in + project Markdown rules
   tools/                 workspace boundary, file/search/edit/git/shell/memory/plugins
   llm/                   Ollama and OpenAI-compatible clients
```

## The task loop

`Orchestrator::run_active` runs one task on the active agent:

1. **Classify.** `intent::classify` scores the request against keyword lists and picks an `Intent`. The classifier is deterministic and costs no model call. Ties follow a fixed priority (bugfix, test, refactor, review, docs, feature, ops, question); a request ending in `?` with only weak action words is a question.
2. **Brief.** The user turn sent to the model is the request followed by a task brief: the intent, whether it is read-only, and the intent's numbered playbook.
3. **Plan.** The first valid action must be `plan`. Any other action before a plan is not executed; the model is told to plan first. A plan may set `intent` to override the classifier; the new playbook is then sent with the acknowledgement.
4. **Act.** `tool`, `parallel_tools`, and `delegate` actions run and their results return as the next user turn. Each tool reports its `Effect` (read, write, execute, state), and the loop tracks whether files changed and whether a command ran after the last change.
5. **Verify.** For intents that change code (feature, bugfix, refactor, test, general), a `final` action sent after file changes with no command run since is answered once with a request to verify. A second `final` is accepted so a model that cannot verify (no test runner, missing toolchain) still finishes, and the handoff panel warns that the change is unverified.
6. **Hand off.** The `final` action becomes a `TaskHandoff` (message, summary, completed, verified, missed, next steps) stored in the `RunSummary` with the intent, step count, and verification state.

The loop stops after `--max-steps` model calls. When two or fewer steps remain, tool results tell the model to finish with a `final` action that reports what is missing.

## Action protocol

The model answers every turn with one JSON object:

| `type` | Fields | Effect |
|---|---|---|
| `plan` | `intent`, `needs_plan`, `summary`, `reason`, `steps`, `files`, `focus` | records the plan |
| `tool` | `tool`, `args`, `reason` | runs one tool |
| `parallel_tools` | `calls: [{tool, args}]`, `max_workers` | runs independent tools concurrently |
| `delegate` | `prompt`, `agent_name`, `max_steps`, `reason` | runs a sub-agent task |
| `final` | `message`, `summary`, `completed`, `verified`, `missed`, `next_steps` | ends the task |

Parsing (`orchestrator/action.rs`) is lenient about form and strict about meaning. It strips `<think>...</think>` reasoning blocks, accepts the object alone, inside a fenced code block, or embedded in prose (preferring an object with a `type` field), maps common aliases (`tool_call`, `name`, `arguments`, `answer`), treats a tool name in the `type` field (`{"type":"read_file","path":"a.rs"}`, common with small coder models) as a call to that tool, infers a missing `type` from distinctive fields, treats bare arguments that fit exactly one tool's argument names (`{"command":"pytest -q"}` is `run_shell`) as a call to that tool, and coerces quoted numbers and booleans. An unknown type, missing required field, or wrong shape produces a `ParseError` whose message is sent back as the correction, for example `Unknown action type "dance". Use one of: plan, tool, parallel_tools, delegate, final.` From the second invalid reply in a row, the correction also includes copyable examples of valid actions, so a model stuck in a loop has an exact shape to follow. When a tool call fails with exactly the same tool and arguments as an earlier failed call in the same task, the result also tells the model not to repeat it. The canonical object, not the raw reply, is stored in the conversation so later turns show the model well-formed examples of its own output.

## Prompt and context

The system prompt (`orchestrator/prompt.rs`) has six parts: identity, workspace, protocol, workflow, rules, and the tool catalogue. Tools are rendered as compact text (`- edit_file: ...` with `*` marking required arguments) rather than JSON schemas, which saves several hundred tokens. System prompts are rebuilt whenever rules, plugins, or the workspace change, and they are never stored in session files.

Each model call sends the system prompt and at most `--max-history` recent messages. The window never starts with an assistant message, and the current task's request is always included even when older turns are dropped. Full history stays in memory and in the session file.

## Tools and the workspace boundary

`ToolExecutor` (`tools/mod.rs`) dispatches by name, validates arguments through `Args` (which accepts `"10"` for 10 and `"true"` for true but rejects wrong kinds with a message naming the argument), and returns a `ToolOutput` with a status, text, and effect flags. Every path goes through `Workspace::resolve`, which rejects absolute paths and `..` components and canonicalizes the nearest existing ancestor so symlinks cannot leave the root. Git commands add a `-- .` pathspec so a workspace nested in a larger repository only sees its own files, and `apply_patch` prepends the workspace's repository prefix so patch paths are workspace-relative.

Processes run through `tools/process.rs`: stdout and stderr are drained on separate threads (so large output cannot deadlock on a full pipe), each child leads its own process group, and on timeout the whole group is killed. Output is truncated on character boundaries.

See [tools](TOOLS.md) for the catalogue and result formats.

## Rules

`RuleSet` (`rules/mod.rs`) loads built-in rules compiled from `crates/jotline-core/rules/*.md`, then project rules from `JOTLINE.md`, `AGENTS.md`, and `.jotline/rules/*.md`, with per-file and total size limits. See [rules](RULES.md).

## Agents and delegation

An orchestrator holds named agents, each with its own conversation, tool history, and delegate history. `delegate` runs a child agent with its own intent classification, plan requirement, and verification gate. The child's step budget defaults to half the parent's and never exceeds it, and nesting stops at `--max-delegation-depth`. File changes and verification in a child carry over to the parent's verification state.

## Sessions

`save_session` writes version 4 JSON: the active agent, the delegate counter, and per agent the non-system messages, tool records, and delegate records. `load_session` also reads version 3 files and the older single-agent layout, skipping malformed entries and invalid agent names instead of failing.
