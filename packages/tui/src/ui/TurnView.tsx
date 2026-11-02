import { Box, Text } from 'ink';
import type { ReactNode } from 'react';
import Spinner from 'ink-spinner';
import { describeChange, summarizeChange } from '../diff.js';
import { clip, expandedText, formatDuration, resultSummary, toolTitle } from '../format.js';
import type { Item, Turn } from '../state.js';
import { turnChanges } from '../state.js';
import { DiffView } from './DiffView.js';
import { GLYPH, theme } from './theme.js';

const COLLAPSED_DIFF_ROWS = 10;
const EXPANDED_TEXT_ROWS = 80;

interface TurnProps {
  turn: Turn;
  /** Key of the selected item, highlighted with a marker. */
  selectedKey: string | null;
  isExpanded: (key: string) => boolean;
}

/** One user prompt and everything the agent did for it. */
export function TurnView({ turn, selectedKey, isExpanded }: TurnProps) {
  return (
    <Box flexDirection="column" marginBottom={1}>
      {turn.prompt !== null && (
        <Box marginBottom={1}>
          <Text color={theme.muted}>&gt; </Text>
          <Text>{turn.prompt}</Text>
        </Box>
      )}
      {turn.items.map((item) => (
        <ItemView
          key={item.key}
          item={item}
          turn={turn}
          selected={item.key === selectedKey}
          expanded={isExpanded(item.key)}
        />
      ))}
    </Box>
  );
}

interface ItemProps {
  item: Item;
  turn: Turn;
  selected: boolean;
  expanded: boolean;
}

function Marker({ selected, color }: { selected: boolean; color: string }) {
  return selected ? <Text color={theme.selected}>{GLYPH.selected} </Text> : <Text color={color}>{GLYPH.bullet} </Text>;
}

function Result({ children }: { children: ReactNode }) {
  return (
    <Box paddingLeft={2}>
      <Text color={theme.muted}>{GLYPH.result}  </Text>
      <Box flexDirection="column" flexGrow={1}>
        {children}
      </Box>
    </Box>
  );
}

function ItemView({ item, turn, selected, expanded }: ItemProps) {
  switch (item.kind) {
    case 'intent':
      return (
        <Box marginBottom={1}>
          <Text color={theme.muted}>
            {'  '}
            {GLYPH.spark} intent <Text color={theme.accent}>{item.intent}</Text>
            {item.readOnly ? ' · read-only' : ''}
          </Text>
        </Box>
      );

    case 'plan':
      return (
        <Box flexDirection="column" marginBottom={1}>
          <Text>
            <Marker selected={selected} color={theme.accent} />
            <Text bold>Plan</Text>
            <Text color={theme.muted}> · {item.plan.summary}</Text>
          </Text>
          <Result>
            {expanded ? (
              <>
                {item.plan.reason !== '' && <Text color={theme.muted}>why: {item.plan.reason}</Text>}
                {item.plan.files.length > 0 && <Text color={theme.muted}>files: {item.plan.files.join(', ')}</Text>}
                {item.plan.steps.map((step, index) => (
                  <Text key={index}>
                    <Text color={theme.muted}>☐ </Text>
                    {step}
                  </Text>
                ))}
              </>
            ) : (
              <Text color={theme.muted}>
                {item.plan.steps.length} step{item.plan.steps.length === 1 ? '' : 's'}
                {item.plan.steps[0] ? ` · 1. ${clip(item.plan.steps[0], 70)}` : ''}
              </Text>
            )}
          </Result>
        </Box>
      );

    case 'tool': {
      const title = toolTitle(item.tool, item.args);
      const color =
        item.status === 'running'
          ? theme.muted
          : item.status === 'ok'
            ? theme.success
            : item.status === 'denied'
              ? theme.warning
              : theme.error;
      const elapsed = formatDuration((item.finishedAt ?? Date.now()) - item.startedAt);
      return (
        <Box flexDirection="column" marginBottom={1}>
          <Text>
            {item.status === 'running' ? (
              <Text color={theme.accent}>
                <Spinner type="dots" />{' '}
              </Text>
            ) : (
              <Marker selected={selected} color={color} />
            )}
            <Text bold>{title.name}</Text>
            <Text>({title.detail})</Text>
            {item.agent !== 'main' && <Text color={theme.muted}> · {item.agent}</Text>}
            {item.status !== 'running' && <Text color={theme.muted}> {elapsed}</Text>}
          </Text>
          {item.status !== 'running' && <ToolResult item={item} expanded={expanded} />}
        </Box>
      );
    }

    case 'notice': {
      const color = item.tone === 'error' ? theme.error : item.tone === 'warning' ? theme.warning : theme.info;
      return (
        <Box marginBottom={1} paddingLeft={2}>
          <Text color={color}>{item.message}</Text>
        </Box>
      );
    }

    case 'answer':
      return <Answer item={item} turn={turn} />;
  }
}

