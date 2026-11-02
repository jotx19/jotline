import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { JotlineEvent } from '../src/protocol.js';
import { expandable, initialState, isExpanded, reducer, turnChanges, type State } from '../src/state.js';

const apply = (state: State, ...events: JotlineEvent[]) =>
  events.reduce((current, event) => reducer(current, { type: 'event', event, now: 1000 }), state);

const ready: JotlineEvent = {
  event: 'ready',
  protocol: 1,
  version: '2.0.0',
  workspace: '/w',
  provider: 'ollama',
  model: 'm',
  planner_model: 'm',
  coder_model: 'm',
  auto_mode: true,
  agent: 'main',
  rules: [],
  tools: ['edit_file'],
};

function runOnce(): State {
  let state = apply(initialState, ready);
  state = reducer(state, { type: 'submitted', prompt: 'fix it' });
  return apply(
    state,
    { event: 'intent', agent: 'main', intent: 'bugfix', read_only: false },
    { event: 'plan', agent: 'main', plan: { intent: null, needs_plan: true, summary: 's', reason: '', steps: ['a'], files: [] } },
    { event: 'tool_started', agent: 'main', id: 7, tool: 'edit_file', args: { path: 'c.py' } },
    {
      event: 'tool_finished',
      agent: 'main',
      id: 7,
      tool: 'edit_file',
      status: 'ok',
      text: 'EDITED: c.py',
      wrote_files: true,
      executed: false,
      changes: [{ path: 'c.py', created: false, before: 'a\n', after: 'b\n', too_large: false }],
    },
    { event: 'run_finished', message: 'done', intent: 'bugfix', steps: 3, files_changed: true, verified: false, handoff: null },
  );
}

test('a run builds one turn with intent, plan, tool, and answer', () => {
  const state = runOnce();
  assert.equal(state.session?.model, 'm');
  assert.equal(state.running, false);
  assert.equal(state.current?.prompt, 'fix it');
  assert.deepEqual(state.current?.items.map((item) => item.kind), ['intent', 'plan', 'tool', 'answer']);
  const tool = state.current?.items[2];
  assert.ok(tool?.kind === 'tool');
  assert.equal(tool.status, 'ok');
  assert.equal(turnChanges(state.current!).length, 1);
});

test('selection moves over expandable items and toggles expansion', () => {
  let state = runOnce();
  assert.equal(expandable(state.current).length, 2);
  state = reducer(state, { type: 'select', delta: -1 });
  assert.equal(state.selected, 1, 'up from nothing selects the last item');
  state = reducer(state, { type: 'select', delta: -5 });
  assert.equal(state.selected, 0, 'clamped at the first item');
  state = reducer(state, { type: 'toggleSelected' });
  assert.ok(isExpanded(state, expandable(state.current)[0]!.key));
  state = reducer(state, { type: 'toggleSelected' });
  assert.ok(!isExpanded(state, expandable(state.current)[0]!.key));
  state = reducer(state, { type: 'toggleExpandAll' });
  assert.ok(isExpanded(state, 'anything'));
});

test('a new prompt commits the previous turn and resets selection', () => {
  let state = reducer(runOnce(), { type: 'select', delta: 1 });
  state = reducer(state, { type: 'submitted', prompt: 'next' });
  assert.equal(state.committed.length, 1);
  assert.equal(state.current?.prompt, 'next');
  assert.equal(state.current?.items.length, 0);
  assert.equal(state.selected, null);
  assert.equal(state.running, true);
});

test('approvals, errors, and thinking update the status', () => {
  let state = apply(initialState, ready, { event: 'thinking', phase: 'plan', message: '' });
  assert.equal(state.thinking?.phase, 'plan');
  state = apply(state, { event: 'approval_request', id: 3, request: 'run shell command\n$ ls' });
  assert.equal(state.approval?.id, 3);
  state = reducer(state, { type: 'approvalAnswered' });
  assert.equal(state.approval, null);
  state = apply(state, { event: 'error', message: 'boom' });
  const last = state.current?.items.at(-1);
  assert.ok(last?.kind === 'notice' && last.tone === 'error' && last.message === 'boom');
});
