# seramys-skillguard

npm launcher for [SkillGuard](https://github.com/Seramys/skillguard), a supply-chain
security scanner for AI agent skills.

> 0.1.0 currently bundles the Windows x64 binary. On Linux/macOS, build from source or use a platform-specific release asset.

```bash
npx seramys-skillguard scan ./skills --fail-on high
npx seramys-skillguard scan ./skills --format sarif > skillguard.sarif
```

The launcher looks for the native binary in this order:

1. `SKILLGUARD_BINARY`
2. `vendor/<platform>-<arch>/skillguard[.exe]`
3. optional platform package `@skillguard/cli-<platform>-<arch>`
4. repository `target/release` or `dist/` (development only)

For publishing, build the binary with `scripts/build.sh` and copy it into
`vendor/` using `scripts/package-npm.sh`, or publish one optional platform
package per OS/arch.