import { structuredPatch } from 'diff';
import type { FileChange } from './protocol.js';

export type DiffLineKind = 'add' | 'remove' | 'context' | 'gap';

/** One rendered diff row. Line numbers are 1-based; `null` where a side has no line. */
export interface DiffLine {
  kind: DiffLineKind;
  oldLine: number | null;
  newLine: number | null;
  text: string;
}

export interface DiffSummary {
  path: string;
  created: boolean;
  tooLarge: boolean;
  added: number;
  removed: number;
  lines: DiffLine[];
}

/** Builds display rows for a change with `context` unchanged lines around each hunk. */
export function summarizeChange(change: FileChange, context = 3): DiffSummary {
  const base = { path: change.path, created: change.created, tooLarge: change.too_large };
  if (change.too_large || change.after === null) {
    return { ...base, added: 0, removed: 0, lines: [] };
  }
  const patch = structuredPatch(change.path, change.path, change.before ?? '', change.after, '', '', {
    context,
  });

  const lines: DiffLine[] = [];
  let added = 0;
  let removed = 0;
  patch.hunks.forEach((hunk, index) => {
    if (index > 0) lines.push({ kind: 'gap', oldLine: null, newLine: null, text: '…' });
    let oldLine = hunk.oldStart;
    let newLine = hunk.newStart;
    for (const raw of hunk.lines) {
      if (raw.startsWith('\\')) continue; // "\ No newline at end of file"
      const marker = raw[0];
      const text = raw.slice(1);
      if (marker === '+') {
        added += 1;
        lines.push({ kind: 'add', oldLine: null, newLine: newLine++, text });
      } else if (marker === '-') {
        removed += 1;
        lines.push({ kind: 'remove', oldLine: oldLine++, newLine: null, text });
      } else {
        lines.push({ kind: 'context', oldLine: oldLine++, newLine: newLine++, text });
      }
    }
  });
  return { ...base, added, removed, lines };
}

/** "Updated calc.py with 1 addition and 1 removal", in the style of the result line. */
export function describeChange(summary: DiffSummary): string {
  if (summary.tooLarge) return `Updated ${summary.path} (too large to show)`;
  const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? '' : 's'}`;
  if (summary.created) return `Created ${summary.path} with ${plural(summary.added, 'line')}`;
  const parts: string[] = [];
  if (summary.added > 0) parts.push(plural(summary.added, 'addition'));
  if (summary.removed > 0) parts.push(plural(summary.removed, 'removal'));
  return parts.length > 0 ? `Updated ${summary.path} with ${parts.join(' and ')}` : `Touched ${summary.path}`;
}
