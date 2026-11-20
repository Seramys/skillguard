from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed
import argparse
import hashlib
import json
import re
import subprocess
import time
import urllib.parse

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "samples" / "real" / "clawhub"
MANIFEST = ROOT / "imported" / "manifest.json"
API = "https://clawhub.ai/api/v1"

EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
TOKEN = re.compile(r"\b(?:sk-[A-Za-z0-9_-]{16,}|ghp_[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{16}|Bearer\s+[A-Za-z0-9._-]{16,})\b")
HOME = re.compile(r"(?:/home/[^/\s]+|/Users/[^/\s]+|[A-Za-z]:\\Users\\[^\\\s]+)")


def get_json(url: str) -> dict:
    last = None
    for attempt in range(3):
        command = [
            "curl",
            "-sS",
            "-L",
            "--connect-timeout",
            "10",
            "--max-time",
            "25",
            "--resolve",
            "clawhub.ai:443:216.150.1.1",
            "-H",
            "User-Agent: skillguard-corpus/1.0",
            url,
        ]
        result = subprocess.run(
            command,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if result.returncode == 0 and result.stdout.strip():
            try:
                return json.loads(result.stdout)
            except json.JSONDecodeError as error:
                last = str(error)
        else:
            last = result.stderr or result.stdout
        time.sleep(1.5 * (attempt + 1))
    raise RuntimeError(f"failed: {url}: {str(last)[:200]}")


def sanitize(text: str) -> tuple:
    redactions = 0
    for pattern, replacement in [(EMAIL, "[redacted-email]"), (TOKEN, "[redacted-token]"), (HOME, "[redacted-home]")]:
        text, count = pattern.subn(replacement, text)
        redactions += count
    return text, redactions


def skill_body(item: dict, detail: dict) -> str:
    description = detail.get("skill", {}).get("description") or item.get("summary") or ""
    if description.lstrip().startswith("---"):
        return description
    license_name = item.get("latestVersion", {}).get("license") or "unknown"
    summary = (item.get("summary") or "").replace('"', "'")
    return (
        f"---\nname: {item['slug']}\ndescription: \"{summary[:200]}\"\n"
        f"version: {item['latestVersion']['version']}\nlicense: {license_name}\n---\n\n"
        + description
    )


def split_for(slug: str) -> str:
    value = int(hashlib.sha256(slug.encode()).hexdigest()[:8], 16) % 3
    return ["dev", "test", "holdout"][value]


def handle(item: dict, delay: float) -> tuple:
    slug = item["slug"]
    encoded = urllib.parse.quote(slug)
    detail = get_json(f"{API}/skills/{encoded}")
    time.sleep(delay)
    version = item["latestVersion"]["version"]
    version_data = get_json(f"{API}/skills/{encoded}/versions/{urllib.parse.quote(version)}")
    security = version_data.get("version", {}).get("security") or {}
    status = (security.get("status") or "").lower()
    if status in {"malicious", "blocked", "infected"}:
        label = "malicious"
    else:
        label = "benign"
    return item, detail, version_data, version, security, status, label


def main() -> int:
    parser = argparse.ArgumentParser(description="Concurrent ClawHub fetch; provisional labels only")
    parser.add_argument("--limit", type=int, default=100)
    parser.add_argument("--page-size", type=int, default=100)
    parser.add_argument("--delay", type=float, default=0.2)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    entries = []
    if MANIFEST.exists():
        entries = json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
    existing = {entry["id"] for entry in entries}

    cursor = None
    fetched = 0
    skipped = 0
    while fetched < args.limit:
        url = f"{API}/skills?limit={args.page_size}"
        if cursor:
            raw_cursor = cursor if isinstance(cursor, str) else json.dumps(cursor, separators=(",", ":"))
            url += "&cursor=" + urllib.parse.quote(raw_cursor, safe="")
        payload = get_json(url)
        items = payload.get("items", [])
        if not items:
            break
        pending = [item for item in items if f"clawhub-{item['slug']}" not in existing]
        with ThreadPoolExecutor(max_workers=args.workers) as pool:
            futures = [pool.submit(handle, item, args.delay) for item in pending[: max(args.limit - fetched, 1)]]
            for future in as_completed(futures):
                try:
                    result = future.result()
                except Exception as error:  # noqa: BLE001
                    print(f"skip: {error}", flush=True)
                    skipped += 1
                    continue
                if result is None:
                    skipped += 1
                    continue
                item, detail, version_data, version, security, status, label = result
                slug = item["slug"]
                sample_id = f"clawhub-{slug}"
                if sample_id in existing:
                    continue
                split = split_for(slug)
                target = OUT / split / label / slug
                body, redactions = sanitize(skill_body(item, detail))
                if target.exists() and not args.force:
                    continue
                target.mkdir(parents=True, exist_ok=True)
                (target / "SKILL.md").write_text(body, encoding="utf-8", newline="\n")
                source = {
                    "source": f"clawhub:{slug}",
                    "source_url": f"https://clawhub.ai/skills/{slug}",
                    "license": item.get("latestVersion", {}).get("license"),
                    "version": version,
                    "security_status": status,
                    "security_scanners": security.get("scanners"),
                    "files": version_data.get("version", {}).get("files", []),
                    "redactions": redactions,
                }
                (target / "SOURCE.json").write_text(
                    json.dumps(source, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
                )
                entries.append(
                    {
                        "id": sample_id,
                        "label": label,
                        "label_quality": "provisional",
                        "label_source": "clawhub_security",
                        "split": split,
                        "path": str(target.relative_to(ROOT.parent)).replace("\\", "/"),
                        "tags": ["real", "clawhub", label],
                    }
                )
                existing.add(sample_id)
                fetched += 1
                print(f"{fetched}/{args.limit} {label} {slug}", flush=True)
        cursor = payload.get("nextCursor")
        if not cursor or fetched >= args.limit:
            break
        MANIFEST.write_text(
            json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )

    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"fetched {fetched}, skipped {skipped}; manifest: {MANIFEST}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())