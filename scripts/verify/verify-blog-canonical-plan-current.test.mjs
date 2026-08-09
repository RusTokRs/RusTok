#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const verifier = path.join(repositoryRoot, 'scripts/verify/verify-blog-canonical-plan-current.mjs');
const files = [
  'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json',
  'crates/rustok-blog/docs/implementation-plan-current.md',
  'crates/rustok-blog/docs/implementation-plan.md',
  'crates/rustok-blog/docs/implementation-plan-slice-97.md',
  'crates/rustok-blog/docs/implementation-plan-slice-98.md',
  'crates/rustok-blog/docs/implementation-plan-slice-99.md',
  'crates/rustok-blog/docs/implementation-plan-slice-100.md',
  'crates/rustok-blog/docs/implementation-plan-slice-101.md',
  'crates/rustok-blog/docs/implementation-plan-slice-102.md',
  'crates/rustok-blog/docs/implementation-plan-slice-103.md',
  'crates/rustok-blog/docs/README.md',
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json',
  'crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
  'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
  'crates/rustok-blog/contracts/evidence/blog-tag-pagination-source.json',
  'crates/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json',
];
function absolute(root, relativePath) { return path.join(root, relativePath); }
function write(root, relativePath, content) {
  const target = absolute(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}
function fixture(mutator = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-current-plan-'));
  for (const relativePath of files) {
    const target = absolute(root, relativePath);
    mkdirSync(path.dirname(target), { recursive: true });
    cpSync(path.join(repositoryRoot, relativePath), target);
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
  try { return run(root); }
  finally { rmSync(root, { recursive: true, force: true }); }
}
function mutateJson(root, relativePath, mutator) {
  const target = absolute(root, relativePath);
  const value = JSON.parse(readFileSync(target, 'utf8'));
  mutator(value);
  write(root, relativePath, `${JSON.stringify(value, null, 2)}\n`);
}

test('accepts canonical Blog current implementation cursor through slice 103', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /cursor=slice-103/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('rejects reopening remote transport', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.remote_comments_transport.status = 'planned';
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /remote Comments track drift/);
});

test('rejects Translation execution promotion', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json', (value) => {
      value.source_contract.postgres_execution_observed = true;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Translation PostgreSQL source status drift/);
});

test('rejects reviving storefront comment form', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json', (value) => {
      value.source_contract.comment_form_present = true;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /write-surface evidence drift/);
});

test('rejects reopening tag pagination', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.tag_list_pagination.status = 'planned';
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag pagination track drift/);
});

test('rejects metadata tags becoming canonical again', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.tag_canonical_projection.metadata_tags_are_canonical = true;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical tag projection track drift/);
});

test('rejects skipping atomic mutation reindex next cursor', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.tag_mutation_atomic_reindex.status = 'done';
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag mutation atomic-reindex cursor drift/);
});

test('rejects premature atomic mutation implementation claim', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json', (value) => {
      value.source_contract.tag_mutation_atomic_reindex_implemented = true;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical tag projection source evidence drift/);
});

test('rejects historical plan before canonical current cursor', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-blog/docs/README.md';
    const source = readFileSync(absolute(root, file), 'utf8');
    const current = '[Current Implementation Cursor](./implementation-plan-current.md)';
    const historical = '[Historical Implementation Plan](./implementation-plan.md)';
    write(root, file, source.replace(current, '__CURRENT__').replace(historical, current).replace('__CURRENT__', historical));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical current cursor must be listed before/);
});
