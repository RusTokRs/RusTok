#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const verifier = path.join(repositoryRoot, 'scripts/verify/verify-blog-tag-canonical-projection-source.mjs');
const files = [
  'crates/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json',
  'crates/rustok-blog/src/services/tag.rs',
  'crates/rustok-blog/tests/taxonomy_tags.rs',
  'crates/rustok-search/src/blog_projector.rs',
  'crates/rustok-search/tests/blog_projection_postgres_test.rs',
  'crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json',
  'crates/rustok-blog/docs/implementation-plan-slice-103.md',
  'crates/rustok-blog/docs/implementation-plan-current.md',
];
function absolute(root, relativePath) { return path.join(root, relativePath); }
function write(root, relativePath, content) {
  const target = absolute(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}
function fixture(mutator = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-tag-source-'));
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

test('accepts canonical Blog tag read/Search source', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /source=blog_post_tags\+taxonomy/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('rejects metadata resurrection on empty relation set', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-blog/src/services/tag.rs';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replace('return Ok(tags_by_post);', 'return Ok(HashMap::new());'));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tags_by_post/);
});

test('rejects removal of the Blog read regression harness', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-blog/tests/taxonomy_tags.rs';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replace('post_read_does_not_resurrect_metadata_tags_after_relations_are_removed', 'removed_read_case'));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /post_read_does_not_resurrect/);
});

test('rejects metadata-backed Search projection', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-search/src/blog_projector.rs';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replace('FROM blog_post_tags relation', "FROM jsonb_array_elements_text(p.metadata -> 'tags') relation"));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forbidden|blog_post_tags/);
});

test('rejects premature atomic mutation claim', () => {
  const result = rejects((root) => {
    const file = 'crates/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json';
    const value = JSON.parse(readFileSync(absolute(root, file), 'utf8'));
    value.source_contract.tag_mutation_atomic_reindex_implemented = true;
    write(root, file, `${JSON.stringify(value, null, 2)}\n`);
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source\/execution drift/);
});
