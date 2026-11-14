#!/usr/bin/env bash
set -euo pipefail
svg="${1:-assets/brand/wren-icon.svg}"
out="${2:-assets/macos/wren.icns}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
iconset="$tmp/icon.iconset"
mkdir -p "$iconset"
base="$(basename "$svg")"

render() {
  local size="$1" dest="$2"
  qlmanage -t -s "$size" -o "$tmp" "$svg" >/dev/null 2>&1 || true
  local src="$tmp/${base}.png"
  if [ ! -f "$src" ]; then
    sips -s format png -z "$size" "$size" "$svg" --out "$src" >/dev/null 2>&1 || true
  fi
  if [ ! -f "$src" ]; then
    echo "failed to rasterize $svg at ${size}" >&2
    exit 1
  fi
  cp "$src" "$dest"
  rm -f "$src"
}

render 16 "$iconset/icon_16x16.png"
render 32 "$iconset/icon_16x16@2x.png"
render 32 "$iconset/icon_32x32.png"
render 64 "$iconset/icon_32x32@2x.png"
render 128 "$iconset/icon_128x128.png"
render 256 "$iconset/icon_128x128@2x.png"
render 256 "$iconset/icon_256x256.png"
render 512 "$iconset/icon_256x256@2x.png"
render 512 "$iconset/icon_512x512.png"
render 1024 "$iconset/icon_512x512@2x.png"

mkdir -p "$(dirname "$out")"
iconutil -c icns "$iconset" -o "$out"
echo "$out"
