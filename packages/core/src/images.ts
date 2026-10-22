import { thumbHashToRGBA } from "thumbhash";
import type { Image } from "./client";

const cache = new Map<string, string>();

/**
 * ThumbHash -> `data:image/png;base64,...`, memoized by image id. Works in
 * browsers and React Native (an `<Image source={{ uri }}>` accepts it).
 *
 * Deliberately not thumbhash's own `thumbHashToDataURL`: it calls `btoa`, which
 * Hermes lacks before React Native 0.74 and Vega ships 0.72. So this module
 * encodes the PNG (stored deflate blocks, CRC32, Adler-32) and base64 itself.
 */
export function placeholderDataUrl(image: Image): string {
  const hit = cache.get(image.id);
  if (hit !== undefined) return hit;
  const { w, h, rgba } = thumbHashToRGBA(base64ToBytes(image.placeholder));
  const url = "data:image/png;base64," + bytesToBase64(encodePng(w, h, rgba));
  cache.set(image.id, url);
  return url;
}

function encodePng(width: number, height: number, rgba: Uint8Array): Uint8Array {
  void width;
  void height;
  void rgba;
  throw new Error("not implemented: IHDR + one IDAT of stored deflate blocks + IEND");
}

function base64ToBytes(b64: string): Uint8Array {
  void b64;
  throw new Error("not implemented: table-driven decode, no atob");
}

function bytesToBase64(bytes: Uint8Array): string {
  void bytes;
  throw new Error("not implemented: table-driven encode, no btoa");
}
