import { expect, test } from "bun:test";
import { episodeHeading, initialSeason, showCounts, showPlay, upNextCountdown, type EpisodeRef, type SeasonSummary } from "../src";

const season = (id: number, index: number): SeasonSummary => ({ id, index, title: index ? `Season ${index}` : "Specials", episode_count: 10, watched_count: 0 });
const ref = (over: Partial<EpisodeRef> = {}): EpisodeRef => ({ id: 41, season_id: 2, season: 1, index: 4, title: "The Fourth", label: "S1 · E4", ...over });

test("counts name numbered seasons and every episode", () => {
  expect(showCounts({ seasons: [season(1, 1), season(2, 2), season(9, 0)], episode_count: 24 })).toBe("2 seasons · 24 episodes");
  expect(showCounts({ seasons: [season(1, 1)], episode_count: 1 })).toBe("1 season · 1 episode");
  expect(showCounts({ seasons: [season(9, 0)], episode_count: 3 })).toBe("3 episodes");
});

test("the primary button starts, resumes, plays next, or starts over", () => {
  const first = { id: 11, label: "S1 · E1" };
  expect(showPlay({ next: ref({ id: 11, index: 1, label: "S1 · E1" }), watched_count: 0 }, first)).toEqual({ kind: "start", episode: ref({ id: 11, index: 1, label: "S1 · E1" }) });
  expect(showPlay({ next: ref({ position_ms: 723_000 }), watched_count: 3 }, first)).toEqual({ kind: "resume", episode: ref({ position_ms: 723_000 }), positionMs: 723_000 });
  expect(showPlay({ next: ref(), watched_count: 3 }, first)).toEqual({ kind: "next", episode: ref() });
  expect(showPlay({ next: null, watched_count: 24 }, first)).toEqual({ kind: "rewatch", episode: first });
  expect(showPlay({ next: null, watched_count: 0 }, null)).toBeNull();
});

test("the page opens on Next Up's season, else the first numbered one", () => {
  const seasons = [season(9, 0), season(1, 1), season(2, 2)];
  expect(initialSeason({ seasons, next: ref({ season_id: 2 }) })).toBe(2);
  expect(initialSeason({ seasons, next: null })).toBe(1);
  expect(initialSeason({ seasons: [season(9, 0)], next: null })).toBe(0);
});

test("the player heading drops a title that only repeats the number", () => {
  expect(episodeHeading({ show_title: "Tower of God", season: 2, index: 4, title: "Ouji no Kikan" })).toEqual(["Tower of God", "S2 E4", "Ouji no Kikan"]);
  expect(episodeHeading({ show_title: "Tower of God", season: 1, index: 4, title: "Episode 4" })).toEqual(["Tower of God", "S1 E4"]);
  expect(episodeHeading({ show_title: "Tower of God", season: 0, index: 2, title: "Special 2" })).toEqual(["Tower of God", "Special 2"]);
});

test("the countdown shows for the last ten seconds and follows playback time", () => {
  expect(upNextCountdown(1300, 1440)).toBeNull();
  expect(upNextCountdown(1430, 1440)).toBe(10);
  expect(upNextCountdown(1434.2, 1440)).toBe(6);
  expect(upNextCountdown(1440, 1440)).toBe(0);
  expect(upNextCountdown(15, 20)).toBeNull();
  expect(upNextCountdown(0, Number.NaN)).toBeNull();
});
