<img src="assets/icon.svg" width="64" height="64" alt="">

# Nuvora

Created in [T3 Code](https://t3.codes).

## Run with Docker

You need Docker with the Compose plugin and a checkout of this repo.

```sh
scripts/install.sh --media ~/Movies
```

The script asks for anything you leave out (`--port`, `--dir`, `--yes` to skip prompts), builds the image, starts it, and prints the address on your LAN. The default install directory is `~/nuvora`.

To use Compose directly:

```sh
mkdir -p data
NUVORA_MEDIA=~/Movies NUVORA_UID=$(id -u) NUVORA_GID=$(id -g) docker compose up -d --build
```

The server listens on port 8640. The database and artwork live in `data/` next to `compose.yaml` (`~/nuvora/data` for the installer), and your media is mounted read-only at `/media`. Open the address in a browser to create the admin account, then add a library from `/media` in the folder picker. The admin adds everyone else.

Everything needs a sign-in, so friends can reach the server from outside your home. Use a private network such as Tailscale, or a reverse proxy with HTTPS (Caddy, nginx, Traefik) that sets `X-Forwarded-Proto`. Do not forward port 8640 straight to the internet: without HTTPS, passwords and session cookies cross the network in plain text.

To update, run `git pull` in the checkout and run the installer again, or run `docker compose up -d --build`. Your data and settings carry over.
