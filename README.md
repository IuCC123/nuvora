<img src="assets/icon.svg" width="64" height="64" alt="">

# Nuvora

A self-hosted media server for your movies, shows and anime, with a clean, fast web app in the spirit of Plex. Run it on a PC, Mac, NAS or Raspberry Pi; everyone in your household signs in with their own account.

- **Plays almost anything.** Files a browser can't play are converted while you watch, and only the part that needs it: often just the audio. You can still jump anywhere, and subtitles stay in sync.
- **Looks after itself.** Point it at a folder and it finds movies, shows and episodes, with posters, descriptions, cast and voice actors from Wikipedia, AniList and TVmaze. No API keys needed.
- **Subtitles done right.** Embedded tracks and `.srt`/`.ass`/`.vtt` files next to the video, styled anime subtitles with their own fonts, and your choice remembered per show.
- **Watch Together.** Invite someone and play anything at the same moment on both screens; pausing, seeking and the next episode stay in sync.
- **Yours.** Progress and Continue Watching per person, a Home screen each person arranges themselves, search across everything, and no account with anyone but your own server.

There's an app for **Fire TV and Android TV** too ([Watch on your TV](#watch-on-your-tv)); a Vega OS app is on the way.

## Set up your server

Nuvora runs in Docker, so it works the same everywhere. It takes about five minutes.

### 1. Install Docker

