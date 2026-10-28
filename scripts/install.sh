#!/usr/bin/env bash
# Install, update or remove Nuvora with Docker.
#
#   curl -fsSL https://getnuvora.online/install.sh | bash
#
# That address redirects here. The installer finds Docker (and helps install
# or start it), asks where your movies and shows are, and runs the published
# image (ghcr.io/iucc123/nuvora) from ~/nuvora. Running it again updates
# Nuvora and keeps your answers. --build builds the image from the checkout
# this script is in instead; --help lists the rest.
#
# Everything happens in main(), at the bottom: a download cut short runs nothing.
set -euo pipefail

IMAGE_REPO="ghcr.io/iucc123/nuvora"
INSTALL_CMD="curl -fsSL https://getnuvora.online/install.sh | bash"
DOCKER_DESKTOP_URL="https://www.docker.com/products/docker-desktop/"
DEFAULT_PORT=8640

DIR="$HOME/nuvora"
MEDIA="" PORT="" LISTEN="" VERSION=""
BUILD=0 ASSUME_YES=0 UNINSTALL=0 DIR_GIVEN=0
DOCKER=(docker)
TMPD="" LOG="" BG_PID="" TTY_OUT=0

usage() {
  cat <<EOF
Install or update Nuvora:  $INSTALL_CMD
With options:              $INSTALL_CMD -s -- --media ~/Media

  --media PATH    folder with your movies and shows (Nuvora only reads it)
  --port PORT     port to serve on (default $DEFAULT_PORT, or the next free one)
  --listen ADDR   address to publish the port on (default 0.0.0.0, every network;
                  127.0.0.1 when a reverse proxy on this machine is the only way in)
  --dir DIR       where Nuvora keeps its settings and data (default ~/nuvora)
  --version TAG   image version to run, e.g. 0.7.2 (default: the newest)
  --build         build the image from this checkout instead of downloading it
  --yes           never ask; use the saved or default answers
  --uninstall     stop and remove Nuvora (asks before deleting its data)

Running it again updates Nuvora and reuses the answers saved in DIR/.env.
EOF
}

# --- Output -----------------------------------------------------------------

style() {
  BOLD="" DIM="" ULINE="" RESET="" RED="" GREEN="" YELLOW="" ACCENT=""
  if [ -t 1 ] && [ "${TERM:-dumb}" != dumb ]; then
    TTY_OUT=1
    if [ -z "${NO_COLOR:-}" ]; then
      BOLD=$'\033[1m' DIM=$'\033[2m' ULINE=$'\033[4m' RESET=$'\033[0m'
      RED=$'\033[31m' GREEN=$'\033[32m' YELLOW=$'\033[33m' ACCENT=$'\033[33m'
      case "${COLORTERM:-}" in truecolor | 24bit) ACCENT=$'\033[38;2;229;160;13m' ;; esac
    fi
  fi
  case "${LC_ALL:-${LC_CTYPE:-${LANG:-}}}" in
    *[Uu][Tt][Ff]-8* | *[Uu][Tt][Ff]8*)
      TICK="✓" CROSS="✗" TO="→" FRAMES=(⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏) ;;
    *) TICK="+" CROSS="x" TO="->" FRAMES=('-' "\\" '|' '/') ;;
  esac
}

say() { printf '  %s\n' "$*"; }
ok() { printf '  %s%s%s %s\n' "$GREEN" "$TICK" "$RESET" "$*"; }
warn() { printf '  %s!%s %s\n' "$YELLOW" "$RESET" "$*"; }
note() { printf '  %s%s%s\n' "$DIM" "$*" "$RESET"; }
row() { printf '  %s%-10s%s %s\n' "$DIM" "$1" "$RESET" "$2"; }

# die PROBLEM [WHAT TO DO...]
die() {
  printf '\n  %s%s %s%s\n' "$RED" "$CROSS" "$1" "$RESET" >&2
  shift
  local line
  for line in "$@"; do printf '    %s\n' "$line" >&2; done
  printf '\n' >&2
  exit 1
}

# The last lines a failed command printed, before die explains it.
show_log() {
  [ -s "$LOG" ] || return 0
  printf '\n' >&2
  tail -n "${1:-12}" "$LOG" | while IFS= read -r line; do
    printf '    %s%s%s\n' "$DIM" "$line" "$RESET" >&2
  done
}

