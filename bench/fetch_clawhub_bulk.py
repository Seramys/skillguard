from pathlib import Path
import argparse
import hashlib
import json
import re
import subprocess
import urllib.parse

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "samples" / "real" / "clawhub"
MANIFEST = ROOT / "imported" / "manifest.json"
API = "https://clawhub.ai/api/v1"

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
            "clawhub.ai:443:216.150.1.1",
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


def split_for(slug: str) -> str:
    value = int(hashlib.sha256(slug.encode()).hexdigest()[:8], 16) % 3
    return ["dev", "test", "holdout"][value]


def body_of(item: dict) -> str:
    slug = item["slug"]
    summary = (item.get("summary") or "").replace('"', "'")
    latest = item.get("latestVersion") or {}
    version = latest.get("version") or "unknown"
    license_name = latest.get("license") or "unknown"
    display = item.get("displayName") or slug
    return (
        f"---\nname: {slug}\ndisplayName: \"{display}\"\n"
        f"description: \"{summary[:300]}\"\nversion: {version}\nlicense: {license_name}\n---\n\n"
        f"# {display}\n\n{summary}\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Bulk import ClawHub listings using the list API summary field")
    parser.add_argument("--limit", type=int, default=500)
    parser.add_argument("--page-size", type=int, default=100)
    args = parser.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)
    entries = []
    if MANIFEST.exists():
        entries = json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
    existing = {entry["id"] for entry in entries}

    cursor = None
    added = 0
    pages = 0
    while added < args.limit and pages < 30:
        url = f"{API}/skills?limit={args.page_size}"
        if cursor:
            raw = cursor if isinstance(cursor, str) else json.dumps(cursor, separators=(",", ":"))
            url += "&cursor=" + urllib.parse.quote(raw, safe="")
        payload = get_json(url)
        pages += 1
        items = payload.get("items", [])
        if not items:
            break
        for item in items:
            if added >= args.limit:
                break
            slug = item["slug"]
            sample_id = f"clawhub-{slug}"
            if sample_id in existing:
                continue
            body, redactions = sanitize(body_of(item))
            split = split_for(slug)
            target = OUT / split / "benign" / slug
            target.mkdir(parents=True, exist_ok=True)
            (target / "SKILL.md").write_text(body, encoding="utf-8", newline="\n")
            source = {
                "source": f"clawhub:{slug}",
                "source_url": f"https://clawhub.ai/skills/{slug}",
                "license": (item.get("latestVersion") or {}).get("license"),
                "version": (item.get("latestVersion") or {}).get("version"),
                "security_status": "",
                "import_mode": "list_summary",
                "redactions": redactions,
            }
            (target / "SOURCE.json").write_text(
                json.dumps(source, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
            )
            entries.append(
                {
                    "id": sample_id,
                    "label": "benign",
                    "label_quality": "provisional",
                    "label_source": "clawhub_security",
                    "split": split,
                    "path": str(target.relative_to(ROOT.parent)).replace("\\", "/"),
                    "tags": ["real", "clawhub", "list-summary"],
                }
            )
            existing.add(sample_id)
            added += 1
        cursor = payload.get("nextCursor")
        if not cursor:
            break
        print(f"page {pages}: added {added}", flush=True)

    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"added {added} list-derived samples over {pages} pages", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())