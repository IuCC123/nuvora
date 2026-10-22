/**
 * Nuvora design tokens. Values are platform-neutral numbers and strings; the
 * web turns them into CSS custom properties (`cssVariables`), the TV apps read
 * them as a plain object in StyleSheet code. Plex-like: near-black canvas,
 * warm amber accent, dense poster grids, quick and quiet motion.
 *
 * Names are semantic (what a value is for), never literal (what it looks like),
 * so a theme change edits values, not call sites. DESIGN.md mirrors this file.
 */
export const tokens = {
  color: {
    canvas: "#0f1012",          // app background
    surface: "#1a1c1f",         // cards, panels
    surfaceRaised: "#24272b",   // menus, hovered rows, active nav item
    hairline: "#2c2f34",        // borders on surfaces
    divider: "#1d1f22",         // top bar and nav rail edges against the canvas
    overlayBorder: "rgba(255,255,255,0.18)", // chips over artwork
    overlayPanel: "rgba(26,28,31,0.82)",     // notices over artwork
    overlayHairline: "rgba(255,255,255,0.08)",
    textPrimary: "#f2f2f2",
    textSecondary: "#a6a8ab",
    textMuted: "#85888d",       // the floor for small text: 5.3:1 on canvas, 4.8:1 on surface
    accent: "#e5a00d",          // Plex-amber: primary action, focus ring, progress
    accentHover: "#f0b232",
    accentText: "#141414",      // text on accent
    focusRing: "#e5a00d",
    danger: "#e5534b",
    placeholder: "#1f2226",     // poster before its ThumbHash decodes (one frame at most)
  },
  font: {
    family: "'Inter Variable', Inter, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
    /** Poster lettering on title cards and the wordmark only. Width comes from `titleCard.stretch`. */
    familyDisplay: "'Archivo Variable', Archivo, 'Arial Narrow', sans-serif",
    weight: { regular: 400, medium: 500, semibold: 600, bold: 700, heavy: 800 },
  },
  /** px. TV apps scale by 1.5 at 1080p ("10-foot" rule). */
  type: {
    caption: { size: 12, line: 16 },
    small: { size: 13, line: 18 },
    body: { size: 14, line: 20 },
    bodyLarge: { size: 16, line: 24 },
    title: { size: 20, line: 28 },
    heading: { size: 28, line: 34 },
    headline: { size: 40, line: 44 },
  },
  /** 4 px grid. */
  space: { 0: 0, 1: 4, 2: 8, 3: 12, 4: 16, 5: 24, 6: 32, 7: 48, 8: 64, 9: 96 },
  radius: { none: 0, sm: 4, md: 6, lg: 10, pill: 999 },
  /** px */
  layout: { topBar: 56, navRail: 232, navRailCompact: 72, gutter: 40, detailPoster: 232 },
  poster: {
    /** Card widths per breakpoint; the `small` image (320 px) covers all at 2x. */
    width: { compact: 120, regular: 150, wide: 176 },
    aspectRatio: 2 / 3,
    gap: 16,
  },
  shadow: {
    /** A hovered or focused poster. */
    lift: "0 12px 28px rgba(0,0,0,0.45)",
    /** The detail poster and the primary button resting over artwork. */
    rest: "0 18px 40px rgba(0,0,0,0.5)",
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
  /**
   * The poster a movie gets when it has no artwork. `@nuvora/core`'s
   * `titleCardLook` picks one palette per title, so every client draws the same
   * card. `bloom` is the soft light, `top` and `bottom` the two gradient stops.
   */
  titleCard: {
    /** Archivo width axis, percent: condensed poster lettering. */
    stretch: 64,
    palettes: [
      { bloom: "#2f8f8a", top: "#0f4448", bottom: "#05181b" },
      { bloom: "#b2433b", top: "#5b1a1f", bottom: "#18070a" },
      { bloom: "#4b67c9", top: "#1c2b62", bottom: "#070b1d" },
      { bloom: "#d39a2e", top: "#6e4b12", bottom: "#1c1205" },
      { bloom: "#9a4fb0", top: "#45204f", bottom: "#12081a" },
      { bloom: "#6f9a3f", top: "#2f4a24", bottom: "#0b1408" },
      { bloom: "#8a9bb3", top: "#3a4350", bottom: "#0e1116" },
      { bloom: "#df6a2b", top: "#7a3418", bottom: "#200c05" },
      { bloom: "#3a8fc4", top: "#123d5a", bottom: "#051521" },
      { bloom: "#c2457a", top: "#5a1838", bottom: "#17060f" },
      { bloom: "#3fae7a", top: "#14503a", bottom: "#04170f" },
      { bloom: "#7a6cd6", top: "#2c2566", bottom: "#0b0920" },
    ],
  },
} as const;

export type Tokens = typeof tokens;
export type TitleCardPalette = Tokens["titleCard"]["palettes"][number];

/** Units for numeric tokens, by path prefix. Anything unlisted is unitless (weights, ratios). */
const UNITS: readonly (readonly [prefix: string, unit: string])[] = [
  ["type.", "px"],
  ["space.", "px"],
  ["radius.", "px"],
  ["layout.", "px"],
  ["poster.width.", "px"],
  ["poster.gap", "px"],
  ["motion.duration.", "ms"],
  ["motion.imageFade", "ms"],
  ["titleCard.stretch", "%"],
];

/**
 * `:root { --nv-color-canvas: #0f1012; --nv-space-4: 16px; ... }` generated
 * from `tokens`, so CSS can never name a token that does not exist here. Pure
 * string building; the web app injects it through a Vite virtual module.
 * Title-card palettes are per title, so they are not variables; see `titleCardLook`.
 */
export function cssVariables(t: Tokens = tokens): string {
  const lines: string[] = [];
  const visit = (value: unknown, path: string[]): void => {
    const dotted = path.join(".");
    if (dotted === "titleCard.palettes") return;
    if (typeof value === "object" && value !== null) {
      for (const [key, child] of Object.entries(value)) visit(child, [...path, key]);
      return;
    }
    const unit = typeof value === "number" ? (UNITS.find(([p]) => dotted.startsWith(p))?.[1] ?? "") : "";
    lines.push(`  --nv-${path.map(kebab).join("-")}: ${String(value)}${unit};`);
  };
  visit(t, []);
  return `:root {\n${lines.join("\n")}\n}\n`;
}

function kebab(key: string): string {
  return key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
}
