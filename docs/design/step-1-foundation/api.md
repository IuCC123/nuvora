# HTTP API

Source of truth: the wire types and `#[utoipa::path]` handlers in
`crates/server/src/api.rs`. `nuvora openapi` prints the spec; it is committed at
`packages/core/openapi.json` (generated from this sketch, not hand-written) and
`openapi-typescript` turns it into `packages/core/src/generated/schema.ts`. A Rust
test fails when the committed spec differs from what the code generates.

```
bun run gen:api   =  cargo run -p nuvora-server -- openapi > packages/core/openapi.json
                     && openapi-typescript openapi.json -o src/generated/schema.ts
```

## Endpoints

| method | path | response (Rust type) | caching |
|---|---|---|---|
| GET | `/api/home` | `HomeResponse { rows: Vec<HomeRow> }`, first page of each row | `no-cache` + ETag `"<catalog_rev>"` -> 304 |
| GET | `/api/rows/{row}?after=&limit=` | `RowPage { items: Vec<PosterCard>, next: Option<String> }` | `no-cache` + ETag `"<catalog_rev>"` |
| GET | `/api/items/{id}` | `ItemDetail` (tagged `kind`: `movie` -> `MovieDetail`) | `no-cache` + ETag `"<catalog_rev>-<profile hash>"`, `Vary: Nuvora-Device` |
| GET | `/api/images/{id}/{size}` | `image/webp` bytes; `size` in `small \| large` | `public, max-age=31536000, immutable` |
| GET | `/api/files/{id}/stream` | file bytes; 200 / 206 / 416 with `Range`, `If-Range`, `HEAD` | `Last-Modified` |
| GET | `/api/scan` | `ScanStatus` (tagged `state`: `idle` / `running`) | none |
| POST | `/api/scan` | 202 `ScanStatus`; idempotent, coalescing | none |
| GET | `/api/openapi.json` | the spec | none |

Request header on every client call: `Nuvora-Device: <JSON DeviceProfile>`. Only
`/api/items/{id}` reads it. Absent -> baseline profile (H.264 8-bit 1080p + AAC
stereo in MP4). Malformed -> 400.

JSON, JS, CSS and HTML are gzip-compressed at the fastest level. `video/*` and `image/*`
never are, so Range responses stay byte-exact.

## Response types (abridged; full definitions in `api.rs`)

```rust
HomeResponse { rows: Vec<HomeRow> }
HomeRow      { kind: RowKind /* recently_added | all_movies */, title: String, total: u32,
               items: Vec<PosterCard> /* <= 40 */, next: Option<String> /* opaque cursor */ }
RowPage      { items: Vec<PosterCard>, next: Option<String> }
PosterCard   { id: i64, kind: ItemKind /* movie */, title: String, year: Option<u16>, poster: Option<Image> }
Image        { id: String /* 32 hex */, aspect_ratio: f32, placeholder: String /* ThumbHash b64 */ }

ItemDetail   = #[serde(tag = "kind")] Movie(MovieDetail)
MovieDetail  { id, title, year, runtime_ms, overview, poster, backdrop, versions: Vec<Version> }
Version      { file_id: i64, file_name, size_bytes, summary: String, media: Option<MediaDetails>, playback: Playback }
MediaDetails { container, duration_ms, bitrate, video: Vec<VideoTrack>, audio: Vec<AudioTrack>, subtitles: Vec<SubtitleTrack> }
Playback     = #[serde(tag = "kind")]
               DirectPlay { url, mime_type } | NeedsTranscode { reasons: Vec<String> } | Unavailable { reason }
               // Step 3 adds Hls { url, .. }; clients treat unknown kinds as NeedsTranscode.

ScanStatus   = #[serde(tag = "state")]
               Idle { last: Option<ScanSummary> }
             | Running { phase: discovering|reading|artwork|cleaning_up, done, total, queued, last }

DeviceProfile { containers: Vec<String>, video: Vec<VideoSupport>, audio: Vec<AudioSupport> }
VideoSupport  { codec, max_width, max_height, max_bit_depth, hdr: Vec<String> }
AudioSupport  { codec, max_channels }
```

## Example: home, 2,000 movies

```json
{ "rows": [
  { "kind": "recently_added", "title": "Recently Added", "total": 2000, "items": [ /* 40 cards */ ], "next": "b64cursor" },
  { "kind": "all_movies", "title": "All Movies", "total": 2000, "next": "b64cursor", "items": [
    { "id": 812, "kind": "movie", "title": "The Matrix", "year": 1999,
      "poster": { "id": "9f3c...e1", "aspect_ratio": 0.6667, "placeholder": "1QcSHQRnh493V4dIh4eXh1h4kJUI" } }
  ] }
] }
```

About 130 bytes per card, so home is two rows of 40 cards, about 10 KB raw for any library
size. Server time: two indexed keyset queries (< 5 ms) and serialization (< 1 ms). A
repeat visit with a matching `catalog_rev` is a 304 with no query.