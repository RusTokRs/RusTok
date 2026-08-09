#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const verifier = path.join(repositoryRoot, 'scripts/verify/verify-blog-tag-mutation-reindex-source.mjs');
const files = [
  'crates/rustok-blog/contracts/evidence/blog-tag-mutation-reindex-source.json',
  'crates/rustok-taxonomy/src/module_term_mutation.rs',
  'crates/rustok-taxonomy/src/lib.rs',
  'crates/rustok-blog/src/services/tag.rs',
  'crates/rustok-blog/src/migrations/m20260328_000002_create_blog_taxonomy_tables.rs',
  'crates/rustok-blog/tests/taxonomy_tags.rs',
  'crates/rustok-blog/docs/implementation-plan-slice-104.md',
  'crates/rustok-blog/docs/implementation-plan-current.md',
];
function absolute(root, relativePath) { return path.join(root, relativePath); }
function write(root, relativePath, content) {
  const target = absolute(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}
function fixture(mutator = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-tag-mutation-'));
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

test('accepts atomic Blog tag mutation/reindex source', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /atomic=taxonomy\+reindex/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('rejects restoring manual relation pre-delete', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-blog/src/services/tag.rs';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replace(
      'let txn = self.db.begin().await.map_err(BlogError::from)?;\n        delete_module_term_in_tx(',
      'blog_post_tag::Entity::delete_many();\n        let txn = self.db.begin().await.map_err(BlogError::from)?;\n        delete_module_term_in_tx(',
    ));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /delete_tag.*forbidden|pre-delete|delete_many/s);
});

test('rejects moving reindex outside update transaction', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-blog/src/services/tag.rs';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replace(
      'publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;',
      '/* reindex omitted */',
    ));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /update_tag.*publish_blog_reindex/s);
});

test('rejects weakening Taxonomy module-scope recheck', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-taxonomy/src/module_term_mutation.rs';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replaceAll('taxonomy_term::Column::ScopeValue.eq(&module_scope)', 'taxonomy_term::Column::ScopeValue.is_not_null()'));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /ScopeValue/);
});

test('rejects premature runtime promotion', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/rustok-blog/contracts/evidence/blog-tag-mutation-reindex-source.json', (value) => {
      value.runtime_status = 'validated';
      value.execution.push({ command: 'not-actually-run' });
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /identity\/status drift|harness\/planning\/execution drift/);
});
