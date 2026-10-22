#!/usr/bin/env bash
# Install or update Nuvora with Docker, building from this repo checkout.
# Usage: scripts/install.sh [--media PATH] [--port PORT] [--dir DIR] [--yes]
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$HOME/nuvora"
MEDIA=""
PORT=""
ASSUME_YES=0

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Usage: scripts/install.sh [--media PATH] [--port PORT] [--dir DIR] [--yes]

  --media PATH  folder with your movies (mounted read-only)
  --port PORT   port to serve on (default 8640)
  --dir DIR     install directory (default ~/nuvora)
  --yes         never prompt; fail if a value is missing

Re-running updates in place and reuses the values saved in DIR/.env.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --media) [ $# -ge 2 ] || die "--media needs a path"; MEDIA="$2"; shift 2 ;;
    --port)  [ $# -ge 2 ] || die "--port needs a number"; PORT="$2"; shift 2 ;;
    --dir)   [ $# -ge 2 ] || die "--dir needs a path"; DIR="$2"; shift 2 ;;
    --yes|-y) ASSUME_YES=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; die "unknown option: $1" ;;
  esac
done

command -v docker >/dev/null 2>&1 || die "Docker is not installed. Get it from https://docs.docker.com/get-docker/"
docker info >/dev/null 2>&1 || die "Docker is installed but its daemon is not running. Start Docker and try again."
docker compose version >/dev/null 2>&1 || die "The Docker Compose plugin is missing. See https://docs.docker.com/compose/install/"
[ -f "$SRC/Dockerfile" ] || die "run this from a Nuvora checkout: $SRC has no Dockerfile"

mkdir -p "$DIR"
DIR="$(cd "$DIR" && pwd)"
ENV_FILE="$DIR/.env"

saved() {
  [ -f "$ENV_FILE" ] || return 0
  sed -n "s/^$1=//p" "$ENV_FILE" | tail -n 1
}

ask() {
  local prompt="$1" default="$2" reply
  [ "$ASSUME_YES" -eq 0 ] || { printf '%s' "$default"; return; }
  if [ -n "$default" ]; then
    read -r -p "$prompt [$default]: " reply
    printf '%s' "${reply:-$default}"
  else
    read -r -p "$prompt: " reply
    printf '%s' "$reply"
  fi
}

[ -n "$MEDIA" ] || MEDIA="$(saved NUVORA_MEDIA)"
[ -n "$MEDIA" ] || MEDIA="$(ask "Movies folder" "")"
[ -n "$MEDIA" ] || die "no movies folder given; pass --media PATH"
case "$MEDIA" in "~"|"~/"*) MEDIA="$HOME${MEDIA#"~"}" ;; esac
[ -d "$MEDIA" ] || die "movies folder does not exist: $MEDIA"
MEDIA="$(cd "$MEDIA" && pwd)"

[ -n "$PORT" ] || PORT="$(saved NUVORA_PORT)"
[ -n "$PORT" ] || PORT="$(ask "Port" "8640")"
case "$PORT" in
  ''|*[!0-9]*) die "port must be a number: $PORT" ;;
esac
{ [ "$PORT" -ge 1 ] && [ "$PORT" -le 65535 ]; } || die "port out of range: $PORT"

TZ_NAME="$(saved TZ)"
if [ -z "$TZ_NAME" ]; then
  if [ -r /etc/timezone ]; then
    TZ_NAME="$(cat /etc/timezone)"
  elif [ -L /etc/localtime ]; then
    TZ_NAME="$(readlink /etc/localtime | sed 's|.*/zoneinfo/||')"
  fi
fi

mkdir -p "$DIR/data"

cat > "$ENV_FILE" <<EOF
NUVORA_SRC=$SRC
NUVORA_MEDIA=$MEDIA
NUVORA_PORT=$PORT
NUVORA_UID=$(id -u)
NUVORA_GID=$(id -g)
TZ=${TZ_NAME:-UTC}
EOF

cat > "$DIR/compose.yaml" <<'EOF'
services:
  nuvora:
    build: ${NUVORA_SRC:?}
    image: nuvora:local
    user: "${NUVORA_UID:-10001}:${NUVORA_GID:-10001}"
    ports:
      - "${NUVORA_PORT:-8640}:8640"
    volumes:
      - ./data:/data
      - ${NUVORA_MEDIA:?}:/media:ro
    environment:
      TZ: ${TZ:-UTC}
    command: ["--bind", "0.0.0.0:8640", "--data-dir", "/data", "--movies", "/media"]
    restart: unless-stopped
EOF

echo "Building and starting Nuvora in $DIR"
docker compose --project-directory "$DIR" up -d --build

lan_ip() {
  local ip=""
  ip="$(hostname -I 2>/dev/null | awk '{print $1}')" || true
  if [ -z "$ip" ] && command -v ipconfig >/dev/null 2>&1; then
    ip="$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || true)"
  fi
  printf '%s' "${ip:-localhost}"
}

answers() {
  if command -v curl >/dev/null 2>&1; then
    curl -s -o /dev/null --max-time 2 "http://127.0.0.1:$PORT/"
  else
    (exec 3<>"/dev/tcp/127.0.0.1/$PORT") 2>/dev/null
  fi
}

echo "Waiting for port $PORT"
for _ in $(seq 1 60); do
  if answers; then
    printf 'Nuvora is running at http://%s:%s\n' "$(lan_ip)" "$PORT"
    exit 0
  fi
  sleep 1
done

docker compose --project-directory "$DIR" logs --tail 20 nuvora >&2 || true
die "nothing answered on port $PORT after 60s; logs above, or run: docker compose --project-directory $DIR logs nuvora"
