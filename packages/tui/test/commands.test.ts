import assert from 'node:assert/strict';
import { test } from 'node:test';
import { runSlash, suggest } from '../src/commands.js';

test('slash commands map to protocol requests', () => {
  assert.deepEqual(runSlash('/clear'), { kind: 'send', request: { type: 'command', name: 'reset' } });
  assert.deepEqual(runSlash('/rules'), { kind: 'send', request: { type: 'command', name: 'rules' } });
  assert.deepEqual(runSlash('/rules add  Use pytest  fixtures'), {
    kind: 'send',
    request: { type: 'command', name: 'rules_add', arg: 'Use pytest  fixtures' },
  });
  assert.deepEqual(runSlash('/rules add'), { kind: 'error', message: 'Usage: /rules add <rule>' });
  assert.equal(runSlash('/help').kind, 'help');
  assert.equal(runSlash('/exit').kind, 'exit');
  assert.equal(runSlash('/nope').kind, 'error');
});

test('suggestions match the typed prefix only', () => {
  assert.deepEqual(suggest('/r').map((c) => c.name), ['/rules']);
  assert.equal(suggest('/').length > 3, true);
  assert.deepEqual(suggest('/rules add'), []);
  assert.deepEqual(suggest('hello'), []);
});
