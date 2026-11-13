#!/usr/bin/env bash
# Signs a dev build (binary or .app) with the stable "Wren Dev (local)" identity.
# Usage: dev-sign.sh [--adhoc-fallback] <path>
# Without the identity, prints a hint and leaves the build as it is (or, with
# --adhoc-fallback, ad-hoc signs it so a bundle still launches).
set -euo pipefail

NAME="Wren Dev (local)"
KEYCHAIN="${WREN_DEV_KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}"
fallback=0
if [ "${1:-}" = "--adhoc-fallback" ]; then
  fallback=1
  shift
fi
target="${1:?usage: dev-sign.sh [--adhoc-fallback] <path>}"

if [ "$(uname -s)" != Darwin ]; then
  exit 0
fi

if security find-identity -v -p codesigning "$KEYCHAIN" | grep -q "\"$NAME\""; then
  codesign --force --deep --keychain "$KEYCHAIN" -s "$NAME" "$target"
else
  echo "hint: no '$NAME' signing identity; the keychain will prompt after every rebuild." >&2
  echo "hint: run 'mise run dev:cert' once to create it." >&2
  if [ "$fallback" = 1 ]; then
    codesign --force --deep --sign - "$target"
  fi
fi
