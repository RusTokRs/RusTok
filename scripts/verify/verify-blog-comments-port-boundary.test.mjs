#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve('scripts/verify/verify-blog-comments-port-boundary.mjs');
const operations = [
  'create_comment',
  'get_comment',
  'list_comments_for_target',
  'list_public_comments_for_target',
  'update_comment',
  'set_comment_status',
  'delete_comment',
];

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  directBypass = false,
  missingDeadline = false,
  missingIdempotency = false,
  missingPublicRead = false,
  statusDrift = false,
  fallbackPromoted = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-comments-port-'));
  const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
  const fallbackEvidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json';
  const servicePath = 'crates/rustok-blog/src/services/comment.rs';
  const providerRegistryPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
  const consumerRegistryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';

  write(
    root,
    servicePath,
    `
comments_thread_port: Arc<dyn CommentsThreadPort>
comments_thread_port: in_process_comments_thread_port(db.clone(), event_bus)
.comments_thread_port
.create_comment(
.get_comment(
.list_comments_for_target(
${missingPublicRead ? '' : '.list_public_comments_for_target('}
.update_comment(
.set_comment_status(
.delete_comment(
comments_write_port_context(
comments_read_port_context(
comments_public_read_port_context(
PortActor::service(PUBLIC_COMMENTS_PORT_ACTOR)
${missingDeadline ? '' : '.with_deadline(std::time::Duration::from_secs(2))'}
${missingIdempotency ? '' : '.with_idempotency_key(format!("{correlation_id}:command:{command_id}"))'}
PortErrorKind::NotFound => rustok_core::error::ErrorKind::NotFound
PortErrorKind::Forbidden => rustok_core::error::ErrorKind::Forbidden
PortErrorKind::Validation => rustok_core::error::ErrorKind::Validation
PortErrorKind::Unavailable => rustok_core::error::ErrorKind::ExternalService
PortErrorKind::Timeout => rustok_core::error::ErrorKind::Timeout
BlogError::Rich(Box::new(
.with_error_code(error.code)
body: input.content
content: record.body
content_text: record.body_text
Self::ensure_blog_target(&existing)?
ensure_post_exists
DomainCreateCommentInput
TARGET_TYPE_BLOG_POST
post_id
${directBypass ? '.comments.get_comment(' : ''}
`,
  );

  const cases = operations.map((operation) => ({
    operation,
    assertions: ['typed_port_error_mapping', 'context_deadline_preserved'],
    runtime_evidence: 'pending',
  }));
  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 2,
      module: 'blog',
      surface: 'comments_port_boundary',
      role: 'consumer',
      provider: 'comments',
      generated_from: consumerRegistryPath,
      status: statusDrift ? 'runtime_verified' : 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      source_contract: {
        consumer_service: servicePath,
        provider_registry: providerRegistryPath,
        consumer_registry: consumerRegistryPath,
      },
      profiles: {
        source_verified: ['in_process'],
        pending: ['remote_adapter_placeholder'],
      },
      cases,
      fallback_smoke: {
        status: 'planned',
        profiles: ['embedded_native'],
        degraded_modes: ['hide_comment_form', 'show_cached_thread_snapshot'],
        runtime_evidence: 'pending',
      },
    }),
  );

  write(
    root,
    fallbackEvidencePath,
    JSON.stringify({
      schema_version: 2,
      module: 'blog',
      role: 'consumer',
      provider: 'comments',
      generated_from: consumerRegistryPath,
      status: 'source_verified_no_compile',
      runner: 'scripts/verify/verify-blog-comments-port-boundary.mjs',
      compile_policy: 'not_run_by_request',
      runtime_status: 'not_run',
      source_contract: {
        consumer_service: servicePath,
        consumer_error_mapping: servicePath,
        provider_port_registry: providerRegistryPath,
      },
      fallback_smoke: {
        status: 'planned',
        profiles: ['embedded_native'],
        degraded_modes: ['hide_comment_form', 'show_cached_thread_snapshot'],
        cases: [
          {
            operation: 'create_comment',
            source_markers: ['ensure_post_exists', 'DomainCreateCommentInput', 'comments_thread_port', 'comments_write_port_context'],
            typed_error_markers: ['PortErrorKind::Forbidden', 'PortErrorKind::Validation', 'PortErrorKind::Unavailable', 'ErrorKind::ExternalService', 'with_error_code(error.code)'],
            degraded_mode: 'hide_comment_form',
          },
          {
            operation: 'list_comments_for_target',
            source_markers: ['list_comments_for_target', 'TARGET_TYPE_BLOG_POST', 'post_id', 'comments_read_port_context'],
            typed_error_markers: ['PortErrorKind::NotFound', 'PortErrorKind::Unavailable', 'PortErrorKind::Timeout', 'ErrorKind::ExternalService', 'ErrorKind::Timeout', 'with_error_code(error.code)'],
            degraded_mode: 'show_cached_thread_snapshot',
          },
        ],
        runtime_evidence: 'pending',
      },
    }),
  );

  write(
    root,
    providerRegistryPath,
    JSON.stringify({
      ports: [{ name: 'CommentsThreadPort', operations }],
    }),
  );
  write(
    root,
    consumerRegistryPath,
    JSON.stringify({
      provider_dependencies: [{ module: 'comments', port: 'CommentsThreadPort', operations }],
      contract_tests: {
        status: 'source_verified_no_compile',
        runtime_status: 'pending',
        cases: operations.map((operation) => ({ operation })),
        fallback_smoke: {
          status: fallbackPromoted ? 'source_verified_no_compile' : 'planned',
        },
      },
    }),
  );
  write(
    root,
    'crates/rustok-blog/docs/implementation-plan.md',
    'blog-comments-consumer-static-matrix.json blog-comments-runtime-fallback-smoke.json verify:blog:comments-port-boundary test:verify:blog:comments-port-boundary source_verified_no_compile degraded UI modes remain planned',
  );

  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve('.'),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
}

function rejects(options) {
  const root = fixture(options);
  try {
    return run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('accepts the canonical Blog Comments consumer port boundary', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a direct CommentsService bypass', () => {
  assert.notEqual(rejects({ directBypass: true }).status, 0);
});

test('rejects a port context without deadline', () => {
  assert.notEqual(rejects({ missingDeadline: true }).status, 0);
});

test('rejects writes without idempotency keys', () => {
  assert.notEqual(rejects({ missingIdempotency: true }).status, 0);
});

test('rejects removal of the approved public-read port operation', () => {
  assert.notEqual(rejects({ missingPublicRead: true }).status, 0);
});

test('rejects runtime status promotion without execution', () => {
  const result = rejects({ statusDrift: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /status drift/);
});

test('rejects source promotion of planned degraded UI modes', () => {
  const result = rejects({ fallbackPromoted: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /contract-test status drift/);
});
