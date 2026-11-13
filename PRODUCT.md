# Product

<!-- impeccable:product-schema 1 -->

## Platform

web, plus TV and desktop apps

The web app is the main app and the reference for the others. The server serves it, so a browser always gets the version that matches. The Mac and Windows apps are shells around that same web app. There are two TV apps: Kotlin for Fire TV, Android TV and Google TV, and React Native for Fire TVs with Vega OS. The Vega app shares the web app's TypeScript data layer.

## Users

The person who runs the server (the admin), their household, and friends and family who stream from that server over the internet. They browse, search and press play, on a laptop, on a TV with a remote, or in the Mac or Windows app. The admin also gets new movies and shows into the library, and sets up the server once and rarely after.

## Product Purpose

Nuvora is a self-hosted, all-in-one streaming service. Find any movie or show, play it right away from your library or straight from a torrent, keep what you like, and watch it anywhere, including together. It replaces a stack of separate tools (a media server, a torrent client, an indexer front end, a request app) with one app that is quick to set up and pleasant to use.

Success means:
- Someone who has never self-hosted is watching within five minutes of running the installer.
- Playback starts in seconds and doesn't stop, at home or away.
- Getting a new title takes one search and one button.

## Positioning

The polish of a commercial streaming service, the freedom of free software, and the reach of a torrent client, in one app you own. Compared with Plex and Jellyfin, Nuvora gets content as well as playing it. Compared with the *arr stack, it's one install and one interface, built for watching rather than for managing.

## Operating Context

- The server runs on the owner's hardware or a rented server: a NAS, a home PC or Mac, a Raspberry Pi, or a VPS with network storage. It is usually installed with Docker.
- Friends connect over the internet, often on slow or patchy connections. Playback has to handle that without settings.
- Libraries are folders of MKV, MP4 and AVI files with mixed codecs (H.264, HEVC, 4K HDR, old XviD) and anime-style names.
- New titles come from the admin's own Torznab indexers (Jackett, Prowlarr), through the built-in torrent client.

## Capabilities and Constraints

- **Watch:** direct play when the device can play the file, otherwise conversion while watching (hardware when available) or a background 1080p copy for heavy files. Subtitles, including styled anime ones, Continue Watching per person, and Watch Together.
- **Get:** search indexers; play a torrent while it downloads; download into the library, named and filed.
- **Organize:** recognizes movies, shows and episodes from file names, with metadata and artwork from Wikipedia, Wikidata, AniList and TVmaze (TMDB with a key), and designed title cards when there's no artwork.
- **Host:** one-command installers, self-updating server and desktop apps, mDNS discovery, accounts with admin and member roles.
- **Stack:** Rust server (axum, SQLite, FFmpeg, librqbit), React web app, shared TypeScript data layer, Kotlin and React Native TV apps.
- **Legal:** Nuvora downloads only what the admin's own indexers offer, and the app reminds admins to download only what they have the right to.

## Brand Commitments

- The name is Nuvora. The wordmark and its play triangle are in `assets/brand`.
- The look is calm, dark and media-first, with a premium streaming service's polish: artwork leads and the interface stays out of the way.
- The look is Nuvora's own since November 2026: white for the primary action and focus, glass on floating chrome, color from the artwork (see DESIGN.md). The TV apps still carry the earlier amber look until they follow.
- Icons are Phosphor in every app.

## Evidence on Hand

- The screenshots in `assets/readme` use Blender's open movies.
- `apps/web/src/dev` holds a made-up library for designing screens without a server (`VITE_FIXTURES=1`).
- A real test server runs on a VPS with media on network storage, which is where the playback and download measurements in the commit history come from.

## Product Principles

- **It just works.** Defaults that suit most people beat settings. When something fails, say what happened and what to do, in plain words.
- **Speed you can see.** One request per screen, artwork that paints before it loads, playback that starts in seconds, and no spinner where a placeholder can stand in.
- **One app, one way.** Watching, getting and organizing share one interface and one search. A title page looks the same whether you have the title yet or not.
- **Finish features.** Ship fewer, finished features rather than many rough ones. Every feature works in the web app first; the TV apps follow.
- **Respect the owner.** No Nuvora account and no telemetry. The server only contacts metadata sources, the owner's indexers, torrent peers, and GitHub to check for updates, which can be turned off.
