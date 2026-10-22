#!/usr/bin/env bash
# Install or update Nuvora with Docker, building from this repo checkout.
# Usage: scripts/install.sh [--media PATH] [--port PORT] [--listen ADDR] [--dir DIR] [--yes]
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$HOME/nuvora"
MEDIA=""
PORT=""
LISTEN=""
ASSUME_YES=0

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Usage: scripts/install.sh [--media PATH] [--port PORT] [--listen ADDR] [--dir DIR] [--yes]

  --media PATH   folder with your media, shown as /media in the setup folder picker (read-only)
  --port PORT    port to serve on (default 8640)
  --listen ADDR  host address to publish the port on (default 0.0.0.0, every interface;
                 127.0.0.1 when a reverse proxy on this machine is the only way in)
  --dir DIR      install directory (default ~/nuvora)
  --yes         never prompt; fail if a value is missing

Re-running updates in place and reuses the values saved in DIR/.env.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --media) [ $# -ge 2 ] || die "--media needs a path"; MEDIA="$2"; shift 2 ;;
    --port)  [ $# -ge 2 ] || die "--port needs a number"; PORT="$2"; shift 2 ;;
    --listen) [ $# -ge 2 ] || die "--listen needs an IPv4 address"; LISTEN="$2"; shift 2 ;;
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

[ -n "$LISTEN" ] || LISTEN="$(saved NUVORA_LISTEN)"
[ -n "$LISTEN" ] || LISTEN="0.0.0.0"
valid_ipv4() {
  local IFS=. part
  # shellcheck disable=SC2086
  set -- $1
  [ $# -eq 4 ] || return 1
  for part in "$@"; do
    case "$part" in ''|*[!0-9]*) return 1 ;; esac
    [ "$part" -le 255 ] || return 1
  done
}
valid_ipv4 "$LISTEN" || die "--listen must be an IPv4 address such as 127.0.0.1: $LISTEN"
HEALTH_HOST="$LISTEN"
[ "$LISTEN" != "0.0.0.0" ] || HEALTH_HOST="127.0.0.1"

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
NUVORA_LISTEN=$LISTEN
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
      - "${NUVORA_LISTEN:-0.0.0.0}:${NUVORA_PORT:-8640}:8640"
    cap_drop: [ALL]
    security_opt: [no-new-privileges:true]
    volumes:
      - ./data:/data
      - ${NUVORA_MEDIA:?}:/media:ro
    environment:
      TZ: ${TZ:-UTC}
      NUVORA_TRUSTED_PROXY: ${NUVORA_TRUSTED_PROXY:-}
    command: ["--bind", "0.0.0.0:8640", "--data-dir", "/data"]
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

healthy() {
  if command -v curl >/dev/null 2>&1; then
    curl -fs -o /dev/null --max-time 2 "http://$HEALTH_HOST:$PORT/api/health"
  else
    (exec 3<>"/dev/tcp/$HEALTH_HOST/$PORT" \
      && printf 'GET /api/health HTTP/1.0\r\nHost: localhost\r\n\r\n' >&3 \
      && head -n 1 <&3 | grep -q ' 200 ') 2>/dev/null
  fi
}

echo "Waiting for Nuvora on port $PORT"
for _ in $(seq 1 60); do
  if healthy; then
    if [ "$LISTEN" = "0.0.0.0" ]; then
      printf 'Nuvora is running at http://%s:%s\n' "$(lan_ip)" "$PORT"
    else
      printf 'Nuvora is running at http://%s:%s (reachable only on that address)\n' "$LISTEN" "$PORT"
    fi
    exit 0
  fi
  sleep 1
done

docker compose --project-directory "$DIR" logs --tail 20 nuvora >&2 || true
die "Nuvora was not healthy on port $PORT after 60s; logs above, or run: docker compose --project-directory $DIR logs nuvora"
