from pathlib import Path
import argparse
import json

ROOT = Path(__file__).resolve().parent
RANK = {"none": 0, "low": 1, "medium": 2, "high": 3, "critical": 4}


def load_json(path: Path) -> dict:
    return {item["id"]: item for item in json.loads(path.read_text(encoding="utf-8")).get("samples", [])}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Semantic rerank for medium-only candidates; never part of the main scan path"
    )
    parser.add_argument("--candidates", type=Path, default=ROOT / "results" / "candidates.jsonl")
    parser.add_argument("--rules", type=Path, default=ROOT / "results" / "latest.json")
    parser.add_argument("--llm", type=Path, default=ROOT / "results" / "llm-candidates-schema.json")
    parser.add_argument("--llm-threshold", type=float, default=0.6)
    parser.add_argument("--out", type=Path, default=ROOT / "results" / "rerank.json")
    args = parser.parse_args()

    queue = [json.loads(line) for line in args.candidates.read_text(encoding="utf-8").splitlines() if line.strip()]
    rules = load_json(args.rules)
    llm = load_json(args.llm)

    decisions = []
    for item in queue:
        sample_id = item["id"]
        rule = rules.get(sample_id, {})
        model = llm.get(sample_id, {})
        rule_rank = int(rule.get("rank", 0))
        risk = float(model.get("risk", 0.0))
        verified = rule_rank >= 3
        escalate = (not verified) and risk >= args.llm_threshold
        decisions.append(
            {
                "id": sample_id,
                "label": item["label"],
                "split": item.get("split", "base"),
                "rule_rank": rule_rank,
                "llm_risk": risk,
                "llm_labels": model.get("labels", []),
                "decision": "verified" if verified else ("escalate" if escalate else "keep"),
            }
        )

    malicious = [item for item in decisions if item["label"] == "malicious"]
    benign = [item for item in decisions if item["label"] == "benign"]
    escalated_malicious = [item for item in malicious if item["decision"] != "keep"]
    escalated_benign = [item for item in benign if item["decision"] != "keep"]

    summary = {
        "mode": "rerank-only",
        "queue": str(args.candidates),
        "samples": len(decisions),
        "llm_threshold": args.llm_threshold,
        "malicious": len(malicious),
        "benign": len(benign),
        "recall": round(len(escalated_malicious) / len(malicious), 4) if malicious else 0.0,
        "fpr": round(len(escalated_benign) / len(benign), 4) if benign else 0.0,
        "escalated_ids": [item["id"] for item in decisions if item["decision"] == "escalate"],
        "decisions": decisions,
    }
    args.out.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {key: summary[key] for key in ("mode", "samples", "malicious", "benign", "recall", "fpr")},
            ensure_ascii=False,
        )
    )
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())