# Detection benchmark

This directory keeps the detection-quality gate reproducible. It is intentionally small, transparent, and replaceable: a hit is not a safety certificate, and a miss is a bug report.

## Corpus

| Group | Count | Source |
|---|---:|---|
| Real bypass samples | 4 | [Trail of Bits `overtly-malicious-skills`](https://github.com/trailofbits/overtly-malicious-skills) at a pinned commit |
| Chinese malicious samples | 10 | `samples/zh`, generated from `make_samples.py` |
| Existing injection fixture | 1 | `tests/fixtures/collection/skill-b` |
| Chinese benign samples | 10 | `samples/zh`, generated from `make_samples.py` |
| Existing clean fixtures | 2 | `tests/fixtures/clean-skill`, `tests/fixtures/collection/skill-a` |
| Generated malicious | 320 | `samples/generated`, `make_corpus.py`, dev/test/holdout |
| Generated benign + hard negatives | 520 | `.env.example`, legitimate `curl`, single `os.getenv`, `find -exec` tests, official registry, pinned MCP, remote MCP, Docker, Base64 |

The Trail of Bits archive is downloaded by `fetch_samples.py` and verified against:

- commit `4ffbf9461ef0505f9ce76a0d3694a18ec33ea531`
- SHA-256 `2a3254e8617b6f8664493ce617464758e910e8a51f6879793f78c0f4e04d21ff`

No malicious binary corpus is vendored in the repository.

## Metrics

A sample is counted as detected when any finding reaches the operating threshold. The gate is:

- operating point chosen at `FPR <= 5%`
- recall at that operating point must be at least 90%
- FPR 95% upper bound must be at most 10%
- per-rule TP/FP and new FP vs `fp-baseline.json` are reported
- Wilson 95% intervals are reported for recall and FPR

## Run

```bash
cargo build --release
python bench/fetch_samples.py
python bench/make_corpus.py          # 320 malicious / 520 benign
python bench/run.py --exe target/release/skillguard --split holdout
```

Results are written to `bench/results/latest.json`, `latest.md`, and a versioned `v<version>-<date>` report. The CI `benchmark` job runs the same commands on Ubuntu.

## Current baseline

| Metric | Result |
|---|---:|
| Malicious (holdout) | 202 |
| Benign (holdout) | 340 |
| Recall @ high | 64.9% (131/202) |
| FPR @ high | 3.5% (12/340) |
| Recall @ critical | 30.7% (62/202) |
| FPR @ critical | 0.0% (0/340) |

Real-corpus expansion: 500 ClawHub benign skills (provisional, reviewed by an agent) and 300 MaliciousSkillBench malicious identities (original benchmark labels, never scanner-generated), plus Cisco and Trail of Bits samples.

## P1/P2 scripts

- `import_samples.py`: import locally provided, licensed Hub samples; no scraping.
- `make_adversarial.py`: deterministic quarterly mutations (Base64, zero-width, split, docx).
- `make_pyc_variants.py`: pyc matrix for the current interpreter.
- `llm_judge.py`: OpenAI-compatible local endpoint prototype; candidate reranking only.
- `calibrate.py`: choose threshold on dev, evaluate holdout.
- `ablation.py`: rules-only vs LLM-only vs combined.
- `candidates.py`: export medium-only candidates.

`private-inference.example.json` documents the local endpoint shape; no model is bundled.

## Limitations

- The corpus is a regression gate, not a representative sample of the whole Skill ecosystem.
- The generated corpus is synthetic and deliberately structured; it tests detection primitives, not every real-world variant.
- Real Hub samples require licensing, sanitization, and human review; use `import_samples.py` rather than scraping.
- The next corpus expansion should add MaliciousSkillBench's held-out split and externally reviewed hard negatives.

## Add a sample

1. Put the sample under `samples/zh/<id>/` with a `SKILL.md`.
2. Add an entry to `manifest.json` with a `label`, `split`, and optional `tags`.
3. Run `python bench/run.py --only <id>` and confirm the expected result.
4. If it is a new malicious primitive, add a matching fixture/test under `tests/` so the engine change is covered outside the benchmark.