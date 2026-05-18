#!/usr/bin/env node
import { execFileSync } from 'node:child_process';

const symbol = 'upsert_flag_without_lifecycle_for_migrations_only(';
const allowedFiles = new Set(['apps/server/src/models/tenant_modules.rs']);

function runRg(args) {
  try {
    return execFileSync('rg', args, { encoding: 'utf8' }).trim();
  } catch (error) {
    if (error.status === 1) return '';
    throw error;
  }
}

const output = runRg(['--line-number', '--no-heading', '--fixed-strings', symbol, 'apps', 'crates']);
const lines = output ? output.split('\n').filter(Boolean) : [];
const violations = lines.filter((line) => {
  const [file] = line.split(':');
  return !allowedFiles.has(file);
});

if (violations.length > 0) {
  console.error('Found forbidden production bypass usage of module lifecycle toggle helper:');
  for (const violation of violations) console.error(`  ${violation}`);
  process.exit(1);
}

console.log(`OK: no forbidden usages of ${symbol}`);
