import type { FetchLike, HomeResponse, HttpMethod, ItemDetail, PosterCard, RowPage, ScanStatus, Version } from "@nuvora/core";

/**
 * Dev-only transport (`VITE_FIXTURES=1`): answers the JSON endpoints from the
 * literals below, typed against the generated schema, so contract drift fails
 * the typecheck. Images and the stream are served by the fixture plugin in
 * `vite.config.ts`.
 *
 * Sample content: Blender Studio open movie titles and years. Runtimes, sizes,
 * codecs and the gradient artwork are invented.
 */

const movies = [
  [1, "Agent 327: Operation Barbershop", 2017],
  [2, "Big Buck Bunny", 2008],
  [3, "Caminandes: Llama Drama", 2013],
  [4, "Charge", 2022],
  [5, "Coffee Run", 2020],
  [6, "Cosmos Laundromat", 2015],
  [7, "Elephants Dream", 2006],
  [8, "Glass Half", 2015],
  [9, "Hero", 2018],
  [10, "Sintel", 2010],
  [11, "Spring", 2019],
  [12, "Sprite Fright", 2021],
  [13, "Tears of Steel", 2012],
  [14, "Wing It!", 2023],
] as const;

const posters: Record<number, NonNullable<PosterCard["poster"]>> = {
  2: { id: "5a1d0c0ffee000000000000000000001", aspect_ratio: 2 / 3, placeholder: "FjoOdQqHd4CIiHh4iIiHh4eAeAiI" },
};
const backdrops: Record<number, NonNullable<ItemDetail["backdrop"]>> = {
  2: { id: "5a1d0c0ffee000000000000000000002", aspect_ratio: 16 / 9, placeholder: "U7YNLIZAh3iJd4eHiIi/dvhrhw==" },
};

const cards: PosterCard[] = movies.map(([id, title, year]) => ({ id, kind: "movie", title, year, poster: posters[id] ?? null }));
const byId = (id: number): PosterCard => cards.find((c) => c.id === id)!;

const recent = [14, 13, 12, 5, 11, 9, 4].map(byId);
const ALL_FIRST_PAGE = 8;

const home = {
  rows: [
    { kind: "recently_added", title: "Recently Added", total: recent.length, items: recent, next: null },
    { kind: "all_movies", title: "All Movies", total: cards.length, items: cards.slice(0, ALL_FIRST_PAGE), next: "after-8" },
  ],
} satisfies HomeResponse;

const allMoviesPage2 = { items: cards.slice(ALL_FIRST_PAGE), next: null } satisfies RowPage;

const directPlay = (id: number): Version["playback"] => ({ kind: "direct_play", url: `/api/files/${id}/stream`, mime_type: "video/mp4" });

function defaultVersion(id: number, title: string, year: number): Version {
  return {
    file_id: id,
    file_name: `${title} (${year}).mp4`,
    size_bytes: 412_000_000 + id * 37_000_000,
    summary: "1080p · H.264 · AAC 2.0",
    playback: directPlay(id),
    media: {
      container: "mp4",
      duration_ms: 540_000 + id * 61_000,
      bitrate: 6_100_000,
      video: [{ codec: "H.264", profile: "High", width: 1920, height: 1080, frame_rate: 24, bit_depth: 8, hdr: null }],
      audio: [{ index: 1, codec: "aac", channels: 2, layout: "Stereo", language: "eng", is_default: true }],
      subtitles: [],
    },
  };
}

const versionOverrides: Record<number, (v: Version) => Version[]> = {
  13: (v) => [
    {
      ...v,
      file_name: "Tears of Steel (2012).webm",
      size_bytes: 726_000_000,
      summary: "1080p · VP9 · Opus 2.0",
      playback: { kind: "direct_play", url: "/api/files/13/stream", mime_type: "video/webm" },
      media: {
        container: "webm",
        duration_ms: 734_000,
        bitrate: 7_900_000,
        video: [{ codec: "VP9", profile: "Profile 0", width: 1920, height: 800, frame_rate: 24, bit_depth: 8, hdr: null }],
        audio: [{ index: 1, codec: "opus", channels: 2, layout: "Stereo", language: "eng", is_default: true }],
        subtitles: [
          { index: 2, codec: "subrip", language: "eng", is_default: false, is_forced: false },
          { index: 3, codec: "subrip", language: "spa", is_default: false, is_forced: false },
        ],
      },
    },
  ],
  10: (v) => [
    {
      ...v,
      file_name: "Sintel (2010).mkv",
      summary: "1080p · H.264 · AC3 5.1",
      playback: { kind: "needs_transcode", reasons: ["AC3 audio isn't supported on this device"] },
      media: {
        ...v.media!,
        container: "mkv",
        duration_ms: 888_000,
        audio: [{ index: 1, codec: "ac3", channels: 6, layout: "5.1", language: "eng", is_default: true }],
      },
    },
  ],
  8: (v) => [{ ...v, summary: "", media: null, playback: { kind: "unavailable", reason: "ffprobe could not read the file's header." } }],
  9: () => [],
};

function item(id: number): ItemDetail | undefined {
  const movie = movies.find(([m]) => m === id);
  if (!movie) return undefined;
  const [, title, year] = movie;
  const base = defaultVersion(id, title, year);
  const versions = versionOverrides[id]?.(base) ?? [base];
  return {
    kind: "movie",
    id,
    title,
    year,
    runtime_ms: versions[0]?.media?.duration_ms ?? null,
    overview: null,
    poster: posters[id] ?? null,
    backdrop: backdrops[id] ?? null,
    versions,
  };
}

/** A scan that is part-way through when the app opens: one file every 6 s, so it ends after about 18 s. */
const SCAN_FILE_MS = 6_000;
const scan = { startedAt: Date.now(), from: 12, total: 15 };
function scanStatus(): ScanStatus {
  const done = scan.from + Math.floor((Date.now() - scan.startedAt) / SCAN_FILE_MS);
  if (done >= scan.total) {
    return {
      state: "idle",
      last: { added: 0, changed: 0, moved: 0, removed: 0, failed: 0, unchanged: 14, duration_ms: 4_200, finished_at: Date.now(), issues: [] },
    };
  }
  return { state: "running", phase: "reading", done, total: scan.total, queued: false, last: null };
}

export const fixtureFetch: FetchLike = async (url, init) => {
  const path = new URL(url, location.origin).pathname;
  await new Promise((r) => setTimeout(r, 60));
  const body = route(init.method, path);
  return body === undefined
    ? { ok: false, status: 404, json: async () => ({ error: "not found" }) }
    : { ok: true, status: 200, json: async () => body };
};

function route(method: HttpMethod, path: string): unknown {
  if (path === "/api/home") return home;
  if (path === "/api/rows/all_movies") return allMoviesPage2;
  if (path === "/api/scan" && method === "POST") {
    if (scanStatus().state === "idle") Object.assign(scan, { startedAt: Date.now(), from: 0 });
    return scanStatus();
  }
  if (path === "/api/scan") return scanStatus();
  const itemMatch = /^\/api\/items\/(\d+)$/.exec(path);
  if (itemMatch) return item(Number(itemMatch[1]));
  return undefined;
}