pretty() {
  case "$1" in
    "$HOME") printf '~' ;;
    "$HOME"/*) printf '~%s' "${1#"$HOME"}" ;;
    *) printf '%s' "$1" ;;
  esac
}

elapsed() {
  local s=$((SECONDS - $1))
  if [ "$s" -lt 3 ]; then return 0; fi
  if [ "$s" -lt 60 ]; then printf '%ss' "$s"; else printf '%dm %02ds' $((s / 60)) $((s % 60)); fi
}

# spin LABEL COMMAND...: runs COMMAND with its output in $LOG and a spinner
# beside LABEL meanwhile. Returns COMMAND's status.
spin() {
  local label="$1" status=0 i=0 start=$SECONDS
  shift
  : >"$LOG"
  if [ "$TTY_OUT" -eq 0 ]; then
    say "$label..."
    ("$@") >>"$LOG" 2>&1 || status=$?
    return "$status"
  fi
  # sudo can't ask for a password under a spinner, so ask first.
  if [ "${DOCKER[0]}" = sudo ]; then sudo -v; fi
  "$@" >>"$LOG" 2>&1 &
  BG_PID=$!
  printf '\033[?25l'
  while kill -0 "$BG_PID" 2>/dev/null; do
    printf '\r\033[2K  %s%s%s %s  %s%s%s' "$ACCENT" "${FRAMES[i % ${#FRAMES[@]}]}" "$RESET" "$label" "$DIM" "$(elapsed "$start")" "$RESET"
    i=$((i + 1))
    sleep 0.1
  done
  wait "$BG_PID" || status=$?
  BG_PID=""
  printf '\r\033[2K\033[?25h'
  return "$status"
}

# wait_for SECONDS COMMAND...: retries COMMAND until it succeeds or time runs out.
wait_for() {
  local until=$((SECONDS + $1))
  shift
  until "$@"; do
    [ "$SECONDS" -lt "$until" ] || return 1
    sleep 2
  done
}

cleanup() {
  local status=$?
  if [ -n "$BG_PID" ]; then
    pkill -P "$BG_PID" 2>/dev/null || true
    kill "$BG_PID" 2>/dev/null || true
    printf '\r\033[2K'
  fi
  if [ "$TTY_OUT" -eq 1 ]; then printf '\033[?25h'; fi
  if [ -n "$TMPD" ]; then rm -rf "$TMPD"; fi
  if [ "$status" -eq 130 ]; then printf '\n  Stopped.\n\n'; fi
}

# --- Questions ----------------------------------------------------------------

# Readline needs to know which prompt bytes take no room on screen.
rl() { if [ -n "$1" ]; then printf '\001%s\002' "$1"; fi; }

# ask PROMPT: reads a line into REPLY, with editing and Tab completion of paths.
ask() {
  REPLY=""
  read -e -r -p "  $(rl "$ACCENT")?$(rl "$RESET") $1 " REPLY || {
    printf '\n'
    exit 130
  }
}

# confirm QUESTION Y|N: yes or no, defaulting to the second argument.
confirm() {
  local hint="y/N"
  if [ "$2" = Y ]; then hint="Y/n"; fi
  if [ "$ASK" -eq 0 ]; then
    case "$2" in Y) return 0 ;; *) return 1 ;; esac
  fi
  ask "$1 $(rl "$DIM")[$hint]$(rl "$RESET")"
  case "${REPLY:-$2}" in [Yy]*) return 0 ;; *) return 1 ;; esac
}

need_sudo() {
  if [ "$(id -u)" -eq 0 ] || sudo -n true 2>/dev/null; then return 0; fi
  command -v sudo >/dev/null 2>&1 || die "This step needs root, and sudo isn't installed." "Run the installer as root."
  [ "$TTY_IN" -eq 1 ] || die "This step needs sudo, which asks for your password." "Run the installer in a terminal."
  sudo -v -p "  Password for sudo: " || die "sudo didn't accept the password."
}

as_root() { if [ "$(id -u)" -eq 0 ]; then "$@"; else sudo "$@"; fi; }

# --- Docker -------------------------------------------------------------------

docker_ok() { "${DOCKER[@]}" info >/dev/null 2>&1; }

# Docker Desktop and OrbStack install their docker command where this shell might not look yet.
find_docker() {
  if command -v docker >/dev/null 2>&1; then return 0; fi
  local d
  for d in /usr/local/bin /opt/homebrew/bin "$HOME/.docker/bin" "$HOME/.orbstack/bin" /Applications/Docker.app/Contents/Resources/bin; do
    if [ -x "$d/docker" ]; then
      PATH="$d:$PATH"
      return 0
    fi
  done
  return 1
}

docker_desktop_ready() { find_docker && docker_ok; }

# Docker is running, but only root may use it: carry on with sudo.
docker_needs_sudo() {
  local err
  [ "${DOCKER[0]}" = docker ] || return 1
  err=$("${DOCKER[@]}" info 2>&1 >/dev/null || true)
  case "$err" in *"permission denied"*) ;; *) return 1 ;; esac
  warn "Your account can't use Docker without sudo, so the installer uses it."
  need_sudo
  DOCKER=(sudo docker)
  docker_ok
}

install_docker() {
  case "$(uname -s)" in
    Darwin)
      [ "$ASK" -eq 1 ] || die "Docker isn't installed." "Install Docker Desktop from $DOCKER_DESKTOP_URL, open it, and run this again."
      warn "Nuvora runs in Docker, which isn't on this Mac yet."
      say "Install Docker Desktop and open it once. The installer carries on when it's running."
      open "$DOCKER_DESKTOP_URL" 2>/dev/null || say "Get it at $DOCKER_DESKTOP_URL"
      note "Press Ctrl-C to stop."
      spin "Waiting for Docker Desktop" wait_for 3600 docker_desktop_ready \
        || die "Docker Desktop didn't start." "Open it, wait until it says it's running, and run this again."
      find_docker
      ;;
    Linux)
      warn "Nuvora runs in Docker, which isn't installed."
      confirm "Install it now with Docker's official script? It needs sudo" Y \
        || die "Docker isn't installed." "Install it with: curl -fsSL https://get.docker.com | sh" "Then run this again."
      command -v curl >/dev/null 2>&1 || die "Installing Docker needs curl." "Install curl and run this again."
      need_sudo
      spin "Installing Docker" install_docker_linux || {
        show_log
        die "Docker didn't install." "See https://docs.docker.com/engine/install/ for this system, then run this again."
      }
      ok "Installed Docker"
      ;;
    *) die "Docker isn't installed." "Get it from https://docs.docker.com/get-docker/ and run this again." ;;
  esac
}

# Functions that run under spin chain with &&: set -e doesn't stop them there.
install_docker_linux() {
  curl -fsSL https://get.docker.com -o "$TMPD/get-docker.sh" && as_root sh "$TMPD/get-docker.sh"
}

start_docker() {
  case "$(uname -s)" in
    Darwin)
      local app
      for app in Docker OrbStack; do
        [ -d "/Applications/$app.app" ] || continue
        open -a "$app"
        [ "$app" = Docker ] && app="Docker Desktop"
        spin "Starting $app" wait_for 180 docker_ok && return 0
        die "$app didn't start within 3 minutes." "Open it, wait until it's running, and run this again."
      done
      if command -v colima >/dev/null 2>&1; then
        spin "Starting Colima" colima start && docker_ok && return 0
      fi
      die "Docker isn't running." "Start it and run this again."
      ;;
    Linux)
      command -v systemctl >/dev/null 2>&1 || die "Docker isn't running." "Start it and run this again."
      confirm "Docker isn't running. Start it now? It needs sudo" Y || die "Docker isn't running." "Start it with: sudo systemctl start docker"
      need_sudo
      as_root systemctl enable --now docker >"$LOG" 2>&1 || { show_log; die "Docker didn't start."; }
      spin "Starting Docker" wait_for 60 docker_ok && return 0
      docker_needs_sudo && return 0
      die "Docker didn't start." "See: sudo systemctl status docker"
      ;;
    *) die "Docker isn't running." "Start it and run this again." ;;
  esac
}

# ensure_docker [install]: makes sure Docker runs, starting it, and installing it when asked to.
ensure_docker() {
  if ! find_docker; then
    [ "${1:-}" = install ] || die "Docker isn't installed, so there's no Nuvora to remove."
    install_docker
  fi
  docker_ok || docker_needs_sudo || start_docker
  "${DOCKER[@]}" compose version >/dev/null 2>&1 \
    || die "Docker Compose is missing." "Install it (https://docs.docker.com/compose/install/linux/) and run this again."
  local version
  version=$("${DOCKER[@]}" version --format '{{.Server.Version}}' 2>/dev/null || true)
  ok "Docker${version:+ $version} is running"
}

compose() { "${DOCKER[@]}" compose --project-directory "$DIR" "$@"; }

image_id() { "${DOCKER[@]}" image inspect -f '{{.Id}}' "$IMAGE" 2>/dev/null || true; }

# The folder the nuvora container was started from, or "-" when it wasn't started with Compose.
container_dir() {
  local dir
  dir=$("${DOCKER[@]}" inspect -f '{{index .Config.Labels "com.docker.compose.project.working_dir"}}' nuvora 2>/dev/null) || return 0
  printf '%s' "${dir:--}"
}

# The host port a running Nuvora container answers on, if one does.
running_port() { { "${DOCKER[@]}" port nuvora 8640/tcp 2>/dev/null || true; } | sed -n '1s/.*://p'; }

# --- Settings -----------------------------------------------------------------

saved() {
  [ -f "$ENV_FILE" ] || return 0
  sed -n "s/^$1=//p" "$ENV_FILE" | tail -n 1 | sed -e "s/^'\(.*\)'\$/\1/" -e 's/^"\(.*\)"$/\1/'
}

valid_ipv4() {
  local IFS=. part
  # shellcheck disable=SC2086
  set -- $1
  [ $# -eq 4 ] || return 1
  for part in "$@"; do
    case "$part" in '' | *[!0-9]*) return 1 ;; esac
    [ "$part" -le 255 ] || return 1
  done
}

# Cleans up a typed or dragged-in path into FOLDER. Fails when it isn't a folder.
resolve_folder() {
  local p="$1"
  p="${p#"${p%%[![:space:]]*}"}"
  p="${p%"${p##*[![:space:]]}"}"
  case "$p" in
    \'*\' | \"*\") p="${p:1:${#p}-2}" ;;
    *\\*) p=$(printf '%s' "$p" | sed 's/\\\(.\)/\1/g') ;;
  esac
  case "$p" in \~) p=$HOME ;; \~/*) p="$HOME/${p#\~/}" ;; esac
  [ -n "$p" ] && [ -d "$p" ] || return 1
  FOLDER=$(cd "$p" && pwd)
}

count_videos() {
  local t=""
  if command -v timeout >/dev/null 2>&1; then t="timeout 5"; fi
  { $t find "$1" -maxdepth "$2" -type f \( -iname '*.mkv' -o -iname '*.mp4' -o -iname '*.m4v' \
    -o -iname '*.avi' -o -iname '*.mov' -o -iname '*.webm' -o -iname '*.wmv' -o -iname '*.m2ts' \
    -o -iname '*.mpg' \) 2>/dev/null || true; } | head -n 1000 | wc -l | tr -d ' '
}

videos() {
  if [ "$1" -ge 1000 ]; then printf '1,000+ videos'
  elif [ "$1" -eq 1 ]; then printf '1 video'
  else printf '%s videos' "$1"; fi
}

# Folders where media often lives, for the media question to suggest.
media_candidates() {
  local d
  printf '%s\n' "$HOME/Media" "$HOME/Movies" "$HOME/Videos"
  [ "$(uname -s)" = Linux ] || return 0
  for d in /media/* /media/"${USER:-root}"/* /mnt/* /mnt/user/* /srv/media /data/media /volume1/video /volume1/Media /share/Multimedia; do
    [ "$d" = "/media/${USER:-root}" ] && continue
    [ -d "$d" ] && printf '%s\n' "$d"
  done
  return 0
}

# Prints "count<TAB>folder" for up to five suggestions that hold videos, most first.
scan_media() {
  local d real seen=$'\n' n
  while IFS= read -r d; do
    [ -d "$d" ] || continue
    real=$(cd "$d" 2>/dev/null && pwd -P) || continue
    case "$seen" in *$'\n'"$real"$'\n'*) continue ;; esac
    seen="$seen$real"$'\n'
    n=$(count_videos "$d" 4)
    if [ "$n" -gt 0 ]; then printf '%s\t%s\n' "$n" "$d"; fi
  done < <(media_candidates) | sort -t $'\t' -k1,1nr | head -n 5
}

ask_media() {
  local paths=() counts=() n line i width=0 question="Folder:"
  printf '\n  %sWhere are your movies and shows?%s\n' "$BOLD" "$RESET"
  note "Nuvora only reads this folder; your files stay as they are."
  if spin "Looking for videos" scan_media; then
    while IFS=$'\t' read -r n line; do
      [ -n "$line" ] || continue
      paths+=("$line")
      counts+=("$n")
      line=$(pretty "$line")
      [ "${#line}" -le "$width" ] || width=${#line}
    done <"$LOG"
  fi
  if [ "${#paths[@]}" -gt 0 ]; then
    printf '\n'
    for ((i = 0; i < ${#paths[@]}; i++)); do
      printf '    %s%d%s  %-*s  %s%s%s\n' "$ACCENT" $((i + 1)) "$RESET" "$width" "$(pretty "${paths[i]}")" "$DIM" "$(videos "${counts[i]}")" "$RESET"
    done
    printf '\n'
    if [ "${#paths[@]}" -eq 1 ]; then question="Folder (1, or a path):"; else question="Folder (1-${#paths[@]}, or a path):"; fi
  fi
  note "Type a path (Tab completes) or drag the folder into this window."
  while :; do
    ask "$question"
    case "$REPLY" in
      *[![:space:]]*) ;;
      *) continue ;;
    esac
    if [ "${#paths[@]}" -gt 0 ] && [[ "$REPLY" =~ ^[[:space:]]*[0-9]+[[:space:]]*$ ]]; then
      i=$(printf '%s' "$REPLY" | tr -d '[:space:]')
      i=$((10#$i))
      if [ "$i" -lt 1 ] || [ "$i" -gt "${#paths[@]}" ]; then
        warn "Pick a number from 1 to ${#paths[@]}, or type a path."
        continue
      fi
      MEDIA=${paths[i - 1]}
      n=${counts[i - 1]}
      break
    fi
    if ! resolve_folder "$REPLY"; then
      warn "There's no folder at $REPLY. Try again."
      continue
    fi
    spin "Looking in $(pretty "$FOLDER")" count_videos "$FOLDER" 6 || true
    n=$(tr -dc 0-9 <"$LOG")
    n=${n:-0}
    if [ "$n" -eq 0 ]; then
      warn "There are no videos in $(pretty "$FOLDER")."
      confirm "Use it anyway?" N || continue
    fi
    MEDIA=$FOLDER
    break
  done
  printf '\n'
  ok "Media folder: $(pretty "$MEDIA") $DIM($(videos "$n"))$RESET"
}

choose_media() {
  if [ -n "$MEDIA" ]; then
    if resolve_folder "$MEDIA"; then
      MEDIA=$FOLDER
      ok "Media folder: $(pretty "$MEDIA")"
      return
    fi
    [ "$ASK" -eq 1 ] || die "There's no folder at $MEDIA." "Pass the folder with your movies and shows: --media PATH"
    warn "There's no folder at $MEDIA anymore."
  fi
  [ "$ASK" -eq 1 ] || die "The installer needs the folder with your movies and shows." \
    "Run it in a terminal to choose one, or pass it:" "$INSTALL_CMD -s -- --media /path/to/media"
  ask_media
}

port_busy() { (exec 3<>"/dev/tcp/$HEALTH_HOST/$1") 2>/dev/null; }

choose_port() {
  local ours
  ours=$(running_port)
  if [ -n "$PORT" ]; then
    case "$PORT" in '' | *[!0-9]*) die "$PORT isn't a port number." "Use a number from 1 to 65535, like --port $DEFAULT_PORT." ;; esac
    { [ "$PORT" -ge 1 ] && [ "$PORT" -le 65535 ]; } || die "$PORT isn't a port number." "Use a number from 1 to 65535, like --port $DEFAULT_PORT."
    if [ "$PORT" != "$ours" ] && port_busy "$PORT"; then
      die "Another program is using port $PORT." "Stop it, or give Nuvora another port: $INSTALL_CMD -s -- --port $((PORT + 1))"
    fi
    return
  fi
  PORT=$DEFAULT_PORT
  if [ "$PORT" != "$ours" ] && port_busy "$PORT"; then
    while port_busy "$PORT"; do PORT=$((PORT + 1)); done
    warn "Port $DEFAULT_PORT is taken, so Nuvora uses $PORT."
  fi
}

timezone() {
  local tz
  tz=$(saved TZ)
  if [ -z "$tz" ] && [ -r /etc/timezone ]; then tz=$(cat /etc/timezone); fi
  if [ -z "$tz" ] && [ -L /etc/localtime ]; then tz=$(readlink /etc/localtime | sed 's|.*/zoneinfo/||'); fi
  printf '%s' "${tz:-UTC}"
}

