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
  'crates/rustok-blog/src/services/category_name_projection.rs',
  'crates/rustok-blog/src/dto/post.rs',
  'crates/rustok-blog/tests/post_category_name_projection.rs',
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

test('canonical Taxonomy category-name projection source passes', () => {
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

test('rejects restoring legacy Blog category translation reads', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/post.rs', (source) =>
      source.replace(
        'use crate::entities::{blog_post, blog_post_channel_visibility, blog_post_translation};',
        'use crate::entities::{blog_category_translation, blog_post, blog_post_channel_visibility, blog_post_translation};',
      ),
    );
    expectFailure(root, 'legacy Blog category translation reads must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping tenant binding from typed Category lookup', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/category_name_projection.rs', (source) =>
      source.replace(
        '.filter(blog_category_taxonomy_binding::Column::TenantId.eq(tenant_id))',
        '',
      ),
    );
    expectFailure(root, 'cross-tenant Category binding projection must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects replacing batch binding lookup with one Category', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/category_name_projection.rs', (source) =>
      source.replace(
        'BlogCategoryId.is_in(category_ids.clone())',
        'BlogCategoryId.eq(category_ids[0])',
      ),
    );
    expectFailure(root, 'non-batch Category binding lookup must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping caller fallback locale from Taxonomy projection', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/src/services/category_name_projection.rs', (source) =>
      source.replace('            fallback_locale,', '            None,'),
    );
    expectFailure(root, 'fallback removal must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects removing legacy-row deletion from ownership harness', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/rustok-blog/tests/post_category_name_projection.rs', (source) =>
      source.replace('blog_category_translation::Entity::delete_many()', 'blog_category_translation::Entity::find()'),
    );
    expectFailure(root, 'ownership proof must delete legacy category translations');
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
