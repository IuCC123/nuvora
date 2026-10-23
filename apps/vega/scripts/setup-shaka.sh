#!/usr/bin/env bash
# Builds Shaka Player with Amazon's Vega patches and polyfills into
# src/w3cmedia, which git ignores: Amazon publishes them under its own
# license, so they are fetched from Amazon's sample app, not kept here.
# Runs after `npm install`; `--force` builds again.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=4.8.5
RELEASE=1.2
SAMPLE_COMMIT=ef8e4da89b22ef7973d0d41beedfa2b42f7bb272
TARBALL_SHA256=f5e475b8a3793e56b2f888936f05424f672658abeac6c55e3d0e7073e06a8b8e
OUT=src/w3cmedia

if [[ -f "$OUT/shakaplayer/dist/shaka-player.compiled.js" && "${1:-}" != --force ]]; then
  echo "Shaka Player is already built in $OUT"
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tarball="shaka-rel-v$VERSION-r$RELEASE.tar.gz"
curl -fsSL -o "$work/$tarball" \
  "https://raw.githubusercontent.com/AmazonAppDev/vega-video-sample/$SAMPLE_COMMIT/shaka-setup/$tarball"
echo "$TARBALL_SHA256  $work/$tarball" | shasum -a 256 -c -

git clone --quiet --depth 1 --branch "v$VERSION" https://github.com/shaka-project/shaka-player.git "$work/shaka"
cd "$work/shaka"
tar -xzf "../$tarball"
git -c user.name=nuvora -c user.email=build@nuvora.invalid am --quiet -3 shaka-rel/shaka-patch/*.patch
# The UI's web fonts are fetched at build time; the player doesn't need them.
sed -i.bak -e '/fonts.googleapis.com/d' ui/controls.less
python3 build/all.py
cd - >/dev/null

rm -rf "$OUT"
mkdir -p "$OUT/shakaplayer/dist" "$OUT/polyfills"
cp "$work/shaka/dist/shaka-player.compiled.js" "$OUT/shakaplayer/dist/"
cp -R "$work/shaka/shaka-rel/src/polyfills/." "$OUT/polyfills/"
cp "$work/shaka/shaka-rel/src/PlayerInterface.ts" "$OUT/"
# Amazon's sources aren't written for this app's TypeScript settings.
find "$OUT" -name '*.ts' -exec sh -c 'printf "// @ts-nocheck\n" | cat - "$1" > "$1.tmp" && mv "$1.tmp" "$1"' _ {} \;
node --check "$OUT/shakaplayer/dist/shaka-player.compiled.js"
echo "Shaka Player $VERSION built into $OUT"
