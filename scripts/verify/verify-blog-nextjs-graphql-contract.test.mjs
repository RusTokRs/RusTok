#!/usr/bin/env node

// Self-test for the Blog client GraphQL contract parity gate.
//
// The gate is only useful if it (a) accepts the current client documents and
// (b) actually rejects a document that drifted away from the Blog schema. The
// negative case is proven by mutating a client source file in place, running the
// gate, and restoring the file.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '..', '..');
const verifier = path.join(repoRoot, 'scripts', 'verify', 'verify-blog-nextjs-graphql-contract.mjs');
const clientFile = path.join(
  repoRoot,
  'apps',
  'next-admin',
  'packages',
  'blog',
  'src',
  'api',
  'categories.ts'
);

function runVerifier() {
  try {
    const stdout = execFileSync('node', [verifier], { cwd: repoRoot, encoding: 'utf8' });
    return { code: 0, output: stdout };
  } catch (error) {
    return {
      code: typeof error.status === 'number' ? error.status : 1,
      output: `${error.stdout ?? ''}${error.stderr ?? ''}`
    };
  }
}

test('the Blog client documents match the Blog GraphQL schema', () => {
  const result = runVerifier();
  assert.equal(result.code, 0, result.output);
});

test('a drifted client selection set is rejected', () => {
  const original = fs.readFileSync(clientFile, 'utf8');
  const drifted = original.replace('      position\n', '      position\n      postsCount\n');
  assert.notEqual(drifted, original, 'the fixture must inject a field that the schema does not expose');

  try {
    fs.writeFileSync(clientFile, drifted);
    const result = runVerifier();
    assert.notEqual(result.code, 0, 'the gate must reject the drifted document');
    assert.match(result.output, /postsCount is not a field of BlogCategory/);
  } finally {
    fs.writeFileSync(clientFile, original);
  }
});

test('a drifted client input object is rejected', () => {
  const original = fs.readFileSync(clientFile, 'utf8');
  const drifted = original.replace(
    'export interface UpdateBlogCategoryInput {',
    'export interface UpdateBlogCategoryInput {\n  isActive?: boolean;'
  );
  assert.notEqual(drifted, original, 'the fixture must inject an input field the schema does not expose');

  try {
    fs.writeFileSync(clientFile, drifted);
    const result = runVerifier();
    assert.notEqual(result.code, 0, 'the gate must reject the drifted input object');
    assert.match(result.output, /UpdateBlogCategoryInput declares "isActive"/);
  } finally {
    fs.writeFileSync(clientFile, original);
  }
});
