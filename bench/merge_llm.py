from pathlib import Path
import json

ROOT = Path(__file__).resolve().parent
BASE = ROOT / "results" / "llm-candidates.json"
RETRY = ROOT / "results" / "llm-malicious-retry.json"
OUT = ROOT / "results" / "llm-candidates-merged.json"


def main() -> int:
    base = json.loads(BASE.read_text(encoding="utf-8"))
    retry = json.loads(RETRY.read_text(encoding="utf-8"))
    by_id = {row["id"]: row for row in base["samples"]}
    for row in retry["samples"]:
        by_id[row["id"]] = row
    merged = list(by_id.values())
    OUT.write_text(
        json.dumps({"model": base["model"], "endpoint": base["endpoint"], "samples": merged}, ensure_ascii=False, indent=2)
        + "\n",
        encoding="utf-8",
    )
    print(f"merged {len(merged)} rows -> {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())