/** Human-friendly names and one-line summaries for tool calls. */

const str = (args: Record<string, unknown>, key: string): string => {
  const value = args[key];
  return typeof value === 'string' ? value : value === undefined ? '' : JSON.stringify(value);
};

export function clip(text: string, max: number): string {
  const single = text.replace(/\s+/g, ' ').trim();
  return single.length <= max ? single : `${single.slice(0, max - 1)}…`;
}

/** `Update(calc.py)`, `Bash(pytest -q)`, `Search("def add")`. */
export function toolTitle(tool: string, args: Record<string, unknown>): { name: string; detail: string } {
  const path = str(args, 'path');
  switch (tool) {
    case 'read_file':
      return { name: 'Read', detail: path };
    case 'write_file':
      return { name: 'Write', detail: path };
    case 'edit_file':
      return { name: 'Update', detail: path };
    case 'replace_python_symbol':
      return { name: 'Update', detail: `${path} · ${str(args, 'symbol')}` };
    case 'structured_patch': {
      const ops = Array.isArray(args['operations']) ? args['operations'].length : 0;
      return { name: 'Patch', detail: `${ops} operation${ops === 1 ? '' : 's'}` };
    }
    case 'apply_patch':
      return { name: 'Patch', detail: 'unified diff' };
    case 'run_shell':
      return { name: 'Bash', detail: clip(str(args, 'command'), 80) };
    case 'search_files':
      return { name: 'Search', detail: `"${clip(str(args, 'pattern'), 50)}"${path ? ` in ${path}` : ''}` };
    case 'find_files':
      return { name: 'Find', detail: str(args, 'pattern') };
    case 'list_files':
      return { name: 'List', detail: path || '.' };
    case 'python_symbol_overview':
      return { name: 'Symbols', detail: path };
    case 'parallel_tools': {
      const calls = Array.isArray(args['calls']) ? args['calls'] : [];
      const names = calls
        .map((call) => (typeof call === 'object' && call !== null ? String((call as Record<string, unknown>)['tool']) : '?'))
        .join(', ');
      return { name: 'Parallel', detail: clip(names, 70) };
    }
    case 'rules_add':
      return { name: 'Rule', detail: clip(str(args, 'rule'), 70) };
    default:
      if (tool.startsWith('git_')) return { name: 'Git', detail: tool.slice(4).replace('_', ' ') };
      if (tool.startsWith('memory_')) return { name: 'Memory', detail: `${tool.slice(7)} ${str(args, 'key')}`.trim() };
      if (tool.startsWith('plugin.')) return { name: 'Plugin', detail: tool.slice(7) };
      return { name: tool, detail: clip(JSON.stringify(args), 60) };
  }
}

export interface ShellResult {
  exitCode: number | null;
  stdout: string;
  stderr: string;
  timedOut: boolean;
}

/** Parses the `EXIT_CODE / STDOUT / STDERR` text returned by run_shell, git, and plugins. */
export function parseShellResult(text: string): ShellResult | null {
  const match = /EXIT_CODE: (-?\d+)\nSTDOUT:\n([\s\S]*?)\nSTDERR:\n([\s\S]*?)(\nNOTE: process timed out.*)?$/.exec(text);
  if (!match) return null;
  const clean = (value: string | undefined) => (value === undefined || value === '<empty>' ? '' : value);
  return {
    exitCode: Number(match[1]),
    stdout: clean(match[2]),
    stderr: clean(match[3]),
    timedOut: match[4] !== undefined,
  };
}

const countLines = (text: string) => (text === '' ? 0 : text.split('\n').length);

/** The one-line result shown under a finished tool call when it changed no files. */
export function resultSummary(tool: string, status: 'ok' | 'error' | 'denied', text: string): string {
  if (status !== 'ok') return clip(text.split('\n')[0] ?? text, 140);
  const shell = parseShellResult(text);
  if (shell) {
    const output = shell.stdout || shell.stderr;
    const lines = countLines(output);
    const state = shell.timedOut ? 'timed out' : `exit ${shell.exitCode}`;
    return lines === 0 ? `${state} · no output` : `${state} · ${lines} line${lines === 1 ? '' : 's'} of output`;
  }
  if (tool === 'read_file') {
    const header = /\(lines (\d+)-(\d+) of (\d+)\)/.exec(text);
    if (header) return `Read lines ${header[1]}-${header[2]} of ${header[3]}`;
  }
  if (text.startsWith('NO_MATCHES')) return 'No matches';
  if (tool === 'search_files' || tool === 'find_files') {
    const lines = text.split('\n').filter((line) => line && !line.startsWith('NOTE:') && !line.startsWith('...'));
    return `${lines.length} match${lines.length === 1 ? '' : 'es'}`;
  }
  if (tool === 'list_files') return `${countLines(text)} entries`;
  return clip(text.split('\n')[0] ?? '', 140);
}

/** Text to show when a result is expanded: shell output without the protocol framing. */
export function expandedText(text: string): string {
  const shell = parseShellResult(text);
  if (!shell) return text;
  const parts = [];
  if (shell.stdout) parts.push(shell.stdout);
  if (shell.stderr) parts.push(`stderr:\n${shell.stderr}`);
  if (shell.timedOut) parts.push('(timed out and was killed)');
  return parts.join('\n') || '(no output)';
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}m${String(seconds % 60).padStart(2, '0')}s`;
}
