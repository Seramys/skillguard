from pathlib import Path
import argparse
import json

ROOT = Path(__file__).resolve().parent


def load(path: Path) -> dict:
    if not path.exists():
        return {}
    return {item["id"]: item for item in json.loads(path.read_text(encoding="utf-8")).get("samples", [])}


def metrics(items: list, detector) -> dict:
    malicious = [item for item in items if item["label"] == "malicious"]
    benign = [item for item in items if item["label"] == "benign"]
    tp = sum(1 for item in malicious if detector(item))
    fp = sum(1 for item in benign if detector(item))
    return {
        "malicious": len(malicious),
        "benign": len(benign),
        "recall": tp / len(malicious) if malicious else 0.0,
        "fpr": fp / len(benign) if benign else 0.0,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Ablation: rules-only vs LLM-only vs combined")
    parser.add_argument("--rules", type=Path, default=ROOT / "results" / "latest.json")
    parser.add_argument("--llm", type=Path, default=ROOT / "results" / "llm.json")
    parser.add_argument("--llm-threshold", type=float, default=0.6)
    parser.add_argument("--candidates", type=Path, help="JSONL from candidates.py; restrict metrics to these ids")
    parser.add_argument("--out", type=Path, default=ROOT / "results" / "ablation.json")
    args = parser.parse_args()

    rules = load(args.rules)
    llm = load(args.llm)
    wanted = None
    if args.candidates:
        wanted = {
            json.loads(line)["id"]
            for line in args.candidates.read_text(encoding="utf-8").splitlines()
            if line.strip()
        }
    items = []
    for sample_id, rule in rules.items():
        if wanted is not None and sample_id not in wanted:
            continue
        model = llm.get(sample_id, {})
        items.append(
            {
                "id": sample_id,
                "label": rule["label"],
                "rules": int(rule.get("rank", 0)) >= 3,
                "llm": float(model.get("risk", 0)) >= args.llm_threshold,
            }
        )
    results = {
        "rules_only": metrics(items, lambda item: item["rules"]),
        "llm_only": metrics(items, lambda item: item["llm"]),
        "rules_or_llm": metrics(items, lambda item: item["rules"] or item["llm"]),
        "rules_and_llm": metrics(items, lambda item: item["rules"] and item["llm"]),
        "llm_threshold": args.llm_threshold,
        "samples": len(items),
        "queue": str(args.candidates) if args.candidates else "all",
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(results, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())