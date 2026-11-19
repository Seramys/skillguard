from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Iterator, Optional


def _platform_dir() -> str:
    if sys.platform == "win32":
        system = "win32"
    elif sys.platform == "darwin":
        system = "darwin"
    else:
        system = "linux"
    machine = platform.machine().lower()
    arch = "arm64" if machine in ("arm64", "aarch64") else "x64"
    return f"{system}-{arch}"


def _binary_name() -> str:
    return "skillguard.exe" if os.name == "nt" else "skillguard"


def _candidates() -> Iterator[Path]:
    exe = _binary_name()
    env = os.environ.get("SKILLGUARD_BINARY")
    if env:
        yield Path(env)

    package_dir = Path(__file__).resolve().parent
    yield package_dir / "_bin" / exe

    for parent in package_dir.parents:
        yield parent / "target" / "release" / exe
        yield parent / "dist" / _platform_dir() / exe

    found = shutil.which(exe)
    if found:
        yield Path(found)


def _find_binary() -> Optional[Path]:
    self_path = Path(sys.argv[0]).resolve()
    for candidate in _candidates():
        try:
            if candidate.is_file():
                resolved = candidate.resolve()
                if resolved != self_path:
                    return resolved
        except OSError:
            continue
    return None


def main() -> int:
    binary = _find_binary()
    if binary is None:
        print(
            "skillguard: native binary not found.\n"
            "Set SKILLGUARD_BINARY, or bundle one with scripts/package-python.sh "
            "before building the wheel.",
            file=sys.stderr,
        )
        return 2

    args = [str(binary), *sys.argv[1:]]
    if os.name == "nt":
        return subprocess.call(args)
    os.execv(str(binary), args)
    return 2