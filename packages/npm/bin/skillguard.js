#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const { resolveBinary } = require('../lib/resolve');

const binary = resolveBinary();
if (!binary) {
  console.error('skillguard: native binary not found.');
  console.error('Set SKILLGUARD_BINARY, install a platform package, or run scripts/package-npm.sh.');
  process.exit(2);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(`skillguard: failed to start native binary: ${result.error.message}`);
  process.exit(2);
}
process.exit(result.status === null ? 2 : result.status);