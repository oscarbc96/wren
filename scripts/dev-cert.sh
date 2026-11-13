#!/usr/bin/env bash
# Creates the "Wren Dev (local)" self-signed code-signing identity in the login
# keychain, once. Idempotent. Signing dev builds with one stable identity keeps
# their designated requirement constant across rebuilds, so the keychain's
# "Always Allow" for Wren's items keeps matching. Nothing here touches the
# system keychain or needs admin rights; macOS asks for your login password once
# (to import the key and to trust the certificate for code signing).
set -euo pipefail

NAME="Wren Dev (local)"
KEYCHAIN="${WREN_DEV_KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}"
# Apple's LibreSSL writes PKCS#12 files the macOS keychain imports.
OPENSSL=/usr/bin/openssl

if [ "$(uname -s)" != Darwin ]; then
  echo "dev:cert is macOS only" >&2
  exit 0
fi

if security find-identity -v -p codesigning "$KEYCHAIN" | grep -q "\"$NAME\""; then
  echo "$NAME already exists and is trusted"
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if security find-certificate -c "$NAME" "$KEYCHAIN" >/dev/null 2>&1; then
  # Imported earlier but never trusted (the prompt was cancelled): trust it.
  security find-certificate -c "$NAME" -p "$KEYCHAIN" >"$tmp/cert.pem"
  security add-trusted-cert -p codeSign -k "$KEYCHAIN" "$tmp/cert.pem"
  echo "Trusted $NAME for code signing."
  exit 0
fi

pass="wren-$$-$RANDOM"

cat >"$tmp/openssl.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
CNF

"$OPENSSL" req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -config "$tmp/openssl.cnf" -keyout "$tmp/key.pem" -out "$tmp/cert.pem" 2>/dev/null
"$OPENSSL" pkcs12 -export -inkey "$tmp/key.pem" -in "$tmp/cert.pem" \
  -out "$tmp/id.p12" -passout "pass:$pass"

security import "$tmp/id.p12" -k "$KEYCHAIN" -P "$pass" -T /usr/bin/codesign
# User trust domain only (no -d): no admin rights, no system keychain change.
security add-trusted-cert -p codeSign -k "$KEYCHAIN" "$tmp/cert.pem"

echo "Created $NAME. Rebuild with 'mise run run' or 'mise run bundle' to sign with it."
