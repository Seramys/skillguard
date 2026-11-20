from pathlib import Path
import collections
import hashlib
import json

ROOT = Path(__file__).resolve().parent
MANIFESTS = [
    "manifest.json",
    "samples/generated/manifest.json",
    "imported/manifest.json",
    "samples/adversarial/manifest.json",
]


def main() -> int:
    samples = []
    for rel in MANIFESTS:
        path = ROOT / rel
        if not path.exists():
            continue
        for sample in json.loads(path.read_text(encoding="utf-8")).get("samples", []):
            sample["_manifest"] = rel
            samples.append(sample)

    print("samples", len(samples))
    print("label", dict(collections.Counter(s.get("label") for s in samples)))
    print("label_source", dict(collections.Counter(s.get("label_source", "synthetic") for s in samples)))
    print("split", dict(collections.Counter(s.get("split", "base") for s in samples)))
    print("quality", dict(collections.Counter(s.get("label_quality", "unknown") for s in samples)))
    print("by split x label:")
    for split in sorted({s.get("split", "base") for s in samples}):
        counts = collections.Counter(s.get("label") for s in samples if s.get("split", "base") == split)
        print(f"  {split}: {dict(counts)}")

    hashes = collections.defaultdict(list)
    unreadable = 0
    for sample in samples:
        path = ROOT.parent / sample["path"] / "SKILL.md"
        if not path.is_file():
            continue
        try:
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError:
            unreadable += 1
            continue
        hashes[digest].append(sample["id"])
    print(f"unreadable skill texts (sandbox-protected): {unreadable}")
    duplicates = [ids for ids in hashes.values() if len(ids) > 1]
    print(f"unique skill texts {len(hashes)}, duplicate groups {len(duplicates)}, extra copies {sum(len(ids) - 1 for ids in duplicates)}")
    for ids in duplicates[:20]:
        print("  dup:", ", ".join(ids))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())