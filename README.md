<img src="assets/icon.svg" width="64" height="64" alt="">

# Nuvora

Created in [T3 Code](https://t3.codes).

## Run with Docker

You need Docker with the Compose plugin and a checkout of this repo. Until login ships, do not expose the server outside your home network.

```sh
scripts/install.sh --media ~/Movies
```

The script asks for anything you leave out (`--port`, `--dir`, `--yes` to skip prompts), builds the image, starts it, and prints the address on your LAN. The default install directory is `~/nuvora`.

To use Compose directly:

```sh
mkdir -p data
NUVORA_MEDIA=~/Movies NUVORA_UID=$(id -u) NUVORA_GID=$(id -g) docker compose up -d --build
```

The server listens on port 8640. The database and artwork live in `data/` next to `compose.yaml` (`~/nuvora/data` for the installer), and your media is mounted read-only at `/media`.

To update, run `git pull` in the checkout and run the installer again, or run `docker compose up -d --build`. Your data and settings carry over.
