# Nuvora Step 1 (Foundation): design

This is the design the Step 1 code is built against. Three Claude models (Opus 5.5, Fable 5.1, Sonnet 5.5) each produced a full design independently. This document is candidate 1 as the base, with grafts from the other two. See [Synthesis decision](#synthesis-decision).

Files: `module-map.md` (crates, packages, public vs private), `api.md` (endpoints and response types). The schema lives at `crates/library/migrations/0001_init.sql`.

## Problem

The goal is to point a Rust server at a movie folder and open a browser that shows the movies and plays one. That is the goal now. Four later steps must then build on this without reshaping it. What makes the shape non-obvious:

- **Later features.** The data model must later take shows, versions, several libraries, per-user state and TMDB overrides without table rewrites.
- **Rescans.** A rescan must be incremental, idempotent and crash-safe on NAS storage that can go offline mid-walk.
- **One request per screen.** Each screen must load in one request and paint artwork before the image bytes arrive.
- **The data layer.** It must run unchanged in React Native 0.72 (Vega), whose Hermes has no `btoa`.
- **The playability rule.** It must serve the web now, two TV platforms later, and the Step 3 transcoder.
- **FFmpeg.** The installed FFmpeg has no WebP encoder and no `zscale`. Fire OS (Android 9 to 11) cannot decode AVIF.

## Usage (caller's view)

Operator, Step 1 "done" check:

```sh
nuvora --movies ~/Movies            # binds 127.0.0.1:8640, serves cached catalog at once, scans in background
open http://127.0.0.1:8640
```

Developer:

```sh
bun install
cargo run -p nuvora-server -- --movies ./fixtures/movies   # API on :8640
bun run dev                                                # Vite on :5173, proxies /api
bun run gen:api                                            # after changing any wire type in api.rs
```

Web app call sites (`apps/web/src`):

```ts
// nuvora.ts: the only place platform APIs enter the data layer
export const client = createClient({ baseUrl: "", fetch: (u, i) => fetch(u, i as RequestInit), device: probeDeviceProfile() });
export const queries = createQueries(client);

// router.tsx: each screen's single request starts on navigation or on link hover/focus
loader: () => queryClient.ensureQueryData(queries.home()),

// Movie.tsx: the server decided; the view only renders the decision
switch (version.playback.kind) {
  case "direct_play":     return <Link to="/movie/$id/play" ...>Play</Link>;
  case "needs_transcode": return <p role="status">Needs transcoding, coming in Step 3: {p.reasons.join("; ")}</p>;
  case "unavailable":     return <p role="status">This file can't be read: {p.reason}</p>;
}

// Artwork.tsx: placeholder in the first frame, WebP fades in on top
<span style={{ aspectRatio: image.aspect_ratio, backgroundImage: `url(${placeholderDataUrl(image)})` }}>
  <img src={client.imageUrl(image, "small")} loading="lazy" decoding="async" ... />
```

Fire TV, Step 5. The same package is used unchanged:

```ts
const client = createClient({ baseUrl: serverUrl, fetch, device: await exoPlayerProfile() /* native module */ });
const { data } = useQuery(createQueries(client).home());
```

Server internals (`crates/server/src/api.rs`):

```rust
let home = lib.home().await?;                                  // domain read model
let item = lib.item(ItemId::from_raw(id), device).await?;      // playback decided per version inside
let status = lib.request_scan();                               // idempotent, coalescing
```

## Shape

### 1. Domain model and IDs

- **`item`.** One table for everything browsable, with columns `kind`, `parent_id` and `index_number`. Step 1 writes only `movie`. Shows, seasons and episodes are new rows and new Rust enum variants, not new tables. `kind` has no CHECK list: the Rust enum is the only list and parses at the row boundary.
- **`media_file`.** One row per file, and each file is a version of an item. Files group into an item by `item.identity_key`, which is `casefold(title)|year` from the filename parser. So `Alien (1979) - 1080p.mkv` and `Alien.1979.2160p.mkv` become two versions of one movie with no extra machinery.
- **Identity rule.** `identity_key` is built from the filename layer only. TMDB matching and user fixes never rewrite it. The one case that does is a rename detected by fingerprint to a title no other item uses: the scanner re-keys the item in place from the new filename, so the item id survives. A TMDB match in Step 2 writes `item_metadata.provider_id` on the `tmdb` row instead. This is what stops a re-probe or a rename from re-deriving a different key and creating a duplicate of a matched item. The cost is that two differently named files of one movie stay two items until Step 2 adds an explicit merge.
- **`media_stream`.** Every stream of every file, normalized, so Step 4 filters such as "4K HDR" are plain SQL.
- **Metadata layers.** Metadata lives in `item_metadata`, keyed `(item_id, source)`. Step 1 writes `filename`. Step 2 adds `tmdb` rows, plus sparse `user` rows that hold fixed fields and a pinned provider id. Precedence lives in one place, the `item_display` view, so re-matching TMDB rewrites only its own layer and never clobbers a user's fix. Artwork follows the same pattern: `artwork` rows are keyed `(item, role, source)`, and the `item_artwork` view picks one.
- **Libraries.** `library` rows are keyed by a stable config `key`. Each library has `library_root` rows. Files store `rel_path` relative to their root, so a remounted share changes one row.
- **IDs.** Item and file ids are SQLite `AUTOINCREMENT` integers. They are stable across rescans because rows update in place. They are stable across renames because moves are detected by fingerprint. They are never reused, so a stale client id cannot land on a different movie. Items whose files all vanish get `missing_since` instead of being deleted, so Step 2 watch state survives a temporarily unplugged drive.
- **Per-user state.** It becomes `user_item_state(user_id, item_id, ...)` in Step 2, which is an added table.

### 2. Module boundaries

The workspace has three crates and two packages. See `module-map.md`.

- **`nuvora-media`** knows things and holds no state: names, ffprobe, artwork rendering, playability. ffprobe JSON is private (`probe::raw`).
- **`nuvora-library`** is the only owner of persistent state: the database, the scanner and the artwork store. Its public surface is `Library` (`open`, `request_scan`, `scan_status`, `watch_scan`, `rev`, `home`, `row`, `item`, `stream_source`, `artwork_path`), plus domain read models. SQL, rows, the pool and the scanner are private modules, so the server cannot write to the catalog except through the scanner (per boundary-discipline).
- **`nuvora-server`** owns config, HTTP, and the wire types. The wire types live only in `api.rs` and are mapped from the read models. They add policy the domain does not have: URLs, display strings, and the playback decision rendered for humans.

### 3. Database

The database library is `rusqlite` with bundled SQLite, which runs the same on macOS, Windows and NAS. There is one writer `Mutex<Connection>` and a small pool of read-only connections, all driven through `spawn_blocking`. The database runs in WAL mode with `synchronous=NORMAL`. Reads run in one transaction, so home's two queries see one snapshot. Writes are short IMMEDIATE transactions, one per file. SQLite allows only one writer anyway, so making that structural removes SQLITE_BUSY retries (per separate-before-serializing-shared-state).

Migrations are `include_str!` SQL files tracked by `PRAGMA user_version`, with no migration crate. All tables are `STRICT`.

### 4. Scanner

The scan has five phases (see `scan/mod.rs`): Discover, Plan, Read, Artwork and Clean up.

- **Change detection.** A file is unchanged when `(root, rel_path)` matches and `(size, mtime_ns)` is equal. An unchanged rescan does one walk, two SELECTs and zero writes. The target is under 2 s, and the walk sets the limit.
- **Renames and moves.** A file that vanished is paired with a new file of equal size and an equal fingerprint, where the fingerprint is blake3 of size, the first 64 KiB and the last 64 KiB. Only equal-size pairs are fingerprinted. The file id survives the move. The item id survives too unless the new name is another existing movie, in which case the file becomes a version of that movie.
- **Idempotency and crash safety.** Every commit leaves a valid catalog. There is no "scan in progress" state on disk. Deletions happen only for files that are missing from a completely walked root. An unreadable root or subfolder (an offline NAS) deletes nothing. A readable root that walks as empty while the database holds files for it also deletes nothing, because that is what an automounter mount point looks like when the share is down (`may_mark_missing`, tested by `empty_walk_of_populated_root_blocks_missing_marks`). The guard is per root: an offline root blocks removals only for files under that root, and the other roots are cleaned up normally. A crash loses only uncommitted work, and the next scan rediscovers it. Two consecutive scans converge, and the second does nothing.
- **Files still being copied.** Files modified less than 60 s ago are left for the next scan.
- **Re-probing.** Bumping `PROBE_VERSION` re-probes every file exactly once.
- **Failed probes.** A probe failure is stored in `probe_error`, so the file is not retried until it changes.
- **Concurrency.** ffprobe runs with at most `min(cores, 8)` processes in flight (`buffer_unordered`), each with a 30 s timeout and `kill_on_drop`. Results commit as they arrive.
- **Requests.** One worker runs scans. A capacity-1 channel coalesces requests: ten POSTs during a scan produce one follow-up scan.
- **Progress.** Progress is a `watch` channel with exactly one sender, the worker. `GET /api/scan` reads it, and the web app polls it once a second only while a scan runs.
- **Startup.** The server binds first, then requests a scan, so the cached catalog is served immediately.

### 5. Artwork

FFmpeg is used only to grab a frame. Everything else happens in-process:

- `image` decodes JPEG, PNG and WebP.
- `fast_image_resize` resizes with Lanczos3 and SIMD.
- `libwebp` encodes through the `webp` crate, built from source, at q82.
- `thumbhash` makes the placeholder.

**Sources.** A poster comes from local artwork only. A movie without a local poster gets no poster image, and the web app renders a typographic poster card (title and year on a token gradient). A cropped video frame makes a poor poster, and TMDB posters arrive in Step 2. A backdrop comes from local artwork, or else from a frame grabbed at 10% of the runtime, but only for SDR video. HDR sources get no grabbed frame, because this FFmpeg has no `zscale` and an untone-mapped PQ frame looks washed out. Tone mapping in Rust was considered and cut from Step 1.

**Format.** WebP, because every target decodes it. AVIF would fail on Fire OS 7 and 8, and SVT-AV1 stills would make the server depend on how the user's FFmpeg was built.

**Sizes.** There are two per role, and the pixel widths are known only to `ImageSize::width`:

| role | small | large |
|---|---|---|
| poster | 320 | 640 |
| backdrop | 960 | 1920 |

Clients ask for `small` or `large`.

**Local artwork.** Generic names (`poster.jpg`, `folder.jpg`, `fanart.jpg`, `backdrop.jpg`) count only in a folder that holds one movie. `<name>-poster.jpg` and `<name>-fanart.jpg` always count.

**Cache layout.** Files are stored at `<data>/artwork/<hh>/<id>/{small,large}.webp`, where `id = blake3(PIPELINE_VERSION, role, source bytes)[..16]`:

- The same source means the same id, so a render is skipped and URLs are immutable.
- A put writes a temp directory and then renames it into place, which is atomic.
- The `artwork` table is the only reference to an id, so GC removes anything on disk that is not in it.

**Placeholder.** About 25 bytes of ThumbHash, sent inline in every `Image`. `@nuvora/core` decodes it into a PNG data URL without `btoa`.

### 6. Playability: the server decides, from a capability profile the client measures

- **What clients send.** Each client measures facts with its own platform API and sends them in the `Nuvora-Device` header. The web uses `MediaSource.isTypeSupported` with RFC 6381 strings. Fire TV will use `MediaCodecList` and Vega its media API.
- **What the server does.** `DeviceProfile::decide` in `nuvora-media` is pure. It checks the container, the primary video stream (codec, resolution, bit depth, HDR) and the default audio stream, and it returns every incompatibility it finds.
- **Why the server.**
  - Step 3 needs exactly this list to choose between remux, audio-only transcode and full transcode, so the decision and the transcode plan share one function.
  - Three clients would otherwise each re-implement codec rules in TypeScript and native code.
  - Only the server has the full probe data, such as Dolby Vision profile and bit depth.
- **Why the header.** It is stateless: there is no session or profile registry, and a server restart breaks nothing. `Vary: Nuvora-Device` keeps HTTP caches honest.
- **Defaults.** A missing header falls back to a conservative baseline. Unknown codec names in a profile are ignored, so an old server accepts a new client.
- **Matroska on the web.** Chromium plays most MKV files but reports `""` for `video/x-matroska`. The web profile therefore declares `matroska` when the browser is Chromium-based (User-Agent Client Hints brands), and the server's codec checks still apply. Safari and Firefox do not declare it. If the browser still fails, the player's `error` event shows the same "needs transcoding" state as a server-side refusal. Most real libraries are MKV, so this keeps H.264/AAC MKVs playable in Step 1 instead of waiting for Step 3.

### 7. API

There is one request per screen: `/api/home`, `/api/items/{id}` (the player reuses it), plus row paging, images, stream and scan endpoints. See `api.md`.

- **Bounded home.** Every home row returns its first page (at most 40 cards) plus `total` and an opaque `next` cursor. `GET /api/rows/{row}?after=<cursor>` returns the following pages with keyset pagination on the row's sort key, so a 10,000-movie library costs the same home payload as a 100-movie one. Row kinds and their sort keys live in one Rust enum.
- **Wire types.** They are `utoipa` `ToSchema` types in `api.rs`, registered through `utoipa-axum` `routes!`. A route cannot exist undocumented, and no path list is kept by hand.
- **Generation and drift.** Spec, then `openapi-typescript`, then `schema.ts`. A Rust test fails when the committed spec is stale. Running the generator on this sketch already caught one bug: `ImageSize` was referenced but not registered.
- **Caching.** A `catalog_rev` counter in the database is bumped by every write transaction that changes visible state. Home and row responses use it as their ETag with `no-cache`, so a repeat visit costs a 304 and no serialization. Item detail uses `"<rev>-<device profile hash>"`, because its body depends on the device. Images are `immutable`. The stream uses tower-http `ServeFile`, which handles Range, If-Range and HEAD and streams without buffering.
- **Compression.** JSON, JS and CSS are gzip-compressed at the fastest level. Brotli at its default level costs more time than the 50 ms home budget allows. `video/*` and `image/*` are never compressed, which keeps Range responses byte-exact.

### 8. Web app

- **Data layer.** `@nuvora/core` is plain TypeScript.
  - **I/O.** It is injected through `FetchLike`.
  - **Methods.** It exposes one method per screen.
  - **Cache policy.** Query definitions are plain objects in the TanStack shape, so the core needs no React or TanStack dependency.
  - **No DOM.** `lib: ["ES2022"]` with `types: []` turns any DOM use into a compile error.
  - **No deep imports.** `exports` exposes only `index.ts`.
- **Routing.** TanStack Router routes are `/`, `/movie/$id` and `/movie/$id/play?file=`.
  - Each loader starts its screen's request.
  - `defaultPreload: "intent"` prefetches the detail on poster hover or focus.
  - The detail screen paints title and placeholder from the cached home card (`findCard`) in the first frame.
  - `index.html` preloads `/api/home` before the JS bundle parses.
- **Design tokens.** `@nuvora/tokens` is a typed object shared with the TV apps. Vite serves it as `:root` CSS variables through a virtual module, so CSS cannot name a token that does not exist. The palette is a near-black canvas with a Plex-like amber accent.
- **Keyboard.** Posters are links in tab order. One shared `:focus-visible` ring comes from tokens. The player handles Escape. `prefers-reduced-motion` is respected.

### 9. Serving

- **Development.** Vite (port 5173) proxies `/api` to `127.0.0.1:8640`.
- **Production.** The built app is embedded in the binary with `rust-embed`. One file ships to Windows, macOS, NAS and Docker. UI and server versions cannot skew, and there is no path to configure. Debug builds read `dist/` from disk, so a rebuild of the server is not needed during web work.
- **Cache headers.** Vite's hashed `/assets/*` files are `immutable`. Everything else is `no-cache`, with an SPA fallback to `index.html`. Unknown `/api/*` paths return 404 and never fall back to `index.html`.
- **Build guard.** `build.rs` creates an empty `dist/` so a fresh clone compiles. When `index.html` is missing, non-API paths return 503 with the command that builds the web app, rather than a blank page.

## Synthesis decision

Three candidates were produced independently on Claude Opus 5.5 (candidate 1), Claude Fable 5.1 (candidate 2) and Claude Sonnet 5.5 (candidate 3), and scored on six criteria by a cross-judge on Claude Fable 5.1 that saw only path labels. Scores out of 30: candidate 1 28, candidate 3 26, candidate 2 19. The lead's own reading picked the same base.

**Base: candidate 1.** It is the only candidate with both a verified OpenAPI to TypeScript pipeline and a shared data layer that is strictly React Native safe (no DOM types, no `btoa`). It has the fewest crates with clean ownership, and its scanner covers the most operational cases: a settle window for files still copying, a memo for failed probes, and coalesced scan requests.

**Grafts from candidate 3.**
- Bounded home rows with keyset paging and a `catalog_rev` ETag, in place of an uncapped All Movies row and a body-hash ETag.
- The identity rule: the grouping key comes from the filename layer only and is never rewritten. Candidate 1 had planned to rewrite `match_key` to a TMDB id, which would duplicate matched items on the next re-probe.

**Grafts from candidate 2.**
- Gzip at the fastest level instead of default-level brotli.
- The empty-root guard for automounted shares.
- A 503 with the build command when the embedded web app is missing.

**Lead decisions not taken from any candidate.**
- No frame-grab posters and no HDR frame grabs. This cuts candidate 1's Rust tone mapping.
- The Chromium Matroska heuristic, which settles candidate 1's open question.

**Rejected.**
- Candidate 3's six crates and `boundaries.rs` test. Three crates already make the internals unreachable.
- Candidate 3's `openapi-fetch` runtime, whose types and runtime use DOM `Request` and `Headers`.
- Candidate 2's wire types in the core crate.
- Candidate 2's effective title columns on `item`. They would give the scanner, the matcher and the editor shared ownership of one column.
- Candidate 2's client-side playability decision, which Step 3 would throw away.

## Tradeoffs accepted

- **Metadata layers.** We accept a join through the `item_display` view, and one more table than "title on item", in exchange for TMDB and user overrides being added rows rather than a reshape.
- **Mapping wire types.** We accept mapping read models to wire types in the server, in exchange for SQL rows and domain enums never reaching the wire and the server owning URLs and display strings.
- **Paged rows.** We accept a second endpoint (`/api/rows/{row}`) and cursor handling in the client, in exchange for a home payload that does not grow with the library.
- **Identity never follows the match.** We accept that two differently named files of one movie stay separate items until Step 2 adds a merge, in exchange for a scan that can never duplicate a matched item.
- **Typographic posters.** We accept that a fresh library without local artwork shows designed title cards instead of images until Step 2, in exchange for no cropped-frame posters and no HDR tone mapping code.
- **C build for WebP.** We accept a C build of libwebp, in exchange for a modern format that every TV decodes and no dependency on how the user's FFmpeg was built.
- **Larger request header.** We accept a few hundred bytes of `Nuvora-Device` header per request, in exchange for a stateless server with no device registry.
- **Missing items stay.** We accept that missing items stay in the database indefinitely (Step 4 adds a purge setting), in exchange for ids and future watch state surviving an unplugged drive.
- **Libraries from config.** We accept config-file libraries keyed by slug in Step 1 (the Step 4 admin UI moves ownership into the database), in exchange for no settings API now.
- **Revision ETag.** We accept one more piece of state (`catalog_rev`, bumped inside the write transaction that changes visible rows), in exchange for 304s that cost no query and no serialization.

## Alternatives considered

- **One crate with modules.** It is simpler to build, but `pub(crate)` lets the HTTP layer reach SQL directly. Separate crates make the catalog's internals impossible to import.
- **A fourth `nuvora-api` crate for wire types.** It adds a crate with one consumer. Wire types stay private in the server.
- **sqlx (async).** Its SQLite driver is a thread wrapper anyway. Compile-time query checks need a database URL or offline data in CI. It gives less control over the writer, the statement cache and pragmas. rusqlite with one explicit writer is simpler and faster.
- **Client-side playability (`canPlayType` in the shared layer).** It needs the DOM, so it cannot live in the shared layer. Each client would duplicate codec rules, and Step 3 would need a second, server-side copy of the same rules.
- **Registering the profile per device (`POST /devices` returning an id).** This is server state that is lost or stale on restart, plus an extra request. The header is stateless.
- **AVIF through FFmpeg and libsvtav1.** Fire OS cannot decode it, SVT-AV1 has minimum-size and even-dimension limits, and it ties artwork to the user's FFmpeg build. JPEG through `image` was the runner-up: pure Rust, but about 30% larger.
- **Serving the web app from a directory.** It adds a configurable path, can mismatch versions after a partial upgrade, and is worse for installers.
- **Mark-and-sweep with a `seen_scan` column.** It writes every unchanged row on every scan. The in-memory diff writes nothing for unchanged files and is equally crash-safe, because deletion waits for a complete walk.
- **`openapi-fetch` in the shared package.** Its types reference DOM `Request` and `Response`, which would force `lib: DOM` into the package that must not have it.
- **Shared React hooks in `@nuvora/core`.** These would pull in React versions and TanStack as peers. Plain query definitions give the same policy reuse at zero coupling.

## Implementation reconciliation

- Running `openapi-typescript` on the sketch's spec failed on an unregistered `ImageSize` schema. Fixed in `ApiDoc` by registering it.
- Scaffold (accepted by the lead): `MatchKey` is renamed `IdentityKey`, matching the SQL column. `Home` and `RowPage` carry the `rev` read in the same transaction as their rows, so an ETag always matches its body, and `Library::rev()` serves only the pre-query 304 check. Row titles are produced in the server (`row_title`), which owns display strings. The web profile declares the container as `mkv`, the server's wire name. `MAX_ROW_LIMIT` is 100. The backdrop frame is taken at 10% of the runtime with no clamp.
- Lead decision after the scaffold: `may_mark_missing` is per root, not per scan. The scaffold had it per scan, which let one offline root block removals in every other root.

## Open questions and risks

- Does `libwebp-sys` cross-compile cleanly for Windows MSVC and Linux arm64 in the Step 5 CI? Fallback: JPEG through `image`, a one-function change in `artwork.rs`.
- Fingerprint reads cost two round trips per new or changed file on SMB. Is that acceptable on a first scan of a 2,000-file NAS library (about 4,000 reads)?
- React Native `fetch` on Android has no HTTP cache by default, so ETag revalidation only helps the web until the TV apps enable OkHttp caching.
- utoipa marks `Option` fields as optional and nullable (`?: T | null`), while serde always emits `null`. Should Option fields be tightened to required-nullable for cleaner TypeScript types?

## Next implementation step

Implement `nuvora-media::name` and `probe` against the table tests and ffmpeg-generated fixtures. Everything else consumes their output.
