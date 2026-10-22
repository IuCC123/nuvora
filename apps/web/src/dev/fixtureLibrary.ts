import {
  DEFAULT_ORDER,
  LIBRARY_SORTS,
  RESOLUTIONS,
  SORT_ORDERS,
  WATCH_FILTERS,
  type Episode,
  type EpisodeRef,
  type HomeRow,
  type IndexGroup,
  type ItemDetail,
  type LibraryPage,
  type LibrarySort,
  type LibrarySummary,
  type PosterCard,
  type Resolution,
  type RowPage,
  type Version,
} from "@nuvora/core";
import { libraries } from "./fixtureAccounts";
import { ARCHIVE, DAY, episodes, findEpisode, findSeason, inOrder, SHOWS, type ArchiveMovie, type FixtureEpisode, type FixtureShow } from "./fixtureCatalog";
import { blenderCards, blenderItem } from "./fixtures";
import { continueWatching, lastActivity, watchState, withWatch } from "./fixtureWatch";
import { NO_DETAILS } from "./noDetails";
import type { Reply } from "./fixtureAccounts";

/**
 * The library half of the fixture server: the nav rail's libraries, the
 * library grid with keyset cursors and its group index, shows, seasons and
 * episodes, and the home rows built from them, with the server's rules for
 * Next Up, sorting, filters and grouping.
 *
 * Library 1 holds the Blender movies, 2 three of them in 4K, 3 the shows and
 * 4 the generated archive. Libraries added in Settings stay empty.
 */

/** A grid entry: the card plus the keys the server sorts and filters by. */
interface Entry {
  card: PosterCard;
  sortTitle: string;
  year: number | null;
  added: number;
  watchedAt: number | null;
  runtime: number | null;
  res: Resolution;
  hdr: boolean;
  state: "unwatched" | "in_progress" | "watched";
}

const ROW_PAGE = 20;
const NEW_EPISODES_DAYS = 14;
const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

export const episodeLabel = (e: Pick<FixtureEpisode, "season" | "index">) => (e.season === 0 ? `Special ${e.index}` : `S${e.season} · E${e.index}`);
const episodeTitle = (e: FixtureEpisode) => e.title ?? (e.season === 0 ? `Special ${e.index}` : `Episode ${e.index}`);

function resolutionOf(width: number, height: number): Resolution {
  if (width >= 3200 || height >= 2000) return "4k";
  if (width >= 1800 || height >= 1000) return "1080p";
  if (width >= 1200 || height >= 700) return "720p";
  return "sd";
}

function stateOf(card: PosterCard): Entry["state"] {
  if (card.progress != null) return "in_progress";
  return card.watched ? "watched" : "unwatched";
}

function blenderEntries(ids?: number[]): Entry[] {
  const recent = [14, 13, 12, 5, 11, 9, 4];
  return blenderCards()
    .filter((c) => !ids || ids.includes(c.id))
    .map((card) => {
      const detail = blenderItem(card.id)!;
      const video = detail.kind === "movie" ? detail.versions[0]?.media?.video[0] : undefined;
      const rank = recent.indexOf(card.id);
      return {
        card,
        sortTitle: sortTitle(card.title),
        year: card.year ?? null,
        added: Date.UTC(2026, 9, 22) - (rank >= 0 ? rank * 3 : 30 + card.id * 11) * DAY,
        watchedAt: watchState(card.id).last_watched_at ?? null,
        runtime: detail.kind === "movie" ? (detail.runtime_ms ?? null) : null,
        res: ids ? "4k" : resolutionOf(video?.width ?? 0, video?.height ?? 0),
        hdr: !!ids,
        state: stateOf(card),
      };
    });
}

const archiveCard = (m: ArchiveMovie): PosterCard => withWatch({ id: m.id, kind: "movie", title: m.title, year: m.year, poster: null, watched: false });

function archiveEntries(): Entry[] {
  return ARCHIVE.map((m) => {
    const card = archiveCard(m);
    return {
      card,
      sortTitle: sortTitle(m.title),
      year: m.year,
      added: m.added_at,
      watchedAt: watchState(m.id).last_watched_at ?? null,
      runtime: m.runtime_ms,
      res: resolutionOf(m.height * (16 / 9), m.height),
      hdr: m.hdr,
      state: stateOf(card),
    };
  });
}

