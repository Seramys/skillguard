#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="$ROOT/packages/npm/vendor"

if [ ! -d "$ROOT/dist" ]; then
  echo "dist/ not found; build platform binaries first." >&2
  exit 1
fi

rm -rf "$VENDOR"
mkdir -p "$VENDOR"

count=0
for dir in "$ROOT"/dist/*/; do
  [ -d "$dir" ] || continue
  plat="$(basename "$dir")"
  for exe in skillguard.exe skillguard; do
    if [ -f "$dir/$exe" ]; then
      mkdir -p "$VENDOR/$plat"
      cp "$dir/$exe" "$VENDOR/$plat/$exe"
      chmod +x "$VENDOR/$plat/$exe" 2>/dev/null || true
      echo "npm vendor: $plat/$exe"
      count=$((count + 1))
      break
    fi
  done
done

if [ "$count" -eq 0 ]; then
  echo "no skillguard binaries found under dist/" >&2
  exit 1
fi

echo "bundled $count platform binaries into packages/npm/vendor/"
