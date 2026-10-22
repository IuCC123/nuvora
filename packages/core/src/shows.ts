import type { Episode, EpisodeDetail, EpisodeRef, ShowDetail } from "./client";

/** `"2 seasons · 24 episodes"`. Specials count as episodes but not as a season. */
export function showCounts(show: Pick<ShowDetail, "seasons" | "episode_count">): string {
  const seasons = show.seasons.filter((s) => s.index > 0).length;
  const parts = seasons > 0 ? [plural(seasons, "season")] : [];
  parts.push(plural(show.episode_count, "episode"));
  return parts.join(" · ");
}

export function plural(n: number, word: string): string {
  return `${n.toLocaleString("en-US")} ${word}${n === 1 ? "" : "s"}`;
}

/**
 * What the show page's primary button does. `next` is the server's Next Up;
 * with every episode watched there is none, and the button starts the show
 * again from `first`, the first episode of the first numbered season.
 */
export type ShowPlay =
  | { kind: "start"; episode: Pick<EpisodeRef, "id" | "label"> }
  | { kind: "resume"; episode: EpisodeRef; positionMs: number }
  | { kind: "next"; episode: EpisodeRef }
  | { kind: "rewatch"; episode: Pick<EpisodeRef, "id" | "label"> };

export function showPlay(show: Pick<ShowDetail, "next" | "watched_count">, first: Pick<EpisodeRef, "id" | "label"> | null): ShowPlay | null {
  const next = show.next;
  if (next) {
    if (next.position_ms != null) return { kind: "resume", episode: next, positionMs: next.position_ms };
    return show.watched_count > 0 ? { kind: "next", episode: next } : { kind: "start", episode: next };
  }
  return first ? { kind: "rewatch", episode: first } : null;
}

/** The season the show page opens on: the one Next Up is in, else the first numbered one. */
export function initialSeason(show: Pick<ShowDetail, "next" | "seasons">): number | undefined {
  const fromNext = show.next ? show.seasons.find((s) => s.id === show.next!.season_id) : undefined;
  return (fromNext ?? show.seasons.find((s) => s.index > 0) ?? show.seasons[0])?.index;
}

/**
 * Whether the server's title is only the number back (`"Episode 4"`), so a
 * heading that already shows the number can leave it out.
 */
export function hasOwnTitle(episode: Pick<Episode, "title" | "index">): boolean {
  return episode.title !== `Episode ${episode.index}` && episode.title !== `Special ${episode.index}`;
}

/** The player's top bar: `"Show · S1 E4 · Episode title"`, without a title that only repeats the number. */
export function episodeHeading(episode: Pick<EpisodeDetail, "show_title" | "season" | "index" | "title">): string[] {
  const number = episode.season === 0 ? `Special ${episode.index}` : `S${episode.season} E${episode.index}`;
  return [episode.show_title, number, ...(hasOwnTitle(episode) ? [episode.title] : [])];
}

/** The player's Next episode card shows for this long before an episode ends, counting down to autoplay. */
export const UP_NEXT_SECONDS = 10;

/**
 * Whole seconds left on the Next episode countdown, or `null` while the card
 * should stay hidden. It runs on playback time, so pausing holds it and a
 * seek back hides the card again.
 */
export function upNextCountdown(timeS: number, durationS: number): number | null {
  if (!Number.isFinite(durationS) || durationS <= UP_NEXT_SECONDS * 3) return null;
  const left = durationS - timeS;
  return left <= UP_NEXT_SECONDS ? Math.max(0, Math.ceil(left)) : null;
}