function showCard(show: FixtureShow, subtitle?: string): PosterCard {
  const all = episodes(show);
  const watched = all.filter((e) => watchState(e.id).watched).length;
  return {
    id: show.id,
    kind: "show",
    title: show.title,
    year: show.years[0],
    poster: show.poster,
    watched: watched === all.length,
    progress: null,
    unwatched_count: all.length - watched,
    ...(subtitle && { subtitle }),
  };
}

function showEntries(): Entry[] {
  return SHOWS.map((show) => {
    const all = episodes(show);
    const card = showCard(show);
    const started = all.some((e) => watchState(e.id).watched || watchState(e.id).position_ms != null);
    const finishes = all.map((e) => watchState(e.id).last_watched_at ?? 0);
    return {
      card,
      sortTitle: sortTitle(show.title),
      year: show.years[0],
      added: Math.max(...all.map((e) => e.added_at)),
      watchedAt: Math.max(...finishes) || null,
      runtime: all.reduce((n, e) => n + e.runtime_ms, 0),
      res: "1080p",
      hdr: false,
      state: card.watched ? "watched" : started ? "in_progress" : "unwatched",
    };
  });
}

function entriesOf(libraryId: number): Entry[] | undefined {
  if (!libraries().some((l) => l.id === libraryId)) return undefined;
  if (libraryId === 1) return blenderEntries();
  if (libraryId === 2) return blenderEntries([6, 10, 13]);
  if (libraryId === 3) return showEntries();
  if (libraryId === 4) return archiveEntries();
  return [];
}

/** `The Matrix` sorts under M, `A Brief Season` under B. */
function sortTitle(title: string): string {
  return title.replace(/^(the|a|an)\s+/i, "");
}

function letterOf(e: Entry): string {
  const c = e.sortTitle.normalize("NFD")[0]?.toUpperCase() ?? "#";
  return /[A-Z]/.test(c) ? c : "#";
}

interface Query {
  sort: LibrarySort;
  order: "asc" | "desc";
  filter: (typeof WATCH_FILTERS)[number];
  res: Resolution | null;
  hdr: boolean;
}

function parseQuery(q: URLSearchParams): Query {
  const sort = LIBRARY_SORTS.find((s) => s === q.get("sort")) ?? "title";
  return {
    sort,
    order: SORT_ORDERS.find((o) => o === q.get("order")) ?? DEFAULT_ORDER[sort],
    filter: WATCH_FILTERS.find((f) => f === q.get("filter")) ?? "all",
    res: RESOLUTIONS.find((r) => r === q.get("res")) ?? null,
    hdr: q.get("hdr") === "true",
  };
}

const byTitle = (a: Entry, b: Entry) => a.sortTitle.localeCompare(b.sortTitle, "en", { sensitivity: "base", numeric: true }) || a.card.id - b.card.id;

/** Sorted and filtered, the server's way: unknown years, never watched and unknown runtimes go last in either order. */
function arrange(all: Entry[], q: Query, movies: boolean): Entry[] {
  const key = (e: Entry): number | null => ({ title: 0, year: e.year, added: e.added, watched: e.watchedAt, runtime: e.runtime })[q.sort];
  const dir = q.order === "asc" ? 1 : -1;
  return all
    .filter((e) => q.filter === "all" || e.state === q.filter)
    .filter((e) => !movies || ((!q.res || e.res === q.res) && (!q.hdr || e.hdr)))
    .sort((a, b) => {
      if (q.sort === "title") return dir * byTitle(a, b);
      const ka = key(a);
      const kb = key(b);
      if (ka === null || kb === null) return ka === kb ? byTitle(a, b) : ka === null ? 1 : -1;
      return dir * (ka - kb) || byTitle(a, b);
    });
}

function groupOf(e: Entry, sort: LibrarySort): { key: string; label: string } | null {
  if (sort === "title") {
    const letter = letterOf(e);
    return { key: letter, label: letter };
  }
  if (sort === "year") {
    if (e.year === null) return { key: "unknown", label: "Unknown year" };
    const decade = Math.floor(e.year / 10) * 10;
    return { key: String(decade), label: `${decade}s` };
  }
  if (sort === "added") {
    const d = new Date(e.added);
    return { key: `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`, label: `${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()}` };
  }
  return null;
}

const cursor = (q: Query, offset: number) => `${q.sort}.${q.order}.${offset}`;

