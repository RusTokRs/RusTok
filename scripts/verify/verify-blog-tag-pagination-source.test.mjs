#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const verifier = path.join(repositoryRoot, 'scripts/verify/verify-blog-tag-pagination-source.mjs');
const files = [
  'crates/rustok-blog/contracts/evidence/blog-tag-pagination-source.json',
  'crates/rustok-blog/src/services/tag.rs',
  'crates/rustok-blog/src/dto/tag.rs',
  'crates/rustok-blog/docs/implementation-plan-slice-102.md',
  'crates/rustok-blog/docs/implementation-plan-current.md',
];

function target(root, relativePath) {
  return path.join(root, relativePath);
}

function write(root, relativePath, content) {
  const file = target(root, relativePath);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, content);
}

function fixture(mutator = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-tag-pagination-'));
  for (const relativePath of files) {
    const destination = target(root, relativePath);
    mkdirSync(path.dirname(destination), { recursive: true });
    cpSync(path.join(repositoryRoot, relativePath), destination);
  }
  mutator(root);
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: repositoryRoot,
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
}

function rejects(mutator) {
  const root = fixture(mutator);
  try {
    return run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function replace(root, relativePath, from, to) {
  const source = readFileSync(target(root, relativePath), 'utf8');
  write(root, relativePath, source.replace(from, to));
}

test('accepts the canonical Blog tag pagination source boundary', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /max_per_page=100/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects removal of the owner page-size clamp', () => {
  const result = rejects((root) => {
    replace(
      root,
      'crates/rustok-blog/src/services/tag.rs',
      'let per_page = bounded_tag_page_size(filter.per_page);',
      'let per_page = filter.per_page.max(1);',
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /bounded_tag_page_size|forbidden/);
});

test('rejects unsafe multiplication in the page offset', () => {
  const result = rejects((root) => {
    replace(
      root,
      'crates/rustok-blog/src/services/tag.rs',
      'let offset = tag_page_offset(page, per_page);',
      'let offset = ((page - 1) * per_page) as usize;',
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag_page_offset|forbidden/);
});

test('rejects removal of the published DTO maximum', () => {
  const result = rejects((root) => {
    replace(
      root,
      'crates/rustok-blog/src/dto/tag.rs',
      '#[param(minimum = 1, maximum = 100)]',
      '#[param(minimum = 1)]',
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /maximum = 100/);
});

test('rejects runtime promotion without execution', () => {
  const result = rejects((root) => {
    const relativePath = 'crates/rustok-blog/contracts/evidence/blog-tag-pagination-source.json';
    const value = JSON.parse(readFileSync(target(root, relativePath), 'utf8'));
    value.runtime_status = 'passed';
    write(root, relativePath, `${JSON.stringify(value, null, 2)}\n`);
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /identity\/status drift/);
});

test('rejects claiming database-side pagination', () => {
  const result = rejects((root) => {
    const relativePath = 'crates/rustok-blog/contracts/evidence/blog-tag-pagination-source.json';
    const value = JSON.parse(readFileSync(target(root, relativePath), 'utf8'));
    value.source_contract.database_side_pagination_claimed = true;
    write(root, relativePath, `${JSON.stringify(value, null, 2)}\n`);
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source contract drift/);
});
