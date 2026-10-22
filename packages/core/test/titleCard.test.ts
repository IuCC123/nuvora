import { expect, test } from "bun:test";
import { titleCardLook } from "../src";

// Expected values come from the hash and palette code in
// docs/design/step-1-foundation/mockup.html, so web, TV and mockup agree.
test("picks the mockup's palette and bloom for each title", () => {
  expect(titleCardLook("Tears of Steel")).toEqual({
    palette: { bloom: "#3fae7a", top: "#14503a", bottom: "#04170f" },
    bloomX: 20,
    bloomY: 12,
  });
  expect(titleCardLook("Sintel")).toEqual({
    palette: { bloom: "#df6a2b", top: "#7a3418", bottom: "#200c05" },
    bloomX: 23,
    bloomY: 14,
  });
  expect(titleCardLook("Big Buck Bunny")).toEqual({
    palette: { bloom: "#2f8f8a", top: "#0f4448", bottom: "#05181b" },
    bloomX: 81,
    bloomY: 31,
  });
});

test("hashes by code point, so accented and empty titles are stable", () => {
  expect(titleCardLook("Amélie")).toEqual({
    palette: { bloom: "#d39a2e", top: "#6e4b12", bottom: "#1c1205" },
    bloomX: 80,
    bloomY: 43,
  });
  expect(titleCardLook("")).toEqual({
    palette: { bloom: "#7a6cd6", top: "#2c2566", bottom: "#0b0920" },
    bloomX: 50,
    bloomY: 27,
  });
});
