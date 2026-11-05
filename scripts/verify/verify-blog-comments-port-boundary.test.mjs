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
  matrix: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json',
  fallback: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  httpEvidence: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-http-port-injection.json',
  graphqlEvidence: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-graphql-port-injection.json',
  storefrontEvidence:
    'crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-native-port-injection.json',
  adminEvidence: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-admin-native-port-injection.json',
  providerRegistry: 'crates/modules/rustok-comments/contracts/comments-fba-registry.json',
  consumerRegistry: 'crates/modules/rustok-blog/contracts/blog-fba-registry.json',
  facade: 'crates/modules/rustok-blog/src/lib.rs',
  service: 'crates/modules/rustok-blog/src/services/comment.rs',
  serviceCommands: 'crates/modules/rustok-blog/src/services/comment/commands.rs',
  snapshot: 'crates/modules/rustok-blog/src/integrations/public_comments_snapshot.rs',
  httpRuntime: 'crates/modules/rustok-blog/src/controllers/mod.rs',
  httpController: 'crates/modules/rustok-blog/src/controllers/comments.rs',
  manifest: 'crates/modules/rustok-blog/rustok-module.toml',
  graphqlModule: 'crates/modules/rustok-blog/src/graphql/mod.rs',
  graphqlRuntime: 'crates/modules/rustok-blog/src/graphql/runtime_data.rs',
  graphqlTypes: 'crates/modules/rustok-blog/src/graphql/types.rs',
  graphqlMutation: 'crates/modules/rustok-blog/src/graphql/mutation.rs',
  storefrontModel: 'crates/modules/rustok-blog/storefront/src/model.rs',
  storefrontGraphql: 'crates/modules/rustok-blog/storefront/src/transport/graphql_adapter.rs',
  storefrontNative: 'crates/modules/rustok-blog/storefront/src/transport/native_server_adapter.rs',
  storefrontUi: 'crates/modules/rustok-blog/storefront/src/ui/leptos.rs',
  storefrontCommentsList: 'crates/modules/rustok-blog/storefront/src/ui/comments_list.rs',
  storefrontPolicy: 'crates/modules/rustok-blog/storefront/src/core/discussion.rs',
  storefrontCore: 'crates/modules/rustok-blog/storefront/src/core.rs',
  storefrontDiscussion: 'crates/modules/rustok-blog/storefront/src/ui/comment.rs',
  storefrontEnglish: 'crates/modules/rustok-blog/storefront/locales/en.ftl',
  storefrontRussian: 'crates/modules/rustok-blog/storefront/locales/ru.ftl',
  adminNative: 'crates/modules/rustok-blog/admin/src/transport/native_server_adapter.rs',
  hostSnapshot: 'apps/server/src/services/blog_public_comments_snapshot.rs',
  hostComposition: 'apps/server/src/services/module_event_dispatcher.rs',
  serverBuild: 'apps/server/build.rs',
  serverSchema: 'apps/server/src/graphql/schema.rs',
  plan: 'crates/modules/rustok-blog/docs/implementation-plan.md',
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

function cleanup(root) {
  const resolved = path.resolve(root);
  assert.equal(path.dirname(resolved), path.resolve(tmpdir()));
  assert.ok(path.basename(resolved).startsWith('rustok-blog-comments-port-'));
  rmSync(resolved, { recursive: true, force: true });
}

function rejects(mutator) {
  const root = fixture(mutator);
  try {
    return run(root);
  } finally {
    cleanup(root);
  }
}

function removeMarker(file, marker) {
  return (root) => mutate(root, file, (source) => {
    assert.ok(source.includes(marker), `negative fixture marker is missing: ${file}: ${marker}`);
    return source.replaceAll(marker, '');
  });
}

test('accepts the canonical Blog Comments consumer and cached snapshot boundary', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    cleanup(root);
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

test('rejects removal of cached outage disclosure from the actual Fluent catalog', () => {
  const result = rejects(
    removeMarker(files.storefrontEnglish,
      'blog-comments-unavailableCached = Comments are temporarily unavailable. Showing a recent cached snapshot.'),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /en\.ftl: missing nonempty Blog comments unavailableCached disclosure/);
});

test('rejects the removal of an outage catalog entry in the second locale', () => {
  const result = rejects((root) => mutate(root, files.storefrontRussian, (source) =>
    source.replace(/^blog-comments-timeoutCached = .+$/m, '')));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /ru\.ftl: missing nonempty Blog comments timeoutCached disclosure/);
});

test('rejects a whitespace-only cached outage catalog disclosure', () => {
  const result = rejects((root) => mutate(root, files.storefrontEnglish, (source) =>
    source.replace(/^blog-comments-timeoutCached = .+$/m, 'blog-comments-timeoutCached =   ')));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /en\.ftl: missing nonempty Blog comments timeoutCached disclosure/);
});

test('rejects a pure availability policy that enables replies during an outage', () => {
  const result = rejects((root) => mutate(root, files.storefrontPolicy, (source) => {
    const marker = 'can_reply: comments.availability == Available';
    assert.ok(source.includes(marker));
    return source.replace(marker, 'can_reply: true');
  }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /core\/discussion\.rs: missing can_reply/);
});

test('rejects policy drift that drops the cached-outage snapshot gate', () => {
  const result = rejects(removeMarker(files.storefrontPolicy, '|| comments.cached_snapshot'));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /core\/discussion\.rs: missing show_snapshot/);
});

test('rejects removal of the disabled discussion section policy', () => {
  const result = rejects(removeMarker(files.storefrontPolicy,
    'show_section: comments.availability != Disabled'));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /core\/discussion\.rs: missing show_section/);
});

test('rejects detaching the pure discussion policy from the actual core module', () => {
  const result = rejects(removeMarker(files.storefrontCore, 'pub mod discussion;'));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /core\.rs: missing pub mod discussion/);
});

test('rejects a comments renderer that exposes replies for cached read-only data', () => {
  const result = rejects(removeMarker(files.storefrontCommentsList,
    'on_reply.filter(|_| state.can_reply)'));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /comments_list\.rs: missing on_reply\.filter/);
});

test('rejects a comments renderer that stops localizing policy disclosures', () => {
  const result = rejects(removeMarker(files.storefrontCommentsList,
    '.map(|(key, fallback)| t(locale.as_deref(), key, fallback))'));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /comments_list\.rs: missing state \.degraded_message/);
});

test('rejects a composer that uses snapshot readability as write availability', () => {
  const result = rejects((root) => mutate(root, files.storefrontDiscussion, (source) => {
    assert.ok(source.includes(').can_reply'));
    return source.replace(').can_reply', ').show_snapshot');
  }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /ui\/comment\.rs: missing crate::core::discussion::comments_view/);
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
  const result = rejects((root) => writeFileSync(path.join(root, files.plan), ''));
  assert.notEqual(result.status, 0);
});
