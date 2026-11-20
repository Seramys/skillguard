from pathlib import Path
import argparse
import json
import re
import subprocess
import time
import urllib.parse

ROOT = Path(__file__).resolve().parent
MANIFEST = ROOT / "imported" / "manifest.json"
API = "https://clawhub.ai/api/v1"
RESOLVE = "clawhub.ai:443:216.150.1.1"

EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
TOKEN = re.compile(r"\b(?:sk-[A-Za-z0-9_-]{16,}|ghp_[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{16}|Bearer\s+[A-Za-z0-9._-]{16,})\b")
HOME = re.compile(r"(?:/home/[^/\s]+|/Users/[^/\s]+|[A-Za-z]:\\Users\\[^\\\s]+)")


def get_json(url: str) -> dict:
    result = subprocess.run(
        [
            "curl",
            "-sS",
            "-L",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--resolve",
            RESOLVE,
            "-H",
            "User-Agent: skillguard-corpus/1.0",
            url,
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if result.returncode != 0 or not result.stdout.strip():
        raise RuntimeError(result.stderr[:200] or "empty response")
    return json.loads(result.stdout)


def sanitize(text: str) -> tuple:
    redactions = 0
    for pattern, replacement in [(EMAIL, "[redacted-email]"), (TOKEN, "[redacted-token]"), (HOME, "[redacted-home]")]:
        text, count = pattern.subn(replacement, text)
        redactions += count
    return text, redactions


def body_of(item: dict, detail: dict) -> str:
    skill = detail.get("skill", {})
    description = skill.get("description") or item.get("summary") or ""
    if description.lstrip().startswith("---"):
        return description
    version = (item.get("latestVersion") or {}).get("version") or "unknown"
    license_name = (item.get("latestVersion") or {}).get("license") or "unknown"
    summary = (skill.get("summary") or item.get("summary") or "").replace('"', "'")
    display = skill.get("displayName") or item.get("displayName") or item["slug"]
    return (
        f"---\nname: {item['slug']}\ndisplayName: \"{display}\"\n"
        f"description: \"{summary[:300]}\"\nversion: {version}\nlicense: {license_name}\n---\n\n"
        + description
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Replace list-summary ClawHub samples with detail descriptions")
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--delay", type=float, default=0.2)
    args = parser.parse_args()

    samples = json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
    targets = [
        sample
        for sample in samples
        if "list-summary" in sample.get("tags", []) or "list-summary" in str(sample.get("tags", []))
    ]
    if args.limit:
        targets = targets[: args.limit]
    print(f"enriching {len(targets)} list-summary samples", flush=True)

    ok = 0
    failed = 0
    for index, sample in enumerate(targets, 1):
        slug = sample["id"].removeprefix("clawhub-")
        encoded = urllib.parse.quote(slug)
        try:
            detail = get_json(f"{API}/skills/{encoded}")
        except Exception as error:  # noqa: BLE001
            failed += 1
            print(f"{index}/{len(targets)} fail {slug}: {str(error)[:120]}", flush=True)
            time.sleep(args.delay)
            continue
        target = ROOT.parent / sample["path"]
        body, redactions = sanitize(body_of({"slug": slug, **detail.get("skill", {})}, detail))
        try:
            (target / "SKILL.md").write_text(body, encoding="utf-8", newline="\n")
        except OSError as error:
            failed += 1
            print(f"{index}/{len(targets)} write-fail {slug}: {error}", flush=True)
            continue
        source_path = target / "SOURCE.json"
        if source_path.exists():
            try:
                source = json.loads(source_path.read_text(encoding="utf-8"))
                source["import_mode"] = "detail"
                source["redactions"] = redactions
                source_path.write_text(json.dumps(source, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            except OSError:
                pass
        tags = [tag for tag in sample.get("tags", []) if tag != "list-summary"]
        tags.append("detail")
        sample["tags"] = tags
        ok += 1
        if index % 20 == 0:
            print(f"{index}/{len(targets)} ok={ok} failed={failed}", flush=True)
        time.sleep(args.delay)

    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": samples}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"done: ok={ok} failed={failed}; manifest updated", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())