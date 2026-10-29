# Architecture

One program, `nuvora`, is the whole server. Every app talks to it over the same HTTP API, and the web app is built into it.

```text
 apps/web ──────────┐
 apps/desktop ──────┤   /api (HTTP, OpenAPI)
 apps/firetv ───────┼──────────────────────────►  nuvora  (crates/server)
 apps/vega ─────────┘   /api/live (WebSocket)        │
                                                     ├── nuvora-torrent    indexers, downloads, torrent streams
                                                     ├── nuvora-transcode  live HLS, streaming copies, Transcoding settings
                                                     ├── nuvora-library    the catalog: SQLite, scans, metadata, accounts, progress
                                                     └── nuvora-media      files: names, ffprobe, playback rules, FFmpeg arguments
```

Dependencies point one way, down the list: `server` uses all four, `torrent` and `transcode` use `library` and `media`, `library` uses `media`, and `media` uses nothing of ours. Nothing points back up.

## The server crates

| Crate | Owns | Main modules |
|---|---|---|
| `nuvora-media` (`crates/media`) | Knowledge about files, no state. What a file name means, what ffprobe says is inside, whether a device can play it as it is, and the FFmpeg arguments to stream or convert it. | `name` (movie and episode names), `release` (torrent release names), `probe`, `playback` (device profiles and the play decision), `hls` (segments and runs), `encoder` (x264, VideoToolbox, NVENC, VA-API), `matroska` and `mp4` (keyframes from the file's own index), `artwork`, `previews`, `subtitles` |
| `nuvora-library` (`crates/library`) | Everything saved, in one SQLite database. The only crate that writes it. | `db` (migrations in `crates/library/migrations`), `scan` (walk, plan, write), `metadata` (Wikipedia, Wikidata, AniList, TVmaze, TMDB), `views` and `browse` (read models, one per screen), `accounts`, `watch_state`, `shows`, `search`, `store` (artwork files) |
| `nuvora-transcode` (`crates/transcode`) | Converting video while it plays. | `transcode` (one FFmpeg run per player, paused when far ahead, restarted on a seek), `copies` (1080p copies of heavy files, made in the background), `tuning` (the Transcoding settings) |
| `nuvora-torrent` (`crates/torrent`) | Getting video over BitTorrent. | `downloads` (Torznab search, the torrent client, filing finished downloads into a library), `streams` (play while it downloads, fetching the watched part first) |
| `nuvora-server` (`crates/server`) | The program: config, the HTTP API and its spec, sign-in, Watch Together, updates, network discovery, and the embedded web app. | `api` (handlers and wire types, one submodule per area), `auth`, `config`, `party`, `update`, `discovery`, `web` |

The HTTP handlers live only in `crates/server/src/api`. The other crates don't know about routes or accounts. When a new area needs more than a module, it gets its own crate, not more of `server`.

## The apps

| App | What it is | Shares |
|---|---|---|
| `apps/web` | The web app: React 19, Vite, TanStack Router and Query. `bun run build` writes `apps/web/dist`, which is embedded into the `nuvora` binary. | `packages/core`, `packages/tokens` |
| `apps/desktop` | The Mac and Windows apps: a Tauri shell that shows the server's own web app and adds finding servers and updating itself. | The web app, as the server serves it |
| `apps/firetv` | The Fire TV, Android TV and Google TV app: Kotlin and Jetpack Compose for TV. Its API models are written by hand. | The API |
| `apps/vega` | The app for Fire TVs with Vega OS: React Native for Vega, with Shaka Player. | `packages/core` |
| `apps/site` | getnuvora.online: Next.js on Cloudflare. It also serves the installers. | Nothing at build time; it copies the title-card palettes |

`packages/core` is the TypeScript data layer: types generated from the server's OpenAPI document, the client, queries, and logic every app needs the same way (subtitle choice, Watch Together, progress, formats). `packages/tokens` holds the design tokens; see [DESIGN.md](DESIGN.md).

## How something plays

1. Every app sends a description of what it can decode (the `Nuvora-Device` header).
2. For each file, the server decides, in `nuvora_media::playback`: play it directly, stream it as HLS (video copied or converted, audio copied or converted), or it can't be played.
3. **Direct:** `/api/files/{id}/stream`, the file itself, with Range requests.
4. **HLS:** `/api/files/{id}/hls/{plan}/main.m3u8?session=…`. `nuvora-transcode` runs one FFmpeg per session. A copied video is cut at its own keyframes, read from the file's index (Matroska, MP4) or a scan. A converted video gets keyframes every few seconds. A run that gets far ahead of the player is paused, and a seek far from what's made restarts it there.
5. **Heavy files** (taller than 1080p or above 40 Mbit/s) get a 1080p copy in the background, and apps play that by default.
6. **Away from home**, the web app asks for HLS even for files it could open directly: the browser keeps only a few seconds ahead of a direct file, while hls.js keeps a minute.
7. **Torrent streams** use the same player. `nuvora-torrent` fetches the pieces around the playhead first and feeds the bytes to FFmpeg over a loopback HTTP link.

## Where things are kept

Everything lives in the data folder (`./data` next to `compose.yaml` with Docker):

| Path | What |
|---|---|
| `nuvora.db` | The SQLite database: libraries, items, files, users, sessions, progress, settings |
| `artwork/`, `previews/`, `subtitles/` | Artwork, seek-preview sheets, and extracted or converted subtitles |
| `copies/` | Streaming copies of heavy files |
| `transcode/` | Live HLS sessions; cleared at startup |
| `downloads/` | The torrent client's state; files being downloaded go to a hidden `.nuvora-downloads` folder in the library |
| `bin/` | A server binary staged by an update |

## Where each setting lives

- **Process settings** (address, data folder, trusted proxy, updates, encoder) come from command-line flags or `NUVORA_*` environment variables, which Docker sets from `.env`, or from an optional `nuvora.toml`. A flag or variable wins over the file. Empty means unset.
- **Everything else** is set in the web app by an admin and saved in the database: libraries, users, Transcoding, Downloads and indexers.
- When a process setting overrides one in the web app (the encoder is the only one), the web app shows it as locked and says why.

## The API

- **One request per screen:** Home, a movie, a show and a library page each come back in one response, shaped for that screen.
- **Wire types live in `crates/server/src/api`** and are built from the library's read models. The OpenAPI document is generated from them. `bun run gen:api` writes it to `packages/core/openapi.json` and regenerates the TypeScript types. A test fails if the committed document is out of date.
- **Access is set by the group a route is in** (public, member, admin), and that same group sets the spec's `security`, so the two can't disagree.
- **State-changing requests** must send `Content-Type: application/json` and, when they carry an `Origin`, a matching host.
- **`/api/live`** is the one WebSocket, for Watch Together. Everything else is plain requests; screens that change on their own (downloads, scans, Activity) poll.

## Watch Together, updates, discovery

- **Watch Together** (`server::party`) is kept in memory. A party is one shared clock: at server time T, the position is P, playing or paused. Each app corrects its own drift against it.
- **Updates** (`server::update`) come from the latest GitHub release. The server binary is checked against `SHA256SUMS`, test-started, then swapped in. With Docker it's staged in `data/bin`, so it survives restarts.
- **Discovery** (`server::discovery`) announces `_nuvora._tcp` over mDNS, so the TV and desktop apps find the server without an address.

## Tests

- **Rust:** each crate tests itself; `cargo test --workspace`. Media tests make their clips with FFmpeg, and server tests run the real router against a temporary library (`api::test_app`).
- **TypeScript:** `bun test` and `bun run typecheck`.
- **Fire TV:** `./gradlew testDebugUnitTest`.
- **CI:** runs these on every push, with `cargo fmt --check` and Clippy with warnings as errors.

Why things are the way they are is in [docs/decisions](docs/decisions), one note per decision.
