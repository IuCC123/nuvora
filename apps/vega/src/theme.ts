import { tokens } from '@nuvora/tokens';

const c = tokens.color;

/** Nuvora's colors, shared with the web and Fire TV apps. */
export const colors = {
  canvas: c.canvas,
  surface: c.surface,
  raised: c.surfaceRaised,
  hairline: c.hairline,
  text: c.textPrimary,
  secondary: c.textSecondary,
  muted: c.textMuted,
  accent: c.accent,
  /** The accent on a focused primary button. */
  accentFocused: '#F5B52A',
  accentText: c.accentText,
  danger: c.danger,
  placeholder: c.placeholder,
} as const;

/**
 * The Fire TV app's type scale. Both lay out in dp on a 960 x 540 screen
 * (2 px each at 1080p), so the same numbers look the same on both.
 */
export const type = {
  display: { fontSize: 40, lineHeight: 46, fontWeight: '700' as const, letterSpacing: -0.5 },
  headlineMedium: { fontSize: 30, lineHeight: 36, fontWeight: '600' as const },
  headlineSmall: { fontSize: 24, lineHeight: 30, fontWeight: '600' as const },
  titleLarge: { fontSize: 20, lineHeight: 26, fontWeight: '600' as const },
  titleMedium: { fontSize: 17, lineHeight: 22, fontWeight: '600' as const },
  titleSmall: { fontSize: 15, lineHeight: 20, fontWeight: '500' as const },
  bodyLarge: { fontSize: 17, lineHeight: 25 },
  bodyMedium: { fontSize: 15, lineHeight: 21 },
  bodySmall: { fontSize: 13, lineHeight: 18 },
  labelLarge: { fontSize: 15, lineHeight: 20, fontWeight: '600' as const },
  labelMedium: { fontSize: 13, lineHeight: 18, fontWeight: '500' as const },
  labelSmall: { fontSize: 11, lineHeight: 14, fontWeight: '600' as const, letterSpacing: 0.6 },
};

/** Screen edges kept clear: TVs crop the outer few percent. */
export const gutter = 48;
export const radius = { sm: 6, md: 8, lg: 10, xl: 14, pill: 999 } as const;

/** The width of a poster in rows and grids: about eight across the screen. */
export const POSTER_WIDTH = 116;
