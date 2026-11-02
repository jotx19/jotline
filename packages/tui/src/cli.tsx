#!/usr/bin/env node
import { render } from 'ink';
import { JotlineClient } from './client.js';
import { App } from './ui/App.js';

const args = process.argv.slice(2);

if (args.includes('--help') || args.includes('-h')) {
  process.stdout.write(`jotline-tui: interactive terminal UI for jotline

Usage: jotline-tui [jotline options]

Every option is passed to \`jotline --stdio\`, for example:
  jotline-tui --model qwen2.5-coder:7b
  jotline-tui -w ./services/api --ask-permissions

The jotline binary is found through $JOTLINE_BIN, then \`jotline\` on PATH.
Run \`jotline --help\` for the full list of options.
`);
  process.exit(0);
}

if (!process.stdin.isTTY) {
  process.stderr.write('jotline-tui needs an interactive terminal. Use `jotline -p "..."` for scripts.\n');
  process.exit(1);
}

const client = new JotlineClient(args);
const app = render(<App client={client} />, { exitOnCtrlC: true });
await app.waitUntilExit();
client.stop();
