import { Box, Text } from 'ink';
import type { DiffSummary } from '../diff.js';
import { theme } from './theme.js';

interface Props {
  summary: DiffSummary;
  /** Maximum rows to show; `undefined` shows everything. */
  limit?: number;
}

/** A unified diff with line numbers and colored backgrounds, collapsible to `limit` rows. */
export function DiffView({ summary, limit }: Props) {
  if (summary.tooLarge) {
    return <Text color={theme.muted}>(file too large to show a diff)</Text>;
  }
  const width = String(
    summary.lines.reduce((max, line) => Math.max(max, line.newLine ?? 0, line.oldLine ?? 0), 0),
  ).length;
  const shown = limit === undefined ? summary.lines : summary.lines.slice(0, limit);
  const hidden = summary.lines.length - shown.length;

  return (
    <Box flexDirection="column">
      {shown.map((line, index) => {
        if (line.kind === 'gap') {
          return (
            <Text key={index} color={theme.muted}>
              {' '.repeat(width)} ⋮
            </Text>
          );
        }
        const number = String((line.kind === 'remove' ? line.oldLine : line.newLine) ?? '').padStart(width);
        const marker = line.kind === 'add' ? '+' : line.kind === 'remove' ? '-' : ' ';
        const background = line.kind === 'add' ? theme.addBg : line.kind === 'remove' ? theme.removeBg : undefined;
        const color = line.kind === 'add' ? theme.addFg : line.kind === 'remove' ? theme.removeFg : undefined;
        return (
          <Text key={index} backgroundColor={background}>
            <Text color={theme.muted}>{number} </Text>
            <Text color={color}>{marker} </Text>
            <Text color={line.kind === 'context' ? undefined : color}>{line.text}</Text>
          </Text>
        );
      })}
      {hidden > 0 && (
        <Text color={theme.muted}>
          … +{hidden} more line{hidden === 1 ? '' : 's'} (select and press enter, or ctrl+o)
        </Text>
      )}
    </Box>
  );
}
