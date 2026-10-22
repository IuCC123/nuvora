import { accountRoute, hasLibraries, signedIn, signedInAsAdmin, type Reply } from "./fixtureAccounts";
import { continueWatching, watchRoute, watchState, withWatch } from "./fixtureWatch";
import type { FetchLike, FontAttachment, HomeResponse, HttpMethod, ItemDetail, PosterCard, RowPage, ScanStatus, Version } from "@nuvora/core";

/**
 * Dev-only transport (`VITE_FIXTURES=1`): answers the JSON endpoints from the
 * literals below, typed against the generated schema, so contract drift fails
 * the typecheck. Images and the stream are served by the fixture plugin in
 * `vite.config.ts`. Watch state follows the server's rules (`fixtureWatch.ts`).
 *
 * A scripted check can make requests fail: `sessionStorage["nuvora-fixtures-fail"]`
 * holds a pattern, optionally after a status (`"401 ^/api/auth/logout$"`), and every
 * matching path and query answers that status, 503 by default, until it is removed.
 *
 * Sample content: Blender Studio open movie titles and years. Runtimes, sizes,
 * codecs and the gradient artwork are invented. Subtitles: Tears of Steel (13)
 * has two SubRip tracks and a PGS one that needs conversion; "Lantern Street - 01"
 * (15, reached only by its player URL until shows exist here) stands in for the
 * owner's anime, with Japanese audio, a default-flagged Spanish ASS track with
 * positioned signs, a French one and an embedded font. The track text and font
 * are files the fixture plugin in `vite.config.ts` serves (`fixtures/subtitles`).
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

const cards = (): PosterCard[] => movies.map(([id, title, year]) => withWatch({ id, kind: "movie", title, year, poster: posters[id] ?? null, watched: false }));
const byId = (id: number): PosterCard => cards().find((c) => c.id === id)!;

const ALL_FIRST_PAGE = 8;

function home(): HomeResponse {
  if (!hasLibraries()) return { rows: [] };
  const recent = [14, 13, 12, 5, 11, 9, 4].map(byId);
  const resume = continueWatching().map(byId);
  const all = cards();
  return {
    rows: [
      ...(resume.length ? [{ kind: "continue_watching" as const, title: "Continue Watching", total: resume.length, items: resume, next: null }] : []),
      { kind: "recently_added", title: "Recently Added", total: recent.length, items: recent, next: null },
      { kind: "all_movies", title: "All Movies", total: all.length, items: all.slice(0, ALL_FIRST_PAGE), next: "after-8" },
    ],
  };
}

const ROW_PAGE = 3;

/** The rest of All Movies, three at a time, so a scroll loads more than one page. */
function allMoviesAfter(after: string | null): RowPage {
  const start = Number(/^after-(\d+)$/.exec(after ?? "")?.[1] ?? ALL_FIRST_PAGE);
  const all = cards();
  const end = start + ROW_PAGE;
  return { items: all.slice(start, end), next: end < all.length ? `after-${end}` : null };
}

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

const EPISODE = [15, "Lantern Street - 01", 2024] as const;

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
          { index: 2, codec: "SRT", format: "srt", renderable: true, language: "eng", is_default: false, is_forced: false, is_hearing_impaired: false },
          { index: 3, codec: "SRT", format: "srt", renderable: true, language: "spa", is_default: false, is_forced: false, is_hearing_impaired: false },
          { index: 4, codec: "PGS", format: "image", renderable: false, language: "eng", title: "Commentary", is_default: false, is_forced: false, is_hearing_impaired: false },
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
  [EPISODE[0]]: (v) => [
    {
      ...v,
      file_name: "[Ñ] Lantern Street - 01 [1080p] [3F2A91C0].mkv",
      size_bytes: 318_000_000,
      summary: "1080p · H.264 · AAC 2.0",
      media: {
        ...v.media!,
        container: "mkv",
        duration_ms: 1_420_000,
        audio: [{ index: 1, codec: "aac", channels: 2, layout: "Stereo", language: "jpn", is_default: true }],
        subtitles: [
          { index: 2, codec: "ASS", format: "ass", renderable: true, language: "spa", title: "Español", is_default: true, is_forced: false, is_hearing_impaired: false },
          { index: 3, codec: "ASS", format: "ass", renderable: true, language: "fre", title: "Complets", is_default: false, is_forced: false, is_hearing_impaired: false },
        ],
      },
    },
  ],
};

/** The fonts the episode's file embeds: Archivo, which its signs name. Served by the fixture plugin. */
function fonts(fileId: number): FontAttachment[] | undefined {
  if (fileId !== EPISODE[0]) return fileId > 0 && fileId <= movies.length ? [] : undefined;
  return [{ index: 5, file_name: "Archivo-Bold.ttf", mime_type: "font/ttf", size_bytes: 40_748, url: `/api/files/${fileId}/fonts/5` }];
}

function item(id: number): ItemDetail | undefined {
  const movie = id === EPISODE[0] ? EPISODE : movies.find(([m]) => m === id);
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
    watch: watchState(id),
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
  const { pathname, searchParams } = new URL(url, location.origin);
  await new Promise((r) => setTimeout(r, 60));
  const body: unknown = init.body ? JSON.parse(init.body) : undefined;
  const [, status = "503", pattern] = /^(?:(\d{3}) )?(.+)$/.exec(sessionStorage.getItem("nuvora-fixtures-fail") ?? "") ?? [];
  if (pattern && new RegExp(pattern).test(pathname + (searchParams.size ? `?${searchParams}` : ""))) {
    console.info(`fixture ${status} ${init.method} ${pathname}`);
    const code = status === "401" ? "unauthorized" : "internal";
    return { ok: false, status: Number(status), json: async () => ({ code, message: "Failed on request", field: null }) };
  }
  const reply = accountRoute(init.method, pathname, searchParams, body) ?? catalogRoute(init.method, pathname, searchParams, body);
  return {
    ok: reply.status < 300,
    status: reply.status,
    json: async () => {
      if (reply.body === undefined) throw new SyntaxError("Unexpected end of JSON input");
      return reply.body;
    },
  };
};

function catalogRoute(method: HttpMethod, path: string, query: URLSearchParams, request: unknown): Reply {
  if (!signedIn()) return { status: 401, body: { code: "unauthorized", message: "Sign in to continue.", field: null } };
  const watch = watchRoute(method, path, request, (id) => item(id)?.versions.map((v) => v.file_id));
  if (watch) return watch;
  if (path === "/api/scan" && method === "POST" && !signedInAsAdmin()) {
    return { status: 403, body: { code: "forbidden", message: "Only an admin can do that.", field: null } };
  }
  const body = route(method, path, query);
  return body === undefined ? { status: 404, body: { code: "not_found", message: "Not found", field: null } } : { status: 200, body };
}

function route(method: HttpMethod, path: string, query: URLSearchParams): unknown {
  if (path === "/api/home") return home();
  if (path === "/api/rows/all_movies") return allMoviesAfter(query.get("after"));
  if (path === "/api/scan" && method === "POST") {
    if (scanStatus().state === "idle") Object.assign(scan, { startedAt: Date.now(), from: 0 });
    return scanStatus();
  }
  if (path === "/api/scan") return scanStatus();
  const fontsMatch = /^\/api\/files\/(\d+)\/fonts$/.exec(path);
  if (fontsMatch) return fonts(Number(fontsMatch[1]));
  const itemMatch = /^\/api\/items\/(\d+)$/.exec(path);
  if (itemMatch) return item(Number(itemMatch[1]));
  return undefined;
}
