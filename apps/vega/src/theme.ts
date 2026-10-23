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
  accentText: c.accentText,
  danger: c.danger,
  placeholder: c.placeholder,
} as const;

/**
 * Layout is in dp on a 960 x 540 screen (2 px each at 1080p, as on Fire TV).
 * Type follows the TV app scale the Fire TV app uses: the web's sizes read
 * as dp, the headline a step smaller.
 */
export const type = {
  small: { fontSize: 12, lineHeight: 16 },
  body: { fontSize: 14, lineHeight: 20 },
  bodyLarge: { fontSize: 16, lineHeight: 22 },
  title: { fontSize: 20, lineHeight: 26, fontWeight: '600' as const },
  heading: { fontSize: 26, lineHeight: 32, fontWeight: '700' as const },
  headline: { fontSize: 34, lineHeight: 40, fontWeight: '700' as const },
};

/** Screen edges kept clear: TVs crop the outer few percent. */
export const gutter = 48;
export const radius = { sm: 4, md: 8, lg: 12, pill: 999 } as const;
