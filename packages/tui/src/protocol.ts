/**
 * Types for the `jotline --stdio` protocol (docs/PROTOCOL.md). Every stdout line is one event;
 * every stdin line is one request.
 */

export type Intent =
  | 'question'
  | 'feature'
  | 'bugfix'
  | 'refactor'
  | 'test'
  | 'review'
  | 'docs'
  | 'ops'
  | 'general';

export interface RuleInfo {
  origin: 'builtin' | 'project';
  name: string;
  chars: number;
  truncated: boolean;
}

export interface Plan {
  intent: Intent | null;
  needs_plan: boolean;
  summary: string;
  reason: string;
  steps: string[];
  files: string[];
}

/** One file modified by a tool call. `before` is null for created files. */
export interface FileChange {
  path: string;
  created: boolean;
  before: string | null;
  after: string | null;
  too_large: boolean;
}

export interface Handoff {
  message: string;
  summary: string;
  completed: string[];
  verified: string[];
  missed: string[];
  next_steps: string[];
}

export type ToolStatus = 'ok' | 'error' | 'denied';

export type JotlineEvent =
  | {
      event: 'ready';
      protocol: number;
      version: string;
      workspace: string;
      provider: string;
      model: string;
      planner_model: string;
      coder_model: string;
      auto_mode: boolean;
      agent: string;
      rules: RuleInfo[];
      tools: string[];
    }
  | { event: 'run_started'; agent: string; prompt: string }
  | { event: 'intent'; agent: string; intent: Intent; read_only: boolean }
  | { event: 'plan'; agent: string; plan: Plan }
  | { event: 'thinking'; phase: string; message: string }
  | { event: 'thinking_done'; ok: boolean }
  | { event: 'tool_started'; agent: string; id: number; tool: string; args: Record<string, unknown> }
  | {
      event: 'tool_finished';
      agent: string;
      id: number;
      tool: string;
      status: ToolStatus;
      text: string;
      wrote_files: boolean;
      executed: boolean;
      changes: FileChange[];
    }
  | { event: 'notice'; kind: 'correction' | 'delegate' | 'delegate_result'; agent: string; message: string }
  | { event: 'approval_request'; id: number; request: string }
  | {
      event: 'run_finished';
      agent?: string;
      message: string;
      intent: Intent | null;
      steps: number;
      files_changed: boolean;
      verified: boolean;
      handoff: Handoff | null;
    }
  | { event: 'rules'; rules: RuleInfo[]; added?: string }
  | { event: 'tools'; tools: { name: string; description: string }[] }
  | { event: 'status'; agent: string; workspace: string; model: string; last_run: unknown }
  | { event: 'reset'; agent: string }
  | { event: 'error'; message: string };

export type JotlineRequest =
  | { type: 'prompt'; text: string }
  | { type: 'command'; name: 'reset' | 'rules' | 'rules_add' | 'reload_rules' | 'tools' | 'status'; arg?: string }
  | { type: 'approval'; id: number; approved: boolean }
  | { type: 'interrupt' }
  | { type: 'exit' };

/** Fields each event must carry, checked before an event reaches the UI. */
const REQUIRED: Record<JotlineEvent['event'], string[]> = {
  ready: ['protocol', 'version', 'workspace', 'model', 'tools', 'rules'],
  run_started: ['prompt'],
  intent: ['intent'],
  plan: ['plan'],
  thinking: ['phase'],
  thinking_done: ['ok'],
  tool_started: ['id', 'tool', 'args'],
  tool_finished: ['id', 'tool', 'status', 'text', 'changes'],
  notice: ['kind', 'message'],
  approval_request: ['id', 'request'],
  run_finished: ['message'],
  rules: ['rules'],
  tools: ['tools'],
  status: ['agent'],
  reset: ['agent'],
  error: ['message'],
};

/** Protocol versions this UI understands. */
export const SUPPORTED_PROTOCOL = 1;

/**
 * Parses one stdout line. Returns an error string instead of throwing, so a malformed line is
 * shown to the user rather than crashing the UI.
 */
export function parseEvent(line: string): JotlineEvent | { parseError: string } {
  let value: unknown;
  try {
    value = JSON.parse(line);
  } catch {
    return { parseError: `jotline printed a non-JSON line: ${line.slice(0, 200)}` };
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return { parseError: 'event is not a JSON object' };
  }
  const record = value as Record<string, unknown>;
  const kind = record['event'];
  if (typeof kind !== 'string' || !(kind in REQUIRED)) {
    return { parseError: `unknown event ${JSON.stringify(kind)}` };
  }
  const missing = REQUIRED[kind as JotlineEvent['event']].filter((key) => !(key in record));
  if (missing.length > 0) {
    return { parseError: `event "${kind}" is missing ${missing.join(', ')}` };
  }
  return record as JotlineEvent;
}

export function isParseError(value: JotlineEvent | { parseError: string }): value is { parseError: string } {
  return 'parseError' in value;
}
