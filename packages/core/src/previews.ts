import type { PreviewInfo } from "./client";

/** One seek preview: which sheet, and the rectangle in it, in the sheet's pixels. */
export interface PreviewFrame {
  sheet: string;
  x: number;
  y: number;
  width: number;
  height: number;
  /** The whole sheet, for scaling it as a CSS background. */
  sheetWidth: number;
  sheetHeight: number;
}

/** The preview for `seconds` into the video. Past the end gives the last one. */
export function previewFrame(info: PreviewInfo, seconds: number): PreviewFrame | null {
  const url = (n: number) => info.sheets[n];
  const index = Math.min(Math.max(0, Math.floor((seconds * 1000) / info.interval_ms)), info.count - 1);
  const perSheet = info.columns * info.rows;
  const sheet = url(Math.floor(index / perSheet));
  if (sheet === undefined) return null;
  const within = index % perSheet;
  return {
    sheet,
    x: (within % info.columns) * info.width,
    y: Math.floor(within / info.columns) * info.height,
    width: info.width,
    height: info.height,
    sheetWidth: info.columns * info.width,
    sheetHeight: info.rows * info.height,
  };
}
