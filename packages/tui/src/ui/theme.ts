/** Colors used across the UI. Ink accepts chalk color names and hex values. */
export const theme = {
  accent: '#d97757',
  muted: 'gray',
  success: 'green',
  warning: 'yellow',
  error: 'red',
  info: 'cyan',
  addBg: '#1f3d2a',
  removeBg: '#4a1f24',
  addFg: 'green',
  removeFg: 'red',
  selected: '#d97757',
} as const;

export const GLYPH = {
  bullet: '●',
  result: '⎿',
  spark: '✻',
  selected: '›',
} as const;
