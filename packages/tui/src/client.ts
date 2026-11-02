import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { EventEmitter } from 'node:events';
import { createInterface } from 'node:readline';
import { isParseError, parseEvent, type JotlineEvent, type JotlineRequest } from './protocol.js';

export interface ClientEvents {
  event: [JotlineEvent];
  protocolError: [string];
  exit: [{ code: number | null; stderr: string; spawnError?: string }];
}

/**
 * Runs `jotline --stdio` as a child process and exchanges protocol messages with it.
 * `binary` defaults to `$JOTLINE_BIN`, then `jotline` on PATH.
 */
/** When `JOTLINE_TUI_LOG` names a file, every protocol line in both directions is appended to it. */
function debugLog(direction: '<-' | '->', line: string): void {
  const file = process.env['JOTLINE_TUI_LOG'];
  if (file) appendFileSync(file, `${direction} ${line}\n`);
}

export class JotlineClient extends EventEmitter<ClientEvents> {
  private child: ChildProcessWithoutNullStreams | null = null;
  private stderr = '';

  constructor(
    private readonly args: string[],
    private readonly binary: string = process.env['JOTLINE_BIN'] ?? 'jotline',
  ) {
    super();
  }

  start(): void {
    const child = spawn(this.binary, ['--stdio', ...this.args], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.child = child;

    createInterface({ input: child.stdout }).on('line', (line) => {
      if (line.trim() === '') return;
      debugLog('<-', line);
      const parsed = parseEvent(line);
      if (isParseError(parsed)) this.emit('protocolError', parsed.parseError);
      else this.emit('event', parsed);
    });

    child.stderr.on('data', (chunk: Buffer) => {
      // Keep only the tail; it is shown if the process exits unexpectedly.
      this.stderr = (this.stderr + chunk.toString()).slice(-4000);
    });

    child.on('error', (error: NodeJS.ErrnoException) => {
      const spawnError =
        error.code === 'ENOENT'
          ? `could not find the jotline binary "${this.binary}". Install it with ` +
            '`cargo install --path crates/jotline --locked`, or set JOTLINE_BIN to its path.'
          : error.message;
      this.emit('exit', { code: null, stderr: this.stderr, spawnError });
    });

    child.on('exit', (code) => this.emit('exit', { code, stderr: this.stderr }));
  }

  send(request: JotlineRequest): void {
    if (this.child && !this.child.stdin.destroyed) {
      const line = JSON.stringify(request);
      debugLog('->', line);
      this.child.stdin.write(`${line}\n`);
    }
  }

  stop(): void {
    this.send({ type: 'exit' });
    this.child?.stdin.end();
  }
}
