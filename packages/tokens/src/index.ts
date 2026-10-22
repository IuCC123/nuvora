/**
 * Nuvora design tokens. Values are platform-neutral numbers and strings; the
 * web turns them into CSS custom properties (`cssVariables`), the TV apps read
 * them as a plain object in StyleSheet code. Plex-like: near-black canvas,
 * warm amber accent, dense poster grids, quick and quiet motion.
 *
 * Names are semantic (what a value is for), never literal (what it looks like),
 * so a theme change edits values, not call sites.
 */
export const tokens = {
  color: {
    canvas: "#0f1012",          // app background
    surface: "#1a1c1f",         // cards, panels
    surfaceRaised: "#24272b",   // menus, focused rows
    scrim: "rgba(0,0,0,0.6)",   // over backdrops
    textPrimary: "#f2f2f2",
    textSecondary: "#a6a8ab",
    textMuted: "#6d7075",
    accent: "#e5a00d",          // Plex-amber: primary action, focus ring, progress
    accentText: "#141414",      // text on accent
    focusRing: "#e5a00d",
    danger: "#e5534b",
    placeholder: "#1f2226",     // poster before its ThumbHash decodes (one frame at most)
  },
  font: {
    family: "'Inter Variable', 'Inter', system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
    familyNumeric: "'Inter Variable', system-ui, sans-serif",
    weight: { regular: 400, medium: 500, semibold: 600, bold: 700 },
  },
  /** px; [size, lineHeight]. TV apps scale by 1.5 at 1080p ("10-foot" rule). */
  type: {
    caption: [12, 16],
    body: [14, 20],
    bodyLarge: [16, 24],
    title: [20, 28],
    headline: [28, 34],
    display: [44, 50],
  },
  /** 4 px grid. */
  space: { 0: 0, 1: 4, 2: 8, 3: 12, 4: 16, 5: 24, 6: 32, 7: 48, 8: 64, 9: 96 },
  radius: { none: 0, sm: 4, md: 6, lg: 10, pill: 999 },
  poster: {
    /** Card widths per breakpoint; the `small` image (320 px) covers all at 2x. */
    width: { compact: 120, regular: 150, wide: 176 },
    aspectRatio: 2 / 3,
    gap: 12,
  },
  motion: {
    /** ms */
    duration: { instant: 80, fast: 160, base: 240, slow: 400 },
    easing: {
      standard: "cubic-bezier(0.2, 0, 0, 1)",
      enter: "cubic-bezier(0, 0, 0, 1)",
      exit: "cubic-bezier(0.3, 0, 1, 1)",
    },
    /** The placeholder -> image cross-fade. */
    imageFade: 240,
  },
} as const;

export type Tokens = typeof tokens;

/**
 * `:root { --nv-color-canvas: #0f1012; --nv-space-4: 16px; ... }` generated
 * from `tokens`, so CSS can never name a token that does not exist here. Pure
 * string building; the web app injects it through a Vite virtual module.
 */
export function cssVariables(t: Tokens = tokens): string {
  void t;
  throw new Error("not implemented: flatten keys with '-', px-suffix numeric space/radius/type, emit :root block");
}
