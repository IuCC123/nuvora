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

/**
 * RGBA8 PNG with one IDAT: a zlib stream of one stored (uncompressed) deflate
 * block per scanline, filter type 0. ThumbHash images are at most 32 px wide,
 * so a row never nears the 65,535-byte stored-block limit.
 */
export function encodePng(width: number, height: number, rgba: Uint8Array): Uint8Array {
  const row = width * 4 + 1;
  const zlib = new ByteWriter(2 + height * (5 + row) + 4);
  zlib.bytes(0x78, 0x01);
  let a = 1;
  let b = 0;
  for (let y = 0; y < height; y++) {
    zlib.bytes(y + 1 < height ? 0 : 1, row & 255, row >> 8, ~row & 255, (row >> 8) ^ 255);
    zlib.bytes(0);
    b = (b + a) % 65521;
    for (let i = y * width * 4, end = i + width * 4; i < end; i++) {
      const u = rgba[i] ?? 0;
      zlib.bytes(u);
      a = (a + u) % 65521;
      b = (b + a) % 65521;
    }
  }
  zlib.u32((b << 16) | a);

  const ihdr = new ByteWriter(13);
  ihdr.u32(width);
  ihdr.u32(height);
  ihdr.bytes(8, 6, 0, 0, 0); // 8-bit, RGBA, deflate, no filter set, no interlace

  const chunks = [chunk("IHDR", ihdr.out), chunk("IDAT", zlib.out), chunk("IEND", new Uint8Array(0))];
  const png = new ByteWriter(8 + chunks.reduce((n, c) => n + c.length, 0));
  png.bytes(137, 80, 78, 71, 13, 10, 26, 10);
  for (const c of chunks) png.append(c);
  return png.out;
}

function chunk(type: string, data: Uint8Array): Uint8Array {
  const w = new ByteWriter(12 + data.length);
  w.u32(data.length);
  for (let i = 0; i < 4; i++) w.bytes(type.charCodeAt(i));
  w.append(data);
  w.u32(crc32(w.out.subarray(4, 8 + data.length)));
  return w.out;
}

class ByteWriter {
  readonly out: Uint8Array;
  private at = 0;
  constructor(size: number) {
    this.out = new Uint8Array(size);
  }
  bytes(...values: number[]): void {
    for (const v of values) this.out[this.at++] = v;
  }
  u32(v: number): void {
    this.bytes((v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255);
  }
  append(data: Uint8Array): void {
    this.out.set(data, this.at);
    this.at += data.length;
  }
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(bytes: Uint8Array): number {
  let c = ~0;
  for (const byte of bytes) c = (CRC_TABLE[(c ^ byte) & 255] ?? 0) ^ (c >>> 8);
  return ~c >>> 0;
}

const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const DECODE = (() => {
  const table = new Int16Array(128).fill(-1);
  for (let i = 0; i < ALPHABET.length; i++) table[ALPHABET.charCodeAt(i)] = i;
  table["-".charCodeAt(0)] = 62; // base64url, in case a server sends it
  table["_".charCodeAt(0)] = 63;
  return table;
})();

/** Standard or URL-safe base64, padded or not. */
export function base64ToBytes(b64: string): Uint8Array {
  const clean = b64.replace(/[=\s]+$/, "");
  const out = new Uint8Array(Math.floor((clean.length * 3) / 4));
  let bits = 0;
  let acc = 0;
  let at = 0;
  for (let i = 0; i < clean.length; i++) {
    const v = DECODE[clean.charCodeAt(i)] ?? -1;
    if (v < 0) throw new Error(`invalid base64 at ${i}`);
    acc = (acc << 6) | v;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[at++] = (acc >> bits) & 255;
    }
  }
  return out;
}

export function bytesToBase64(bytes: Uint8Array): string {
  let s = "";
  let i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = ((bytes[i] ?? 0) << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0);
    s += ALPHABET[n >> 18]! + ALPHABET[(n >> 12) & 63]! + ALPHABET[(n >> 6) & 63]! + ALPHABET[n & 63]!;
  }
  const rest = bytes.length - i;
  if (rest > 0) {
    const n = ((bytes[i] ?? 0) << 16) | (rest === 2 ? (bytes[i + 1] ?? 0) << 8 : 0);
    s += ALPHABET[n >> 18]! + ALPHABET[(n >> 12) & 63]! + (rest === 2 ? ALPHABET[(n >> 6) & 63]! : "=") + "=";
  }
  return s;
}
