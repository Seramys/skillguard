from pathlib import Path
import argparse
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "samples" / "real" / "cisco"
MANIFEST = ROOT / "imported" / "manifest.json"
COMMIT = "c558c421157c546ec417cf23778ce81a3a8f35fd"
BASE = f"https://raw.githubusercontent.com/cisco-ai-defense/skill-scanner/{COMMIT}"
PATHS = [
    "evals/test_skills/malicious/ascii-smuggling",
    "evals/test_skills/malicious/eicar-test",
    "evals/test_skills/malicious/exfiltrator",
    "evals/test_skills/malicious/flowise-cve-2025-59528",
    "evals/test_skills/malicious/mcp-atlassian-cve-2026-27825",
    "evals/test_skills/malicious/mcpwn-cve-2026-33032",
    "evals/test_skills/malicious/prompt-injection",
    "evals/test_skills/safe/atr-benign-control",
    "evals/test_skills/safe/simple-formatter",
]


def fetch(url: str) -> bytes:
    result = subprocess.run(
        ["curl", "-sS", "-L", "--max-time", "30", url],
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.decode("utf-8", "replace"))
    return result.stdout


def split_for(slug: str) -> str:
    return ["dev", "test", "holdout"][int(hashlib.sha256(slug.encode()).hexdigest()[:8], 16) % 3]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    entries = []
    if MANIFEST.exists():
        entries = json.loads(MANIFEST.read_text(encoding="utf-8")).get("samples", [])
    imported = 0
    for path in PATHS:
        label = "malicious" if "/malicious/" in path else "benign"
        slug = path.rsplit("/", 1)[-1]
        target = OUT / split_for(slug) / label / slug
        if target.exists() and not args.force:
            continue
        target.mkdir(parents=True, exist_ok=True)
        for name in ("SKILL.md",):
            data = fetch(f"{BASE}/{path}/{name}")
            (target / name).write_bytes(data)
        listing = fetch(f"https://api.github.com/repos/cisco-ai-defense/skill-scanner/contents/{path}?ref={COMMIT}")
        for item in json.loads(listing):
            if item["type"] != "file" or item["name"] == "SKILL.md":
                continue
            if item["name"].endswith((".bin", ".exe", ".dll", ".so", ".dylib")):
                continue
            if item["name"].startswith("_expected"):
                continue
            (target / item["name"]).write_bytes(fetch(item["download_url"]))
        source = {
            "source": "cisco-ai-defense/skill-scanner",
            "source_url": f"https://github.com/cisco-ai-defense/skill-scanner/tree/{COMMIT}/{path}",
            "commit": COMMIT,
            "license": "Apache-2.0",
            "label_source": "cisco_test_skills",
            "label_quality": "curated",
        }
        (target / "SOURCE.json").write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
        entry = {
            "id": f"cisco-{label}-{slug}",
            "label": label,
            "label_quality": "curated",
            "label_source": "cisco_test_skills",
            "split": split_for(slug),
            "path": str(target.relative_to(ROOT.parent)).replace("\\", "/"),
            "tags": ["real", "cisco", label],
        }
        entries = [item for item in entries if item["id"] != entry["id"]]
        entries.append(entry)
        imported += 1
        print(f"imported {label}/{slug}")
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"imported {imported} cisco samples")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())