#!/usr/bin/env node
import { execFileSync } from 'node:child_process';

const symbol = 'upsert_flag_without_lifecycle_for_migrations_only(';

function runSearch(symbol, paths) {
  try {
    return execFileSync('rg', ['--line-number', '--no-heading', '--fixed-strings', symbol, ...paths], {
      encoding: 'utf8',
    }).trim();
  } catch (error) {
    if (error.status === 1) return '';
    if (error.code === 'ENOENT') {
      try {
        return execFileSync('git', ['grep', '-n', '-F', symbol, '--', ...paths], {
          encoding: 'utf8',
        }).trim();
      } catch (gitError) {
        if (gitError.status === 1) return '';
        throw gitError;
      }
    }
    throw error;
  }
}

const output = runSearch(symbol, ['apps', 'crates']);
const lines = output ? output.split('\n').filter(Boolean) : [];
const violations = lines;

if (violations.length > 0) {
  console.error('Found a forbidden module lifecycle toggle bypass:');
  for (const violation of violations) console.error(`  ${violation}`);
  process.exit(1);
}

console.log(`OK: no lifecycle toggle bypasses match ${symbol}`);