function index(sorted: Entry[], q: Query): IndexGroup[] {
  const groups: IndexGroup[] = [];
  sorted.forEach((e, i) => {
    const g = groupOf(e, q.sort);
    if (!g) return;
    const last = groups[groups.length - 1];
    if (last?.key === g.key) last.count++;
    else groups.push({ ...g, count: 1, after: i === 0 ? null : cursor(q, i) });
  });
  return groups;
}

function bad(message: string): Reply {
  return { status: 400, body: { code: "bad_request", message, field: "after" } };
}

function page(libraryId: number, query: URLSearchParams): Reply {
  const all = entriesOf(libraryId);
  if (!all) return { status: 404, body: { code: "not_found", message: "No such library.", field: null } };
  const q = parseQuery(query);
  const movies = libraries().find((l) => l.id === libraryId)?.kind === "movies";
  const sorted = arrange(all, q, movies);
  const after = query.get("after");
  let start = 0;
  if (after !== null) {
    const m = /^(\w+)\.(asc|desc)\.(\d+)$/.exec(after);
    if (!m || m[1] !== q.sort || m[2] !== q.order) return bad("That cursor belongs to another sort.");
    start = Number(m[3]);
  }
  const limit = Math.min(200, Math.max(1, Number(query.get("limit") ?? 60) || 60));
  const end = start + limit;
  return {
    status: 200,
    body: { total: sorted.length, items: sorted.slice(start, end).map((e) => e.card), next: end < sorted.length ? cursor(q, end) : null } satisfies LibraryPage,
  };
}

function nextUp(show: FixtureShow): FixtureEpisode | null {
  const order = inOrder(show).filter((e) => e.season > 0);
  let last = -1;
  order.forEach((e, i) => watchState(e.id).watched && (last = i));
  return order.slice(last + 1).find((e) => !watchState(e.id).watched) ?? null;
}

function episodeRef(e: FixtureEpisode, seasonId: number): EpisodeRef {
  return {
    id: e.id,
    season_id: seasonId,
    season: e.season,
    index: e.index,
    title: episodeTitle(e),
    label: episodeLabel(e),
    position_ms: watchState(e.id).position_ms ?? null,
    thumb: e.thumb,
  };
}

const seasonIdOf = (show: FixtureShow, e: FixtureEpisode) => show.seasons.find((s) => s.episodes.includes(e))!.id;

function episodeVersion(show: FixtureShow, e: FixtureEpisode): Version {
  const number = e.season === 0 ? `S00E${String(e.index).padStart(2, "0")}` : `S${String(e.season).padStart(2, "0")}E${String(e.index).padStart(2, "0")}`;
  return {
    file_id: e.id,
    file_name: `${show.title} - ${number}${e.title ? ` - ${e.title}` : ""}.mkv`,
    size_bytes: 380_000_000 + e.index * 9_000_000,
    summary: "1080p · H.264 · AAC 2.0",
    playback: { kind: "direct_play", url: `/api/files/${e.id}/stream`, mime_type: "video/mp4" },
    media: {
      container: "mkv",
      duration_ms: e.runtime_ms,
      bitrate: 4_200_000,
      video: [{ codec: "H.264", profile: "High", width: 1920, height: 1080, frame_rate: 23.976, bit_depth: 8, hdr: null }],
      audio: [{ index: 1, codec: "aac", channels: 2, layout: "Stereo", language: "jpn", is_default: true }],
      subtitles: [],
    },
  };
}

function episodeRow(show: FixtureShow, e: FixtureEpisode): Episode {
  return {
    id: e.id,
    index: e.index,
    title: episodeTitle(e),
    label: episodeLabel(e),
    runtime_ms: e.runtime_ms,
    thumb: e.thumb,
    watch: watchState(e.id),
    versions: [episodeVersion(show, e)],
  };
}

function archiveItem(m: ArchiveMovie): ItemDetail {
  const width = Math.round(m.height * (16 / 9));
  const res = resolutionOf(width, m.height);
  const summary = [res === "4k" ? "4K" : res === "sd" ? "SD" : res, res === "4k" ? "HEVC" : "H.264", ...(m.hdr ? ["HDR10"] : []), "AAC 2.0"].join(" · ");
  return {
    kind: "movie",
    details: NO_DETAILS,
    id: m.id,
    title: m.title,
    year: m.year,
    runtime_ms: m.runtime_ms,
    overview: null,
    poster: null,
    backdrop: null,
    watch: watchState(m.id),
    versions: [
      {
        file_id: m.id,
        file_name: `${m.title}${m.year ? ` (${m.year})` : ""}.mp4`,
        size_bytes: Math.round(m.runtime_ms * (m.height / 400)),
        summary,
        playback: { kind: "direct_play", url: `/api/files/${m.id}/stream`, mime_type: "video/mp4" },
        media: {
          container: "mp4",
          duration_ms: m.runtime_ms,
          bitrate: 2_000_000 + m.height * 4_000,
          video: [{ codec: res === "4k" ? "HEVC" : "H.264", profile: "Main", width, height: m.height, frame_rate: 24, bit_depth: m.hdr ? 10 : 8, hdr: m.hdr ? "HDR10" : null }],
          audio: [{ index: 1, codec: "aac", channels: 2, layout: "Stereo", language: "eng", is_default: true }],
          subtitles: [],
        },
      },
    ],
  };
}

