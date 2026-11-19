# skillguard-cli (Python)

Python entry point for [SkillGuard](https://github.com/Seramys/skillguard).

```bash
pip install skillguard-cli
skillguard scan ./skills --fail-on high
```

The package runs the native `skillguard` binary. Resolution order:

1. `SKILLGUARD_BINARY`
2. bundled `skillguard/_bin/skillguard[.exe]`
3. repository `target/release` or `dist/` (development only)
4. `skillguard` on `PATH`

To create a wheel with the binary bundled:

```bash
scripts/package-python.sh
# or
python -m pip install build
python -m build
```