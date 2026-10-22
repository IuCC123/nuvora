#!/usr/bin/env bash
# Install or update Nuvora with Docker.
#
#   curl -fsSL https://raw.githubusercontent.com/IuCC123/nuvora/main/scripts/install.sh | bash
#
# Downloads the published image (ghcr.io/iucc123/nuvora); --build builds it
# from the checkout this script is in instead. Re-running updates in place.
set -euo pipefail

IMAGE_REPO="ghcr.io/iucc123/nuvora"
DIR="$HOME/nuvora"
MEDIA=""
PORT=""
LISTEN=""
VERSION=""
BUILD=0
ASSUME_YES=0

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Usage: install.sh [--media PATH] [--port PORT] [--listen ADDR] [--dir DIR] [--version TAG] [--build] [--yes]

  --media PATH    folder with your movies and shows, shown as /media in the
                  library folder picker (mounted read-only)
  --port PORT     port to serve on (default 8640)
  --listen ADDR   host address to publish the port on (default 0.0.0.0, every
                  interface; 127.0.0.1 when a reverse proxy on this machine is
                  the only way in)
  --dir DIR       install directory (default ~/nuvora)
  --version TAG   image version to run, e.g. 0.2.0 (default latest)
  --build         build the image from this checkout instead of downloading it
  --yes           never prompt; fail if a value is missing

Re-running updates to the newest image and reuses the values saved in DIR/.env.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --media) [ $# -ge 2 ] || die "--media needs a path"; MEDIA="$2"; shift 2 ;;
    --port)  [ $# -ge 2 ] || die "--port needs a number"; PORT="$2"; shift 2 ;;
    --listen) [ $# -ge 2 ] || die "--listen needs an IPv4 address"; LISTEN="$2"; shift 2 ;;
    --dir)   [ $# -ge 2 ] || die "--dir needs a path"; DIR="$2"; shift 2 ;;
    --version) [ $# -ge 2 ] || die "--version needs a tag"; VERSION="$2"; shift 2 ;;
    --build) BUILD=1; shift ;;
    --yes|-y) ASSUME_YES=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; die "unknown option: $1" ;;
  esac
done

command -v docker >/dev/null 2>&1 || die "Docker is not installed. Get it from https://docs.docker.com/get-docker/"
docker info >/dev/null 2>&1 || die "Docker is installed but not running. Start Docker (Docker Desktop on Windows and macOS) and try again."
docker compose version >/dev/null 2>&1 || die "The Docker Compose plugin is missing. See https://docs.docker.com/compose/install/"

SRC=""
if [ "$BUILD" -eq 1 ]; then
  SRC="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." 2>/dev/null && pwd || true)"
  [ -f "$SRC/Dockerfile" ] || die "--build needs a Nuvora checkout; run scripts/install.sh from inside one"
fi

# Piped from curl, stdin is this script; prompts read the terminal instead.
TTY=/dev/stdin
[ -t 0 ] || TTY=/dev/tty

mkdir -p "$DIR"
DIR="$(cd "$DIR" && pwd)"
ENV_FILE="$DIR/.env"

saved() {
  [ -f "$ENV_FILE" ] || return 0
  sed -n "s/^$1=//p" "$ENV_FILE" | tail -n 1
}

ask() {
  local prompt="$1" default="$2" reply=""
  if [ "$ASSUME_YES" -eq 1 ] || ! { : <"$TTY"; } 2>/dev/null; then
    printf '%s' "$default"
    return
  fi
  if [ -n "$default" ]; then
    read -r -p "$prompt [$default]: " reply <"$TTY"
    printf '%s' "${reply:-$default}"
  else
    read -r -p "$prompt: " reply <"$TTY"
    printf '%s' "$reply"
  fi
}

[ -n "$MEDIA" ] || MEDIA="$(saved NUVORA_MEDIA)"
[ -n "$MEDIA" ] || MEDIA="$(ask "Folder with your movies and shows" "")"
[ -n "$MEDIA" ] || die "no media folder given; pass --media PATH"
case "$MEDIA" in "~"|"~/"*) MEDIA="$HOME${MEDIA#"~"}" ;; esac
[ -d "$MEDIA" ] || die "media folder does not exist: $MEDIA"
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

if [ "$BUILD" -eq 1 ]; then
  IMAGE="nuvora:local"
else
  # Keep a pinned version across updates; a local build's tag doesn't carry over.
  [ -n "$VERSION" ] || VERSION="$(saved NUVORA_IMAGE | sed -n "s|^$IMAGE_REPO:||p")"
  IMAGE="$IMAGE_REPO:${VERSION:-latest}"
fi

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
NUVORA_IMAGE=$IMAGE
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
    image: ${NUVORA_IMAGE:?}
    container_name: nuvora
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
    restart: unless-stopped
EOF

if [ "$BUILD" -eq 1 ]; then
  echo "Building the Nuvora image from $SRC"
  docker build -t "$IMAGE" "$SRC"
else
  echo "Downloading $IMAGE"
  docker compose --project-directory "$DIR" pull
fi
echo "Starting Nuvora in $DIR"
docker compose --project-directory "$DIR" up -d

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
      printf '\nNuvora is running. Open http://%s:%s in a browser to create your account.\n' "$(lan_ip)" "$PORT"
    else
      printf '\nNuvora is running at http://%s:%s (reachable only on that address).\n' "$LISTEN" "$PORT"
    fi
    exit 0
  fi
  sleep 1
done

docker compose --project-directory "$DIR" logs --tail 20 nuvora >&2 || true
die "Nuvora was not healthy on port $PORT after 60s; logs above, or run: docker compose --project-directory $DIR logs nuvora"
