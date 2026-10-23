import { describe, expect, test } from "bun:test";
import { previewFrame } from "../src/previews";

const info = {
  interval_ms: 10_000,
  width: 320,
  height: 180,
  columns: 10,
  rows: 10,
  count: 250,
  sheets: ["/api/previews/k/0", "/api/previews/k/1", "/api/previews/k/2"],
};

describe("previewFrame", () => {
  test("finds the sheet and the tile for a moment", () => {
    expect(previewFrame(info, 125)).toEqual({
      sheet: "/api/previews/k/0",
      x: 640,
      y: 180,
      width: 320,
      height: 180,
      sheetWidth: 3200,
      sheetHeight: 1800,
    });
    expect(previewFrame(info, 1000)).toMatchObject({ sheet: "/api/previews/k/1", x: 0, y: 0 });
  });

  test("before the start is the first preview, past the end the last", () => {
    expect(previewFrame(info, -5)).toMatchObject({ sheet: "/api/previews/k/0", x: 0, y: 0 });
    expect(previewFrame(info, 99_999)).toMatchObject({ sheet: "/api/previews/k/2", x: 2880, y: 720 });
  });

  test("a sheet the server didn't list gives nothing", () => {
    expect(previewFrame({ ...info, sheets: [] }, 5)).toBeNull();
  });
});
