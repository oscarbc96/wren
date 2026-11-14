#!/usr/bin/env bash
# Build Wren and save a screenshot of its window.
# Usage: SHOT_NAME=<name> scripts/shot.sh [URL...]  ->  target/shots/<name>.png
set -euo pipefail
target="${CARGO_TARGET_DIR:-target}"
out="target/shots/${SHOT_NAME:-window}.png"
mkdir -p target/shots
profile="$(mktemp -d)"
trap 'rm -rf "$profile"' EXIT
cargo build -p wren
scripts/dev-sign.sh "$target/debug/wren"
"$target/debug/wren" --screenshot "$out" --profile-dir "$profile" "$@"
echo "$out"