write_files() {
  local managed='^(NUVORA_IMAGE|NUVORA_MEDIA|NUVORA_PORT|NUVORA_LISTEN|NUVORA_UID|NUVORA_GID|TZ)='
  mkdir -p "$DIR/data"
  # Lines the installer doesn't manage, like NUVORA_TRUSTED_PROXY, stay as they are.
  {
    printf '%s\n' "NUVORA_IMAGE=$IMAGE" "NUVORA_MEDIA=$MEDIA" "NUVORA_PORT=$PORT" "NUVORA_LISTEN=$LISTEN" \
      "NUVORA_UID=$(id -u)" "NUVORA_GID=$(id -g)" "TZ=$(timezone)"
    if [ -f "$ENV_FILE" ]; then grep -Ev "$managed" "$ENV_FILE" || true; fi
  } >"$TMPD/env"
  mv "$TMPD/env" "$ENV_FILE"

  cat >"$DIR/compose.yaml" <<'EOF'
# Written by the Nuvora installer, which rewrites it on every update. Settings
# go in .env next to it; for changes of your own, such as passing in a graphics
# chip, create compose.override.yaml here, which Docker Compose reads too.
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
      NUVORA_HARDWARE_ENCODING: ${NUVORA_HARDWARE_ENCODING:-auto}
    restart: unless-stopped
EOF
}

