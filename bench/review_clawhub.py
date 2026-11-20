from pathlib import Path
import argparse
import json
import re

ROOT = Path(__file__).resolve().parent
IMPORTED = ROOT / "imported" / "manifest.json"
OUT = ROOT / "results" / "clawhub-review"

SIGNALS = [
    ("network", re.compile(r"(curl|wget|netcat|\bnc\b|Invoke-WebRequest|requests\.(get|post)|fetch\()", re.I)),
    ("exec", re.compile(r"(eval|exec|os\.system|subprocess|child_process|bash -c|powershell)", re.I)),
    ("secret", re.compile(r"(\.ssh|\.aws|id_rsa|\.env\b|credential|token|api[_-]?key|password)", re.I)),
    ("obfusc", re.compile(r"(base64\s+-d|atob\(|fromCharCode|\bchr\(|\\x[0-9a-f]{2})", re.I)),
    ("external", re.compile(r"https?://[^\s)\"']+", re.I)),
]


def load_clawhub() -> list:
    if not IMPORTED.exists():
        return []
    samples = json.loads(IMPORTED.read_text(encoding="utf-8")).get("samples", [])
    return [sample for sample in samples if sample.get("label_source") == "clawhub_security"]


def read_body(sample: dict) -> str:
    path = ROOT.parent / sample["path"] / "SKILL.md"
    if not path.is_file():
        return ""
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def export(size: int) -> None:
    samples = load_clawhub()
    if not samples:
        raise SystemExit("no clawhub samples in imported manifest")
    OUT.mkdir(parents=True, exist_ok=True)
    for start in range(0, len(samples), size):
        chunk = samples[start : start + size]
        lines = []
        for sample in chunk:
            body = read_body(sample)
            hits = [name for name, pattern in SIGNALS if pattern.search(body)]
            urls = re.findall(r"https?://[^\s)\"']+", body)[:4]
            flat = " ".join(body.split())
            lines.append(f"### {sample['id']} | {sample.get('label')} | {sample.get('split')} | signals={hits}")
            if urls:
                lines.append("urls: " + ", ".join(urls))
            lines.append(flat[:380])
            lines.append("")
        part = OUT / f"part-{start // size + 1:02d}.txt"
        part.write_text("\n".join(lines), encoding="utf-8")
        print(f"wrote {part} ({len(chunk)} samples)")


def apply(review: Path) -> None:
    samples = json.loads(IMPORTED.read_text(encoding="utf-8")).get("samples", [])
    verdicts = {}
    for line in review.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        item = json.loads(line)
        verdicts[item["id"]] = item
    changed = 0
    for sample in samples:
        item = verdicts.get(sample["id"])
        if not item:
            continue
        sample["label"] = item["label"]
        sample["label_quality"] = "reviewed"
        sample["label_source"] = "clawhub_security+agent_review"
        tags = set(sample.get("tags", []))
        tags.add("reviewed")
        if item.get("note"):
            sample["review_note"] = item["note"]
        sample["tags"] = sorted(tags)
        changed += 1
    IMPORTED.write_text(
        json.dumps({"version": 1, "samples": samples}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"applied {changed} verdicts to {IMPORTED}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Export ClawHub samples for agent review and apply verdicts")
    parser.add_argument("--export", action="store_true")
    parser.add_argument("--size", type=int, default=50)
    parser.add_argument("--apply", type=Path)
    args = parser.parse_args()
    if args.export:
        export(args.size)
    if args.apply:
        apply(args.apply)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())