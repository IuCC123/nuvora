#!/usr/bin/env bash
# Times GET /api/home on a fresh server whose library holds N generated movies.
# The server runs on its own port and data folder, with outbound HTTPS pointed
# at a dead proxy so the made-up titles never reach the metadata services.
#
#   scripts/bench-home.sh [--bin target/release/nuvora] [--movies 1000] [--runs 300] [--port 8659]
#
# Needs ffmpeg and curl. Prints the median and 95th percentile response time
# for a full (non-304) Home response, and its size.
set -euo pipefail

BIN=target/release/nuvora
MOVIES=1000
RUNS=300
PORT=8659
while [ $# -gt 0 ]; do
  case "$1" in
    --bin) BIN="$2"; shift 2 ;;
    --movies) MOVIES="$2"; shift 2 ;;
    --runs) RUNS="$2"; shift 2 ;;
    --port) PORT="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
WORK="$(mktemp -d)"
URL="http://127.0.0.1:$PORT"
JAR="$WORK/cookies"
cleanup() { [ -n "${PID:-}" ] && kill "$PID" 2>/dev/null; rm -rf "$WORK"; }
trap cleanup EXIT

echo "Making $MOVIES movies in $WORK/media"
mkdir -p "$WORK/media/Movies" "$WORK/data"
ffmpeg -v error -f lavfi -i color=c=black:s=320x180:d=1 -f lavfi -i anullsrc=r=48000:cl=stereo \
  -t 1 -c:v libx264 -c:a aac -shortest "$WORK/sample.mp4"
for i in $(seq 1 "$MOVIES"); do
  cp "$WORK/sample.mp4" "$WORK/media/Movies/Test Film $i ($((1950 + i % 75))).mp4"
done
# The scanner skips files that changed moments ago (they may still be copying).
touch -t 202001010000 "$WORK/media/Movies/"*

echo "Starting $("$BIN" -V) on $URL"
HTTPS_PROXY=http://127.0.0.1:9 HTTP_PROXY=http://127.0.0.1:9 \
  "$BIN" --bind "127.0.0.1:$PORT" --data-dir "$WORK/data" --discovery false >"$WORK/server.log" 2>&1 &
PID=$!
until curl -fs "$URL/api/health" >/dev/null; do sleep 0.2; done

api() { curl -fsS -b "$JAR" -c "$JAR" -H "Origin: $URL" -H "Content-Type: application/json" "$@"; }
api -X POST "$URL/api/setup" -d '{"username":"bench","display_name":"Bench","password":"bench-password-123"}' >/dev/null
api -X POST "$URL/api/admin/libraries" -d "{\"name\":\"Movies\",\"kind\":\"movies\",\"paths\":[\"$WORK/media/Movies\"]}" >/dev/null

echo "Scanning"
api -X POST "$URL/api/scan" >/dev/null
sleep 1
until api "$URL/api/scan" | grep -q '"idle"'; do sleep 0.5; done
# Refuse to time anything until Home really holds every movie.
TOTAL=$(api "$URL/api/home" | grep -o '"kind":"library","title":"Movies","total":[0-9]*' | grep -o '[0-9]*$')
if [ "${TOTAL:-0}" != "$MOVIES" ]; then
  echo "Home lists ${TOTAL:-0} movies, expected $MOVIES; see $WORK/server.log" >&2
  trap - EXIT; kill "$PID"; exit 1
fi
echo "Home lists $TOTAL movies"

echo "Timing $RUNS requests"
for i in $(seq 1 20); do api -o /dev/null "$URL/api/home"; done
for i in $(seq 1 "$RUNS"); do
  api -o /dev/null -w '%{time_total} %{size_download}\n' "$URL/api/home"
done >"$WORK/times"

sort -n "$WORK/times" | awk -v runs="$RUNS" '
  { t[NR] = $1 * 1000; size = $2 }
  END {
    printf "Home: median %.2f ms, p95 %.2f ms, %.1f KB (%d runs)\n", t[int(runs * 0.5)], t[int(runs * 0.95)], size / 1024, runs
  }'
