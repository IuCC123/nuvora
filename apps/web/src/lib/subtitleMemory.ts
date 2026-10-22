import type { RememberedSubtitle } from "@nuvora/core";

/**
 * What the player remembers on this browser: the subtitle look (per device,
 * as the design says) and the viewer's last track choice per show or movie
 * (per user, so two people sharing a browser keep their own).
 */

export const SUBTITLE_SIZES = ["small", "medium", "large"] as const;
export const SUBTITLE_BACKGROUNDS = ["none", "shadow", "box"] as const;

export interface SubtitleStyle {
  size: (typeof SUBTITLE_SIZES)[number];
  background: (typeof SUBTITLE_BACKGROUNDS)[number];
}

const STYLE_KEY = "nuvora.subtitles.style";
const DEFAULT_STYLE: SubtitleStyle = { size: "medium", background: "shadow" };

export function loadSubtitleStyle(): SubtitleStyle {
  const saved = read(STYLE_KEY);
  if (!saved || typeof saved !== "object") return DEFAULT_STYLE;
  const { size, background } = saved as Record<string, unknown>;
  return {
    size: SUBTITLE_SIZES.find((s) => s === size) ?? DEFAULT_STYLE.size,
    background: SUBTITLE_BACKGROUNDS.find((b) => b === background) ?? DEFAULT_STYLE.background,
  };
}

export function saveSubtitleStyle(style: SubtitleStyle) {
  write(STYLE_KEY, style);
}

/** `"show:12"` or `"movie:4"`: the scope a choice carries across. */
export type SubtitleScope = `show:${number}` | `movie:${number}`;

const choiceKey = (userId: number, scope: SubtitleScope) => `nuvora.subtitles.choice.${userId}.${scope}`;

export function loadSubtitleChoice(userId: number, scope: SubtitleScope): RememberedSubtitle | null {
  const saved = read(choiceKey(userId, scope));
  if (!saved || typeof saved !== "object") return null;
  const v = saved as Record<string, unknown>;
  if (v.kind === "off") return { kind: "off" };
  const text = (x: unknown) => (typeof x === "string" ? x : null);
  if (v.kind !== "track" || typeof v.forced !== "boolean" || typeof v.hearingImpaired !== "boolean" || typeof v.styled !== "boolean") return null;
  return { kind: "track", language: text(v.language), title: text(v.title), forced: v.forced, hearingImpaired: v.hearingImpaired, styled: v.styled };
}

export function saveSubtitleChoice(userId: number, scope: SubtitleScope, choice: RememberedSubtitle) {
  write(choiceKey(userId, scope), choice);
}

function read(key: string): unknown {
  try {
    return JSON.parse(localStorage.getItem(key) ?? "null");
  } catch {
    return null; // private mode or a corrupt value
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage full or blocked: the setting lasts for this session only.
  }
}
