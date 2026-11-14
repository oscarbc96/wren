#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
cargo build --release -p wren
bash scripts/make-icns.sh assets/brand/wren-icon.svg assets/macos/wren.icns
cargo packager -p wren --release --formats app --out-dir "$target_dir/release"
scripts/dev-sign.sh --adhoc-fallback "$target_dir/release/Wren.app"
if [ "$target_dir/release" != "$root/target/release" ]; then
  mkdir -p "$root/target/release"
  rm -rf "$root/target/release/Wren.app"
  cp -R "$target_dir/release/Wren.app" "$root/target/release/Wren.app"
fi
echo "$root/target/release/Wren.app"
