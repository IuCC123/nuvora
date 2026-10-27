# Mac and Windows apps, and updates

Requested by the owner: a Mac and a Windows app that use exactly what the browser does and look exactly like it, native where possible, and a way to update the apps and the server to the latest version. TVs are left out.

## The apps (`apps/desktop`)

The apps are Tauri 2 shells around the system's own web view: WebKit on macOS, WebView2 (Edge) on Windows. They don't bundle a browser, so the Mac app is about 10 MB.

The window doesn't show a copy of the web app. It shows the web app that the server serves, the same pages a browser gets. It always matches the server's version, and nothing is built twice. The app adds only what a browser can't do:

- **`connect.html`.** This is a second entry in the web app's Vite build (`apps/web/src/connect`), built from the sign-in screen's components and bundled into the app. It reconnects to the remembered server. If the server moved, it looks for the same server id on the network. Otherwise it lists the servers that announce `_nuvora._tcp` over mDNS and takes a typed address, using the same candidates as the TV app: a bare host tries port 8640 first. Once a server answers, the page navigates to it.
- **Navigation.** The window may show the connect page and the connected server. Any other link opens in the default browser.
- **Native details.** The window has a dark title bar in the canvas colour, follows the page title, and remembers its size and position. A second launch focuses the open window. HTML full screen works: the macOS private API flag turns on WebKit's element full screen, and Tauri handles it on Windows. The macOS menu adds Switch Server, Reload, Back and Forward.
- **Bridge.** `apps/web/src/platform/desktop.ts` calls the Rust commands and is `null` in a browser. Server pages are remote origins, so `capabilities/server.json` lets them call only `switch_server`, `app_info`, `check_app_update` and `install_app_update`. The connect page gets the rest.

## Updating the apps

The apps use Tauri's updater with GitHub's `releases/latest/download/latest.json`. The release workflow builds the update packages and writes `latest.json`. They are signed with a minisign key: the public half is in `tauri.conf.json`, and the private half is the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets, with a backup in `~/.nuvora/nuvora-desktop-updater.key` and `.password`. Never regenerate it: apps that are already installed only accept updates signed with this key.

In the web app, a card offers the update once per new version, and Settings › Account › This app shows the version and a Check again button.

## Updating the server

`crates/server/src/update.rs`, `GET` and `POST /api/admin/update` (admins only).

- **Check.** The server asks GitHub's API for the latest release when an admin looks, and caches the answer for 6 hours. Check again asks at most once a minute. `NUVORA_UPDATES=false` turns checking off, and then the server never contacts GitHub.
- **Install.** The server downloads `nuvora-server-<target triple>` from the release and checks it against the release's `SHA256SUMS`. It then runs `--version` on the download and expects the new version back, which catches a build for the wrong processor or system. Next it puts the binary in place:
  - **Native installs.** When the binary's folder is writable, the download is renamed over the running binary. On Windows the running `.exe` is renamed aside first.
  - **Docker.** `/usr/local/bin` belongs to root, so the binary goes to `<data dir>/bin/nuvora-<version>`. At startup, a staged binary newer than the running one takes over, so the update survives container restarts. Staged binaries older than the running one are deleted, so a newer image wins.
- **Restart.** The server stops taking requests, gives open connections 5 seconds, then replaces itself with the new binary and the same arguments. On Unix this is `exec`, which keeps the pid, so launchd, systemd and tini carry on. On Windows the new process starts and the old one exits.
- **Web app.** While the update runs, the page polls `/api/health` and reloads when the new version answers. Every open page also rechecks `/api/health` on focus and every 10 minutes. When the version changed, it offers Reload, because it is still running the old web app.

The release workflow builds the server for macOS (arm64 and x86-64), Linux (x86-64 and arm64, on Ubuntu 22.04 for an older glibc) and Windows. If a platform's build fails, that platform updates by hand. `NUVORA_UPDATE_FEED` (hidden) points the check somewhere else, for a fork or a stand-in when testing.
