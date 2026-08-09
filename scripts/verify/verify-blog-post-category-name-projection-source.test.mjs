#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const verifier = path.join(
  repositoryRoot,
  'scripts/verify/verify-blog-post-category-name-projection-source.mjs',
);
const files = [
  'crates/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json',
  'crates/rustok-blog/src/services/post.rs',
  'crates/rustok-blog/src/dto/post.rs',
  'crates/rustok-blog/tests/post_category_name_projection.rs',
  'crates/rustok-blog/docs/implementation-plan-slice-105.md',
  'crates/rustok-blog/docs/implementation-plan-current.md',
];

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-category-name-'));
  for (const relativePath of files) {
    const source = path.join(repositoryRoot, relativePath);
    const target = path.join(root, relativePath);
    mkdirSync(path.dirname(target), { recursive: true });
    cpSync(source, target);
  }
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
}

function mutate(root, relativePath, transform) {
  const target = path.join(root, relativePath);
  const source = readFileSync(target, 'utf8');
  writeFileSync(target, transform(source));
}

function expectFailure(root, message) {
  const result = run(root);
  assert.notEqual(result.status, 0, `${message}\nstdout=${result.stdout}\nstderr=${result.stderr}`);
}

test('canonical category-name projection source passes', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, `stdout=${result.stdout}\nstderr=${result.stderr}`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects restoring permanent None category projection', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/post.rs', (source) =>
      source.replace('category_name,', 'category_name: None,'),
    );
    expectFailure(root, 'permanent None detail projection must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping tenant binding from category translation query', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/post.rs', (source) =>
      source.replace(
        '.filter(blog_category_translation::Column::TenantId.eq(tenant_id))',
        '',
      ),
    );
    expectFailure(root, 'cross-tenant category projection must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects replacing batch category lookup with non-batch lookup', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/post.rs', (source) =>
      source.replace(
        'blog_category_translation::Column::CategoryId.is_in(category_ids.clone())',
        'blog_category_translation::Column::CategoryId.eq(category_ids[0])',
      ),
    );
    expectFailure(root, 'non-batch category lookup must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping caller fallback locale', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/post.rs', (source) =>
      source.replace(
        'translations,\n                locale,\n                fallback_locale,',
        'translations,\n                locale,\n                None,',
      ),
    );
    expectFailure(root, 'fallback removal must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects fake execution promotion', () => {
  const root = fixture();
  try {
    mutate(
      root,
      'crates/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json',
      (source) => source.replace('"execution": []', '"execution": ["fake"]'),
    );
    expectFailure(root, 'fake execution claim must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects current cursor rollback before slice 105', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/docs/implementation-plan-current.md', (source) =>
      source.replace(
        'canonical_source_cursor_actualized_through_slice_105',
        'canonical_source_cursor_actualized_through_slice_104',
      ),
    );
    expectFailure(root, 'cursor rollback must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
