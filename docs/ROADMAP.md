# Roadmap

Nuvora is a self-hosted, all-in-one streaming service: find any movie or show, play it right away from your library or straight from a torrent, keep what you like, and watch it anywhere. See [PRODUCT.md](../PRODUCT.md) for who it's for and the principles behind it.

Each stage ends with something that works end to end in the web app. The TV apps follow within the same stage where it matters for watching.

## Done: a clean base (October 2026)

- Library: movies, shows and anime from file names, metadata without API keys, title cards, per-person Continue Watching and Home.
- Watching: direct play, conversion while watching on the graphics chip, 1080p copies of heavy files, steady playback over the internet, styled subtitles, Watch Together.
- Getting: Torznab search, torrent downloads filed into the Movies or TV Shows library, torrent streams that play while they download.
- Hosting: installers for Linux, macOS and Windows, self-updating server and desktop apps, Fire TV, Android TV, Google TV and Vega OS apps.
- Code: torrents and transcoding in crates of their own, one icon set (Phosphor), and current docs (ARCHITECTURE, CONTRIBUTING, decisions).

## Next: one place to get anything

The goal: one search for everything, and one title page whether you have the title yet or not.

1. **Titles you don't have yet.** Search shows matching movies and shows from the metadata sources alongside your library. Their page has the same layout as a library title, with **Play now** (stream a torrent) and **Get** (download) instead of the file list.
2. **Pick a release for you.** Rank releases by quality, size, seeders and language, and offer the best one first, with the full list one tap away.
3. **Shows, season by season.** On a show's page, get one episode, a season or everything missing. Play an episode straight from a torrent.
4. **Downloads page.** Speed, ETA, peers and disk space for each download, plus pause, resume and remove, and limits on upload ratio and seeding time.
5. **Requests.** Members ask for a title, and admins approve or auto-approve. It downloads, and the member gets a notice when it's ready.
6. **Subtitles in torrent streams.** Embedded subtitles show while streaming, as they do in the library.
7. **Torrents through a VPN.** An optional SOCKS5 proxy for all torrent traffic, and a switch for router port forwarding (UPnP).

## Then: set and forget

1. **Follow a show.** New episodes download as they're released, at the quality you chose.
2. **Quality preferences.** Preferred resolution, maximum size, HDR or not, and release groups to prefer or avoid. Upgrade a title when a better release appears.
3. **Indexer setup in the app.** Add Prowlarr or Jackett with a guided form, test it, and see which indexers answer.
4. **Notifications.** Download finished, request approved, new episode, in the web app and on the TV.

## Then: feels like a product

1. **Its own look.** Replace the Plex-inherited amber with Nuvora's own accent across web, TV and site, and refresh the screenshots.
2. **Safe remote access.** HTTPS out of the box (a bundled reverse proxy with automatic certificates, or a Tailscale guide in the app), so nobody exposes plain HTTP to the internet.
3. **Phones.** A web layout made for phones, installable to the home screen.
4. **One-command releases.** A script that raises the version in every file and tags the release.
5. **Generated TV models.** Generate the Fire TV app's API models from the OpenAPI document, as the web and Vega apps do.

## Not planned

Music, photos, live TV and a Nuvora cloud account. Nuvora stays one server you own, for movies and shows.
