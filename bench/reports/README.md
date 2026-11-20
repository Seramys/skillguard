# Benchmark reports

Committed files are summaries and Markdown reports only. Raw `*.json` reports embed per-sample metadata and are ignored by `.gitignore`.

## Two different suites

| Suite | Run in CI | Purpose | Current result |
| --- | --- | --- | --- |
| `public` | yes | Harness, rule, and packaging smoke test. Base fixtures + generated corpus only. | 335 malicious / 532 benign; recall 100%, FPR 0% |
| `extended` | no | Detection-quality signal. Generated holdout + authorized real holdout (MaliciousSkillBench, ClawHub). | 202 malicious / 340 benign; recall 64.9%, FPR 1.5% |

The public suite is deliberately easy. Its 100% recall is **not** evidence that SkillGuard detects real-world malicious skills. The extended suite is the quality number, but its real samples cannot be redistributed through this repository, so public CI cannot run it and cannot claim its result.

## Reproduce

```bash
cargo build --release
python bench/fetch_samples.py
python bench/make_corpus.py

# CI smoke suite
python bench/run.py --exe target/release/skillguard --split all --no-imported \
  --json-out bench/results/public.json --md-out bench/results/public.md

# Local quality suite (requires bench/imported/manifest.json and samples/real)
python bench/run.py --exe target/release/skillguard --split holdout \
  --imported-manifest bench/imported/manifest.json \
  --json-out bench/results/extended.json --md-out bench/results/extended.md
```

## Extended holdout composition

| Source | Malicious | Benign |
| --- | ---: | ---: |
| Generated corpus (`holdout`) | 106 | 173 |
| MaliciousSkillBench (`holdout`) | 96 | 0 |
| ClawHub reviewed (`holdout`) | 0 | 167 |
| **Total** | **202** | **340** |

Cisco and Trail of Bits samples are not in the `holdout` split; they are part of the base smoke fixtures. The extended report is a snapshot, not a CI artifact. Regeneration requires locally reviewed data under `bench/samples/real/` and the ignored `bench/imported/manifest.json`.

## Gate policy

`bench/manifest.json` sets:

- `target_fpr: 0.05`
- `max_fpr_upper: 0.10`
- `min_recall_at_fpr: 0.64` (regression floor, not the product target)
- `target_recall_at_fpr: 0.90` (aspirational target)

The extended suite currently passes the regression floor at 64.9% recall and 1.5% FPR, and misses the 90% target. That gap is real: most misses are MaliciousSkillBench rewritten malicious prompts, and the current rules-only engine is intentionally conservative about semantic intent. Do not describe the gate as "90% recall achieved."
