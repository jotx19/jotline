/**
 * UI state, updated by a pure reducer so it can be tested without a terminal.
 *
 * A conversation is a list of turns. Finished turns are printed once into terminal scrollback;
 * only the current (latest) turn stays interactive, so its tool calls can be expanded.
 */
import type { FileChange, Handoff, Intent, JotlineEvent, Plan, RuleInfo, ToolStatus } from './protocol.js';

export type Item =
  | { kind: 'intent'; key: string; intent: Intent; readOnly: boolean }
  | { kind: 'plan'; key: string; agent: string; plan: Plan }
  | {
      kind: 'tool';
      key: string;
      id: number;
      agent: string;
      tool: string;
      args: Record<string, unknown>;
      status: ToolStatus | 'running';
      text: string;
      changes: FileChange[];
      startedAt: number;
      finishedAt: number | null;
    }
  | { kind: 'notice'; key: string; tone: 'info' | 'warning' | 'error'; message: string }
  | {
      kind: 'answer';
      key: string;
      message: string;
      handoff: Handoff | null;
      filesChanged: boolean;
      verified: boolean;
      steps: number;
      intent: Intent | null;
    };

export interface Turn {
  id: number;
  prompt: string | null;
  items: Item[];
}

export interface SessionInfo {
  version: string;
  workspace: string;
  provider: string;
  model: string;
  plannerModel: string;
  coderModel: string;
  autoMode: boolean;
  agent: string;
  rules: RuleInfo[];
  tools: string[];
}

export interface State {
  session: SessionInfo | null;
  committed: Turn[];
  current: Turn | null;
  running: boolean;
  thinking: { phase: string; since: number } | null;
  approval: { id: number; request: string } | null;
  expanded: Set<string>;
  expandAll: boolean;
  selected: number | null;
  nextTurnId: number;
  fatal: string | null;
}

export const initialState: State = {
  session: null,
  committed: [],
  current: null,
  running: false,
  thinking: null,
  approval: null,
  expanded: new Set(),
  expandAll: false,
  selected: null,
  nextTurnId: 1,
  fatal: null,
};

export type Action =
  | { type: 'event'; event: JotlineEvent; now: number }
  | { type: 'submitted'; prompt: string }
  | { type: 'notice'; tone: 'info' | 'warning' | 'error'; message: string }
  | { type: 'select'; delta: number }
  | { type: 'clearSelection' }
  | { type: 'toggleSelected' }
  | { type: 'toggleExpandAll' }
  | { type: 'approvalAnswered' }
  | { type: 'fatal'; message: string };

/** Items that can be selected and expanded. */
export function expandable(turn: Turn | null): Item[] {
  return (turn?.items ?? []).filter((item) => item.kind === 'tool' || item.kind === 'plan');
}

export function isExpanded(state: State, key: string): boolean {
  return state.expandAll || state.expanded.has(key);
}

let noticeCounter = 0;

function startTurn(state: State, prompt: string | null): State {
  return {
    ...state,
    committed: state.current ? [...state.committed, state.current] : state.committed,
    current: { id: state.nextTurnId, prompt, items: [] },
    nextTurnId: state.nextTurnId + 1,
    expanded: new Set(),
    expandAll: false,
    selected: null,
  };
}

function addItem(state: State, item: Item): State {
  const current = state.current ?? { id: state.nextTurnId, prompt: null, items: [] };
  return {
    ...state,
    current: { ...current, items: [...current.items, item] },
    nextTurnId: state.current ? state.nextTurnId : state.nextTurnId + 1,
  };
}

function updateTool(state: State, id: number, update: (item: Extract<Item, { kind: 'tool' }>) => Item): State {
  if (!state.current) return state;
  return {
    ...state,
    current: {
      ...state.current,
      items: state.current.items.map((item) => (item.kind === 'tool' && item.id === id ? update(item) : item)),
    },
  };
}

function notice(state: State, tone: 'info' | 'warning' | 'error', message: string): State {
  noticeCounter += 1;
  return addItem(state, { kind: 'notice', key: `notice-${noticeCounter}`, tone, message });
}

