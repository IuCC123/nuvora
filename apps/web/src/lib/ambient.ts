import { averageColor, type Image } from "@nuvora/core";

/**
 * The glow a screen takes from its art: the image's average color (from its
 * ThumbHash, so it is known in the first frame) moved into a lightness and
 * chroma band that reads as light on a dark canvas. A grey poster gives a
 * soft grey glow; a teal one gives teal, never a muddy brown.
 */
export function ambientColor(image: Image | null | undefined): string | undefined {
  const rgb = image ? averageColor(image) : null;
  if (!rgb) return undefined;
  const [l, c, h] = oklch(rgb);
  const lightness = clamp(l, 0.34, 0.5);
  const chroma = clamp(c * 1.7, 0.02, 0.13);
  return `oklch(${lightness.toFixed(3)} ${chroma.toFixed(3)} ${h.toFixed(1)})`;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** sRGB 0-255 to OKLCH (L 0-1, C, H degrees). Björn Ottosson's matrices. */
function oklch([r, g, b]: [number, number, number]): [number, number, number] {
  const lin = (u: number) => {
    const v = u / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const [lr, lg, lb] = [lin(r), lin(g), lin(b)];
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  const C = Math.hypot(A, B);
  const H = ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360;
  return [L, C, H];
}
