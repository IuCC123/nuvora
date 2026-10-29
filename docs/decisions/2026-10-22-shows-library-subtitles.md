# Shows, library browsing and subtitles

Requested by the owner after testing with a real anime release: 13 episodes named `[Ñ] Tower of God 2 - Ouji no Kikan - 01 [1080p] [080DD9EA].mkv` (HEVC, Japanese AAC, Spanish ASS subtitles embedded). In a movie library they became 13 "movies" titled "Ñ Tower of God 2 Ouji no Kikan 01". This design groups episodes into shows, adds a real library page with sorting, filtering and grouping, and adds subtitles that look right, including styled anime subtitles.

Pulled forward from Step 2 (shows) and Step 3 (subtitles). TMDB matching is still Step 2. Everything here works from file names and file contents alone.

## 1. Shows

### Library types

- A library is **Movies** or **TV Shows**, chosen when it's added (as in Plex), with an icon for each type in the add dialog.
- **Suggestion when adding.** `GET /api/admin/fs/inspect?path=` returns `{ video_files, episode_like }`, a quick count over at most 2,000 files. When more than half look like episodes, the dialog suggests TV Shows.
- **Changing the type later.** It's allowed after a confirmation ("Items in this library are rebuilt; watch progress in it is reset"). The next scan reclassifies everything.

### Parsing episode names

This lives in `crates/media` `name.rs`. Before matching, bracketed tags are stripped:

- every `[...]` segment: release groups such as `[Ñ]`, `[1080p]` and CRC tags like `[080DD9EA]`
- `(...)` segments, unless they hold a four-digit year
- the same release tokens movies already strip: resolution, codec, source, bitrate

Episode patterns, in priority order:

1. `S01E02`, `s1e2`, including multi-episode `S01E02E03` and `S01E02-E03`. A multi-episode file maps to its first episode for now.
2. `1x02`.
3. `Season 1/…Episode 2` or `…E02` inside a season folder.
4. The anime absolute number: the last ` - 02` (or `- 02v2`) before the tags or the end of the name, with 1 to 4 digits. Season comes from a `Season N`/`S1` folder, otherwise 1.
5. `Ep02` or `Episode 02`.

Show title:

- The text before the episode token, cleaned up. A show folder (the first folder under the library root, ignoring `Season N` folders) wins when present, so episodes named slightly differently still group together.
- `Tower of God 2 - Ouji no Kikan` keeps its hyphen: only the final ` - NN` is the episode.

Other fields:

- **Episode title.** The text after the episode token (`S01E02 - The Title`), when there is any.
- **Specials.** `S00`, or a `Specials` folder, become season 0.

**Identity.** A show is `casefold(show title)|year?` within a library. A season is `(show, season number)`. An episode is `(season, episode number)`. These use the existing `item` rows with `kind`, `parent_id` and `index_number`.

**Parser version.** Add a `parse_version` to `media_file`. When `name.rs` changes, the version bumps, and the next scan re-derives each file's filename metadata layer and classification. The identity of an unchanged movie never changes, but its displayed title does. This also lets a library type change rebuild items.

### Watch state and Next Up

Watch state stays per item, which for shows means per episode. On top of it:

- **Show progress.** The show's next episode is the first unwatched one after the last watched one in order. With nothing watched, it's S1 E1.
- **Continue Watching** lists in-progress items (movies and episodes), plus a **Next Up** entry for any show whose last watched episode has a successor. The entry shows that successor at 0% with the label "Next episode". Everything is ordered by last activity.
- **Bulk marks.** Mark watched or unwatched works on a show (all its episodes) and on a season.

### API (wire types are additive)

- **`ItemDetail`** gains `show` and `episode` variants.
  - `ShowDetail { id, title, year_range, poster, backdrop, seasons: SeasonSummary[], next: EpisodeRef | null, watched_count, episode_count }`
  - `SeasonSummary { id, index, title, episode_count, watched_count }`
- **`GET /api/items/{season_id}/episodes`** returns `Episode[]`.
  - `Episode { id, index, title, runtime_ms, thumb: Image | null, watch, versions }`
  - `versions` carries the playback decision, as `MovieDetail` already does.
- **Episode thumbnails** are a 16:9 frame grab at 10% of the episode, for SDR video only (the same rule as movie backdrops).
- **`PosterCard`** gains `kind: "show"` and an optional `subtitle` (for example "S1 · E4" or "Next episode"). Continue Watching cards for episodes carry the show's poster and title, the episode subtitle, and the episode id to play.
- **Mark watched** on a show or season id applies to all of its episodes.

## 2. Library page (sorting, filtering, grouping)

- **The route.** `/library/:id`. Each library gets a nav rail entry with its type icon, and the home row titles link to it.
- **Endpoint.** `GET /api/libraries/{id}/items?sort=&order=&filter=&res=&hdr=&after=&limit=` with keyset paging. It returns cards: movies, or shows for a TV library.
  - Sorts: `title`, `year`, `added`, `watched` (last watched), `runtime`.
  - Filters: `all`, `unwatched`, `in_progress`, `watched`. Resolution: `4k`, `1080p`, `720p`, `sd`. `hdr=true`.
