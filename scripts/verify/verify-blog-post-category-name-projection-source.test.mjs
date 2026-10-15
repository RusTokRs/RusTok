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
  'crates/modules/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json',
  'crates/modules/rustok-blog/src/services/post/queries.rs',
  'crates/modules/rustok-blog/src/services/category_name_projection.rs',
  'crates/modules/rustok-blog/src/dto/post.rs',
  'crates/modules/rustok-blog/tests/post_category_name_projection.rs',
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
    mutate(root, 'crates/modules/rustok-blog/src/services/post/queries.rs', (source) =>
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
    mutate(
      root,
      'crates/modules/rustok-blog/src/services/post/queries.rs',
      (source) => `use crate::entities::blog_category_translation;\n${source}`,
    );
    expectFailure(root, 'legacy Blog category translation reads must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping the bounded membership Category read', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/modules/rustok-blog/src/services/category_name_projection.rs', (source) =>
      source.replace('Some(&category_ids)', 'None'),
    );
    expectFailure(root, 'bounded Tenant/membership Category read must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping the strict Taxonomy coverage read', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/modules/rustok-blog/src/services/category_name_projection.rs', (source) =>
      source.replace('load_scoped_categories_strict', 'load_scoped_categories'),
    );
    expectFailure(root, 'strict Taxonomy Category coverage read must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping caller fallback locale from Taxonomy projection', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/modules/rustok-blog/src/services/category_name_projection.rs', (source) =>
      source.replace('            fallback_locale,', '            None,'),
    );
    expectFailure(root, 'fallback removal must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects dropping the donor-storage retirement precondition', () => {
  const root = fixture();
  try {
    mutate(root, 'crates/modules/rustok-blog/tests/post_category_name_projection.rs', (source) =>
      source.replace('has_table("blog_category_translations")', 'has_table("blog_posts")'),
    );
    expectFailure(root, 'projection harness must prove donor storage retirement first');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects fake execution promotion', () => {
  const root = fixture();
  try {
    mutate(
      root,
      'crates/modules/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json',
      (source) => source.replace('"execution": []', '"execution": ["fake"]'),
    );
    expectFailure(root, 'fake execution claim must fail');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
