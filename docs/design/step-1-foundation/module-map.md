# Module map

```
nuvora/
├─ Cargo.toml                 workspace: 3 crates
├─ rust-toolchain.toml        stable
├─ package.json               Bun workspace: packages/*, apps/*; gen:api, dev, build, typecheck
├─ schema.sql -> crates/library/migrations/0001_init.sql
├─ crates/
│  ├─ media/      nuvora-media     stateless media knowledge
│  ├─ library/    nuvora-library   the catalog: DB + scanner + artwork store + read models
│  └─ server/     nuvora-server    bin `nuvora`: config, HTTP, OpenAPI, embedded web app
├─ packages/
│  ├─ core/       @nuvora/core     data layer for web + TV (no DOM, no React)
│  └─ tokens/     @nuvora/tokens   design tokens for web + TV
└─ apps/
   └─ web/        @nuvora/web      React + Vite app (views only)
```

Dependency direction is one way: `server -> library -> media`, `web -> core, tokens`.
Nothing points back up.

## Rust crates

### `nuvora-media` (stateless; owns knowledge, not state)

| module | public | private |
|---|---|---|
| `name` | `classify_path`, `PathClass`, `MovieName::parse`, `MovieName::match_key`, `MatchKey` | token tables, year heuristics |
| `probe` | `MediaInfo` and stream types, codec enums with `as_str`/`from_name`, `Container`, `HdrFormat`, `PROBE_VERSION`, `probe()` | `raw::*` (ffprobe JSON), `parse()` |
| `artwork` | `ArtworkRole`, `ImageSize` (+ `width`), `ArtworkSource`, `Frame`, `RenderedArtwork::render`, `MediaTools::grab_frame`, `Placeholder`, `PIPELINE_VERSION` | decode/resize/WebP/ThumbHash, HDR tone-map |
| `playback` | `DeviceProfile` (+ `baseline`, `decide`), `VideoSupport`, `AudioSupport`, `PlaybackDecision`, `Incompatibility` | rule helpers |
| `tools` | `MediaTools::locate` | binary paths, timeouts |

Tests: filename table tests; ffprobe tests on ffmpeg-generated fixtures (mp4/h264/aac,
mkv/hevc10/eac3 HDR10-tagged, webm/vp9/opus, mkv/av1/flac, avi/mpeg4/ac3, subtitles,
attached cover art); playability table tests; render tests (sizes, never upscale, aspect clamp).

### `nuvora-library` (the only owner of persistent state)

Public: `Library` (`open`, `request_scan`, `scan_status`, `watch_scan`, `home`, `item`,
`stream_source`, `artwork_path`), `LibraryConfig`, `LibrarySpec`, `LibraryKind`, ids
(`ItemId`, `FileId`, `ArtworkId`), read models (`Home`, `Row`, `Card`, `Artwork`, `Item`,
`Movie`, `Version`, `VersionMedia`, `StreamSource`), scan status types.

Private (unreachable from the server, so it cannot write SQL or bypass the scanner):

| module | owns |
|---|---|
| `db` | connection pool, pragmas, migrations (`PRAGMA user_version`) |
| `views` | every read query and the row -> domain mapping |
| `scan/mod` | scan worker, coalescing requests, status `watch` channel |
| `scan/walk` | directory walk, `fingerprint` |
| `scan/plan` | pure diff: unchanged/changed/new/gone/moved, artwork wanted-source |
| `scan/write` | every write transaction |
| `store` | content-addressed artwork files, atomic put, GC |

Call chain for any request: `server::api` handler -> `Library` method -> `views` fn. Three files.

### `nuvora-server` (bin `nuvora`)

| module | owns |
|---|---|
| `main` | CLI (`nuvora`, `nuvora openapi`), startup order |
| `config` | `nuvora.toml` + flags -> validated `Config`; the only reader of config input |
| `api` | wire types (`ToSchema`), handlers, `Device` header extractor, ETag helper, compression policy, OpenAPI via `utoipa-axum` `routes!` |
| `web` | embedded `apps/web/dist` (rust-embed), SPA fallback, cache headers |

## TypeScript packages

### `@nuvora/core`

`package.json#exports` exposes only `src/index.ts`. `tsconfig` has `lib: ["ES2022"]`,
`types: []`: DOM globals are a compile error (verified: `window` and `btoa` fail).

| file | owns |
|---|---|
| `generated/schema.ts` | `openapi-typescript` output from `openapi.json` (never edited) |
| `transport.ts` | `FetchLike` and friends: the injected I/O boundary; `ApiError` |
| `client.ts` | `createClient` -> one method per screen/action; path literals checked against generated `paths` |
| `queries.ts` | `createQueries` -> TanStack-shaped `{queryKey, queryFn, staleTime}`; `findCard`, `isScanRunning` |
| `images.ts` | `placeholderDataUrl` (ThumbHash -> PNG data URL, no `btoa`) |

### `@nuvora/tokens`

`tokens` object (color, font, type, space, radius, poster, motion) and `cssVariables()`.

### `@nuvora/web`

| path | role |
|---|---|
| `src/nuvora.ts` | singletons: client (with `fetch` + device profile), queries, QueryClient |
| `src/router.tsx` | TanStack Router: `/`, `/movie/$id`, `/movie/$id/play?file=` with loaders |
| `src/screens/` | `Home`, `Movie`, `Player` |
| `src/components/` | `PosterRow`, `PosterCard`, `Artwork` (placeholder cross-fade), `ScanBanner` |
| `src/platform/deviceProfile.ts` | `MediaSource.isTypeSupported` probing (DOM; app-only) |
| `src/styles/global.css` | base + focus ring, all values from token variables |
| `vite.config.ts` | `virtual:nuvora-tokens.css` plugin, `/api` proxy to `127.0.0.1:8640` |
