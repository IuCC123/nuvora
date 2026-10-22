import { expect, test } from "bun:test";
import { cssVariables } from "../src";

const css = cssVariables();

test("emits one :root block", () => {
  expect(css.startsWith(":root {\n")).toBe(true);
  expect(css.endsWith("\n}\n")).toBe(true);
});

test("kebab-cases nested keys and keeps strings verbatim", () => {
  expect(css).toContain("  --nv-color-canvas: #0f1012;\n");
  expect(css).toContain("  --nv-color-text-muted: #85888d;\n");
  expect(css).toContain("  --nv-motion-easing-standard: cubic-bezier(0.2, 0, 0, 1);\n");
});

test("suffixes numbers with the unit of their group", () => {
  expect(css).toContain("  --nv-space-4: 16px;\n");
  expect(css).toContain("  --nv-radius-pill: 999px;\n");
  expect(css).toContain("  --nv-type-headline-size: 40px;\n");
  expect(css).toContain("  --nv-type-headline-line: 44px;\n");
  expect(css).toContain("  --nv-poster-width-regular: 150px;\n");
  expect(css).toContain("  --nv-poster-gap: 16px;\n");
  expect(css).toContain("  --nv-motion-duration-fast: 160ms;\n");
  expect(css).toContain("  --nv-motion-image-fade: 240ms;\n");
  expect(css).toContain("  --nv-title-card-stretch: 64%;\n");
});

test("leaves weights unitless", () => {
  expect(css).toContain("  --nv-font-weight-semibold: 600;\n");
});

test("leaves per-title palettes out", () => {
  expect(css).not.toContain("palettes");
  expect(css).not.toContain("#3fae7a");
});

test("every line is a declaration with a value", () => {
  const body = css.split("\n").slice(1, -2);
  for (const line of body) expect(line).toMatch(/^ {2}--nv-[a-z0-9-]+: \S.*;$/);
  expect(css).not.toMatch(/undefined|NaN|\[object/);
});