/** Archive movies, shows, seasons and episodes. The Blender movies stay in `fixtures.ts`. */
export function catalogItem(id: number): ItemDetail | undefined {
  const archived = ARCHIVE.find((m) => m.id === id);
  if (archived) return archiveItem(archived);
  const show = SHOWS.find((s) => s.id === id);
  if (show) {
    const all = episodes(show);
    const next = nextUp(show);
    const numbered = show.seasons.filter((s) => s.index > 0).sort((a, b) => a.index - b.index);
    return {
      kind: "show",
      details: NO_DETAILS,
      id: show.id,
      title: show.title,
      year_range: show.years[0] === show.years[1] ? String(show.years[0]) : `${show.years[0]}–${show.years[1]}`,
      overview: show.overview,
      poster: show.poster,
      backdrop: show.backdrop,
      seasons: [...numbered, ...show.seasons.filter((s) => s.index === 0)].map((s) => ({
        id: s.id,
        index: s.index,
        title: s.index === 0 ? "Specials" : `Season ${s.index}`,
        episode_count: s.episodes.length,
        watched_count: s.episodes.filter((e) => watchState(e.id).watched).length,
      })),
      next: next ? episodeRef(next, seasonIdOf(show, next)) : null,
      watched_count: all.filter((e) => watchState(e.id).watched).length,
      episode_count: all.length,
    };
  }
  const season = findSeason(id);
  if (season) {
    const { show: owner, season: s } = season;
    return {
      kind: "season",
      id: s.id,
      show_id: owner.id,
      show_title: owner.title,
      index: s.index,
      title: s.index === 0 ? "Specials" : `Season ${s.index}`,
      poster: owner.poster,
      backdrop: owner.backdrop,
      episode_count: s.episodes.length,
      watched_count: s.episodes.filter((e) => watchState(e.id).watched).length,
    };
  }
  const found = findEpisode(id);
  if (!found) return undefined;
  const { show: parent, season: s, episode: e } = found;
  const order = inOrder(parent);
  const after = order[order.indexOf(e) + 1];
  const next = after && (after.season > 0 || e.season === 0) ? after : undefined;
  return {
    kind: "episode",
    id: e.id,
    show_id: parent.id,
    season_id: s.id,
    show_title: parent.title,
    season: e.season,
    index: e.index,
    title: episodeTitle(e),
    label: episodeLabel(e),
    runtime_ms: e.runtime_ms,
    overview: null,
    poster: parent.poster,
    backdrop: parent.backdrop,
    thumb: e.thumb,
    versions: [episodeVersion(parent, e)],
    watch: watchState(e.id),
    next: next ? episodeRef(next, seasonIdOf(parent, next)) : null,
  };
}

/** File ids for a progress report: movies and episodes. */
export function catalogFiles(id: number): number[] | undefined {
  const item = catalogItem(id);
  return item?.kind === "movie" || item?.kind === "episode" ? item.versions.map((v) => v.file_id) : undefined;
}

/** A show's or season's episodes, for a mark on the whole of it. */
export function catalogEpisodes(id: number): number[] | undefined {
  const show = SHOWS.find((s) => s.id === id);
  if (show) return episodes(show).map((e) => e.id);
  return findSeason(id)?.season.episodes.map((e) => e.id);
}

function episodeCard(show: FixtureShow, e: FixtureEpisode, subtitle: string): PosterCard {
  const w = watchState(e.id);
  const progress = w.position_ms != null ? w.position_ms / e.runtime_ms : null;
  return { id: e.id, kind: "episode", title: show.title, year: show.years[0], poster: show.poster, watched: w.watched, progress, subtitle, show_id: show.id };
}

