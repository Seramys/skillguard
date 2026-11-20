from pathlib import Path
import json

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "samples" / "real" / "clawhub"
MANIFEST = ROOT / "imported" / "manifest.json"


def main() -> int:
    samples = json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
    by_id = {sample["id"]: sample for sample in samples}
    added = 0
    for source in sorted(OUT.glob("*/*/*/SOURCE.json")):
        meta = json.loads(source.read_text(encoding="utf-8"))
        slug = meta.get("source", "").split(":", 1)[-1]
        if not slug:
            continue
        sample_id = f"clawhub-{slug}"
        if sample_id in by_id:
            continue
        split, label = source.parent.parent.parent.name, source.parent.parent.name
        entry = {
            "id": sample_id,
            "label": label,
            "label_quality": "provisional",
            "label_source": "clawhub_security",
            "split": split,
            "path": str(source.parent.relative_to(ROOT.parent)).replace("\\", "/"),
            "tags": ["real", "clawhub", label],
        }
        by_id[sample_id] = entry
        added += 1
    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": list(by_id.values())}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"added {added} clawhub entries; manifest has {len(by_id)} samples")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())