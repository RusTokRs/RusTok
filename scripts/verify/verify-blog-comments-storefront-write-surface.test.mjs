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
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  'crates/modules/rustok-blog/contracts/blog-fba-registry.json',
  'crates/modules/rustok-comments/contracts/comments-fba-registry.json',
  'crates/modules/rustok-blog/storefront/README.md',
  'crates/modules/rustok-blog/storefront/src/ui/leptos.rs',
  'crates/modules/rustok-blog/storefront/src/transport/graphql_adapter.rs',
  'crates/modules/rustok-blog/storefront/src/transport/native_server_adapter.rs',
  'crates/modules/rustok-blog/storefront/src/transport/mod.rs',
  'crates/modules/rustok-blog/storefront/src/model.rs',
  'crates/modules/rustok-blog/src/services/comment.rs',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-100.md',
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

test('accepts the canonical active storefront Comments write surface', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects removal of the active storefront comment composer', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/modules/rustok-blog/storefront/src/ui/leptos.rs',
      (source) => source.replaceAll('CommentComposer', 'RemovedComposer'),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects loss of post-create public visibility revalidation', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/modules/rustok-blog/src/services/comment.rs',
      (source) =>
        source.replace(
          'match self\n            .ensure_public_post_visible(tenant_id, post_id, public_channel_slug)',
          'match self\n            .ensure_post_exists(tenant_id, post_id)',
        ),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of public-target compensation after visibility loss', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/modules/rustok-blog/src/services/comment.rs',
      (source) => source.replaceAll('"delete-after-public-target-loss"', '"delete-after-target-loss"'),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects narrowing compensation to only target-loss failures', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/modules/rustok-blog/src/services/comment.rs',
      (source) =>
        source
          .replace(
            'Err(revalidation_error) => {',
            'Err(BlogError::PostNotFound(_)) => {',
          )
          .replace(
            'return Err(revalidation_error);',
            'return Err(BlogError::post_not_found(post_id));',
          ),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects caller-owned security context for compensation', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/modules/rustok-blog/src/services/comment.rs',
      (source) =>
        source.replace(
          'let system_security = SecurityContext::system();',
          'let system_security = security.clone();',
        ),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of the storefront native create-comment transport', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/modules/rustok-blog/storefront/src/transport/native_server_adapter.rs',
      (source) => source.replace('endpoint = "blog/comment-create"', 'endpoint = "blog/comment-create-removed"'),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of the storefront GraphQL create-comment mutation', () => {
  const result = rejects((root) =>
    mutate(
      root,
      'crates/modules/rustok-blog/storefront/src/transport/graphql_adapter.rs',
      (source) => source.replace('mutation CreateBlogComment', 'removed CreateBlogComment'),
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects evidence that regresses the active write-surface status', () => {
  const result = rejects((root) =>
    mutateJson(
      root,
      'crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
      (evidence) => {
        evidence.status = 'source_verified_absent';
      },
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects fallback actualization drift away from the active write surface', () => {
  const result = rejects((root) =>
    mutateJson(
      root,
      'crates/modules/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
      (evidence) => {
        evidence.storefront_write_surface.active_comment_form = false;
      },
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects loss of legacy registry compatibility vocabulary without schema migration', () => {
  const result = rejects((root) =>
    mutateJson(root, 'crates/modules/rustok-blog/contracts/blog-fba-registry.json', (registry) => {
      registry.provider_dependencies[0].degraded_modes = ['show_cached_thread_snapshot'];
    }),
  );
  assert.notEqual(result.status, 0);
});

test('rejects runtime or browser execution claims', () => {
  const result = rejects((root) =>
    mutateJson(
      root,
      'crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
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
      path.join(root, 'crates/modules/rustok-blog/docs/implementation-plan-slice-100.md'),
      '',
    ),
  );
  assert.notEqual(result.status, 0);
});
