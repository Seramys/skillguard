from pathlib import Path
import argparse
import json
import shutil

ROOT = Path(__file__).resolve().parent
DEST = ROOT / "samples" / "real"
MANIFEST = ROOT / "imported" / "manifest.json"


def main() -> int:
    parser = argparse.ArgumentParser(description="Import locally provided SkillHub/ClawHub samples; no scraping")
    parser.add_argument("--source", type=Path, required=True, help="directory containing Skill folders")
    parser.add_argument("--label", choices=["malicious", "benign"], required=True)
    parser.add_argument("--split", choices=["dev", "test", "holdout"], default="holdout")
    parser.add_argument("--prefix", default="real")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    if not args.source.is_dir():
        raise SystemExit(f"source not found: {args.source}")
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    entries = []
    if MANIFEST.exists():
        entries = json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])

    imported = 0
    for child in sorted(args.source.iterdir()):
        if not child.is_dir() or not (child / "SKILL.md").is_file():
            continue
        target = DEST / args.split / args.label / child.name
        if target.exists() and not args.force:
            print(f"skip existing {target}")
            continue
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(child, target)
        entry = {
            "id": f"{args.prefix}-{args.label}-{args.split}-{child.name}",
            "label": args.label,
            "split": args.split,
            "path": str(target.relative_to(ROOT.parent)).replace("\\", "/"),
            "tags": ["real", args.prefix],
        }
        entries = [item for item in entries if item["id"] != entry["id"]]
        entries.append(entry)
        imported += 1

    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"imported {imported} samples; manifest: {MANIFEST}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())