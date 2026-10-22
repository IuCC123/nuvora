#!/bin/sh
# Rebuild each styled fixture's WebVTT conversion the way the server makes it
# (ffmpeg's webvtt encoder), for the player's plain-text fallback.
set -e
cd "$(dirname "$0")"
for ass in *.ass; do
  ffmpeg -loglevel error -y -i "$ass" -c:s webvtt "${ass%.ass}.vtt"
done
