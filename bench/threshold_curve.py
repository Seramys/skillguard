from pathlib import Path
import json

ROOT = Path(__file__).resolve().parent
LATEST = ROOT / "results" / "latest.json"
RANK = {"none": 0, "low": 1, "medium": 2, "high": 3, "critical": 4}


def main() -> int:
    payload = json.loads(LATEST.read_text(encoding="utf-8"))
    samples = [item for item in payload.get("samples", []) if item.get("ok")]
    rows = []
    for name, threshold in RANK.items():
        if threshold == 0:
            continue
        malicious = [item for item in samples if item["label"] == "malicious"]
        benign = [item for item in samples if item["label"] == "benign"]
        tp = sum(1 for item in malicious if item.get("rank", 0) >= threshold)
        fp = sum(1 for item in benign if item.get("rank", 0) >= threshold)
        rows.append(
            {
                "threshold": name,
                "recall": round(tp / len(malicious), 4) if malicious else 0,
                "fpr": round(fp / len(benign), 4) if benign else 0,
                "tp": tp,
                "fp": fp,
                "malicious": len(malicious),
                "benign": len(benign),
            }
        )
    out = ROOT / "results" / "threshold-curve.json"
    out.write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
    for row in rows:
        print(row)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())