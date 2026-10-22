import { expect, test } from "bun:test";
import { rgbaToThumbHash, thumbHashToDataURL } from "thumbhash";
import { placeholderDataUrl } from "../src";
import { base64ToBytes, bytesToBase64 } from "../src/images";

function sampleImage(w: number, h: number, seed: number): Uint8Array {
  const rgba = new Uint8Array(w * h * 4);
  for (let i = 0; i < rgba.length; i++) rgba[i] = (i * 37 + seed * 101 + ((i * i) >> 3)) & 255;
  return rgba;
}

test("matches thumbhash's own btoa-based data URL byte for byte", () => {
  for (const [w, h, seed] of [[100, 75, 1], [64, 100, 2], [100, 100, 3], [1, 1, 4]] as const) {
    const hash = rgbaToThumbHash(w, h, sampleImage(w, h, seed));
    const placeholder = Buffer.from(hash).toString("base64");
    const ours = placeholderDataUrl({ id: `img-${seed}`, aspect_ratio: w / h, placeholder });
    expect(ours).toBe(thumbHashToDataURL(hash));
  }
});

test("produces a PNG data URL", () => {
  const hash = rgbaToThumbHash(30, 40, sampleImage(30, 40, 9));
  const url = placeholderDataUrl({ id: "png", aspect_ratio: 0.75, placeholder: Buffer.from(hash).toString("base64") });
  expect(url.startsWith("data:image/png;base64,iVBORw0KGgo")).toBe(true);
});

test("accepts unpadded and URL-safe placeholders", () => {
  const bytes = new Uint8Array([251, 255, 191, 0, 1]);
  const padded = Buffer.from(bytes).toString("base64");
  expect(padded).toBe("+/+/AAE=");
  expect([...base64ToBytes(padded)]).toEqual([251, 255, 191, 0, 1]);
  expect([...base64ToBytes("+/+/AAE")]).toEqual([251, 255, 191, 0, 1]);
  expect([...base64ToBytes("-_-_AAE")]).toEqual([251, 255, 191, 0, 1]);
});

test("base64 round-trips every length against Buffer", () => {
  for (let n = 0; n < 64; n++) {
    const bytes = Uint8Array.from({ length: n }, (_, i) => (i * 73 + n * 11) & 255);
    const expected = Buffer.from(bytes).toString("base64");
    expect(bytesToBase64(bytes)).toBe(expected);
    expect([...base64ToBytes(expected)]).toEqual([...bytes]);
  }
});

test("rejects characters outside the alphabet", () => {
  expect(() => base64ToBytes("ab$d")).toThrow("invalid base64 at 2");
});