# --- Running Nuvora -----------------------------------------------------------

# http_get HOST PORT PATH: Nuvora's answer, without the headers.
http_get() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsS --max-time 2 "http://$1:$2$3" 2>/dev/null
  else
    (exec 3<>"/dev/tcp/$1/$2" && printf 'GET %s HTTP/1.0\r\nHost: localhost\r\n\r\n' "$3" >&3 && sed '1,/^\r*$/d' <&3) 2>/dev/null
  fi
}

# The version of the Nuvora answering on PORT, if one is.
version_on() {
  [ -n "$1" ] || return 0
  { http_get "$HEALTH_HOST" "$1" /api/health || true; } | sed -n 's/.*"version":"\([^"]*\)".*/\1/p'
}

healthy() {
  case "$(http_get "$HEALTH_HOST" "$PORT" /api/health || true)" in *'"status":"ok"'*) return 0 ;; esac
  # A container that stopped or keeps restarting won't get healthy by waiting.
  case "$("${DOCKER[@]}" inspect -f '{{.State.Status}}' nuvora 2>/dev/null || true)" in
    exited | dead | restarting) exit 2 ;;
  esac
  return 1
}

start_nuvora() {
  compose up -d --remove-orphans && wait_for 90 healthy
}

fetch_image() {
  local before after hint=""
  before=$(image_id)
  if [ "$BUILD" -eq 1 ]; then
    spin "Building Nuvora from $(pretty "$SRC"), which takes a while" "${DOCKER[@]}" build -t "$IMAGE" "$SRC" \
      || { show_log 20; die "The build failed."; }
    ok "Built Nuvora"
  else
    spin "Downloading Nuvora" "${DOCKER[@]}" pull "$IMAGE" || {
      show_log
      case "$(cat "$LOG")" in
        *"no matching manifest"*) hint="Nuvora runs on 64-bit Intel/AMD and ARM; this machine is $(uname -m)." ;;
        *"manifest unknown"* | *"not found"*) hint="There's no Nuvora ${VERSION:-$IMAGE}. Versions: https://github.com/IuCC123/nuvora/releases" ;;
        *) hint="Check the internet connection and run this again." ;;
      esac
      die "Couldn't download Nuvora." "$hint"
    }
  fi
  after=$(image_id)
  NEW_IMAGE=1
  if [ -n "$before" ] && [ "$before" = "$after" ]; then NEW_IMAGE=0; fi
  if [ "$BUILD" -eq 0 ] && [ "$NEW_IMAGE" -eq 1 ]; then ok "Downloaded Nuvora"; fi
}

