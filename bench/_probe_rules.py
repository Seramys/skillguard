import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
D = json.loads((ROOT / "results" / "latest.json").read_text(encoding="utf-8"))
fp = set(D.get("false_positives", []))
rules = json.loads((ROOT.parent / "rules" / "core.json").read_text(encoding="utf-8"))
rules = rules["rules"] if isinstance(rules, dict) else rules
by_id = {rule["id"]: rule for rule in rules}


def pattern_of(rule_id):
    match = by_id[rule_id]["match"]
    return re.compile(match["pattern"])


for sample in D["samples"]:
    if sample["id"] not in fp:
        continue
    text = ""
    try:
        text = (ROOT.parent / sample["path"] / "SKILL.md").read_text(encoding="utf-8", errors="replace")
    except OSError:
        text = ""
    print("===", sample["id"], sample.get("rules"))
    for rule_id in sample.get("rules", []):
        if rule_id not in by_id or by_id[rule_id]["match"].get("type") != "regex":
            continue
        pattern = pattern_of(rule_id)
        for line in text.splitlines():
            if pattern.search(line):
                print(f"  {rule_id}: {line.strip()[:220]}")
                break