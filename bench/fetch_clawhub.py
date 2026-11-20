from pathlib import Path
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
    for attempt in range(4):
        result = subprocess.run(
            ["curl", "-sS", "-L", "--max-time", "60", "-H", "User-Agent: skillguard-corpus/1.0", url],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if result.returncode == 0 and result.stdout.strip():
            return json.loads(result.stdout)
        last = result.stderr or result.stdout
        time.sleep(2 * (attempt + 1))
    raise RuntimeError(f"failed: {url}: {last}")


def sanitize(text: str) -> tuple[str, int]:
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


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch public ClawHub SKILL.md metadata; provisional labels only")
    parser.add_argument("--limit", type=int, default=100)
    parser.add_argument("--page-size", type=int, default=50)
    parser.add_argument("--delay", type=float, default=0.8)
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
        for item in items:
            if fetched >= args.limit:
                break
            slug = item["slug"]
            sample_id = f"clawhub-{slug}"
            if sample_id in existing:
                continue
            try:
                detail = get_json(f"{API}/skills/{urllib.parse.quote(slug)}")
                time.sleep(args.delay)
                version = item["latestVersion"]["version"]
                version_data = get_json(f"{API}/skills/{urllib.parse.quote(slug)}/versions/{urllib.parse.quote(version)}")
                time.sleep(args.delay)
            except Exception as error:  # noqa: BLE001
                print(f"skip {slug}: {error}", flush=True)
                skipped += 1
                continue

            security = version_data.get("version", {}).get("security") or {}
            status = (security.get("status") or "").lower()
            if status in {"clean", "safe", "benign"}:
                label = "benign"
            elif status in {"malicious", "blocked", "infected"}:
                label = "malicious"
            else:
                skipped += 1
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
        if not cursor:
            break
        time.sleep(args.delay)

    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"fetched {fetched}, skipped {skipped}; manifest: {MANIFEST}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())