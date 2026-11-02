import { homedir } from 'node:os';
import { Box, Text } from 'ink';
import type { SessionInfo } from '../state.js';
import { GLYPH, theme } from './theme.js';

const tildify = (path: string) => (path.startsWith(homedir()) ? `~${path.slice(homedir().length)}` : path);

export function Header({ session }: { session: SessionInfo }) {
  const builtin = session.rules.filter((rule) => rule.origin === 'builtin').length;
  const project = session.rules.length - builtin;
  const models =
    session.plannerModel === session.coderModel
      ? session.model
      : `${session.plannerModel} (plan) · ${session.coderModel} (code)`;
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box borderStyle="round" borderColor={theme.accent} paddingX={1} flexDirection="column">
        <Text>
          <Text color={theme.accent}>{GLYPH.spark} </Text>
          <Text bold>jotline</Text>
          <Text color={theme.muted}> {session.version}</Text>
        </Text>
        <Text color={theme.muted}>  {tildify(session.workspace)}</Text>
        <Text color={theme.muted}>
          {'  '}
          {models} · {session.provider} · {session.autoMode ? 'auto-approve' : 'ask before commands'}
        </Text>
        <Text color={theme.muted}>
          {'  '}rules: {builtin} built-in, {project} project · {session.tools.length} tools
        </Text>
      </Box>
      <Text color={theme.muted}>
        {' '}Describe a task · /help for commands · ↑↓ select · enter expand · ctrl+o expand all
      </Text>
    </Box>
  );
}
