# Third-party notices

Nuvora includes or uses the following software. Each is © its authors and used under its license. The full list of Rust and JavaScript dependencies is in `Cargo.lock` and `bun.lock`; their licenses ship with each package.

## Shipped in the web app

- **JASSUB** (styled ASS/SSA subtitles in the browser) and the libraries it is built from: libass (ISC), FreeType (FTL or GPL-2.0-or-later), FriBidi (LGPL-2.1-or-later), HarfBuzz (MIT) and others. License: `LGPL-2.1-or-later AND (FTL OR GPL-2.0-or-later) AND MIT AND MIT-Modern-Variant AND ISC AND NTP AND Zlib AND BSL-1.0`. JASSUB is loaded as a separate module, unmodified; its source is at https://github.com/ThaUnknown/jassub.
- **hls.js**, Apache-2.0. https://github.com/video-dev/hls.js
- **Mediabunny** (reads Matroska and writes MP4 in the browser, for files the browser can't open itself), MPL-2.0. Loaded as a separate module, unmodified; its source is at https://github.com/Vanilagy/mediabunny.
- **React** and **React DOM**, MIT. https://react.dev
- **TanStack Router** and **TanStack Query**, MIT. https://tanstack.com
- **Inter** and **Archivo** typefaces, SIL Open Font License 1.1, via Fontsource.

## Shipped in the TV app

- **AndroidX** (Jetpack Compose, Compose for TV, Media3 ExoPlayer, Navigation, Lifecycle, Activity, Core), Apache-2.0. https://developer.android.com/jetpack/androidx
- **Kotlin** standard library, kotlinx.coroutines and kotlinx.serialization, Apache-2.0. https://kotlinlang.org
- **OkHttp** and **Okio**, Apache-2.0. https://square.github.io/okhttp
- **Coil**, Apache-2.0. https://coil-kt.github.io/coil
- The ThumbHash decoder is a port of Evan Wallace's ThumbHash, MIT. https://github.com/evanw/thumbhash

## Used by the server

- **FFmpeg** (ffprobe and ffmpeg), run as separate programs, never linked. The Docker image installs Debian's `ffmpeg` package, built under GPL-2.0-or-later; its source is available from Debian (https://packages.debian.org/source/trixie/ffmpeg).
- Rust crates including axum, tokio, rusqlite (with SQLite, public domain), reqwest, serde, image, utoipa and rust-embed, under MIT and/or Apache-2.0.

## Data

Descriptions, artwork and cast come from Wikipedia and Wikidata (CC BY-SA and CC0; images from Wikimedia Commons under their own licenses), AniList, TVmaze (CC BY-SA) and, when you add a key, TMDB. This product uses the TMDB API but is not endorsed or certified by TMDB.
