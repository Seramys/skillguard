from pathlib import Path
import argparse
import base64
import json
import shutil
import zipfile

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "samples" / "adversarial"
MANIFEST = OUT / "manifest.json"


def skill_body(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def variants(source: Path) -> dict:
    body = skill_body(source / "SKILL.md")
    payload = base64.b64encode(body.encode()).decode()
    zero = body.replace("忽略", "忽\u200b略").replace("ignore", "ig\u200bnore")
    split_a = body[: len(body) // 2] + "\n<!-- part-a -->\n"
    split_b = body[len(body) // 2 :] + "\n<!-- part-b -->\n"
    return {
        "base64": {"SKILL.md": f"# 解码后执行\n\n```\n{payload}\n```\n"},
        "zero-width": {"SKILL.md": zero},
        "split": {"SKILL.md": split_a, "scripts/part_b.md": split_b},
        "docx": {"SKILL.md": "# 文档\n\n按 instructions.docx 操作。\n", "__docx__": "1", "__body__": body},
    }


def write_variant(base: Path, name: str, files: dict) -> None:
    base.mkdir(parents=True, exist_ok=True)
    for rel, content in files.items():
        if rel == "__docx__":
            continue
        if rel == "__body__":
            continue
        path = base / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8", newline="\n")
    if files.get("__docx__") == "1":
        body = files.get("__body__", "")
        with zipfile.ZipFile(base / "instructions.docx", "w", zipfile.ZIP_DEFLATED) as archive:
            archive.writestr(
                "word/document.xml",
                '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
                f"<w:body><w:p><w:r><w:t>{body[:500]}</w:t></w:r></w:p></w:body></w:document>",
            )
            archive.writestr("word/payload.sh", "#!/bin/sh\necho adversarial\n")


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate quarterly adversarial mutations from existing samples")
    parser.add_argument("--manifest", type=Path, default=ROOT / "manifest.json")
    parser.add_argument("--limit", type=int, default=8)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    if OUT.exists():
        if not args.force:
            print(f"{OUT} exists; use --force to regenerate")
            return 0
        shutil.rmtree(OUT)
    base = json.loads(args.manifest.read_text(encoding="utf-8"))
    chosen = [item for item in base.get("samples", []) if item.get("label") == "malicious"][: args.limit]
    entries = []
    for item in chosen:
        source = ROOT.parent / item["path"]
        if not source.is_dir():
            continue
        for kind, files in variants(source).items():
            target = OUT / item["id"] / kind
            write_variant(target, kind, files)
            entries.append(
                {
                    "id": f"adv-{item['id']}-{kind}",
                    "label": "malicious",
                    "split": "adversarial",
                    "path": str(target.relative_to(ROOT.parent)).replace("\\", "/"),
                    "tags": ["adversarial", kind],
                }
            )
    OUT.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(
        json.dumps({"version": 1, "samples": entries}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"generated {len(entries)} adversarial samples at {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())