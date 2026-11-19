#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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

PLAT_DIR="${SKILLGUARD_PLATFORM_DIR:-${PLAT}-${ARCH}}"
SRC="$ROOT/dist/$PLAT_DIR/$EXE"
if [ ! -f "$SRC" ]; then
  echo "missing prebuilt binary: $SRC" >&2
  exit 1
fi

BIN_DIR="$ROOT/packages/python/src/skillguard/_bin"
mkdir -p "$BIN_DIR"
cp "$SRC" "$BIN_DIR/$EXE"
chmod +x "$BIN_DIR/$EXE" 2>/dev/null || true
echo "bundled binary: $BIN_DIR/$EXE"

if python -c "import build" >/dev/null 2>&1; then
  cd "$ROOT/packages/python"
  python -m build
else
  echo "tip: python -m pip install build && scripts/package-python-from-dist.sh" >&2
fi