function ToolResult({ item, expanded }: { item: Extract<Item, { kind: 'tool' }>; expanded: boolean }) {
  if (item.changes.length > 0 && item.status === 'ok') {
    return (
      <Result>
        {item.changes.map((change) => {
          const summary = summarizeChange(change);
          return (
            <Box key={change.path} flexDirection="column">
              <Text>{describeChange(summary)}</Text>
              <DiffView summary={summary} limit={expanded ? undefined : COLLAPSED_DIFF_ROWS} />
            </Box>
          );
        })}
      </Result>
    );
  }

  const color = item.status === 'ok' ? undefined : item.status === 'denied' ? theme.warning : theme.error;
  const summary = resultSummary(item.tool, item.status === 'running' ? 'ok' : item.status, item.text);
  if (!expanded) {
    return (
      <Result>
        <Text color={color}>{summary}</Text>
      </Result>
    );
  }
  const lines = expandedText(item.text).split('\n');
  const shown = lines.slice(0, EXPANDED_TEXT_ROWS);
  return (
    <Result>
      <Text color={color}>{summary}</Text>
      <Box flexDirection="column" borderStyle="single" borderColor={theme.muted} paddingX={1}>
        {shown.map((line, index) => (
          <Text key={index} color={theme.muted}>
            {line === '' ? ' ' : line}
          </Text>
        ))}
        {lines.length > shown.length && (
          <Text color={theme.muted}>… {lines.length - shown.length} more lines</Text>
        )}
      </Box>
    </Result>
  );
}

function Answer({ item, turn }: { item: Extract<Item, { kind: 'answer' }>; turn: Turn }) {
  const handoff = item.handoff;
  const changes = turnChanges(turn).map((change) => summarizeChange(change));
  const files = new Set(changes.map((change) => change.path));
  const added = changes.reduce((sum, change) => sum + change.added, 0);
  const removed = changes.reduce((sum, change) => sum + change.removed, 0);
  const list = (title: string, items: string[], color: string, glyph: string) =>
    items.length > 0 && (
      <Box flexDirection="column" marginTop={1}>
        <Text color={theme.muted}>{title}</Text>
        {items.map((entry, index) => (
          <Text key={index}>
            <Text color={color}>{glyph} </Text>
            {entry}
          </Text>
        ))}
      </Box>
    );

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text>
        <Text color={handoff ? theme.success : theme.warning}>{GLYPH.bullet} </Text>
        {item.message}
      </Text>
      {handoff && (
        <Box flexDirection="column" paddingLeft={2}>
          {list('completed', handoff.completed, theme.success, '✓')}
          {list('verified', handoff.verified, theme.success, '✓')}
          {list('missed', handoff.missed, theme.warning, '!')}
          {list('next steps', handoff.next_steps, theme.info, '→')}
        </Box>
      )}
      <Box paddingLeft={2} marginTop={1}>
        <Text color={theme.muted}>
          {files.size > 0 ? (
            <>
              {files.size} file{files.size === 1 ? '' : 's'} changed (<Text color={theme.addFg}>+{added}</Text>{' '}
              <Text color={theme.removeFg}>−{removed}</Text>) ·{' '}
            </>
          ) : null}
          {item.steps} step{item.steps === 1 ? '' : 's'}
          {item.intent ? ` · ${item.intent}` : ''}
          {item.filesChanged && !item.verified ? (
            <Text color={theme.warning}> · not verified after the last edit</Text>
          ) : null}
        </Text>
      </Box>
    </Box>
  );
}
