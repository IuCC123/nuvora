<img src="assets/icon.svg" width="64" height="64" alt="">

# Nuvora

Created in [T3 Code](https://t3.codes).

## Run with Docker

You need Docker with the Compose plugin and a checkout of this repo.

```sh
scripts/install.sh --media ~/Movies
```

The script asks for anything you leave out (`--port`, `--dir`, `--yes` to skip prompts), builds the image, starts it, and prints the address on your LAN. The default install directory is `~/nuvora`. `--listen 127.0.0.1` publishes the port on this machine only (see Remote access).

To use Compose directly:

```sh
mkdir -p data
NUVORA_MEDIA=~/Movies NUVORA_UID=$(id -u) NUVORA_GID=$(id -g) docker compose up -d --build
```

The server listens on port 8640. The database and artwork live in `data/` next to `compose.yaml` (`~/nuvora/data` for the installer), and your media is mounted read-only at `/media`. Open the address in a browser to create the admin account, then add a library from `/media` in the folder picker. The admin adds everyone else.

The container drops every Linux capability and cannot gain new privileges. The data directory is readable by its owner only.

### Remote access

Everything needs a sign-in, so friends can reach the server from outside your home. Use a private network such as Tailscale, or a reverse proxy with HTTPS (Caddy, nginx, Traefik). Do not forward port 8640 straight to the internet: without HTTPS, passwords and session cookies cross the network in plain text.

When a reverse proxy on the same machine is the only way in, publish Nuvora on loopback so nothing else can reach it directly. Set `NUVORA_LISTEN=127.0.0.1` in the `.env` next to `compose.yaml`, or pass `--listen 127.0.0.1` to the installer, then run `docker compose up -d` again.

The proxy must pass the original host and scheme, and the client's address:

```nginx
location / {
    proxy_pass http://127.0.0.1:8640;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
```

Then tell Nuvora to trust the proxy with `NUVORA_TRUSTED_PROXY` (an address or CIDR block, comma-separated; `--trusted-proxy` outside Docker). Sign-in limits then apply to each client address instead of to the proxy, and requests are checked against the host in `X-Forwarded-Host` when the proxy sends one. In Docker the proxy's requests arrive from the Compose network's gateway, so trust that network, usually `NUVORA_TRUSTED_PROXY=172.16.0.0/12`. Only do this with `NUVORA_LISTEN=127.0.0.1`: otherwise anyone who can reach the port can claim any address.

To update, run `git pull` in the checkout and run the installer again, or run `docker compose up -d --build`. Your data and settings carry over.
