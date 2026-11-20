from pathlib import Path
import argparse
import subprocess
import time

ROOT = Path(__file__).resolve().parent
DEFAULT_OUT = ROOT.parent / ".cache" / "models" / "qwen2.5-7b-instruct-q4_k_m"
MODEL = "Qwen/Qwen2.5-7B-Instruct-GGUF"
FILES = {
    "qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf": 3993201344,
    "qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf": 689872288,
}
SOURCES = [
    "https://modelscope.cn/models/{model}/resolve/master/{name}",
    "https://hf-mirror.com/{model}/resolve/main/{name}",
    "https://huggingface.co/{model}/resolve/main/{name}",
]
CHUNK = 64 * 1024 * 1024
RESOLVE_IPS = {"modelscope.cn": "39.99.133.195"}


def fetch(url: str, target: Path, total: int) -> None:
    if target.exists() and target.stat().st_size > total:
        target.unlink()
    pos = target.stat().st_size if target.exists() else 0
    failures = 0
    with target.open("ab") as out:
        while pos < total:
            end = min(pos + CHUNK - 1, total - 1)
            part = target.parent / (target.name + ".part")
            if part.exists():
                part.unlink()
            host = url.split("/")[2]
            command = [
                "curl",
                "-sSL",
                "--connect-timeout",
                "20",
                "--max-time",
                "300",
                "--retry",
                "8",
                "--retry-all-errors",
                "--retry-delay",
                "2",
                "-r",
                f"{pos}-{end}",
                "-o",
                str(part),
                url,
            ]
            if RESOLVE_IPS.get(host):
                command[1:1] = ["--resolve", f"{host}:443:{RESOLVE_IPS[host]}"]
            subprocess.run(command)
            got = part.stat().st_size if part.exists() else 0
            expected = end - pos + 1
            if got == total:
                part.replace(target)
                print(f"{target.name} complete via full-body response", flush=True)
                return
            if got != expected:
                failures += 1
                print(f"chunk {pos}-{end} got {got}, retry {failures}", flush=True)
                if failures > 30:
                    raise RuntimeError(f"too many chunk failures for {target.name}")
                time.sleep(3)
                continue
            with part.open("rb") as source:
                out.write(source.read())
            out.flush()
            pos += got
            part.unlink()
            failures = 0
            print(f"{target.name} {pos}/{total} {pos * 100 // total}%", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Chunked, resumable GGUF download via curl range requests")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for name, total in FILES.items():
        target = args.out / name
        if target.exists() and target.stat().st_size == total:
            print(f"skip {name} (already {total} bytes)", flush=True)
            continue
        for template in SOURCES:
            url = template.format(model=MODEL, name=name)
            print(f"GET {url}", flush=True)
            try:
                fetch(url, target, total)
                break
            except Exception as error:  # noqa: BLE001
                print(f"source failed: {error}", flush=True)
        else:
            raise SystemExit(f"all sources failed for {name}")
    print(f"done: {args.out}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())