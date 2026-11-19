# Contributing

Read [ARCHITECTURE.md](ARCHITECTURE.md) first: it says which part of the code owns what. [docs/ROADMAP.md](docs/ROADMAP.md) says where Nuvora is going.

## Run it

You need [Rust](https://rustup.rs) (the version in `rust-toolchain.toml` installs itself), [Bun](https://bun.sh) 1.3 and FFmpeg 6 or newer.

```sh
bun install
bun run build                                   # the web app, which the server embeds
cargo run -p nuvora-server -- --data-dir ./data # http://localhost:8640
bun run dev                                     # web app with hot reload, proxying /api to :8640
```

To point the web app's dev server at another server, set `NUVORA_SERVER=http://host:8640` for `bun run dev`. To work on screens without a server, `VITE_FIXTURES=1 bun run dev` serves a made-up library.

**The TV app** (`apps/firetv`) is Kotlin with Jetpack Compose for TV. With JDK 17+ and the Android SDK, `./gradlew installDebug` there builds it and installs it on a connected TV or emulator.

**The Vega OS app** (`apps/vega`) is React Native for Vega. With the [Vega SDK](https://developer.amazon.com/docs/vega/latest/install-vega-sdk.html), run `npm install` there, which also builds Shaka Player with Amazon's patches. Then `npm run build:release` makes `build/<arch>-release/nuvora-vega_<arch>.vpkg`:

- `armv7` is for Fire TV devices.
- `aarch64` or `x86_64` (your Mac's) is for the Vega Virtual Device, where `vega run-app build/aarch64-release/nuvora-vega_aarch64.vpkg app.nuvora.vega.main -d VirtualDevice` runs it. The virtual device claims to decode HEVC but can't; the app notices and asks the server for H.264 instead.

**The Mac and Windows apps** (`apps/desktop`) are Tauri. `bun run --cwd apps/desktop dev` opens one against your local server.

**To try your checkout as the Docker image**, run `scripts/install.sh --build`. To run your checkout on a server you reach over SSH, run `scripts/deploy-dev.sh user@host`: it builds the web app locally and the server program on the server, only when Rust code changed, so a UI change is live in seconds; `--rollback` goes back to the released image. To keep a build running on a Mac without Docker, `scripts/macos-service.sh install --bin target/release/nuvora --data-dir ~/Nuvora` makes it a background service; `status` and `uninstall` do what they say.

## Test it

| What | Command |
|---|---|
| Server | `cargo test --workspace` (needs FFmpeg; create `apps/web/dist` first if you haven't built the web app) |
| Server style | `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` |
| Web, core, site | `bun test` and `bun run typecheck` |
| Fire TV | `./gradlew testDebugUnitTest` in `apps/firetv`, and `./gradlew connectedDebugAndroidTest` with an emulator running |

CI runs all of these on every push and pull request. A change that alters the API must regenerate the TypeScript client: run `bun run gen:api` and commit `packages/core`. A test fails while the committed spec is out of date.

## How the code is written

- **Put code where it belongs.** File knowledge goes in `media`, anything saved goes in `library`, converting goes in `transcode`, BitTorrent goes in `torrent`, and HTTP goes in `server/src/api`. A new area that needs more than a module gets its own crate.
- **Comments say why, not what.** Each module starts with a few lines on what it owns and how it works. Leave out comments that repeat the code.
- **Parse at the edges.** Check outside data (requests, config, ffprobe output, indexer feeds) where it comes in, and trust your own types after that.
- **Test what a user sees.** Call the code the way its users do and check the exact result. Media tests make their own clips with FFmpeg rather than shipping files.
- **Write plainly.** Use short, plain sentences in the UI, the docs and commit messages. `DESIGN.md` covers the look.

## Commits and pull requests

- **Subject:** one line saying what changed for the person using Nuvora, in the present tense. For example, "Docker installs leave the encoder to Settings again" or "Every icon is Phosphor, in every app".
- **Body:** why, what you measured, and anything surprising, wrapped at about 72 characters.
- **Scope:** one change per commit. Moving code and changing it go in separate commits.

## Releases

1. Raise the version everywhere it's written: the workspace `Cargo.toml`, `apps/desktop/src-tauri/Cargo.toml`, `apps/firetv/app/build.gradle.kts` (name and code), `apps/vega/manifest.toml`, `apps/vega/package.json`, `scripts/install.sh`, and the version in `README.md`.
2. Commit as "Release X.Y.Z".
3. Push the tag `vX.Y.Z`. The Release workflow publishes the Docker image (`latest` and `X.Y.Z`), the TV and desktop apps, and the server binaries that running servers update themselves from.

Every push to `main` also publishes the `edge` image.

## Design decisions

When you make a choice someone will later ask about, add a short note to [docs/decisions](docs/decisions), named `YYYY-MM-DD-topic.md`. Say what was decided, what else was considered, and why.
