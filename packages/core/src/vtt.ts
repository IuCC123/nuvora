/**
 * WebVTT, as the server sends plain text tracks (`?format=vtt`), parsed into
 * what a subtitle overlay draws. Pure, so the TV apps can draw the same cues.
 */

export interface Cue {
  /** Seconds. */
  start: number;
  /** Seconds, exclusive. */
  end: number;
  /** `"top"` when the cue asks for the upper part of the picture (`line:0`, `line:10%`). */
  placement: "bottom" | "top";
  /** One entry per line of text. */
  lines: CueRun[][];
}

/** A stretch of text with the same styling. */
export interface CueRun {
  text: string;
  italic: boolean;
  bold: boolean;
  underline: boolean;
}

const TIMING = /^((?:\d+:)?\d{1,2}:\d{2}[.,]\d{1,3})\s+-->\s+((?:\d+:)?\d{1,2}:\d{2}[.,]\d{1,3})(.*)$/;

/** Cues in start order. Malformed blocks are skipped rather than failing the track. */
export function parseVtt(text: string): Cue[] {
  const cues: Cue[] = [];
  const blocks = text.replace(/^﻿/, "").replace(/\r\n?/g, "\n").split(/\n{2,}/);
  for (const block of blocks) {
    const rows = block.split("\n");
    const at = rows.findIndex((row) => row.includes("-->"));
    if (at < 0) continue;
    const timing = TIMING.exec(rows[at]!.trim());
    if (!timing) continue;
    const start = seconds(timing[1]!);
    const end = seconds(timing[2]!);
    const lines = rows
      .slice(at + 1)
      .map(cueRuns)
      .filter((runs) => runs.length > 0);
    if (end <= start || lines.length === 0) continue;
    cues.push({ start, end, placement: placement(timing[3] ?? ""), lines });
  }
  return cues.sort((a, b) => a.start - b.start || a.end - b.end);
}

/** The cues showing at `time` seconds, in start order. */
export function activeCues(cues: readonly Cue[], time: number): Cue[] {
  return cues.filter((cue) => cue.start <= time && time < cue.end);
}

function seconds(stamp: string): number {
  const parts = stamp.replace(",", ".").split(":").map(Number);
  return parts.reduce((total, part) => total * 60 + part, 0);
}

function placement(settings: string): Cue["placement"] {
  const line = /(?:^|\s)line:(-?[\d.]+)(%?)/.exec(settings);
  if (!line) return "bottom";
  const value = Number(line[1]);
  if (line[2] === "%") return value < 50 ? "top" : "bottom";
  return value >= 0 && value < 4 ? "top" : "bottom";
}

const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", nbsp: " ", lrm: "‎", rlm: "‏", quot: '"', apos: "'" };

/** One line of cue text into styled runs. `<i>`, `<b>` and `<u>` style; other tags and ruby text are dropped. */
function cueRuns(line: string): CueRun[] {
  const runs: CueRun[] = [];
  const style = { italic: 0, bold: 0, underline: 0 };
  let inRuby = 0;
  for (const token of line.split(/(<[^>]*>)/)) {
    if (!token) continue;
    const tag = /^<(\/?)([a-z]+)/i.exec(token);
    if (tag) {
      const delta = tag[1] ? -1 : 1;
      const name = tag[2]!.toLowerCase();
      if (name === "i") style.italic = Math.max(0, style.italic + delta);
      else if (name === "b") style.bold = Math.max(0, style.bold + delta);
      else if (name === "u") style.underline = Math.max(0, style.underline + delta);
      else if (name === "rt") inRuby = Math.max(0, inRuby + delta);
      continue;
    }
    if (token.startsWith("<") || inRuby) continue;
    const text = token.replace(/&(#\d+|[a-z]+);/gi, (match, name: string) =>
      name.startsWith("#") ? String.fromCodePoint(Number(name.slice(1))) : (ENTITIES[name.toLowerCase()] ?? match),
    );
    const run = { text, italic: style.italic > 0, bold: style.bold > 0, underline: style.underline > 0 };
    const last = runs.at(-1);
    if (last && last.italic === run.italic && last.bold === run.bold && last.underline === run.underline) last.text += text;
    else runs.push(run);
  }
  return runs.some((run) => run.text.trim()) ? runs : [];
}
