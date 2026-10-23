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

## Shipped in the Vega TV app

- **Shaka Player** 4.8.5, Apache-2.0, https://github.com/shaka-project/shaka-player, with Amazon's patches and polyfills for Vega from https://github.com/AmazonAppDev/vega-video-sample (the sample is MIT-0; the polyfills are under Amazon's terms for Vega apps). They are downloaded and built when the app's dependencies install, not kept in this repository.
- **React Native for Vega**, the W3C Media API and Async Storage for Vega, © Amazon, under the Vega SDK license. **React** and **React Native**, MIT.
- **base-64**, **xmldom** and **thumbhash**, MIT; **fastestsmallesttextencoderdecoder**, CC0-1.0.

## Used by the server

- **FFmpeg** (ffprobe and ffmpeg), run as separate programs, never linked. The Docker image installs Debian's `ffmpeg` package, built under GPL-2.0-or-later; its source is available from Debian (https://packages.debian.org/source/trixie/ffmpeg).
- Rust crates including axum, tokio, rusqlite (with SQLite, public domain), reqwest, serde, image, utoipa and rust-embed, under MIT and/or Apache-2.0.

## Data

Descriptions, artwork and cast come from Wikipedia and Wikidata (CC BY-SA and CC0; images from Wikimedia Commons under their own licenses), AniList, TVmaze (CC BY-SA) and, when you add a key, TMDB. This product uses the TMDB API but is not endorsed or certified by TMDB.
