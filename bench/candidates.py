from pathlib import Path
import argparse
import json

ROOT = Path(__file__).resolve().parent


def main() -> int:
    parser = argparse.ArgumentParser(description="Build the LLM candidate queue from medium-confidence findings")
    parser.add_argument("--rules", type=Path, default=ROOT / "results" / "latest.json")
    parser.add_argument("--out", type=Path, default=ROOT / "results" / "candidates.jsonl")
    args = parser.parse_args()
    payload = json.loads(args.rules.read_text(encoding="utf-8"))
    rows = []
    for sample in payload.get("samples", []):
        if not sample.get("ok"):
            continue
        if sample.get("rank", 0) == 2 or sample.get("max_severity") == "medium":
            rows.append(
                {
                    "id": sample["id"],
                    "label": sample["label"],
                    "split": sample.get("split", "base"),
                    "path": sample["path"],
                    "rules": sample.get("rules", []),
                    "reason": "medium-only candidate for semantic reranking",
                }
            )
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(json.dumps(row, ensure_ascii=False) for row in rows) + ("\n" if rows else ""), encoding="utf-8")
    print(f"wrote {len(rows)} candidates to {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())