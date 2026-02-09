// Shared Zen Brutalism theme colors
export const colors = {
  // Primary palette
  sage: '#8a9e78',
  sageDim: '#637556',
  ochre: '#c4976a',
  terracotta: '#b56e6e',
  terracottaDim: '#8a5555',
  mauve: '#9b7489',
  mauveMid: '#b8909f',
  mauveDeep: '#5e4757',
  mauveLight: '#d4b0bf',

  // Background
  void: '#15121a',
  bgBase: '#1e1b23',
  bgSurface: '#282230',
  bgElevated: '#322b3a',
  bgDeep: '#100e14',

  // Borders
  borderFaint: '#2e2734',
  border: '#3e3646',

  // Text
  textPrimary: '#ddd5d9',
  textSecondary: '#c4bcc0',
  textMuted: '#9a9297',
  textGhost: '#433b3f',

  // Bright variants (for terminal)
  brightBlack: '#433b3f',
  brightRed: '#c47f7f',
  brightGreen: '#9eb08c',
  brightYellow: '#d4a87a',
  brightBlue: '#b8909f',
  brightMagenta: '#d4b0bf',
  brightCyan: '#8a9599',
  brightWhite: '#ebe5e8',

  // Teal accent
  teal: '#7a8589',
  tealLight: '#8a9599',
};

/**
 * Terminal configuration for xterm.js
 * Centralized here to prepare for future settings panel
 */
export const terminalConfig = {
  /** Enable cursor blinking */
  cursorBlink: true,
  /** Cursor style: 'block' | 'underline' | 'bar' */
  cursorStyle: 'block' as const,
  /** Font size in pixels */
  fontSize: 13,
  /** Font family stack with fallbacks */
  fontFamily: '"JetBrains Mono", "Fira Code", Monaco, Consolas, monospace',
  /** Line height multiplier */
  lineHeight: 1.75,
  /** Letter spacing in pixels */
  letterSpacing: 0,
  /** Enable proposed xterm.js APIs */
  allowProposedApi: true,
};

// Xterm.js terminal theme
export const terminalTheme = {
  background: colors.void,
  foreground: colors.textPrimary,
  cursor: colors.mauveMid,
  cursorAccent: colors.void,
  selectionBackground: colors.mauveDeep,
  selectionForeground: colors.textPrimary,
  // ANSI colors - full Zen Brutalism
  black: colors.void,
  red: colors.terracotta,
  green: colors.sage,
  yellow: colors.ochre,
  blue: colors.mauve,
  magenta: colors.mauveMid,
  cyan: colors.teal,
  white: colors.textPrimary,
  // Bright variants
  brightBlack: colors.brightBlack,
  brightRed: colors.brightRed,
  brightGreen: colors.brightGreen,
  brightYellow: colors.brightYellow,
  brightBlue: colors.brightBlue,
  brightMagenta: colors.brightMagenta,
  brightCyan: colors.brightCyan,
  brightWhite: colors.brightWhite,
};

// ReactFlow/topology specific colors (subset)
export const topologyColors = {
  sage: colors.sage,
  sageDim: colors.sageDim,
  ochre: colors.ochre,
  terracotta: colors.terracotta,
  terracottaDim: colors.terracottaDim,
  mauve: colors.mauve,
  borderFaint: colors.borderFaint,
  bgSurface: colors.bgSurface,
};
