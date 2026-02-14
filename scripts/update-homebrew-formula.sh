#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <version-without-v> <sha256>" >&2
  exit 1
fi

VERSION="$1"
SHA256="$2"

sed \
  -e "s/__VERSION__/${VERSION}/g" \
  -e "s/__SHA256__/${SHA256}/g" \
  packaging/homebrew/presence-bridge.rb.tmpl > packaging/homebrew/presence-bridge.rb

echo "wrote packaging/homebrew/presence-bridge.rb"
