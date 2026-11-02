import assert from 'node:assert/strict';
import { test } from 'node:test';
import { isParseError, parseEvent } from '../src/protocol.js';

test('parses a valid event', () => {
  const event = parseEvent('{"event":"thinking_done","ok":true}');
  assert.ok(!isParseError(event));
  assert.equal(event.event, 'thinking_done');
});

test('rejects non-JSON, non-objects, unknown events, and missing fields', () => {
  const cases: [string, RegExp][] = [
    ['hello', /non-JSON/],
    ['[1,2]', /not a JSON object/],
    ['{"event":"dance"}', /unknown event "dance"/],
    ['{"event":"tool_finished","id":1,"tool":"x"}', /missing status, text, changes/],
  ];
  for (const [line, pattern] of cases) {
    const result = parseEvent(line);
    assert.ok(isParseError(result), line);
    assert.match(result.parseError, pattern);
  }
});
