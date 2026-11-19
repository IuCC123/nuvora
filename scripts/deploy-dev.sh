#!/usr/bin/env bash
# Runs this checkout on a server that runs Nuvora with Docker Compose, without
# a GitHub release, fast enough to try every change:
#
# - The web app is built here and copied over; the server reads it from a
#   folder (--web-dir), so a UI change is live in seconds, with no restart.
# - The server program is rebuilt only when Rust code changed, on the server,
#   in a container whose Cargo cache is a plain folder. Docker's build cache
#   is emptied whenever the disk runs low, which made every build start over.
#   Nuvora's own crates are optimized less than a release's (dependencies
#   fully), so a change rebuilds in about 25 s instead of over 2 minutes.
# - compose.dev.yaml runs that program and web app on the installed image,
#   which brings FFmpeg; compose.override.yaml, if you have one, still applies.
#
#   scripts/deploy-dev.sh root@my-server             build and run this checkout
#   scripts/deploy-dev.sh root@my-server --rollback  back to the released image
#
# A plain `docker compose up -d` in the server's folder also goes back.
#
# NUVORA_DIR is the folder the installer set up (default /root/nuvora).
set -euo pipefail
host=${1:?usage: deploy-dev.sh <user@host> [--rollback]}
dir=${NUVORA_DIR:-/root/nuvora}
src=/root/nuvora-src
dev=/root/nuvora-dev
started=$SECONDS
# One connection for every step.
ssh="ssh -o ControlMaster=auto -o ControlPath=$HOME/.ssh/nuvora-deploy-%C -o ControlPersist=60"

if [ "${2:-}" = --rollback ]; then
  $ssh "$host" "cd $dir && rm -f compose.dev.yaml && docker compose up -d"
  exit
fi
cd "$(dirname "$0")/.."

echo "Building the web app"
(cd apps/web && bun x vite build --logLevel warn)

echo "Copying to $host"
$ssh "$host" "mkdir -p $src $dev/web"
# By content, and without keeping times: a changed file lands with a fresh
# mtime, so Cargo sees it changed. Only what Cargo reads: without the web
# app's dist the program embeds none, so a UI change never rebuilds it.
rsync -e "$ssh" -rlzcR --delete --exclude target \
  crates Cargo.toml Cargo.lock rust-toolchain.toml "$host:$src/"
# Assets before index.html (rsync goes in name order), and no --delete: a page
# loaded before this deploy can still fetch its old chunks.
rsync -e "$ssh" -rlzc apps/web/dist/ "$host:$dev/web/"

$ssh "$host" bash -s -- "$src" "$dev" "$dir" <<'REMOTE'
set -euo pipefail
src=$1 dev=$2 dir=$3
cd "$src"
profile=(
  --config 'profile.release.incremental=true'
  --config 'profile.release.opt-level=1'
  --config 'profile.release.codegen-units=256'
  --config 'profile.release.package."*".opt-level=3'
  --config 'profile.release.package."*".codegen-units=16'
)
sum=$({ find crates Cargo.toml Cargo.lock rust-toolchain.toml -type f -print0 \
  | sort -z | xargs -0 sha256sum; echo "${profile[*]}"; } | sha256sum | cut -c1-64)
restart=
if [ "$sum" != "$(cat "$dev/built" 2>/dev/null)" ] || [ ! -x "$dev/bin/nuvora" ]; then
  echo "Building the server (Rust code changed)"
  mkdir -p "$dev/bin" "$dev/cache"
  # The whole log stays on the server; a failed build stops here, with its end shown.
  if ! docker run --rm --name nuvora-build \
    -v "$src:/src" -w /src \
    -v "$dev/cache/registry:/usr/local/cargo/registry" \
    -v "$dev/cache/git:/usr/local/cargo/git" \
    -v "$dev/cache/target:/target" \
    -e CARGO_TARGET_DIR=/target -e RUSTUP_TOOLCHAIN=stable \
    rust:trixie cargo build --release --locked -p nuvora-server "${profile[@]}" > "$dev/build.log" 2>&1; then
    tail -n 25 "$dev/build.log"
    exit 1
  fi
  grep -E '^ +Finished' "$dev/build.log" || true
  install -m 755 "$dev/cache/target/release/nuvora" "$dev/bin/nuvora"
  echo "$sum" > "$dev/built"
  restart=--force-recreate
fi

cd "$dir"
cat > compose.dev.yaml <<YAML
# Written by scripts/deploy-dev.sh: the server program and web app built from
# a checkout, on the installed image. \`deploy-dev.sh <host> --rollback\` removes it.
# The program goes over the image's, never in its entrypoint: that would drop
# the image's command, and with it --data-dir, so the server would open an
# empty catalog. A new program recreates the container, which mounts it anew.
services:
  nuvora:
    volumes:
      - $dev/bin/nuvora:/usr/local/bin/nuvora:ro
      - $dev/web:/opt/dev/web:ro
    environment:
      NUVORA_WEB_DIR: /opt/dev/web
YAML
files=(-f compose.yaml)
[ -f compose.override.yaml ] && files+=(-f compose.override.yaml)
files+=(-f compose.dev.yaml)
# Recreates only when something changed: the program, or this file.
docker compose "${files[@]}" up -d $restart 2>&1 | grep -v -E 'Running|^$' || true
port=$(sed -n 's/^NUVORA_PORT=//p' .env)
for _ in $(seq 90); do
  curl -fsS -o /dev/null "http://127.0.0.1:${port:-8640}/api/health" 2>/dev/null && break
  sleep 1
done
docker ps --filter name='^nuvora$' --format '{{.Status}}'
REMOTE
echo "Done in $((SECONDS - started)) s"