export function reducer(state: State, action: Action): State {
  switch (action.type) {
    case 'submitted':
      return { ...startTurn(state, action.prompt), running: true };
    case 'notice':
      return notice(state, action.tone, action.message);
    case 'fatal':
      return { ...state, fatal: action.message, running: false, thinking: null, approval: null };
    case 'approvalAnswered':
      return { ...state, approval: null };
    case 'toggleExpandAll':
      return { ...state, expandAll: !state.expandAll };
    case 'clearSelection':
      return { ...state, selected: null };
    case 'select': {
      const count = expandable(state.current).length;
      if (count === 0) return state;
      const start = state.selected ?? (action.delta < 0 ? count : -1);
      const next = Math.min(count - 1, Math.max(0, start + action.delta));
      return { ...state, selected: next };
    }
    case 'toggleSelected': {
      const item = state.selected === null ? undefined : expandable(state.current)[state.selected];
      if (!item) return state;
      const expanded = new Set(state.expanded);
      if (expanded.has(item.key)) expanded.delete(item.key);
      else expanded.add(item.key);
      return { ...state, expanded };
    }
    case 'event':
      return applyEvent(state, action.event, action.now);
  }
}

function applyEvent(state: State, event: JotlineEvent, now: number): State {
  switch (event.event) {
    case 'ready':
      return {
        ...state,
        session: {
          version: event.version,
          workspace: event.workspace,
          provider: event.provider,
          model: event.model,
          plannerModel: event.planner_model,
          coderModel: event.coder_model,
          autoMode: event.auto_mode,
          agent: event.agent,
          rules: event.rules,
          tools: event.tools,
        },
      };
    case 'run_started':
      return { ...state, running: true };
    case 'intent':
      return addItem(state, {
        kind: 'intent',
        key: `intent-${state.current?.id ?? 0}`,
        intent: event.intent,
        readOnly: event.read_only,
      });
    case 'plan':
      return addItem(state, {
        kind: 'plan',
        key: `plan-${state.current?.id ?? 0}-${state.current?.items.length ?? 0}`,
        agent: event.agent,
        plan: event.plan,
      });
    case 'thinking':
      return { ...state, thinking: { phase: event.phase, since: now } };
    case 'thinking_done':
      return { ...state, thinking: null };
    case 'tool_started':
      return addItem(state, {
        kind: 'tool',
        key: `tool-${event.id}`,
        id: event.id,
        agent: event.agent,
        tool: event.tool,
        args: event.args,
        status: 'running',
        text: '',
        changes: [],
        startedAt: now,
        finishedAt: null,
      });
    case 'tool_finished':
      return updateTool(state, event.id, (item) => ({
        ...item,
        status: event.status,
        text: event.text,
        changes: event.changes,
        finishedAt: now,
      }));
    case 'notice':
      if (event.kind === 'correction') return notice(state, 'warning', `Corrected the model: ${event.message}`);
      return notice(state, 'info', event.message);
    case 'approval_request':
      return { ...state, approval: { id: event.id, request: event.request } };
    case 'run_finished':
      return {
        ...addItem(state, {
          kind: 'answer',
          key: `answer-${state.current?.id ?? 0}`,
          message: event.message,
          handoff: event.handoff,
          filesChanged: event.files_changed,
          verified: event.verified,
          steps: event.steps,
          intent: event.intent,
        }),
        running: false,
        thinking: null,
        approval: null,
      };
    case 'rules': {
      const session = state.session ? { ...state.session, rules: event.rules } : null;
      const withSession = { ...state, session };
      const lines = event.rules.map((rule) => `  ${rule.origin === 'builtin' ? 'built-in' : 'project '}  ${rule.name}`);
      const header = event.added ? `Rule added to ${event.added}. Loaded rules:` : 'Loaded rules:';
      return notice(withSession, 'info', [header, ...lines].join('\n'));
    }
    case 'tools':
      return notice(state, 'info', ['Tools:', ...event.tools.map((tool) => `  ${tool.name}  ${tool.description}`)].join('\n'));
    case 'status':
      return notice(state, 'info', `agent ${event.agent} · model ${event.model} · ${event.workspace}`);
    case 'reset':
      return notice(startTurn(state, null), 'info', `Cleared the conversation for agent "${event.agent}".`);
    case 'error':
      return notice({ ...state, running: false, thinking: null }, 'error', event.message);
  }
}

/** Totals over every change in a turn, for the answer footer. */
export function turnChanges(turn: Turn): FileChange[] {
  return turn.items.flatMap((item) => (item.kind === 'tool' ? item.changes : []));
}