start() {
  spin "Starting Nuvora" start_nuvora && return 0
  show_log
  case "$(cat "$LOG")" in
    *"port is already allocated"* | *"address already in use"*)
      die "Another program took port $PORT." "Give Nuvora another port: $INSTALL_CMD -s -- --port $((PORT + 1))" ;;
  esac
  printf '\n' >&2
  { compose logs --no-log-prefix --tail 15 nuvora 2>/dev/null || true; } | while IFS= read -r line; do
    printf '    %s%s%s\n' "$DIM" "$line" "$RESET" >&2
  done
  die "Nuvora didn't start." "Its log is above; for all of it, run: ${DOCKER[*]} logs nuvora"
}

lan_ip() {
  local ip="" iface=""
  if command -v ip >/dev/null 2>&1; then
    ip=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' || true)
  elif command -v ipconfig >/dev/null 2>&1; then
    iface=$(route -n get default 2>/dev/null | sed -n 's/.*interface: //p' || true)
    if [ -n "$iface" ]; then ip=$(ipconfig getifaddr "$iface" 2>/dev/null || true); fi
  fi
  if [ -z "$ip" ]; then ip=$({ hostname -I 2>/dev/null || true; } | awk '{print $1}'); fi
  printf '%s' "${ip:-localhost}"
}

