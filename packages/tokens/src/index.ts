/**
 * Nuvora design tokens. Values are platform-neutral numbers and strings; the
 * web turns them into CSS custom properties (`cssVariables`), the TV apps read
 * them as a plain object in StyleSheet code. The look is a premium streaming
 * service in Apple TV's manner: a deep, slightly cool canvas, artwork that
 * owns the color (the ambient wash comes from the featured art), white for
 * the primary action, focus and progress, and glass used with restraint on
 * chrome that floats over content.
 *
 * Names are semantic (what a value is for), never literal (what it looks like),
 * so a theme change edits values, not call sites. DESIGN.md mirrors this file.
 */
export const tokens = {
  color: {
    canvas: "#060708",          // app background
    surface: "#14161a",         // cards, panels, placeholders under art
    surfaceRaised: "#1e2126",   // hovered rows, highlighted menu items, inputs
    hairline: "rgba(255,255,255,0.12)", // borders on surfaces
    divider: "rgba(255,255,255,0.08)",  // separators against the canvas
    overlayBorder: "rgba(255,255,255,0.32)", // chips over artwork
    overlayPanel: "rgba(22,24,28,0.72)",     // notices over artwork
    overlayHairline: "rgba(255,255,255,0.1)",
    textPrimary: "rgba(255,255,255,0.96)",
    textSecondary: "rgba(235,235,245,0.68)", // 9.6:1 on canvas
    textMuted: "rgba(235,235,245,0.52)",     // the floor for small text: 6.1:1 on canvas, 5.2:1 on surface
    accent: "#ffffff",          // primary action, focus, progress: white, like Apple TV
    accentHover: "#e9e9ec",
    accentText: "#050505",      // text on accent
    focusRing: "#ffffff",
    danger: "#ff5f57",
    placeholder: "#16181c",     // poster before its ThumbHash decodes (one frame at most)
    posterHover: "rgba(6,7,8,0.28)", // dims a hovered poster under its play badge
    video: "#000000",           // the player, so letterboxing disappears
  },
  /** Labels over artwork and glass, as opacities of white. */
  label: {
    primary: "rgba(255,255,255,0.96)",
    secondary: "rgba(255,255,255,0.72)",
    tertiary: "rgba(255,255,255,0.5)",
  },
  /**
   * Glass for chrome that floats over content (the top bar once scrolled,
   * the tab bar, sheets, player controls, buttons over art). Never on
   * content cards. Each material is a tint, a backdrop filter and one stroke.
   */
  material: {
    thin: "rgba(30,32,38,0.38)",
    regular: "rgba(26,28,34,0.56)",
    thick: "rgba(18,20,24,0.78)",
    blurThin: "blur(20px) saturate(170%)",
    blurRegular: "blur(36px) saturate(180%)",
    blurThick: "blur(56px) saturate(190%)",
    stroke: "inset 0 0 0 0.5px rgba(255,255,255,0.14), inset 0 1px 0 rgba(255,255,255,0.1)",
    /** Where backdrop-filter is unavailable or transparency is reduced. */
    solid: "#17191d",
  },
  font: {
    family: "'Inter Variable', Inter, -apple-system, BlinkMacSystemFont, system-ui, 'Segoe UI', Roboto, sans-serif",
    /** Poster lettering on title cards only. Width comes from `titleCard.stretch`. */
    familyDisplay: "'Archivo Variable', Archivo, 'Arial Narrow', sans-serif",
    weight: { regular: 400, medium: 500, semibold: 600, bold: 700, heavy: 800 },
  },
  /** px. TV apps scale by 1.5 at 1080p ("10-foot" rule). */
  type: {
    caption: { size: 12, line: 16 },
    small: { size: 13, line: 18 },
    body: { size: 15, line: 22 },
    bodyLarge: { size: 17, line: 25 },
    title: { size: 22, line: 28 },
    heading: { size: 28, line: 34 },
    headline: { size: 34, line: 40 },
    /** Hero titles; fluid on the web between `displayMin` and `size`. */
    display: { size: 88, line: 88 },
    displayMin: 44,
    /** Primary and secondary buttons. */
    button: { size: 16, line: 20 },
    /** Technical badges. */
    chip: { size: 11, line: 14 },
    wordmark: { size: 20, line: 24 },
  },
  /** 4 px grid. */
  space: { 0: 0, 1: 4, 2: 8, 3: 12, 4: 16, 5: 24, 6: 32, 7: 48, 8: 64, 9: 96 },
  radius: { none: 0, sm: 8, md: 10, card: 12, lg: 20, sheet: 28, pill: 999 },
  /** px */
  layout: { topBar: 72, tabBar: 64, navRail: 232, navRailCompact: 72, gutter: 56, gutterCompact: 16 },
  poster: {
    /** Card widths per breakpoint; the `small` image (320 px) covers all at 2x. */
    width: { compact: 116, regular: 156, wide: 176 },
    aspectRatio: 2 / 3,
    gap: 16,
  },
  shadow: {
    /** A card at rest. */
    card: "0 1px 2px rgba(0,0,0,0.4), 0 8px 24px rgba(0,0,0,0.32)",
    /** A hovered or focused card. */
    lift: "0 2px 6px rgba(0,0,0,0.4), 0 24px 48px rgba(0,0,0,0.55)",
    /** The detail poster and sheets resting over artwork. */
    rest: "0 18px 48px rgba(0,0,0,0.5)",
    /** The primary button and the poster play badge. */
    raised: "0 8px 24px rgba(0,0,0,0.3)",
  },
  motion: {
    /** ms */
    duration: { instant: 80, fast: 160, base: 240, slow: 400, spring: 520, ambient: 1200 },
    easing: {
      standard: "cubic-bezier(0.2, 0, 0, 1)",
      enter: "cubic-bezier(0, 0, 0, 1)",
      exit: "cubic-bezier(0.3, 0, 1, 1)",
      /** A critically damped spring with a touch of overshoot: hover lifts, presses. */
      springSnappy: "linear(0, 0.22 7%, 0.6 16%, 0.86 26%, 0.98 36%, 1.02 46%, 1.01 60%, 1)",
      /** An overdamped spring: page and sheet movement. */
      springSmooth: "linear(0, 0.13 8%, 0.43 19%, 0.72 31%, 0.9 44%, 0.98 58%, 1)",
    },
    /** The placeholder -> image cross-fade. */
    imageFade: 320,
    /** A hovered or focused card's scale. */
    liftScale: 1.06,
  },
  /**
   * The poster a movie gets when it has no artwork. `@nuvora/core`'s
   * `titleCardLook` picks one palette per title, so every client draws the same
   * card. `bloom` is the soft light, `top` and `bottom` the two gradient stops.
   */
  titleCard: {
    /** Archivo width axis, percent: condensed poster lettering. */
    stretch: 64,
    ink: "#f7f2ea",
    inkMuted: "rgba(247,242,234,0.7)",
    vignette: "rgba(0,0,0,0.6)",
    edge: "rgba(255,255,255,0.06)",
    /** px: title and year lettering on a row card, the compact detail poster and the detail poster. */
    type: { title: 23, titleMedium: 26, titleLarge: 36, year: 10.5, yearLarge: 12 },
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
  ["titleCard.type.", "px"],
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
