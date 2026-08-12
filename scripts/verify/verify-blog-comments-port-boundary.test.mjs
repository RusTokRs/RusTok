#!/usr/bin/env node

import './verify-blog-comments-http-port-injection.test.mjs';
import './verify-blog-comments-graphql-port-injection.test.mjs';
import './verify-blog-comments-storefront-native-port-injection.test.mjs';
import './verify-blog-comments-admin-native-port-injection.test.mjs';
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

const verifier = path.resolve('scripts/verify/verify-blog-comments-port-boundary.mjs');
const files = {
  matrix: 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json',
  fallback: 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  httpEvidence: 'crates/rustok-blog/contracts/evidence/blog-comments-http-port-injection.json',
  graphqlEvidence: 'crates/rustok-blog/contracts/evidence/blog-comments-graphql-port-injection.json',
  storefrontEvidence:
    'crates/rustok-blog/contracts/evidence/blog-comments-storefront-native-port-injection.json',
  adminEvidence: 'crates/rustok-blog/contracts/evidence/blog-comments-admin-native-port-injection.json',
  providerRegistry: 'crates/rustok-comments/contracts/comments-fba-registry.json',
  consumerRegistry: 'crates/rustok-blog/contracts/blog-fba-registry.json',
  facade: 'crates/rustok-blog/src/lib.rs',
  service: 'crates/rustok-blog/src/services/comment.rs',
  snapshot: 'crates/rustok-blog/src/public_comments_snapshot.rs',
  httpRuntime: 'crates/rustok-blog/src/controllers/mod.rs',
  httpController: 'crates/rustok-blog/src/controllers/comments.rs',
  manifest: 'crates/rustok-blog/rustok-module.toml',
  graphqlModule: 'crates/rustok-blog/src/graphql/mod.rs',
  graphqlRuntime: 'crates/rustok-blog/src/graphql/runtime_data.rs',
  graphqlTypes: 'crates/rustok-blog/src/graphql/types.rs',
  graphqlMutation: 'crates/rustok-blog/src/graphql/mutation.rs',
  storefrontModel: 'crates/rustok-blog/storefront/src/model.rs',
  storefrontGraphql: 'crates/rustok-blog/storefront/src/transport/graphql_adapter.rs',
  storefrontNative: 'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs',
  storefrontUi: 'crates/rustok-blog/storefront/src/ui/leptos.rs',
  adminNative: 'crates/rustok-blog/admin/src/transport/native_server_adapter.rs',
  hostSnapshot: 'apps/server/src/services/blog_public_comments_snapshot.rs',
  hostComposition: 'apps/server/src/services/module_event_dispatcher.rs',
  serverBuild: 'apps/server/build.rs',
  serverSchema: 'apps/server/src/graphql/schema.rs',
  plan: 'crates/rustok-blog/docs/implementation-plan.md',
  slice99: 'crates/rustok-blog/docs/implementation-plan-slice-99.md',
};

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
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-comments-port-'));
  for (const relativePath of Object.values(files)) copy(root, relativePath);
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
  return (root) => mutate(root, file, (source) => source.replaceAll(marker, ''));
}

test('accepts the canonical Blog Comments consumer and cached snapshot boundary', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a direct Comments service bypass', () => {
  const result = rejects((root) =>
    mutate(root, files.service, (source) => `${source}\nself.comments.get_comment(`),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of the approved-only snapshot validation', () => {
  const result = rejects(
    removeMarker(files.snapshot, 'item.post_id == identity.post_id && item.status == "approved"'),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of the bounded snapshot payload limit', () => {
  const result = rejects(
    removeMarker(files.snapshot, 'MAX_PUBLIC_COMMENTS_SNAPSHOT_BYTES: usize = 256 * 1024'),
  );
  assert.notEqual(result.status, 0);
});

test('rejects snapshot fallback for non-availability errors', () => {
  const result = rejects(
    removeMarker(files.snapshot, 'return Err(error);'),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of GraphQL cached-snapshot disclosure', () => {
  const result = rejects(
    removeMarker(files.graphqlTypes, 'cached_snapshot: read.cached_snapshot'),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of the native snapshot store lookup', () => {
  const result = rejects(
    removeMarker(
      files.storefrontNative,
      'shared_get::<Arc<dyn PublicCommentsSnapshotStore>>()',
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of stale snapshot UI disclosure', () => {
  const result = rejects(
    removeMarker(files.storefrontUi, 'Showing a recent cached snapshot.'),
  );
  assert.notEqual(result.status, 0);
});

test('rejects a host snapshot adapter that creates a Redis client directly', () => {
  const result = rejects((root) =>
    mutate(root, files.hostSnapshot, (source) => `${source}\nredis::Client::open("redis://example")`),
  );
  assert.notEqual(result.status, 0);
});

test('rejects removal of host runtime snapshot registration', () => {
  const result = rejects(
    removeMarker(
      files.hostComposition,
      'blog_public_comments_snapshot::register(&mut extensions, &runtime_ctx);',
    ),
  );
  assert.notEqual(result.status, 0);
});

test('rejects fallback evidence that demotes cached snapshot to planned', () => {
  const result = rejects((root) =>
    mutateJson(root, files.fallback, (evidence) => {
      evidence.storefront_read_degradation.cached_thread_snapshot = 'planned';
    }),
  );
  assert.notEqual(result.status, 0);
});

test('rejects fallback evidence that claims comment-form completion', () => {
  const result = rejects((root) =>
    mutateJson(root, files.fallback, (evidence) => {
      evidence.storefront_read_degradation.comment_form_fallback = 'source_verified_no_compile';
    }),
  );
  assert.notEqual(result.status, 0);
});

test('rejects runtime status promotion without execution', () => {
  const result = rejects((root) =>
    mutateJson(root, files.fallback, (evidence) => {
      evidence.runtime_status = 'passed';
    }),
  );
  assert.notEqual(result.status, 0);
});

test('rejects cached snapshot plan drift', () => {
  const result = rejects((root) => writeFileSync(path.join(root, files.slice99), ''));
  assert.notEqual(result.status, 0);
});