open_browser() {
  [ "$TTY_IN" -eq 1 ] && [ -z "${SSH_CONNECTION:-}${SSH_TTY:-}" ] || return 1
  case "$(uname -s)" in
    Darwin) open "$1" 2>/dev/null ;;
    Linux)
      [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && command -v xdg-open >/dev/null 2>&1 || return 1
      xdg-open "$1" >/dev/null 2>&1 &
      ;;
    *) return 1 ;;
  esac
}

finish() {
  local version url state
  version=$(version_on "$PORT")
  state=$(http_get "$HEALTH_HOST" "$PORT" /api/auth/state || true)
  if [ -n "$OLD_VERSION" ] && [ "$OLD_VERSION" != "$version" ]; then
    ok "Updated Nuvora $OLD_VERSION $TO $version"
  elif [ "$UPDATING" -eq 1 ] && [ "$NEW_IMAGE" -eq 0 ]; then
    ok "Nuvora $version is up to date"
  elif [ "$UPDATING" -eq 1 ]; then
    ok "Updated Nuvora to the newest $version"
  else
    ok "Nuvora $version is running"
  fi

  if [ "$LISTEN" = 0.0.0.0 ]; then url="http://$(lan_ip):$PORT"; else url="http://$LISTEN:$PORT"; fi
  printf '\n'
  case "$state" in
    *'"setup_required":true'*)
      say "${BOLD}Open $ACCENT$ULINE$url$RESET$BOLD to create your account.$RESET"
      if [ "$UPDATING" -eq 0 ] && open_browser "$url"; then note "It's opening in your browser now."; fi
      ;;
    *) say "${BOLD}Open $ACCENT$ULINE$url$RESET" ;;
  esac
  if [ "$LISTEN" = 0.0.0.0 ]; then
    note "Phones, laptops and the TV app on your network use this address too."
  else
    note "Only this address reaches it, as you asked with --listen."
  fi

  printf '\n'
  row "Folder" "$(pretty "$DIR")"
  row "Update" "run the same command again"
  row "Logs" "${DOCKER[*]} logs -f nuvora"
  row "Uninstall" "$INSTALL_CMD -s -- --uninstall"
  if [ "${DOCKER[0]}" = sudo ]; then
    printf '\n'
    note "To use Docker without sudo: sudo usermod -aG docker $USER, then sign out and back in."
  fi
  printf '\n'
}

