import type { JotlineRequest } from './protocol.js';

export interface SlashCommand {
  name: string;
  usage: string;
  description: string;
}

export const COMMANDS: SlashCommand[] = [
  { name: '/help', usage: '/help', description: 'Show commands and keyboard shortcuts' },
  { name: '/clear', usage: '/clear', description: 'Start a fresh conversation (keeps rules and memory)' },
  { name: '/rules', usage: '/rules [add <rule> | reload]', description: 'List rules, save a new one, or re-read rule files' },
  { name: '/tools', usage: '/tools', description: 'List the tools the agent can use' },
  { name: '/status', usage: '/status', description: 'Show agent, model, and workspace' },
  { name: '/expand', usage: '/expand', description: 'Expand or collapse every item in the current turn' },
  { name: '/exit', usage: '/exit', description: 'Quit jotline' },
];

export const SHORTCUTS: [string, string][] = [
  ['enter', 'send the message'],
  ['↑ / ↓', 'select a tool call or plan (when the input is empty)'],
  ['enter / space', 'expand or collapse the selected item'],
  ['ctrl+o', 'expand or collapse everything in the current turn'],
  ['esc', 'interrupt the running task, or clear the selection'],
  ['tab', 'complete a slash command'],
  ['y / n', 'answer an approval prompt'],
  ['ctrl+c', 'quit'],
];

export type SlashResult =
  | { kind: 'send'; request: JotlineRequest; echo?: string }
  | { kind: 'help' }
  | { kind: 'expand' }
  | { kind: 'exit' }
  | { kind: 'error'; message: string };

/** Interprets a line that starts with `/`. */
export function runSlash(line: string): SlashResult {
  const [command = '', ...rest] = line.trim().split(/\s+/);
  const argument = line.trim().slice(command.length).trim();
  switch (command) {
    case '/help':
      return { kind: 'help' };
    case '/clear':
    case '/reset':
      return { kind: 'send', request: { type: 'command', name: 'reset' } };
    case '/rules': {
      const [sub = ''] = rest;
      if (sub === '') return { kind: 'send', request: { type: 'command', name: 'rules' } };
      if (sub === 'reload') return { kind: 'send', request: { type: 'command', name: 'reload_rules' } };
      if (sub === 'add') {
        const rule = argument.slice(3).trim();
        if (rule === '') return { kind: 'error', message: 'Usage: /rules add <rule>' };
        return { kind: 'send', request: { type: 'command', name: 'rules_add', arg: rule } };
      }
      return { kind: 'error', message: 'Usage: /rules [add <rule> | reload]' };
    }
    case '/tools':
      return { kind: 'send', request: { type: 'command', name: 'tools' } };
    case '/status':
      return { kind: 'send', request: { type: 'command', name: 'status' } };
    case '/expand':
      return { kind: 'expand' };
    case '/exit':
    case '/quit':
      return { kind: 'exit' };
    default:
      return { kind: 'error', message: `Unknown command ${command}. Type /help for the list.` };
  }
}

/** Commands whose name starts with what has been typed so far. */
export function suggest(input: string): SlashCommand[] {
  if (!input.startsWith('/') || input.includes(' ')) return [];
  return COMMANDS.filter((command) => command.name.startsWith(input));
}
