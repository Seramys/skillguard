from pathlib import Path
import argparse
import json

ROOT = Path(__file__).resolve().parent


def load(path: Path) -> list:
    if not path.exists():
        return []
    return json.loads(path.read_text(encoding="utf-8")).get("samples", [])


def metrics(items: list, threshold: float) -> dict:
    malicious = [item for item in items if item["label"] == "malicious"]
    benign = [item for item in items if item["label"] == "benign"]
    tp = sum(1 for item in malicious if item.get("score", 0) >= threshold)
    fp = sum(1 for item in benign if item.get("score", 0) >= threshold)
    return {
        "threshold": threshold,
        "malicious": len(malicious),
        "benign": len(benign),
        "recall": tp / len(malicious) if malicious else 0.0,
        "fpr": fp / len(benign) if benign else 0.0,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Calibrate LLM scores on dev, evaluate holdout")
    parser.add_argument("--rules", type=Path, default=ROOT / "results" / "latest.json")
    parser.add_argument("--llm", type=Path, default=ROOT / "results" / "llm.json")
    parser.add_argument("--target-fpr", type=float, default=0.05)
    parser.add_argument("--out", type=Path, default=ROOT / "results" / "calibration.json")
    args = parser.parse_args()

    llm = {item["id"]: item for item in load(args.llm)}
    rules = {item["id"]: item for item in load(args.rules)}
    items = []
    for sample_id, item in llm.items():
        rule = rules.get(sample_id, {})
        items.append(
            {
                "id": sample_id,
                "label": item["label"],
                "split": item.get("split", rule.get("split", "base")),
                "score": float(item.get("risk", 0)),
            }
        )

    dev = [item for item in items if item["split"] == "dev"]
    holdout = [item for item in items if item["split"] == "holdout"]
    best = None
    for step in range(0, 101):
        threshold = step / 100
        result = metrics(dev, threshold)
        if result["fpr"] <= args.target_fpr and (best is None or result["recall"] > best["recall"]):
            best = result
    if best is None:
        best = metrics(dev, 1.0)
    evaluation = metrics(holdout, best["threshold"])
    output = {"target_fpr": args.target_fpr, "calibration": best, "holdout": evaluation}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(output, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())