# --- Commands -----------------------------------------------------------------

header() { printf '\n  %s%sNuvora%s  %s%s%s\n\n' "$BOLD" "$ACCENT" "$RESET" "$DIM" "$1" "$RESET"; }

install() {
  local other
  UPDATING=0
  if [ -f "$ENV_FILE" ]; then UPDATING=1; fi
  if [ "$UPDATING" -eq 1 ]; then header "Updating $(pretty "$DIR")"; else header "Installing in $(pretty "$DIR")"; fi

  if [ "$BUILD" -eq 1 ]; then
    SRC=$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." 2>/dev/null && pwd || true)
    [ -f "$SRC/Dockerfile" ] || die "--build needs a Nuvora checkout." "Run scripts/install.sh --build from inside one."
  fi

  ensure_docker install
  other=$(container_dir)
  case "$other" in
    "" | "$DIR") ;;
    -) die "There's already a container named nuvora." "Remove it with ${DOCKER[*]} rm -f nuvora (its data stays where it is), then run this again." ;;
    *) die "Nuvora already runs from $(pretty "$other")." "To update it: $INSTALL_CMD -s -- --dir $other" "To move it here, first remove that one: ${DOCKER[*]} rm -f nuvora" ;;
  esac

  [ -n "$LISTEN" ] || LISTEN=$(saved NUVORA_LISTEN)
  LISTEN=${LISTEN:-0.0.0.0}
  valid_ipv4 "$LISTEN" || die "--listen needs an IPv4 address, such as 127.0.0.1." "Got: $LISTEN"
  HEALTH_HOST=$LISTEN
  if [ "$LISTEN" = 0.0.0.0 ]; then HEALTH_HOST=127.0.0.1; fi

  if [ "$BUILD" -eq 1 ]; then
    IMAGE="nuvora:local"
  else
    # A pinned version stays pinned across updates; a local build's tag doesn't carry over.
    [ -n "$VERSION" ] || VERSION=$(saved NUVORA_IMAGE | sed -n "s|^$IMAGE_REPO:||p")
    IMAGE="$IMAGE_REPO:${VERSION:-latest}"
  fi

  [ -n "$MEDIA" ] || MEDIA=$(saved NUVORA_MEDIA)
  choose_media
  [ -n "$PORT" ] || PORT=$(saved NUVORA_PORT)
  choose_port

  OLD_VERSION=$(version_on "$(running_port)")
  # Settings are saved only once there's an image to run with them.
  fetch_image
  write_files
  start
  finish
}

