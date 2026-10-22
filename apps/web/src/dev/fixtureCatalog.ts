import type { Image } from "@nuvora/core";

/**
 * The shows and the large movie archive behind the fixture server's TV and
 * Film Archive libraries. Pure data, deterministic on every load.
 *
 * Sample content: every show, episode and archive title, year and runtime here
 * is invented. The one thumbnail reuses the Big Buck Bunny fixture backdrop.
 */

export const DAY = 86_400_000;

export interface FixtureEpisode {
  id: number;
  season: number;
  index: number;
  /** `null`: the file name had no title, so the server says "Episode N". */
  title: string | null;
  runtime_ms: number;
  thumb: Image | null;
  added_at: number;
}

export interface FixtureSeason {
  id: number;
  index: number;
  episodes: FixtureEpisode[];
}

export interface FixtureShow {
  id: number;
  title: string;
  years: [number, number];
  overview: string;
  poster: Image | null;
  backdrop: Image | null;
  seasons: FixtureSeason[];
}

const THUMB: Image = { id: "5a1d0c0ffee000000000000000000002", aspect_ratio: 16 / 9, placeholder: "U7YNLIZAh3iJd4eHiIi/dvhrhw==" };
const START = Date.UTC(2026, 9, 22, 12);

function season(showId: number, index: number, titles: (string | null)[], runtimeMs: number, addedDaysAgo: number, thumbs = false): FixtureSeason {
  return {
    id: 600 + (showId - 500) * 20 + index,
    index,
    episodes: titles.map((title, i) => ({
      id: 10_000 + (showId - 500) * 1000 + index * 100 + i + 1,
      season: index,
      index: i + 1,
      title,
      runtime_ms: runtimeMs + ((i * 37_000) % 180_000),
      thumb: thumbs && i % 3 === 0 ? THUMB : null,
      added_at: START - addedDaysAgo * DAY + i * 60_000,
    })),
  };
}

export const SHOWS: FixtureShow[] = [
  {
    id: 501,
    title: "Harbor Lights",
    years: [2023, 2025],
    overview: "A night ferry crew keeps a fading port town running, one crossing at a time.",
    poster: null,
    backdrop: null,
    seasons: [
      season(501, 1, ["The Last Crossing", "Fog Bell", "Ballast", "Low Tide", "The Lantern Keeper", "Salt", "Moorings", "Undertow", "Signal Fires", "Dawn Run"], 1_440_000, 400, true),
      season(501, 2, ["New Charts", "Harbor Master", "Squall Line", "The Lighthouse Debt", "Driftwood", "Port of Call", "Starboard", "Home Water"], 1_500_000, 2, true),
      season(501, 0, ["Behind the Crossing", "The Making of Fog Bell"], 600_000, 300),
    ],
  },
  {
    id: 502,
    title: "The Long Field",
    years: [2024, 2024],
    overview: "Sixty-four days of one summer on a farm at the edge of the map.",
    poster: null,
    backdrop: null,
    seasons: [season(502, 1, Array.from({ length: 64 }, () => null), 1_420_000, 30)],
  },
  {
    id: 503,
    title: "Night Shift Diaries",
    years: [2015, 2023],
    overview: "Nine years of the same hospital corridor after midnight.",
    poster: null,
    backdrop: null,
    seasons: Array.from({ length: 9 }, (_, s) =>
      season(503, s + 1, ["Admissions", "Graveyard", "Code Amber", "Rounds", "Handover", "Morning"], 2_520_000, 3000 - s * 300),
    ),
  },
];

export const episodes = (show: FixtureShow) => show.seasons.flatMap((s) => s.episodes);
/** Watch order: numbered seasons, then Specials. */
export const inOrder = (show: FixtureShow) => [...show.seasons.filter((s) => s.index > 0), ...show.seasons.filter((s) => s.index === 0)].flatMap((s) => s.episodes);

export function findEpisode(id: number): { show: FixtureShow; season: FixtureSeason; episode: FixtureEpisode } | undefined {
  for (const show of SHOWS) {
    for (const s of show.seasons) {
      const episode = s.episodes.find((e) => e.id === id);
      if (episode) return { show, season: s, episode };
    }
  }
  return undefined;
}

export function findSeason(id: number): { show: FixtureShow; season: FixtureSeason } | undefined {
  for (const show of SHOWS) {
    const s = show.seasons.find((x) => x.id === id);
    if (s) return { show, season: s };
  }
  return undefined;
}

export interface ArchiveMovie {
  id: number;
  title: string;
  year: number | null;
  runtime_ms: number;
  height: number;
  hdr: boolean;
  added_at: number;
}

/** mulberry32: the archive is the same on every load. */
function random(seed: number) {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

// Nothing sorts under X or Y, so those letters show dimmed on the jump bar. "The" and "A" are skipped when sorting.
const FIRST = ["Amber", "Broken", "Cold", "Distant", "Electric", "Fallen", "Golden", "Hidden", "Iron", "Jade", "Kind", "Last", "Midnight", "Northern", "Open", "Paper", "Red", "Silent", "Twelve", "Under", "Velvet", "Winter", "Zero", "The Quiet", "The Long", "A Brief"];
const SECOND = ["Harbor", "Season", "Garden", "Signal", "Avenue", "Orchard", "Lantern", "Frontier", "Archive", "Circuit", "Meridian", "Canyon", "Letters", "Tides", "Engine", "Echo", "Summer", "Highway", "Island", "Mirror", "Rooms", "Station", "Theory", "Weather"];
const NUMBERED = ["2 Days in Lisbon", "9 Lives of Ada", "24 Frames", "100 Rooftops", "1984 Again", "7 Bridges"];

export const ARCHIVE: ArchiveMovie[] = (() => {
  const rand = random(7);
  const titles = new Set<string>(NUMBERED);
  for (const a of FIRST) for (const b of SECOND) if (rand() < 0.94) titles.add(`${a} ${b}`);
  const heights = [2160, 1080, 1080, 1080, 1080, 720, 720, 480];
  return [...titles].slice(0, 600).map((title, i) => {
    const height = heights[Math.floor(rand() * heights.length)]!;
    return {
      id: 1000 + i,
      title,
      year: rand() < 0.03 ? null : 1950 + Math.floor(rand() * 76),
      runtime_ms: (78 + Math.floor(rand() * 100)) * 60_000,
      height,
      hdr: height === 2160 && rand() < 0.6,
      added_at: START - Math.floor(rand() * 900) * DAY,
    };
  });
})();
