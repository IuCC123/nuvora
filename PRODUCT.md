# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The web app comes first. A Fire TV app (Android APK) and a Vega OS app (`.vpkg`) follow in Step 5. Those are separate React Native clients that share the TypeScript data layer and design tokens, not the web views.

## Users

The owner's household, plus friends and family who stream remotely from the owner's server, the way Plex sharing works. People browse a personal movie and TV library and press play. Before the TV apps exist, the web app is mostly used in a laptop or desktop browser with mouse and keyboard.

## Product Purpose

Nuvora is a self-hosted media server and player that remakes Plex and Jellyfin from scratch. A Rust server scans the owner's media folders, and clients browse and play the library. Success means it feels as polished as Plex and noticeably faster to load. Movie and series screens appear instantly, and playback starts quickly.

## Positioning

A Plex-quality experience with speed as a feature you can see. Screens arrive in one request each, artwork has instant placeholders, and the server plays the original file whenever the device can decode it.

## Operating Context

- The owner runs the server on their own hardware, often a NAS or a home machine. Remote friends connect over the internet.
- Libraries are folders of movie files, mostly MKV and MP4, with mixed codecs (H.264, HEVC, 4K HDR).
- The roadmap has five steps: Foundation, Library, Playback, Polish, and TV apps and release. Step 1 is movies only, with no accounts and no TMDB metadata.

## Capabilities and Constraints

- Step 1: scan a movie folder, show Home (Recently Added, All Movies), a movie detail screen, and direct play in the browser. Files the browser can't play show a "needs transcoding" state until Step 3.
- Step 2 adds TV shows, TMDB metadata and artwork, user accounts, resume, Continue Watching and search.
- Until Step 2, a movie without local artwork has no poster image and shows a designed title card.
- Stack: Rust server (axum, SQLite), React web app, shared TypeScript data layer for the later React Native TV apps.

## Brand Commitments

- The owner wants the look and feel as close to Plex as possible.
- The name is Nuvora. The teal "N" icon at `assets/icon.svg` is a placeholder, not the logo.

## Evidence on Hand

- A synthetic test library at `/tmp/nuvora-media/Movies` (seven short generated clips). There is no real artwork or metadata yet. Mockups must label any invented titles or imagery as sample content.

## Product Principles

- Speed is a feature: one request per screen, artwork that paints before it loads, no spinners where a placeholder can stand in.
- Plex is the reference. Departures need a reason a Plex user would thank us for.
- Ship fewer, finished features rather than many rough ones.
- Every client shares one data layer, so the TV apps behave like the web app.
