from pathlib import Path
import json

ROOT = Path(__file__).resolve().parent
SAMPLES = ROOT / "samples" / "real" / "clawhub"
MANIFEST = ROOT / "imported" / "manifest.json"


def main() -> int:
    entries = []
    for source in sorted(SAMPLES.rglob("SOURCE.json")):
        target = source.parent
        relative = target.relative_to(ROOT.parent).as_posix()
        parts = relative.split("/")
        split, label, slug = parts[-3], parts[-2], parts[-1]
        meta = json.loads(source.read_text(encoding="utf-8"))
        entries.append(
            {
                "id": f"clawhub-{slug}",
                "label": label,
                "label_quality": "provisional",
                "label_source": "clawhub_security",
                "split": split,
                "path": relative,
                "tags": ["real", "clawhub", label],
                "security_status": meta.get("security_status"),
                "license": meta.get("license"),
                "source_url": meta.get("source_url"),
            }
        )
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"indexed {len(entries)} samples")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())