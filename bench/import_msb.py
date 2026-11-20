from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parent
DEST = ROOT / "samples" / "real" / "msb"
MANIFEST = ROOT / "imported" / "manifest.json"
LABEL_FIELDS = ("label", "y", "is_malicious", "malicious")
TEXT_FIELDS = ("skill_text", "public_skill_text", "text", "content")


def load_rows(path: Path) -> list:
    if path.suffix in {".jsonl", ".ndjson"}:
        return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if path.suffix == ".json":
        payload = json.loads(path.read_text(encoding="utf-8"))
        return payload if isinstance(payload, list) else payload.get("data", [])
    if path.suffix == ".parquet":
        import pyarrow.parquet as pq

        return pq.read_table(path).to_pylist()
    raise SystemExit(f"unsupported input: {path}")


def norm_label(row: dict) -> str:
    for field in LABEL_FIELDS:
        if field not in row:
            continue
        value = row[field]
        if isinstance(value, bool):
            return "malicious" if value else "benign"
        text = str(value).strip().lower()
        if text in {"1", "true", "malicious", "mal"}:
            return "malicious"
        if text in {"0", "false", "benign", "safe"}:
            return "benign"
    raise SystemExit(f"no usable label in row: {sorted(row)[:8]}")


def text_of(row: dict) -> str:
    for field in TEXT_FIELDS:
        value = row.get(field)
        if value:
            return str(value)
    raise SystemExit(f"no skill text in row: {sorted(row)[:8]}")


def split_for(key: str) -> str:
    return ["dev", "test", "holdout"][int(hashlib.sha256(key.encode()).hexdigest()[:8], 16) % 3]


def main() -> int:
    parser = argparse.ArgumentParser(description="Import MaliciousSkillBench rows as SKILL.md directories")
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--label", choices=["malicious", "benign"], help="only import this label")
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--prefix", default="msb")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    rows = load_rows(args.input)
    entries = []
    if MANIFEST.exists():
        entries = json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
    seen_ids = {entry["id"] for entry in entries}
    seen_hashes = {}
    for entry in entries:
        path = ROOT.parent / entry["path"] / "SKILL.md"
        if path.is_file():
            seen_hashes[hashlib.sha256(path.read_bytes()).hexdigest()] = entry["id"]

    imported = 0
    skipped = 0
    for index, row in enumerate(rows):
        label = norm_label(row)
        if args.label and label != args.label:
            continue
        if args.limit and imported >= args.limit:
            break
        text = text_of(row)
        if not text.strip():
            skipped += 1
            continue
        digest = hashlib.sha256(text.encode()).hexdigest()
        if digest in seen_hashes:
            skipped += 1
            continue
        raw_id = str(
            row.get("benchmark_id") or row.get("id") or row.get("identity") or row.get("uid") or f"row{index}"
        )
        slug = "".join(ch if ch.isalnum() or ch in "-_" else "-" for ch in raw_id)[:80]
        split = split_for(digest)
        sample_id = f"{args.prefix}-{label}-{split}-{slug}"
        if sample_id in seen_ids:
            skipped += 1
            continue
        target = DEST / split / label / slug
        if target.exists() and not args.force:
            skipped += 1
            continue
        target.mkdir(parents=True, exist_ok=True)
        (target / "SKILL.md").write_text(text, encoding="utf-8", newline="\n")
        source = {
            "source": "protectskills/MaliciousSkillBench",
            "source_id": raw_id,
            "benchmark_id": row.get("benchmark_id"),
            "source_registry": row.get("source") or row.get("source_id") or row.get("registry"),
            "label_source": "malicious_skill_bench",
            "label_quality": "curated",
            "attack_category": row.get("attack_category") or row.get("category"),
            "sha256": digest,
        }
        (target / "SOURCE.json").write_text(
            json.dumps(source, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        entries.append(
            {
                "id": sample_id,
                "label": label,
                "label_quality": "curated",
                "label_source": "malicious_skill_bench",
                "split": split,
                "path": str(target.relative_to(ROOT.parent)).replace("\\", "/"),
                "tags": ["real", "msb", label],
            }
        )
        seen_ids.add(sample_id)
        seen_hashes[digest] = sample_id
        imported += 1

    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"imported {imported}, skipped {skipped}; manifest has {len(entries)} samples")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())