/** Movies and episodes in progress, plus each show's Next Up, by last activity. */
function continueRow(movie: (id: number) => PosterCard | undefined): PosterCard[] {
  const entries: { at: number; card: PosterCard }[] = [];
  for (const id of continueWatching()) {
    const ep = findEpisode(id);
    const card = ep ? episodeCard(ep.show, ep.episode, episodeLabel(ep.episode)) : movie(id);
    if (card) entries.push({ at: lastActivity(id) ?? 0, card });
  }
  for (const show of SHOWS) {
    const next = nextUp(show);
    const watched = episodes(show).filter((e) => watchState(e.id).watched);
    if (!next || watched.length === 0 || watchState(next.id).position_ms != null) continue;
    const at = Math.max(...watched.map((e) => lastActivity(e.id) ?? 0));
    entries.push({ at, card: episodeCard(show, next, `Next episode · ${episodeLabel(next)}`) });
  }
  return entries.sort((a, b) => b.at - a.at).map((e) => e.card);
}

function recentRow(movies: PosterCard[]): PosterCard[] {
  const now = Date.UTC(2026, 9, 22, 12);
  const shows = SHOWS.flatMap((show) => {
    const fresh = episodes(show).filter((e) => now - e.added_at < NEW_EPISODES_DAYS * DAY);
    if (!fresh.length) return [];
    return [{ at: Math.max(...fresh.map((e) => e.added_at)), card: showCard(show, `${fresh.length} new episode${fresh.length === 1 ? "" : "s"}`) }];
  });
  const dated = movies.map((card, i) => ({ at: now - (i + 1) * 3 * DAY, card }));
  return [...shows, ...dated].sort((a, b) => b.at - a.at).map((e) => e.card);
}

/** Continue Watching, Recently Added, then one row per library by title. */
export function homeRows(recentMovies: PosterCard[], movie: (id: number) => PosterCard | undefined): HomeRow[] {
  const resume = continueRow(movie);
  const recent = recentRow(recentMovies);
  const rows: HomeRow[] = [
    ...(resume.length ? [{ kind: "continue_watching" as const, title: "Continue Watching", total: resume.length, items: resume, next: null }] : []),
    { kind: "recently_added", title: "Recently Added", total: recent.length, items: recent, next: null },
  ];
  for (const lib of libraries()) {
    const sorted = arrange(entriesOf(lib.id) ?? [], parseQuery(new URLSearchParams()), false);
    if (!sorted.length) continue;
    rows.push({
      kind: "library",
      title: lib.name,
      library_id: lib.id,
      total: sorted.length,
      items: sorted.slice(0, ROW_PAGE).map((e) => e.card),
      next: sorted.length > ROW_PAGE ? `title.asc.${ROW_PAGE}` : null,
    });
  }
  return rows;
}

/** `GET /api/libraries`, the grid, its index, a library row's pages and a season's episodes; `undefined` for anything else. */
export function libraryRoute(path: string, query: URLSearchParams): Reply | undefined {
  if (path === "/api/libraries") {
    const body: LibrarySummary[] = libraries().map((l) => ({ id: l.id, name: l.name, kind: l.kind, item_count: entriesOf(l.id)?.length ?? 0 }));
    return { status: 200, body };
  }
  const lib = /^\/api\/libraries\/(\d+)\/(items|index)$/.exec(path);
  if (lib) {
    const id = Number(lib[1]);
    if (lib[2] === "items") return page(id, query);
    const all = entriesOf(id);
    if (!all) return { status: 404, body: { code: "not_found", message: "No such library.", field: null } };
    const q = parseQuery(query);
    const movies = libraries().find((l) => l.id === id)?.kind === "movies";
    return { status: 200, body: { groups: index(arrange(all, q, movies), q) } };
  }
  if (path === "/api/rows/library") {
    const after = query.get("after");
    const reply = page(Number(query.get("library")), new URLSearchParams({ ...(after && { after }), limit: String(ROW_PAGE) }));
    if (reply.status !== 200) return reply;
    const { items, next } = reply.body as LibraryPage;
    return { status: 200, body: { items, next: next ?? null } satisfies RowPage };
  }
  const eps = /^\/api\/items\/(\d+)\/episodes$/.exec(path);
  if (eps) {
    const found = findSeason(Number(eps[1]));
    if (!found) return { status: 404, body: { code: "not_found", message: "Not a season.", field: null } };
    return { status: 200, body: found.season.episodes.map((e) => episodeRow(found.show, e)) };
  }
  return undefined;
}
