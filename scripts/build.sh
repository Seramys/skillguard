#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release

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

DEST="$ROOT/dist/${PLAT}-${ARCH}"
mkdir -p "$DEST"
cp "$ROOT/target/release/$EXE" "$DEST/$EXE"
echo "single binary: $DEST/$EXE"