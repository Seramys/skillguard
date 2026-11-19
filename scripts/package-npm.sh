#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/build.sh"

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) PLAT="win32"; EXE="skillguard.exe" ;;
  Darwin*) PLAT="darwin"; EXE="skillguard" ;;
  *) PLAT="linux"; EXE="skillguard" ;;
esac
case "$(uname -m)" in
  x86_64|amd64) ARCH="x64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *) ARCH="$(uname -m)" ;;
esac

DEST="$ROOT/packages/npm/vendor/${PLAT}-${ARCH}"
mkdir -p "$DEST"
cp "$ROOT/dist/${PLAT}-${ARCH}/$EXE" "$DEST/$EXE"
chmod +x "$ROOT/packages/npm/bin/skillguard.js"
echo "npm launcher ready: node packages/npm/bin/skillguard.js --version"
echo "publish with: cd packages/npm && npm pack"