- **Windows or macOS:** install [Docker Desktop](https://www.docker.com/products/docker-desktop/) and start it.
- **Linux:** `curl -fsSL https://get.docker.com | sh`, then `sudo usermod -aG docker $USER` and log out and back in.
- **NAS (Synology, Unraid, TrueNAS, ...):** use its container app with the Compose file below.

### 2. Start Nuvora

**Linux and macOS:** run this in a terminal.

```sh
curl -fsSL https://raw.githubusercontent.com/IuCC123/nuvora/main/scripts/install.sh | bash
```

It asks for the folder that holds your movies and shows, and a port (8640 is fine), then downloads and starts Nuvora in `~/nuvora`. You can answer up front instead:

```sh
curl -fsSL https://raw.githubusercontent.com/IuCC123/nuvora/main/scripts/install.sh | bash -s -- --media ~/Media --port 8640
```

**Windows, a NAS, or by hand:** use Docker Compose. Make a folder (say `nuvora`), save [`compose.yaml`](compose.yaml) in it, and next to it create a file named `.env`:

```sh
# The folder with your movies and shows. Nuvora only reads it.
NUVORA_MEDIA=D:/Media
```

On Linux, add `NUVORA_UID=` and `NUVORA_GID=` lines with the output of `id -u` and `id -g`, so Nuvora can write its `data` folder (the installer does this for you).

Then, in that folder:

```sh
docker compose up -d
```

### 3. Create your account

Open **http://your-server:8640** in a browser (the installer prints the exact address; on the same computer it is http://localhost:8640). The first visit asks you to create the admin account, then to add a library: name it, choose **Movies** or **Shows**, and pick its folder under `/media`, which is the media folder you gave the installer.

### 4. Add the others

In **Settings › Users**, add an account for each person. They sign in from any browser on your network at the same address, or from the TV app.

## Watch on your TV

The Nuvora TV app runs on Amazon Fire TV and on Android TV and Google TV. It has everything the web app has for watching: Home, your libraries, search, subtitles (styled anime ones too), audio tracks, Continue Watching and Watch Together. It tells the server what the TV can decode, so most files play untouched, HEVC, HDR and surround sound included.

**On a Fire TV:**

1. Install **Downloader** (by AFTVnews) from the Amazon Appstore.
2. Allow it to install apps: **Settings › My Fire TV › Developer options › Install unknown apps**, then turn on Downloader. No Developer options? Go to **Settings › My Fire TV › About** and press OK on the device name seven times, then look again.
3. Open Downloader and enter your server's address followed by **/tv**, for example `http://192.168.1.20:8640/tv`. It downloads the newest TV app; choose **Install**.
4. Open **Nuvora**, type the server's address (`192.168.1.20` is enough when it uses port 8640) and sign in.

**On Android TV or Google TV:** the same, with Downloader from the Play Store; allow it to install apps when the TV asks.

To update the app, repeat step 3; you stay signed in. The app is also on the [Releases](https://github.com/IuCC123/nuvora/releases/latest) page as `nuvora-tv.apk`.

## Organizing your files

Nuvora reads folder and file names, so a little order goes a long way:

```text
Media/
├── Movies/
│   └── Big Buck Bunny (2008)/
│       ├── Big Buck Bunny (2008).mkv
│       └── Big Buck Bunny (2008).en.srt
└── Shows/
    └── Tower of God/
        └── Season 02/
            └── Tower of God - S02E01.mkv
```

Movies want a year in the name. Episodes want a number: `S02E01`, `2x01` and anime-style `Show - 01 [1080p].mkv` all work. Subtitle files next to a video are picked up with their language (`.en.srt`, `.English.forced.srt`, or a `Subs/` folder).

## Updating

Run the installer again, or in the folder with `compose.yaml`:

```sh
docker compose pull && docker compose up -d
```

Your accounts, progress and settings carry over. To stay on one version, set `NUVORA_IMAGE=ghcr.io/iucc123/nuvora:0.2.0` in `.env` (or pass `--version 0.2.0` to the installer).

**Backing up:** everything Nuvora knows lives in the `data` folder next to `compose.yaml` (`~/nuvora/data` for the installer). Copy it while Nuvora is stopped.

## Settings

Set these in `.env` next to `compose.yaml`, then run `docker compose up -d`:

| Setting | Default | |
|---|---|---|
| `NUVORA_MEDIA` | (required) | Your media folder, mounted read-only at `/media` |
| `NUVORA_PORT` | `8640` | Port on this machine |
| `NUVORA_LISTEN` | `0.0.0.0` | `127.0.0.1` to accept connections from this machine only (behind a reverse proxy) |
| `NUVORA_IMAGE` | `ghcr.io/iucc123/nuvora:latest` | Image and version to run |
| `NUVORA_UID`, `NUVORA_GID` | `10001` | User and group Nuvora runs as; must be able to read your media and write `data` (the installer sets yours) |
| `NUVORA_TRUSTED_PROXY` | | Your reverse proxy's address or network, see below |
| `TZ` | `UTC` | Time zone, e.g. `Europe/Amsterdam` |

The image is published for x86-64 and ARM64 (Raspberry Pi 4/5, Apple Silicon, most recent NAS boxes).

## Remote access

Everything needs a sign-in, so friends can reach your server from outside your home. Use a private network such as [Tailscale](https://tailscale.com) (easiest: install it on the server and on their device), or a reverse proxy with HTTPS (Caddy, nginx, Traefik). Do not forward port 8640 straight to the internet: without HTTPS, passwords and session cookies cross the network in plain text.

When a reverse proxy on the same machine is the only way in, publish Nuvora on loopback so nothing else can reach it directly: set `NUVORA_LISTEN=127.0.0.1` (or pass `--listen 127.0.0.1` to the installer) and run `docker compose up -d` again.

The proxy must pass the original host and scheme, the client's address, and WebSocket upgrades (Watch Together uses one):

```nginx
location / {
    proxy_pass http://127.0.0.1:8640;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
```

Then tell Nuvora to trust the proxy with `NUVORA_TRUSTED_PROXY` (an address or CIDR block, comma-separated). Sign-in limits then apply to each client address instead of to the proxy, and requests are checked against the host in `X-Forwarded-Host` when the proxy sends one. In Docker the proxy's requests arrive from the Compose network's gateway, so trust that network, usually `NUVORA_TRUSTED_PROXY=172.16.0.0/12`. Only do this with `NUVORA_LISTEN=127.0.0.1`: otherwise anyone who can reach the port can claim any address.

## Development

You need [Rust](https://rustup.rs) (stable), [Bun](https://bun.sh) 1.3 and FFmpeg 6 or newer.

```sh
bun install
bun run build                                   # the web app, which the server embeds
cargo run -p nuvora-server -- --data-dir ./data # http://localhost:8640
bun run dev                                     # web app with hot reload, proxying /api to :8640
```

The TV app is in `apps/firetv` (Kotlin, Jetpack Compose for TV); with JDK 17+ and the Android SDK, `./gradlew installDebug` there builds it and installs it on a connected TV or emulator.

Tests: `cargo test --workspace` and `bun test`; `bun run typecheck` for TypeScript; `./gradlew testDebugUnitTest` in `apps/firetv`. After changing the API, `bun run gen:api` regenerates the TypeScript client's types. To try the Docker image from your checkout, run `scripts/install.sh --build`.

The layout: `crates/` is the server (`media` knows files, `library` owns the database, `server` is the HTTP API), `packages/core` is the TypeScript data layer, `apps/web` is the web app, and `apps/firetv` is the TV app.

## Credits

Nuvora stands on [FFmpeg](https://ffmpeg.org), [JASSUB](https://github.com/ThaUnknown/jassub) and [hls.js](https://github.com/video-dev/hls.js), among others; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Descriptions and artwork come from [Wikipedia](https://www.wikipedia.org) and [Wikidata](https://www.wikidata.org), [AniList](https://anilist.co), [TVmaze](https://www.tvmaze.com) and, with your own key, [TMDB](https://www.themoviedb.org).

Made with [T3 Code](https://t3.codes).
