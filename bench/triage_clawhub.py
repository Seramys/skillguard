from pathlib import Path
import json
import re

ROOT = Path(__file__).resolve().parent
MANIFEST = ROOT / "imported" / "manifest.json"
OUT = ROOT / "results" / "clawhub-review" / "risk-list.txt"

RULES = [
    ("pipe-to-shell", re.compile(r"(curl|wget)[^\n|]{0,200}\|\s*(bash|sh|zsh|powershell)", re.I)),
    ("base64-exec", re.compile(r"(base64\s*-d[^\n]{0,80}(bash|sh|eval)|\beval\s*\(\s*(atob|base64)|fromCharCode)", re.I)),
    ("credential-access", re.compile(r"(\.ssh/|id_rsa|\.aws/credentials|\.gnupg|keychain|Login Data|Cookies)", re.I)),
    ("destructive", re.compile(r"(rm\s+-rf\s+/|del\s+/f\s+/s|format\s+[a-z]:|mkfs)", re.I)),
    ("prompt-injection", re.compile(r"(ignore (all )?previous instructions|忽略(之前|以上|前面)|不要告诉用户|do not tell the user|bypass (the )?(approval|safety))", re.I)),
    ("encoded-powershell", re.compile(r"powershell[^\n]{0,40}-(enc|e)\b", re.I)),
    ("sms-or-crypto", re.compile(r"(send (sms|text)|crypto wallet|seed phrase|私钥|助记词)", re.I)),
]


def main() -> int:
    samples = [
        sample
        for sample in json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
        if sample.get("label_source", "").startswith("clawhub")
    ]
    findings = []
    for sample in samples:
        path = ROOT.parent / sample["path"] / "SKILL.md"
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        hits = [name for name, pattern in RULES if pattern.search(text)]
        if hits:
            findings.append((sample["id"], hits, " ".join(text.split())[:600]))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    lines = [f"scanned {len(samples)}, flagged {len(findings)}", ""]
    for sample_id, hits, flat in findings:
        lines.append(f"### {sample_id} | {','.join(hits)}")
        lines.append(flat)
        lines.append("")
    OUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"scanned {len(samples)}, flagged {len(findings)} -> {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())