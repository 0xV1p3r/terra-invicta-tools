#!/usr/bin/env sh
# Regenerates src-tauri/icons and the favicon from icon.svg and icon-small.svg.
# icon-small.svg has heavier strokes and is used for 32 px and below. Needs ImageMagick.
set -eu
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pnpm -s tauri icon branding/icon.svg
rm -rf src-tauri/icons/android src-tauri/icons/ios

pnpm -s tauri icon branding/icon-small.svg -o "$tmp" -p 16 -p 24 -p 30 -p 32
pnpm -s tauri icon branding/icon.svg -o "$tmp" -p 48 -p 64 -p 256
cp "$tmp/32x32.png" src-tauri/icons/32x32.png
cp "$tmp/30x30.png" src-tauri/icons/Square30x30Logo.png
magick "$tmp/16x16.png" "$tmp/24x24.png" "$tmp/32x32.png" "$tmp/48x48.png" "$tmp/64x64.png" "$tmp/256x256.png" src-tauri/icons/icon.ico

cp branding/icon-small.svg src/lib/assets/favicon.svg
