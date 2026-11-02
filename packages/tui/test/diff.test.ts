import assert from 'node:assert/strict';
import { test } from 'node:test';
import { describeChange, summarizeChange } from '../src/diff.js';

test('summarizes an edit with line numbers', () => {
  const summary = summarizeChange({
    path: 'calc.py',
    created: false,
    before: 'def add(a, b):\n    return a - b\n',
    after: 'def add(a, b):\n    return a + b\n',
    too_large: false,
  });
  assert.equal(summary.added, 1);
  assert.equal(summary.removed, 1);
  assert.deepEqual(
    summary.lines.map((line) => [line.kind, line.oldLine, line.newLine, line.text]),
    [
      ['context', 1, 1, 'def add(a, b):'],
      ['remove', 2, null, '    return a - b'],
      ['add', null, 2, '    return a + b'],
    ],
  );
  assert.equal(describeChange(summary), 'Updated calc.py with 1 addition and 1 removal');
});

test('created files count every line as an addition', () => {
  const summary = summarizeChange({ path: 'n.txt', created: true, before: null, after: 'a\nb\n', too_large: false });
  assert.equal(summary.added, 2);
  assert.equal(describeChange(summary), 'Created n.txt with 2 lines');
});

test('separate hunks are joined by a gap row', () => {
  const before = Array.from({ length: 20 }, (_, i) => `line ${i + 1}`).join('\n') + '\n';
  const after = before.replace('line 2\n', 'LINE 2\n').replace('line 19\n', 'LINE 19\n');
  const summary = summarizeChange({ path: 'f', created: false, before, after, too_large: false });
  assert.equal(summary.lines.filter((line) => line.kind === 'gap').length, 1);
  assert.equal(summary.added, 2);
});

test('too-large changes produce no rows', () => {
  const summary = summarizeChange({ path: 'big', created: false, before: null, after: null, too_large: true });
  assert.equal(summary.lines.length, 0);
  assert.match(describeChange(summary), /too large/);
});
