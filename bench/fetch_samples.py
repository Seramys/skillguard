from pathlib import Path
import argparse
import hashlib
import shutil
import sys
import tempfile
import urllib.request
import zipfile

COMMIT = "4ffbf9461ef0505f9ce76a0d3694a18ec33ea531"
URL = f"https://github.com/trailofbits/overtly-malicious-skills/archive/{COMMIT}.zip"
SHA256 = "2a3254e8617b6f8664493ce617464758e910e8a51f6879793f78c0f4e04d21ff"
DEST = Path(__file__).resolve().parent / "samples" / "tob"


def copy_from_local(source: Path) -> None:
    if not source.is_dir():
        raise SystemExit(f"local ToB source not found: {source}")
    if DEST.exists():
        shutil.rmtree(DEST)
    shutil.copytree(source, DEST)
    print(f"copied ToB samples from {source} to {DEST}")


def download() -> None:
    if DEST.exists() and any(DEST.iterdir()):
        print(f"ToB samples already present at {DEST}; use --force to refresh")
        return
    with tempfile.TemporaryDirectory() as tmp:
        archive = Path(tmp) / "tob.zip"
        request = urllib.request.Request(URL, headers={"User-Agent": "skillguard-bench/1.0"})
        with urllib.request.urlopen(request, timeout=90) as response:
            data = response.read()
        digest = hashlib.sha256(data).hexdigest()
        if digest != SHA256:
            raise SystemExit(
                f"ToB archive checksum mismatch: expected {SHA256}, got {digest}. "
                "Refusing to use an unpinned corpus."
            )
        archive.write_bytes(data)
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(tmp)
        roots = [path for path in Path(tmp).iterdir() if path.is_dir()]
        if len(roots) != 1:
            raise SystemExit("unexpected ToB archive layout")
        if DEST.exists():
            shutil.rmtree(DEST)
        shutil.copytree(roots[0], DEST)
    print(f"downloaded ToB samples to {DEST}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch the pinned Trail of Bits malicious-skill corpus")
    parser.add_argument("--from-local", type=Path, help="copy from a local checkout instead of downloading")
    parser.add_argument("--force", action="store_true", help="refresh an existing corpus")
    args = parser.parse_args()
    if args.from_local:
        copy_from_local(args.from_local)
    else:
        if args.force and DEST.exists():
            shutil.rmtree(DEST)
        download()
    return 0


if __name__ == "__main__":
    sys.exit(main())