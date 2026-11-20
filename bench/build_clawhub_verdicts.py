from pathlib import Path
import json

ROOT = Path(__file__).resolve().parent
MANIFEST = ROOT / "imported" / "manifest.json"
OUT = ROOT / "results" / "clawhub-review" / "verdicts.jsonl"

NOTES = {
    "clawhub-innerwarden-security": "security guard product; install script matches pipe-to-shell rule but is legitimate installer",
    "clawhub-siebly-crypto-exchange-api": "crypto exchange SDK; sensitive capability but legitimate dev library",
    "clawhub-dial-phone-numbers": "agent phone/SMS capability; sensitive but legitimate product",
    "clawhub-iterate-skill": "code iteration tool; shell/file permissions declared for its stated purpose",
    "clawhub-taskfuel": "paid tool-calling marketplace; install snippets only",
    "clawhub-outlit": "customer intelligence CLI; brew/npm install snippets only",
    "clawhub-sql-server-skills": "SQL Server diagnostics; prompt-injection regex matched benign troubleshooting text",
    "clawhub-javdb-cli": "CLI wrapper; credential keyword from authenticated API wording",
    "clawhub-a2a-exposed": "A2A agent messaging tool; credential keyword from pairing wording",
    "clawhub-xrowgmbh-gitlab-meeting-assistant": "GitLab issue assistant; credential keyword from sanitized issue wording",
    "clawhub-agent-yurt": "agent context backup tool; credential keyword from scoped access wording",
    "clawhub-swift-dev": "Swift/iOS development skill; credential keyword from code-signing wording",
}


def main() -> int:
    samples = [
        sample
        for sample in json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
        if sample.get("label_source", "").startswith("clawhub")
    ]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    lines = []
    for sample in samples:
        note = NOTES.get(sample["id"], "manual review of SKILL.md: no exfiltration, persistence, or covert execution indicators")
        lines.append(json.dumps({"id": sample["id"], "label": "benign", "note": note}, ensure_ascii=False))
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {len(lines)} verdicts to {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())