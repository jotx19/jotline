import { Box, Static, Text, useApp, useInput } from 'ink';
import Spinner from 'ink-spinner';
import TextInput from 'ink-text-input';
import { useEffect, useReducer, useState } from 'react';
import type { JotlineClient } from '../client.js';
import { COMMANDS, runSlash, SHORTCUTS, suggest } from '../commands.js';
import { formatDuration } from '../format.js';
import { SUPPORTED_PROTOCOL } from '../protocol.js';
import { expandable, initialState, isExpanded, reducer, type Turn } from '../state.js';
import { Header } from './Header.js';
import { GLYPH, theme } from './theme.js';
import { TurnView } from './TurnView.js';

type StaticEntry = { kind: 'header'; key: string } | { kind: 'turn'; key: string; turn: Turn };

const HELP_TEXT = [
  'Commands:',
  ...COMMANDS.map((command) => `  ${command.usage.padEnd(30)}${command.description}`),
  '',
  'Shortcuts:',
  ...SHORTCUTS.map(([keys, action]) => `  ${keys.padEnd(30)}${action}`),
].join('\n');

export function App({ client }: { client: JotlineClient }) {
  const { exit } = useApp();
  const [state, dispatch] = useReducer(reducer, initialState);
  const [input, setInput] = useState('');
  const [, setTick] = useState(0);

  useEffect(() => {
    const onEvent: Parameters<typeof client.on<'event'>>[1] = (event) => {
      if (event.event === 'ready' && event.protocol !== SUPPORTED_PROTOCOL) {
        dispatch({
          type: 'notice',
          tone: 'warning',
          message: `jotline speaks protocol ${event.protocol}; this UI expects ${SUPPORTED_PROTOCOL}. Update one of them if things look wrong.`,
        });
      }
      dispatch({ type: 'event', event, now: Date.now() });
    };
    const onProtocolError = (message: string) => dispatch({ type: 'notice', tone: 'error', message });
    const onExit = ({ code, stderr, spawnError }: { code: number | null; stderr: string; spawnError?: string }) => {
      const detail = spawnError ?? `jotline exited with code ${code}.${stderr ? `\n${stderr.trim()}` : ''}`;
      dispatch({ type: 'fatal', message: detail });
    };
    client.on('event', onEvent);
    client.on('protocolError', onProtocolError);
    client.on('exit', onExit);
    client.start();
    return () => {
      client.off('event', onEvent);
      client.off('protocolError', onProtocolError);
      client.off('exit', onExit);
    };
  }, [client]);

  // Re-render while something is running so elapsed times stay live.
  useEffect(() => {
    if (!state.running) return;
    const timer = setInterval(() => setTick((tick) => tick + 1), 250);
    return () => clearInterval(timer);
  }, [state.running]);

  const items = expandable(state.current);
  const selectedKey = state.selected === null ? null : (items[state.selected]?.key ?? null);
  const suggestions = suggest(input);
  const inputActive = state.session !== null && !state.running && state.approval === null && state.fatal === null;

  useInput((key, keys) => {
    if (keys.ctrl && key === 'o') {
      dispatch({ type: 'toggleExpandAll' });
      return;
    }
    if (state.approval) {
      if (key === 'y' || key === 'Y') {
        client.send({ type: 'approval', id: state.approval.id, approved: true });
        dispatch({ type: 'approvalAnswered' });
      } else if (key === 'n' || key === 'N' || keys.escape) {
        client.send({ type: 'approval', id: state.approval.id, approved: false });
        dispatch({ type: 'approvalAnswered' });
      }
      return;
    }
    if (state.running) {
      if (keys.escape) {
        client.send({ type: 'interrupt' });
        dispatch({ type: 'notice', tone: 'warning', message: 'Interrupting after the current step…' });
      }
      return;
    }
    if (keys.tab && suggestions[0]) {
      setInput(`${suggestions[0].name} `);
      return;
    }
    if (input === '') {
      if (keys.upArrow) dispatch({ type: 'select', delta: -1 });
      else if (keys.downArrow) dispatch({ type: 'select', delta: 1 });
      else if (keys.escape) dispatch({ type: 'clearSelection' });
      else if (key === ' ' && state.selected !== null) dispatch({ type: 'toggleSelected' });
    }
  });

  const submit = (value: string) => {
    const text = value.trim();
    if (text === '') {
      if (state.selected !== null) dispatch({ type: 'toggleSelected' });
      return;
    }
    setInput('');
    if (text.startsWith('/')) {
      const result = runSlash(text);
      switch (result.kind) {
        case 'send':
          client.send(result.request);
          return;
        case 'help':
          dispatch({ type: 'notice', tone: 'info', message: HELP_TEXT });
          return;
        case 'expand':
          dispatch({ type: 'toggleExpandAll' });
          return;
        case 'exit':
          client.stop();
          exit();
          return;
        case 'error':
          dispatch({ type: 'notice', tone: 'warning', message: result.message });
          return;
      }
    }
    dispatch({ type: 'submitted', prompt: text });
    client.send({ type: 'prompt', text });
  };

  const staticEntries: StaticEntry[] = [
    ...(state.session ? [{ kind: 'header' as const, key: 'header' }] : []),
    ...state.committed.map((turn) => ({ kind: 'turn' as const, key: `turn-${turn.id}`, turn })),
  ];

  return (
    <Box flexDirection="column">
      <Static items={staticEntries}>
        {(entry) =>
          entry.kind === 'header' && state.session ? (
            <Header key={entry.key} session={state.session} />
          ) : entry.kind === 'turn' ? (
            <TurnView key={entry.key} turn={entry.turn} selectedKey={null} isExpanded={() => false} />
          ) : null
        }
      </Static>

      {state.current && (
        <TurnView
          turn={state.current}
          selectedKey={selectedKey}
          isExpanded={(key) => isExpanded(state, key)}
        />
      )}

      {state.fatal !== null && (
        <Box borderStyle="round" borderColor={theme.error} paddingX={1} flexDirection="column">
          <Text color={theme.error} bold>
            jotline stopped
          </Text>
          <Text>{state.fatal}</Text>
          <Text color={theme.muted}>Press ctrl+c to quit.</Text>
        </Box>
      )}

      {state.session === null && state.fatal === null && (
        <Text color={theme.muted}>
          <Spinner type="dots" /> starting jotline…
        </Text>
      )}

      {state.approval && (
        <Box borderStyle="round" borderColor={theme.warning} paddingX={1} flexDirection="column">
          <Text color={theme.warning} bold>
            Approval needed
          </Text>
          {state.approval.request.split('\n').map((line, index) => (
            <Text key={index}>{line}</Text>
          ))}
          <Text>
            <Text color={theme.success} bold>
              y
            </Text>{' '}
            allow ·{' '}
            <Text color={theme.error} bold>
              n
            </Text>{' '}
            deny
          </Text>
        </Box>
      )}

      {state.running && state.approval === null && (
        <Text>
          <Text color={theme.accent}>
            <Spinner type="dots" />{' '}
          </Text>
          <Text color={theme.accent}>{state.thinking ? 'Thinking' : 'Working'}…</Text>
          <Text color={theme.muted}>
            {state.thinking ? ` ${formatDuration(Date.now() - state.thinking.since)}` : ''} (esc to interrupt)
          </Text>
        </Text>
      )}

      {state.session !== null && (
        <Box borderStyle="round" borderColor={inputActive ? theme.muted : 'blackBright'} paddingX={1}>
          <Text color={inputActive ? theme.accent : theme.muted}>&gt; </Text>
          <TextInput
            value={input}
            onChange={(value) => {
              // Tab completes slash commands in useInput; never insert it as text.
              if (value.includes('\t')) return;
              // Space toggles the selected item; do not also type it into the input.
              if (input === '' && value === ' ' && state.selected !== null) return;
              setInput(value);
            }}
            onSubmit={submit}
            focus={inputActive}
            showCursor={inputActive}
            placeholder={inputActive ? 'Describe a task, or type / for commands' : ''}
          />
        </Box>
      )}

      {inputActive && suggestions.length > 0 && (
        <Box flexDirection="column" paddingX={2}>
          {suggestions.map((command, index) => (
            <Text key={command.name} color={index === 0 ? theme.accent : theme.muted}>
              {command.usage.padEnd(30)}
              <Text color={theme.muted}>{command.description}</Text>
            </Text>
          ))}
        </Box>
      )}

      {state.session !== null && (
        <Box justifyContent="space-between" paddingX={2}>
          <Text color={theme.muted}>
            {selectedKey !== null
              ? `${GLYPH.selected} enter/space to expand · ↑↓ move · esc clear`
              : state.expandAll
                ? 'expanded · ctrl+o to collapse'
                : '/help · ↑↓ select · ctrl+o expand all'}
          </Text>
          <Text color={theme.muted}>
            {state.session.autoMode ? 'auto' : 'ask'} · {state.session.model}
          </Text>
        </Box>
      )}
    </Box>
  );
}
