# jotline-tui

An interactive terminal UI for [jotline](../../README.md), written in TypeScript with [Ink](https://github.com/vadimdemedes/ink). It runs `jotline --stdio` and renders what the agent does as live, collapsible items instead of a scrolling log.

```text
> the test in test_calc.py fails, fix the bug in calc.py

  ✻ intent bugfix

● Plan · Fix add in calc.py
  ⎿  3 steps · 1. Read calc.py

● Update(calc.py) 0.3s
  ⎿  Updated calc.py with 1 addition and 1 removal
     1   def add(a, b):
     2 -     return a - b
     2 +     return a + b

● Bash(python3 -c "import test_calc; test_calc.test_add()") 0.1s
  ⎿  exit 0 · 1 line of output

● Fixed add: it now returns a + b.
    verified
    ✓ python3 test -> ok
  1 file changed (+1 −1) · 4 steps · bugfix
╭──────────────────────────────────────────────────────╮
│ > Describe a task, or type / for commands            │
╰──────────────────────────────────────────────────────╯
  /help · ↑↓ select · ctrl+o expand all      auto · qwen2.5-coder:7b
```

## Features

- **Live tool calls**: each call appears as `Update(calc.py)`, `Bash(pytest -q)`, `Search("def add")` with a spinner while it runs and a one-line result when it finishes.
- **Diffs of every change**: edits, writes, patches, and symbol replacements show a colored diff with line numbers, built from the exact before and after contents.
- **Expand and collapse**: diffs and outputs start collapsed. Select an item with ↑/↓ and press enter or space to expand it, or press ctrl+o to expand everything in the current turn.
- **Inline approvals**: with `--ask-permissions`, shell commands, plugins, and commits ask `y`/`n` in place.
- **Interrupt**: esc stops the running task after its current step.
- **Slash commands with completion**: type `/` to see commands; tab completes.
- **Clean scrollback**: finished turns are printed once into the terminal's scrollback; only the latest turn stays interactive.

## Install and run

You need Node.js 18.18 or newer and the `jotline` binary.

```bash
# 1. Install the jotline binary (from the repository root)
cargo install --path crates/jotline --locked

# 2. Build and link the UI
cd packages/tui
npm ci
npm run build
npm link            # makes `jotline-tui` available everywhere

# 3. Run it in a project
cd ~/my-project
jotline-tui --model qwen2.5-coder:7b
```

Every argument is passed to `jotline --stdio`, so `-w`, `--model`, `--ask-permissions`, `--session-file`, and the rest work as in [configuration](../../docs/CONFIGURATION.md). The UI finds the binary through `$JOTLINE_BIN`, then `jotline` on `PATH`. To use a local build without installing it:

```bash
JOTLINE_BIN=../../target/release/jotline node dist/src/cli.js -w ~/my-project
```

## Keys

| Key | Action |
|---|---|
| enter | send the message |
| ↑ / ↓ | select a tool call or plan in the current turn (when the input is empty) |
| enter / space | expand or collapse the selected item |
| ctrl+o | expand or collapse everything in the current turn |
| esc | interrupt the running task, deny a pending approval, or clear the selection |
| tab | complete a slash command |
| y / n | answer an approval prompt |
| ctrl+c | quit |

## Commands

| Command | Effect |
|---|---|
| `/help` | commands and shortcuts |
| `/clear` | start a fresh conversation (rules and memory are kept) |
| `/rules`, `/rules add <rule>`, `/rules reload` | list, add, or re-read rules |
| `/tools` | list available tools |
| `/status` | agent, model, and workspace |
| `/expand` | same as ctrl+o |
| `/exit` | quit |

## Development

```bash
npm ci
npm run typecheck
npm test            # builds, then runs node:test on dist/test/*.test.js
npm start -- -w ~/jotline-playground --model qwen2.5-coder:7b
```

Set `JOTLINE_TUI_LOG=/path/to/file` to append every protocol line exchanged with `jotline --stdio` to that file, which is the quickest way to see what the agent sent.

Source layout: `src/protocol.ts` (event and request types, validated parsing), `src/client.ts` (child process), `src/state.ts` (pure reducer), `src/diff.ts` and `src/format.ts` (display logic), `src/ui/` (Ink components). The protocol is specified in [docs/PROTOCOL.md](../../docs/PROTOCOL.md).