uninstall() {
  header "Uninstalling from $(pretty "$DIR")"
  [ -f "$DIR/compose.yaml" ] || die "Nuvora isn't installed in $(pretty "$DIR")." "If it's somewhere else, add --dir with that folder."
  say "This stops Nuvora and removes it. Your movies and shows aren't touched."
  if [ "$ASK" -eq 1 ]; then
    printf '\n'
    confirm "Remove Nuvora?" N || { say "Nothing changed."; printf '\n'; return; }
  fi
  ensure_docker
  spin "Removing Nuvora" compose down --rmi all --remove-orphans || { show_log; die "Couldn't remove Nuvora."; }
  ok "Removed Nuvora"
  if [ "$ASK" -eq 1 ] && confirm "Also delete its data: accounts, watch progress and settings?" N; then
    rm -rf "$DIR/data" "$DIR/.env" "$DIR/compose.yaml"
    rmdir "$DIR" 2>/dev/null || true
    ok "Deleted its data"
  else
    ok "Kept its data in $(pretty "$DIR"). Install again to pick up where you left off."
  fi
  printf '\n'
}

main() {
  style
  while [ $# -gt 0 ]; do
    case "$1" in
      --media) [ $# -ge 2 ] || die "--media needs a path"; MEDIA="$2"; shift 2 ;;
      --port) [ $# -ge 2 ] || die "--port needs a number"; PORT="$2"; shift 2 ;;
      --listen) [ $# -ge 2 ] || die "--listen needs an IPv4 address"; LISTEN="$2"; shift 2 ;;
      --dir) [ $# -ge 2 ] || die "--dir needs a path"; DIR="$2"; DIR_GIVEN=1; shift 2 ;;
      --version) [ $# -ge 2 ] || die "--version needs a tag, like 0.7.2"; VERSION="$2"; shift 2 ;;
      --build) BUILD=1; shift ;;
      --yes | -y) ASSUME_YES=1; shift ;;
      --uninstall) UNINSTALL=1; shift ;;
      -h | --help) usage; exit 0 ;;
      *) usage >&2; die "Unknown option: $1" ;;
    esac
  done

  TMPD=$(mktemp -d "${TMPDIR:-/tmp}/nuvora-install.XXXXXX")
  LOG="$TMPD/log"
  trap cleanup EXIT
  trap 'exit 130' INT TERM
  TTY_IN=0
  if [ -t 0 ]; then TTY_IN=1; fi
  ASK=$TTY_IN
  if [ "$ASSUME_YES" -eq 1 ]; then ASK=0; fi

  case "$DIR" in \~) DIR=$HOME ;; \~/*) DIR="$HOME/${DIR#\~/}" ;; /*) ;; *) DIR="$PWD/$DIR" ;; esac
  DIR=${DIR%/}
  ENV_FILE="$DIR/.env"
  # Without --dir, work on the Nuvora that's already running, wherever it was installed.
  if [ "$DIR_GIVEN" -eq 0 ] && [ ! -f "$ENV_FILE" ] && find_docker && docker_ok; then
    local other
    other=$(container_dir)
    if [ -f "$other/.env" ]; then
      DIR=$other
      ENV_FILE="$DIR/.env"
    fi
  fi

  if [ "$UNINSTALL" -eq 1 ]; then uninstall; else install; fi
}

# Piped from curl, stdin is this script; answers come from the terminal instead.
if [ ! -t 0 ] && { : </dev/tty; } 2>/dev/null; then
  main "$@" </dev/tty
else
  main "$@"
fi
