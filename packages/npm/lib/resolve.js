'use strict';

const fs = require('node:fs');
const path = require('node:path');

function platformDir() {
  return `${process.platform}-${process.arch}`;
}

function exeName() {
  return process.platform === 'win32' ? 'skillguard.exe' : 'skillguard';
}

function resolveBinary() {
  const exe = exeName();
  const packageDir = path.join(__dirname, '..');
  const candidates = [];

  if (process.env.SKILLGUARD_BINARY) {
    candidates.push(process.env.SKILLGUARD_BINARY);
  }

  candidates.push(path.join(packageDir, 'vendor', platformDir(), exe));

  try {
    const platformPackage = `@skillguard/cli-${platformDir()}`;
    candidates.push(require.resolve(`${platformPackage}/bin/${exe}`));
  } catch (_) {
    // Optional platform package is not installed.
  }

  candidates.push(path.resolve(packageDir, '..', '..', 'target', 'release', exe));
  candidates.push(path.resolve(packageDir, '..', '..', 'dist', platformDir(), exe));

  for (const candidate of candidates) {
    try {
      if (candidate && fs.statSync(candidate).isFile()) {
        return candidate;
      }
    } catch (_) {
      // Try the next candidate.
    }
  }
  return null;
}

module.exports = { resolveBinary, platformDir, exeName };