#!/usr/bin/env bash
# Cross-build all release targets. macOS targets need a macOS host (or osxcross).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

targets=(
  "x86_64-pc-windows-msvc|win32-x64|skillguard.exe"
  "aarch64-pc-windows-msvc|win32-arm64|skillguard.exe"
  "x86_64-unknown-linux-musl|linux-x64|skillguard"
  "aarch64-unknown-linux-musl|linux-arm64|skillguard"
  "x86_64-apple-darwin|darwin-x64|skillguard"
  "aarch64-apple-darwin|darwin-arm64|skillguard"
)

for entry in "${targets[@]}"; do
  IFS='|' read -r target plat exe <<< "$entry"
  rustup target add "$target" >/dev/null 2>&1 || true
  if cargo build --release --target "$target"; then
    mkdir -p "dist/$plat"
    cp "target/$target/release/$exe" "dist/$plat/$exe"
    echo "built dist/$plat/$exe"
  else
    echo "skip $target (toolchain or linker unavailable on this host)" >&2
  fi
done