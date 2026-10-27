import type { ReleaseContent, ReleaseResult } from "./client";
import { languageName } from "./languages";

/**
 * What a release holds, short enough for a badge: `"Movie"`, `"S1 · E5"`,
 * `"S1 · E1–12"`, `"Episode 5"`, `"Season 2"`, `"Seasons 1–3"`, `"Complete series"`.
 */
export function contentLabel(content: ReleaseContent): string {
  const range = (first: number, last: number) => (first === last ? `${first}` : `${first}–${last}`);
  switch (content.kind) {
    case "movie":
      return "Movie";
    case "episodes":
      if (content.season != null) return `S${content.season} · E${range(content.first, content.last)}`;
      return `${content.first === content.last ? "Episode" : "Episodes"} ${range(content.first, content.last)}`;
    case "seasons":
      return `${content.first === content.last ? "Season" : "Seasons"} ${range(content.first, content.last)}`;
    case "complete":
      return "Complete series";
  }
}

/**
 * For someone who wants English: `english` names English audio; `other` names
 * only other languages; `unknown` names none (usually the original language)
 * or two tracks without saying which.
 */
export type ReleaseLanguage = "english" | "other" | "unknown";

export function releaseLanguage(release: Pick<ReleaseResult, "audio_languages" | "dual_audio">): ReleaseLanguage {
  if (release.audio_languages.includes("en")) return "english";
  if (release.audio_languages.length > 0 && !release.dual_audio) return "other";
  return "unknown";
}

/** One line per thing the name says about sound and subtitles, in that order: `"Russian dub"`, `"Dual audio"`, `"English subtitles"`. */
export function languageLabels(
  release: Pick<ReleaseResult, "audio_languages" | "subtitle_languages" | "dual_audio" | "dubbed" | "hardcoded_subs">,
): string[] {
  const labels: string[] = [];
  const audio = release.audio_languages.map(languageName);
  if (release.dubbed && audio.length === 1) labels.push(`${audio[0]} dub`);
  else {
    labels.push(...audio);
    if (release.dubbed) labels.push("Dubbed");
  }
  if (release.dual_audio && audio.length < 2) labels.push("Dual audio");
  const subtitles = release.subtitle_languages.map((code) => (code === "mul" ? "Many" : languageName(code)));
  const burned = release.hardcoded_subs ? "burned-in " : "";
  if (subtitles.length > 0) labels.push(`${subtitles.join(", ")} ${burned}subtitles`);
  else if (release.hardcoded_subs) labels.push("Burned-in subtitles");
  return labels;
}
