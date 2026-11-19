#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
bash "$ROOT/scripts/build.sh"

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

BIN_DIR="$ROOT/packages/python/src/skillguard/_bin"
mkdir -p "$BIN_DIR"
cp "$ROOT/dist/${PLAT}-${ARCH}/$EXE" "$BIN_DIR/$EXE"
echo "bundled binary: $BIN_DIR/$EXE"

if python -c "import build" >/dev/null 2>&1; then
  cd "$ROOT/packages/python"
  python -m build
else
  echo "tip: python -m pip install build && scripts/package-python.sh to create sdist/wheel"
fi