import type { PosterCard, ProgressRequest, WatchState } from "@nuvora/core";
import type { Reply } from "./fixtureAccounts";

/**
 * The watch-state half of the fixture server, with the real server's rules:
 * under 60 s clears progress, 90% or more finishes (once per 3 hours), and
 * marking by hand works like finishing. State lives in sessionStorage next to
 * the accounts and starts over with any `?fixtures=` URL. One viewer's state
 * for every fixture user: the per-user split is the server's job.
 */

interface Entry {
  position_ms: number | null;
  duration_ms: number | null;
  file_id: number | null;
  play_count: number;
  last_watched_at: number | null;
  updated_at: number;
}

const STORAGE_KEY = "nuvora-fixtures-watch";
const MIN_PROGRESS_MS = 60_000;
const FINISHED = 0.9;
const RECOUNT_AFTER_MS = 3 * 3_600_000;
const OVERSHOOT_MS = 5_000;
const MINUTE = 60_000;

function seed(): Record<number, Entry> {
  const now = Date.now();
  const watching = (file: number, position: number, duration: number, ago: number): Entry => ({
    position_ms: position,
    duration_ms: duration,
    file_id: file,
    play_count: 0,
    last_watched_at: null,
    updated_at: now - ago,
  });
  const watched = (ago: number): Entry => ({ position_ms: null, duration_ms: null, file_id: null, play_count: 1, last_watched_at: now - ago, updated_at: now - ago });
  return {
    2: watching(2, 410_000, 662_000, 20 * MINUTE),
    13: watching(13, 431_000, 734_000, 26 * 60 * MINUTE),
    5: watching(5, 178_000, 845_000, 3 * 24 * 60 * MINUTE),
    7: watched(5 * 24 * 60 * MINUTE),
    12: watched(9 * 24 * 60 * MINUTE),
    3: watched(30 * 24 * 60 * MINUTE),
  };
}

const store: Record<number, Entry> = load();

function load(): Record<number, Entry> {
  const fresh = new URLSearchParams(location.search).has("fixtures");
  const stored = sessionStorage.getItem(STORAGE_KEY);
  const next = fresh || !stored ? seed() : (JSON.parse(stored) as Record<number, Entry>);
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  return next;
}

function save() {
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(store));
}

export function watchState(id: number): WatchState {
  const e = store[id];
  return { position_ms: e?.position_ms ?? null, file_id: e?.file_id ?? null, watched: (e?.play_count ?? 0) > 0, play_count: e?.play_count ?? 0, last_watched_at: e?.last_watched_at ?? null };
}

/** A catalog card with this viewer's progress and watched mark. */
export function withWatch(card: PosterCard): PosterCard {
  const e = store[card.id];
  const progress = e?.position_ms != null && e.duration_ms ? e.position_ms / e.duration_ms : null;
  return { ...card, progress, watched: (e?.play_count ?? 0) > 0 };
}

/** Item ids in progress, most recently watched first. */
export function continueWatching(): number[] {
  return Object.entries(store)
    .filter(([, e]) => e.position_ms != null)
    .sort(([, a], [, b]) => b.updated_at - a.updated_at)
    .map(([id]) => Number(id));
}

function invalid(message: string, field: string): Reply {
  return { status: 400, body: { code: "validation", message, field } };
}

function finish(id: number, now: number) {
  const e = store[id];
  store[id] = { position_ms: null, duration_ms: null, file_id: null, play_count: (e?.play_count ?? 0) + 1, last_watched_at: now, updated_at: now };
}

/** `PUT /api/items/{id}/progress`, `PUT` and `DELETE /api/items/{id}/watched`; `undefined` for other routes. */
export function watchRoute(method: string, path: string, body: unknown, fileIds: (id: number) => number[] | undefined): Reply | undefined {
  const match = /^\/api\/items\/(\d+)\/(progress|watched)$/.exec(path);
  if (!match) return undefined;
  const id = Number(match[1]);
  const files = fileIds(id);
  if (!files) return { status: 404, body: { code: "not_found", message: "Not found", field: null } };
  const now = Date.now();
  if (match[2] === "watched") {
    if (method === "PUT") finish(id, now);
    else if (method === "DELETE") store[id] = { position_ms: null, duration_ms: null, file_id: null, play_count: 0, last_watched_at: null, updated_at: now };
    else return { status: 405 };
    save();
    return { status: 204 };
  }
  if (method !== "PUT") return { status: 405 };
  const report = body as ProgressRequest;
  if (!files.includes(report.file_id)) return invalid("That file isn't a version of this movie.", "file_id");
  if (report.duration_ms < 1) return invalid("Duration must be at least 1 ms.", "duration_ms");
  if (report.position_ms > report.duration_ms + OVERSHOOT_MS) return invalid("Position is past the end.", "position_ms");
  const e = store[id];
  if (report.position_ms >= FINISHED * report.duration_ms) {
    const recent = e?.position_ms == null && e?.last_watched_at != null && now - e.last_watched_at < RECOUNT_AFTER_MS;
    if (!recent) finish(id, now);
  } else {
    const kept = { play_count: e?.play_count ?? 0, last_watched_at: e?.last_watched_at ?? null, updated_at: now };
    store[id] =
      report.position_ms < MIN_PROGRESS_MS
        ? { ...kept, position_ms: null, duration_ms: null, file_id: null }
        : { ...kept, position_ms: report.position_ms, duration_ms: report.duration_ms, file_id: report.file_id };
  }
  save();
  return { status: 204 };
}
