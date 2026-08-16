#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

function run(args, options = {}) {
  return execFileSync(process.execPath, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], ...options });
}

const root = resolve(new URL('.', import.meta.url).pathname);
const tmp = mkdtempSync(join(tmpdir(), 'rustok-loadtest-'));
try {
  for (const file of [
    'k6/comparison.js',
    'fixtures/generate.mjs',
    'fixtures/import-rustok.mjs',
    'fixtures/import-magento.mjs',
    'evidence/create-run.mjs',
    'evidence/collect-process.mjs',
    'evidence/summarize.mjs',
  ]) {
    run(['--check', resolve(root, file)]);
  }
  for (const file of ['config/rustok.example.json', 'config/magento.example.json', 'evidence/topology.example.json']) {
    JSON.parse(readFileSync(resolve(root, file), 'utf8'));
  }
  const outA = resolve(tmp, 'a');
  const outB = resolve(tmp, 'b');
  run([resolve(root, 'fixtures/generate.mjs'), '--count', '100', '--seed', 'verification-seed', '--out', outA]);
  run([resolve(root, 'fixtures/generate.mjs'), '--count', '100', '--seed', 'verification-seed', '--out', outB]);
  const manifestA = JSON.parse(readFileSync(resolve(outA, 'manifest.json'), 'utf8'));
  const manifestB = JSON.parse(readFileSync(resolve(outB, 'manifest.json'), 'utf8'));
  if (manifestA.manifest_core_sha256 !== manifestB.manifest_core_sha256) throw new Error('Fixture manifest digest is not deterministic');
  if (readFileSync(resolve(outA, 'products.jsonl'), 'utf8') !== readFileSync(resolve(outB, 'products.jsonl'), 'utf8')) throw new Error('products.jsonl is not byte-deterministic');
  if (readFileSync(resolve(outA, 'products.csv'), 'utf8') !== readFileSync(resolve(outB, 'products.csv'), 'utf8')) throw new Error('products.csv is not byte-deterministic');
  if (manifestA.search_cases.reduce((sum, item) => sum + item.expected_matches, 0) !== 100) throw new Error('Search cardinality does not cover all fixtures exactly once');
  process.stdout.write('loadtest static verification: PASS\n');
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
