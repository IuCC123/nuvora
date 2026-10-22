import { tokens, type TitleCardPalette } from "@nuvora/tokens";

export interface TitleCardLook {
  palette: TitleCardPalette;
  /** Center of the light bloom, in percent of the card's width and height. */
  bloomX: number;
  bloomY: number;
}

/**
 * The designed poster for a movie without artwork. Deterministic in the title
 * alone, so the web and TV apps draw the same card and the detail page's
 * ambient wash matches it.
 */
export function titleCardLook(title: string): TitleCardLook {
  const h = hash(title);
  const palettes = tokens.titleCard.palettes;
  return {
    palette: palettes[h % palettes.length]!,
    bloomX: 18 + ((h >>> 5) % 64),
    bloomY: 8 + ((h >>> 11) % 36),
  };
}

/** FNV-1a over code points, then the murmur3 finalizer to spread short titles. */
function hash(s: string): number {
  let h = 2166136261;
  for (const c of s) {
    h ^= c.codePointAt(0)!;
    h = Math.imul(h, 16777619);
  }
  h ^= h >>> 16;
  h = Math.imul(h, 0x85ebca6b);
  h ^= h >>> 13;
  h = Math.imul(h, 0xc2b2ae35);
  h ^= h >>> 16;
  return h >>> 0;
}