- **Group index.** `GET /api/libraries/{id}/index?sort=&filter=...` returns the group headers with their counts and cursors:
  - letters for a title sort (`#`, then A to Z)
  - decades for a year sort
  - months for an added sort
- **Toolbar.**
  - A Sort menu with an ascending/descending toggle.
  - Filter chips (All, Unwatched, In progress, Watched).
  - A resolution and HDR menu (hidden for TV libraries).
  - A Group toggle, on by default for the title, year and added sorts.
  - The current choice is kept per library in the URL and in `localStorage`.
- **Grid.**
  - It's virtualized for thousands of items, with responsive columns and the same `PosterCard`.
  - Group headers stick to the top while their group scrolls.
  - With a title sort, an A to Z jump bar runs along the right edge. Clicking a letter scrolls to its group; letters with no items are dimmed.
  - It works with the keyboard.
- **Home** becomes:
  - Continue Watching
  - Recently Added: movies, plus shows with new episodes labelled "3 new episodes", newest first
  - one row per library, by title, with "See all" opening the library page

## 3. Subtitles

### Sources

- **Embedded tracks.** Already probed and stored in `media_stream`.
- **External sidecar files** next to the video (`Name.srt`, `Name.en.srt`, `Name.eng.forced.srt`, `.ass`, `.ssa`, `.vtt`). They need scanner changes, so they land in a follow-up after the shows work merges.

Text formats work: SubRip, ASS/SSA, WebVTT, mov_text. Image formats (PGS, VobSub, DVB) are listed but marked "needs conversion", because burning them in comes with transcoding in Step 3.

### Server

- **The subtitle track.** `GET /api/files/{file_id}/subtitles/{stream_index}?format=vtt|ass` extracts the stream with ffmpeg (`-map 0:{idx} -c:s webvtt|ass`).
  - The result is cached under `<data>/subtitles/<file fingerprint hex>/<idx>.<ext>`.
  - It is sent with `Cache-Control: private, max-age=31536000, immutable`, which is safe because the path is tied to the file's fingerprint.
  - At most 2 extractions run at once, each with a 120 s timeout.
  - The first request extracts. Later requests are file reads.
- **Fonts.** `GET /api/files/{file_id}/fonts` lists the MKV font attachments, read by ffprobe at request time with no scanner change. `GET /api/files/{file_id}/fonts/{index}` serves one, extracted with `-dump_attachment` and cached alongside. Anime releases embed their fonts, and styled subtitles need them.
- **Track list.** `SubtitleTrack` in the detail response gains:
  - `stream_index`
  - `format` (`srt`, `ass`, `vtt`, `image`)
  - `renderable`
  - `default`, `forced` and `hearing_impaired` flags
  - `language` (ISO 639-2 as probed) and `title`
- **User preferences.** `user.subtitle_mode` (`auto`, `always` or `off`, default `auto`) and `user.subtitle_language` (ISO 639-2, null means "use the device language"). They go in migration `0006`, through an additive `PATCH /api/auth/me`.

Migration numbers are fixed in advance: `0005` belongs to shows (parse version and any indexes), `0006` to subtitle preferences.

### Choosing a track

A pure function in `@nuvora/core`, shared with the TV apps:

1. Mode `off` gives no subtitles.
2. Mode `always` picks the preferred-language track. Failing that it takes the default-flagged track, then none.
3. Mode `auto` (the default):
   - If the audio language differs from the preferred language and a preferred-language track exists, it's used. A non-forced track beats a forced one.
   - Otherwise, a forced track in the preferred language is used.
   - Otherwise, a default-flagged track is used. That's the owner's anime case: Japanese audio and a Spanish track flagged default.
   - Otherwise, none.
4. A choice the viewer makes in the player is remembered per show (or per movie) for that user in `localStorage` (web) and wins next time.
5. The web maps the browser language (`navigator.language`, ISO 639-1) to ISO 639-2 through a table in core.

### The player

- **Subtitles button.** A CC button joins the bottom bar. Its menu lists Off and every track, each labelled with its language name, plus the title and "Forced", "SDH" and "Styled" markers where they apply. Image tracks are disabled, with "needs conversion".
- **Style settings** in the same menu: size (Small, Medium, Large), and background (None, Shadow, Box). They're stored on the device.
- **Keyboard.** C toggles subtitles, and Shift+C cycles through the tracks.
- **Plain text tracks** (SubRip, WebVTT, mov_text) are fetched as VTT and drawn by Nuvora's own overlay, not native `::cue`, so they look the same in every browser:
  - Inter 600, white with a soft outline and shadow
  - centered, at most 2 lines, at 60ch
  - they rise smoothly above the control bar while the controls show
- **Styled tracks** (ASS/SSA) are drawn by libass in WebAssembly ([JASSUB](https://github.com/ThaUnknown/jassub), MIT) on a canvas over the video:
  - The positioning, signs and karaoke stay intact.
  - Fonts come from the file's attachments, then a bundled fallback font.
  - The renderer is loaded only when a styled track is chosen, so it doesn't weigh on any other page.
  - If it fails to load, the track falls back to its VTT conversion.
- **While switching tracks or loading the first extraction,** a small "Loading subtitles…" note appears in the menu, never over the video.
