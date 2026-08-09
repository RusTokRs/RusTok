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

const verifier = path.resolve(
  'scripts/verify/verify-blog-comments-storefront-native-port-injection.mjs',
);
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-storefront-native-port-injection.json';
const facadePath = 'crates/rustok-blog/src/lib.rs';
const nativeAdapterPath =
  'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs';
const servicePath = 'crates/rustok-blog/src/services/comment.rs';
const consumerMatrixPath =
  'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const fallbackEvidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const fixtureFiles = [
  evidencePath,
  facadePath,
  nativeAdapterPath,
  servicePath,
  consumerMatrixPath,
  fallbackEvidencePath,
  planPath,
];

function copy(root, relativePath) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, readFileSync(relativePath));
}

function mutate(root, relativePath, transform) {
  const target = path.join(root, relativePath);
  const source = readFileSync(target, 'utf8');
  writeFileSync(target, transform(source));
}

function mutateJson(root, relativePath, transform) {
  mutate(root, relativePath, (source) => {
    const value = JSON.parse(source);
    transform(value);
    return JSON.stringify(value, null, 2);
  });
}

function fixture(mutator) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-storefront-native-comments-'));
  for (const file of fixtureFiles) copy(root, file);
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

function removeMarker(file, marker) {
  return (root) => mutate(root, file, (source) => source.replace(marker, ''));
}

test('accepts the canonical storefront native Comments composition boundary', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects removal of the Blog facade port re-export', () => {
  assert.notEqual(
    rejects(removeMarker(facadePath, 'pub use rustok_comments::CommentsThreadPort;')).status,
    0,
  );
});

test('rejects removal of the host Comments port lookup', () => {
  assert.notEqual(
    rejects(
      removeMarker(
        nativeAdapterPath,
        'runtime_ctx.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>()',
      ),
    ).status,
    0,
  );
});

test('rejects removal of the injected selector branch', () => {
  assert.notEqual(
    rejects(
      removeMarker(
        nativeAdapterPath,
        'rustok_blog::CommentService::with_comments_thread_port(',
      ),
    ).status,
    0,
  );
});

test('rejects removal of the in-process fallback branch', () => {
  assert.notEqual(
    rejects(
      removeMarker(
        nativeAdapterPath,
        'rustok_blog::CommentService::new(runtime_ctx.db_clone(), event_bus)',
      ),
    ).status,
    0,
  );
});

test('rejects direct extra provider construction in the storefront read', () => {
  assert.notEqual(
    rejects((root) =>
      mutate(root, nativeAdapterPath, (source) => `${source}\nrustok_blog::CommentService::new(`),
    ).status,
    0,
  );
});

test('rejects removal of the storefront selector handoff', () => {
  assert.notEqual(
    rejects(
      removeMarker(
        nativeAdapterPath,
        'let comments = comment_service(&runtime_ctx, event_bus.clone());',
      ),
    ).status,
    0,
  );
});

test('rejects removal of the approved-only public port operation', () => {
  assert.notEqual(
    rejects(removeMarker(servicePath, '.list_public_comments_for_target(')).status,
    0,
  );
});

test('rejects removal of the shared snapshot store lookup', () => {
  assert.notEqual(
    rejects(
      removeMarker(
        nativeAdapterPath,
        'runtime_ctx.shared_get::<Arc<dyn PublicCommentsSnapshotStore>>()',
      ),
    ).status,
    0,
  );
});

test('rejects removal of the shared snapshot helper handoff', () => {
  assert.notEqual(
    rejects(removeMarker(nativeAdapterPath, 'list_public_comments_with_snapshot(')).status,
    0,
  );
});

test('rejects removal of cached-snapshot disclosure', () => {
  assert.notEqual(
    rejects(removeMarker(nativeAdapterPath, 'cached_snapshot: public_comments.cached_snapshot')).status,
    0,
  );
});

test('rejects broad storefront degradation for every error', () => {
  assert.notEqual(
    rejects((root) =>
      mutate(root, nativeAdapterPath, (source) => `${source}\nErr(_) => BlogCommentList`),
    ).status,
    0,
  );
});

test('rejects removal of typed timeout mapping', () => {
  assert.notEqual(
    rejects(removeMarker(nativeAdapterPath, 'PublicCommentsAvailability::Timeout')).status,
    0,
  );
});

test('rejects removal of the compile-only selector harness', () => {
  assert.notEqual(
    rejects(
      removeMarker(
        nativeAdapterPath,
        'fn storefront_native_runtime_exposes_comments_port_selection()',
      ),
    ).status,
    0,
  );
});

test('rejects fallback evidence that demotes cached snapshot to planned', () => {
  const result = rejects((root) =>
    mutateJson(root, fallbackEvidencePath, (evidence) => {
      evidence.storefront_read_degradation.cached_thread_snapshot = 'planned';
    }),
  );
  assert.notEqual(result.status, 0);
});

test('rejects runtime promotion without execution', () => {
  assert.notEqual(
    rejects((root) =>
      mutateJson(root, evidencePath, (evidence) => {
        evidence.runtime_status = 'passed';
      }),
    ).status,
    0,
  );
});

test('rejects stale admin native SSR pending status after composition', () => {
  assert.notEqual(
    rejects((root) =>
      mutateJson(root, evidencePath, (evidence) => {
        evidence.profiles.pending.unshift('admin_native_ssr_composition');
      }),
    ).status,
    0,
  );
});

test('rejects remote transport promotion without implementation', () => {
  assert.notEqual(
    rejects((root) =>
      mutateJson(root, evidencePath, (evidence) => {
        evidence.profiles.source_verified.push('remote_transport_implementation');
        evidence.profiles.pending = [];
      }),
    ).status,
    0,
  );
});

test('rejects unearned Blog FBA package-chain registration', () => {
  assert.notEqual(
    rejects((root) =>
      mutateJson(root, evidencePath, (evidence) => {
        evidence.registration.blog_fba_package_chain = 'registered';
      }),
    ).status,
    0,
  );
});

test('rejects canonical-plan drift', () => {
  assert.notEqual(
    rejects((root) => writeFileSync(path.join(root, planPath), '')) .status,
    0,
  );
});
