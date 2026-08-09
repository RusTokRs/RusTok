#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve('scripts/verify/verify-blog-comments-storefront-write-surface.mjs');
const files = [
  'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
  'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  'crates/rustok-blog/contracts/blog-fba-registry.json',
  'crates/rustok-comments/contracts/comments-fba-registry.json',
  'crates/rustok-blog/storefront/README.md',
  'crates/rustok-blog/storefront/src/ui/leptos.rs',
  'crates/rustok-blog/storefront/src/transport/graphql_adapter.rs',
  'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs',
  'crates/rustok-blog/storefront/src/transport/mod.rs',
  'crates/rustok-blog/storefront/src/model.rs',
  'crates/rustok-blog/docs/implementation-plan-slice-100.md',
];

function copy(root, relativePath) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, readFileSync(relativePath));
}

function mutate(root, relativePath, transform) {
  const target = path.join(root, relativePath);
  writeFileSync(target, transform(readFileSync(target, 'utf8')));
}

function mutateJson(root, relativePath, transform) {
  mutate(root, relativePath, (source) => {
    const value = JSON.parse(source);
    transform(value);
    return JSON.stringify(value, null, 2);
  });
}

function fixture(mutator) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-storefront-write-surface-'));
  files.forEach((file) => copy(root, file));
  mutator?.(root);
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve('.'),
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

test('accepts the canonical absent storefront Comments write surface', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a newly added storefront comment form', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/rustok-blog/storefront/src/ui/leptos.rs',
      (source) => `${source}\n<form><textarea></textarea></form>`,
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects a storefront create-comment transport', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs',
      (source) => `${source}\ncreate_comment(`,
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects a storefront GraphQL mutation', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/rustok-blog/storefront/src/transport/graphql_adapter.rs',
      (source) => source.replace('query StorefrontBlog', 'mutation StorefrontBlog'),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects inventory promotion to a present form', () => {
  const result = rejects((root) =>
    mutateJson(
      root,
      'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
      (evidence) => {
        evidence.source_contract.comment_form_present = true;
      },
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects fallback actualization back to an implementation target', () => {
  const result = rejects((root) =>
    mutateJson(
      root,
      'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
      (evidence) => {
        evidence.storefront_write_surface.comment_form_fallback = 'planned';
      },
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects loss of legacy registry compatibility vocabulary without schema migration', () => {
  const result = rejects((root) =>
    mutateJson(root, 'crates/rustok-blog/contracts/blog-fba-registry.json', (registry) => {
      registry.provider_dependencies[0].degraded_modes = ['show_cached_thread_snapshot'];
    }),
  );
  assert.notEqual(result.status, 0);
});

test('rejects runtime or browser execution claims', () => {
  const result = rejects((root) =>
    mutateJson(
      root,
      'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
      (evidence) => {
        evidence.source_contract.runtime_execution_observed = true;
      },
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects slice-100 planning drift', () => {
  const result = rejects((root) =>
    writeFileSync(
      path.join(root, 'crates/rustok-blog/docs/implementation-plan-slice-100.md'),
      '',
    ),
  );
  assert.notEqual(result.status, 0);
});
