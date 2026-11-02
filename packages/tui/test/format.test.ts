import assert from 'node:assert/strict';
import { test } from 'node:test';
import { expandedText, formatDuration, parseShellResult, resultSummary, toolTitle } from '../src/format.js';

test('tool titles read like actions', () => {
  assert.deepEqual(toolTitle('edit_file', { path: 'calc.py' }), { name: 'Update', detail: 'calc.py' });
  assert.deepEqual(toolTitle('run_shell', { command: 'pytest -q' }), { name: 'Bash', detail: 'pytest -q' });
  assert.deepEqual(toolTitle('search_files', { pattern: 'def add', path: 'src' }), {
    name: 'Search',
    detail: '"def add" in src',
  });
  assert.equal(toolTitle('git_status', {}).name, 'Git');
  assert.equal(toolTitle('plugin.lint', {}).detail, 'lint');
  assert.equal(toolTitle('parallel_tools', { calls: [{ tool: 'read_file' }, { tool: 'list_files' }] }).detail, 'read_file, list_files');
});

const shell = 'EXIT_CODE: 0\nSTDOUT:\nok\nline two\nSTDERR:\n<empty>';

test('parses shell results', () => {
  assert.deepEqual(parseShellResult(shell), { exitCode: 0, stdout: 'ok\nline two', stderr: '', timedOut: false });
  const timedOut = parseShellResult('EXIT_CODE: -1\nSTDOUT:\n<empty>\nSTDERR:\n<empty>\nNOTE: process timed out and was killed');
  assert.equal(timedOut?.timedOut, true);
  assert.equal(parseShellResult('PATCH APPLIED'), null);
});

test('result summaries are one short line', () => {
  assert.equal(resultSummary('run_shell', 'ok', shell), 'exit 0 · 2 lines of output');
  assert.equal(resultSummary('read_file', 'ok', 'FILE: a (lines 1-2 of 2)\n    1: x'), 'Read lines 1-2 of 2');
  assert.equal(resultSummary('search_files', 'ok', 'a:1: x\nb:2: y'), '2 matches');
  assert.equal(resultSummary('search_files', 'ok', "NO_MATCHES: 'x' under ."), 'No matches');
  assert.equal(resultSummary('edit_file', 'error', "ERROR: 'old' text not found\nCurrent text..."), "ERROR: 'old' text not found");
  assert.equal(expandedText(shell), 'ok\nline two');
});

test('durations', () => {
  assert.equal(formatDuration(250), '250ms');
  assert.equal(formatDuration(2500), '2.5s');
  assert.equal(formatDuration(125_000), '2m05s');
});
