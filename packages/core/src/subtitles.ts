import type { AudioTrack, SubtitleMode, SubtitleTrack } from "./client";
import { languageCode, languageName } from "./languages";

/** The viewer's account setting, with a missing language already resolved to the device's (`preferredLanguage`). */
export interface SubtitlePrefs {
  mode: SubtitleMode;
  /** ISO 639-2 in any form, or null when neither the account nor the device names one. */
  language: string | null;
}

/**
 * The viewer's last choice for a show or a movie. A track is remembered by what
 * it is rather than its stream index, so the choice carries over to the next
 * episode even when that file orders its tracks differently.
 */
export type RememberedSubtitle =
  | { kind: "off" }
  | { kind: "track"; language: string | null; title: string | null; forced: boolean; hearingImpaired: boolean; styled: boolean };

/**
 * The track the player starts with, or null for none. The rules are in
 * docs/design/shows-library-subtitles.md ("Choosing a track"). Tracks the
 * server can't deliver as text are never chosen.
 */
export function chooseSubtitle(
  tracks: readonly SubtitleTrack[],
  audioLanguage: string | null | undefined,
  prefs: SubtitlePrefs,
  remembered: RememberedSubtitle | null,
): SubtitleTrack | null {
  const usable = tracks.filter((t) => t.renderable);
  if (remembered?.kind === "off") return null;
  if (remembered?.kind === "track") {
    const match = closest(usable, remembered);
    if (match) return match;
  }

  const preferred = languageCode(prefs.language);
  const inPreferred = preferred ? usable.filter((t) => languageCode(t.language) === preferred) : [];
  const flaggedDefault = usable.find((t) => t.is_default) ?? null;

  switch (prefs.mode) {
    case "off":
      return null;
    case "always":
      return best(inPreferred) ?? flaggedDefault;
    case "auto": {
      const audio = languageCode(audioLanguage);
      // Unknown audio counts as the viewer's language: subtitles don't appear over a film they may understand.
      const foreignAudio = audio !== null && audio !== preferred;
      if (foreignAudio && inPreferred.length) return best(inPreferred);
      return best(inPreferred.filter((t) => t.is_forced)) ?? flaggedDefault;
    }
  }
}

/** What to remember after the viewer picks `track` (null is Off). */
export function rememberSubtitle(track: SubtitleTrack | null): RememberedSubtitle {
  if (!track) return { kind: "off" };
  return {
    kind: "track",
    language: languageCode(track.language),
    title: track.title ?? null,
    forced: track.is_forced,
    hearingImpaired: track.is_hearing_impaired,
    styled: track.format === "ass",
  };
}

/** The account's language, else the device's (`navigator.language` on the web), as ISO 639-2/B. */
export function preferredLanguage(account: string | null | undefined, device: string | null | undefined): string | null {
  return languageCode(account) ?? languageCode(device);
}

/** The language the viewer hears: the default-flagged audio track's, else the first track's. */
export function audioLanguage(audio: readonly AudioTrack[]): string | null {
  return (audio.find((a) => a.is_default) ?? audio[0])?.language ?? null;
}

/** Shift+C: Off, then each renderable track in file order, then Off again. */
export function nextSubtitle(tracks: readonly SubtitleTrack[], current: SubtitleTrack | null): SubtitleTrack | null {
  const usable = tracks.filter((t) => t.renderable);
  const at = current ? usable.findIndex((t) => t.index === current.index) : -1;
  return usable[at + 1] ?? null;
}

export type SubtitleTag = "Forced" | "SDH" | "Styled";

export interface SubtitleLabel {
  /** `"Spanish"`. */
  name: string;
  /** The track's own title when it adds something: `"Signs & Songs"`. */
  title: string | null;
  tags: SubtitleTag[];
  /** Image tracks: shown, but they need conversion before they can be drawn. */
  needsConversion: boolean;
}

/** How a menu names a track. `name` is a function so the web can pass `Intl.DisplayNames`. */
export function subtitleLabel(track: SubtitleTrack, name: (code: string | null | undefined) => string = languageName): SubtitleLabel {
  const language = name(track.language);
  const title = track.title?.trim() || null;
  const tags: SubtitleTag[] = [];
  if (track.is_forced) tags.push("Forced");
  if (track.is_hearing_impaired) tags.push("SDH");
  if (track.format === "ass") tags.push("Styled");
  const redundant = !title || title.toLowerCase() === language.toLowerCase() || tags.some((tag) => tag.toLowerCase() === title.toLowerCase());
  return { name: language, title: redundant ? null : title, tags, needsConversion: !track.renderable };
}

/** Full sentences for a screen reader or a pulse: `"Spanish, Signs & Songs, Styled"`. */
export function subtitleDescription(label: SubtitleLabel): string {
  return [label.name, label.title, ...label.tags, label.needsConversion && "needs conversion"].filter(Boolean).join(", ");
}

/** Non-forced first, then the file's default flag, then not SDH, then file order. */
function best(tracks: readonly SubtitleTrack[]): SubtitleTrack | null {
  const rank = (t: SubtitleTrack) => (t.is_forced ? 4 : 0) + (t.is_default ? 0 : 2) + (t.is_hearing_impaired ? 1 : 0);
  return tracks.reduce<SubtitleTrack | null>((winner, t) => (winner === null || rank(t) < rank(winner) ? t : winner), null);
}

/** The same-language track most like the remembered one, or null when the file has none in that language. */
function closest(tracks: readonly SubtitleTrack[], want: Extract<RememberedSubtitle, { kind: "track" }>): SubtitleTrack | null {
  const candidates = tracks.filter((t) => languageCode(t.language) === want.language);
  const score = (t: SubtitleTrack) =>
    ((t.title ?? null) === want.title ? 8 : 0) +
    (t.is_forced === want.forced ? 4 : 0) +
    (t.is_hearing_impaired === want.hearingImpaired ? 2 : 0) +
    ((t.format === "ass") === want.styled ? 1 : 0);
  return candidates.reduce<SubtitleTrack | null>((winner, t) => (winner === null || score(t) > score(winner) ? t : winner), null);
}
