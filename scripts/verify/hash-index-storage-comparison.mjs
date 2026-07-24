#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';

const prefix = '[hash-index-storage-comparison]';
const fail = (message) => {
  console.error(`${prefix} ${message}`);
  process.exit(1);
};

const args = process.argv.slice(2);
if (args.includes('--help') || args.includes('-h')) {
  console.log('Usage: node scripts/verify/hash-index-storage-comparison.mjs <comparison.json>');
  process.exit(0);
}
if (args.length !== 1) fail('exactly one comparison.json path is required');

const filename = args[0];
if (!existsSync(filename)) fail(`missing comparison file: ${filename}`);
const digest = createHash('sha256').update(readFileSync(filename)).digest('hex');
process.stdout.write(`${digest}\n`);
