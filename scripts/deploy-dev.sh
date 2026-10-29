#!/usr/bin/env bash
# Builds this checkout on a server that runs Nuvora with Docker Compose and
# switches it over, without a GitHub release. The build runs on the server
# itself, whose Cargo cache keeps later builds to a few minutes.
#
#   scripts/deploy-dev.sh root@my-server            build and run this checkout
#   scripts/deploy-dev.sh root@my-server --rollback back to the released image
#
# NUVORA_DIR is the folder the installer set up (default /root/nuvora).
set -euo pipefail

host=${1:?usage: deploy-dev.sh <user@host> [--rollback]}
dir=${NUVORA_DIR:-/root/nuvora}
src=/root/nuvora-src

if [ "${2:-}" = --rollback ]; then
  ssh "$host" "cd $dir && docker compose up -d"
  exit
fi

cd "$(dirname "$0")/.."
echo "Copying the source to $host:$src"
# By content, and without keeping times: a changed file lands with a fresh
# mtime, so the server's cached Cargo build sees it changed.
rsync -rlz --checksum --delete --exclude-from=.dockerignore --exclude .git ./ "$host:$src/"
echo "Building on $host"
# The server stage always reruns, so files copied in keep their fresh mtimes;
# the Cargo cache mount keeps the build incremental.
ssh "$host" "cd $src && docker build --no-cache-filter server -t nuvora:dev . 2>&1 | tail -n 5"
echo "Restarting Nuvora on nuvora:dev"
ssh "$host" "cd $dir && NUVORA_IMAGE=nuvora:dev docker compose up -d && docker ps --filter name=nuvora --format '{{.Image}} {{.Status}}'"
