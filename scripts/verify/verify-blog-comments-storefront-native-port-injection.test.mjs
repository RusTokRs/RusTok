#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
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
const harnessTest =
  'transport::native_server_adapter::tests::storefront_native_runtime_exposes_comments_port_selection';
const harnessCommand =
  `cargo test -p rustok-blog-storefront --features ssr ${harnessTest} -- --exact`;

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingFacade = false,
  missingLookup = false,
  missingInjected = false,
  missingFallback = false,
  directReadConstruction = false,
  missingSelectorHandoff = false,
  missingPublicRead = false,
  broadFallback = false,
  missingAvailability = false,
  missingHarness = false,
  runtimePromoted = false,
  adminPromoted = false,
  remotePromoted = false,
  registrationPromoted = false,
  planDrift = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-storefront-native-comments-'));

  write(
    root,
    facadePath,
    missingFacade ? '' : 'pub use rustok_comments::CommentsThreadPort;',
  );

  write(
    root,
    nativeAdapterPath,
    `
use std::sync::Arc;
#[server(prefix = "/api/fn", endpoint = "blog/storefront-data")]
let runtime_ctx = expect_context::<HostRuntimeContext>();
fn comment_service(
runtime_ctx: &rustok_api::HostRuntimeContext,
${missingLookup ? '' : 'runtime_ctx.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>()'}
${missingInjected ? '' : 'rustok_blog::CommentService::with_comments_thread_port('}
${missingFallback ? '' : 'rustok_blog::CommentService::new(runtime_ctx.db_clone(), event_bus)'}
${
  missingSelectorHandoff
    ? ''
    : 'comment_service(&runtime_ctx, event_bus.clone())'
}
${directReadConstruction ? 'rustok_blog::CommentService::new(runtime_ctx.db_clone(), event_bus.clone())' : ''}
.list_for_post_with_locale_fallback(
SecurityContext::public_read()
fn comments_read_availability(
ErrorKind::ExternalService
${missingAvailability ? '' : 'BlogCommentsAvailability::Unavailable'}
ErrorKind::Timeout
${missingAvailability ? '' : 'BlogCommentsAvailability::Timeout'}
let Some(availability) = comments_read_availability(&error) else
return Err(ServerFnError::new(error));
${broadFallback ? 'Err(_) => BlogCommentList' : ''}
${
  missingHarness
    ? ''
    : `
fn storefront_native_runtime_exposes_comments_port_selection()
) -> rustok_blog::CommentService = comment_service;
`
}
`,
  );

  write(
    root,
    servicePath,
    `
pub fn with_comments_thread_port(
${missingPublicRead ? '' : '.list_public_comments_for_target('}
comments_public_read_port_context(
PortActor::service(PUBLIC_COMMENTS_PORT_ACTOR)
`,
  );

  write(
    root,
    consumerMatrixPath,
    JSON.stringify({
      schema_version: 3,
      status: 'source_verified_no_compile',
      profiles: {
        source_verified: ['in_process'],
        pending: ['remote_adapter_placeholder'],
      },
    }),
  );

  write(
    root,
    fallbackEvidencePath,
    JSON.stringify({
      schema_version: 2,
      status: 'source_verified_no_compile',
      runtime_status: 'not_run',
      storefront_read_degradation: {
        availability_states: ['AVAILABLE', 'UNAVAILABLE', 'TIMEOUT'],
        degraded_error_kinds: ['ExternalService', 'Timeout'],
        propagated_error_policy: 'all_other_blog_errors',
      },
    }),
  );

  const sourceVerified = ['in_process_fallback', 'host_injected_port_selection'];
  const pending = ['admin_native_ssr_composition', 'remote_transport_implementation'];
  if (adminPromoted) {
    sourceVerified.push('admin_native_ssr_composition');
    pending.splice(pending.indexOf('admin_native_ssr_composition'), 1);
  }
  if (remotePromoted) {
    sourceVerified.push('remote_transport_implementation');
    pending.splice(pending.indexOf('remote_transport_implementation'), 1);
  }

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'comments_storefront_native_port_injection',
      role: 'consumer',
      provider: 'comments',
      status: 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: runtimePromoted ? 'passed' : 'not_run',
      source_contract: {
        blog_facade: facadePath,
        native_adapter: nativeAdapterPath,
        consumer_service: servicePath,
        consumer_matrix: consumerMatrixPath,
        fallback_evidence: fallbackEvidencePath,
      },
      profiles: {
        source_verified: sourceVerified,
        pending,
      },
      composition: {
        host_context: 'rustok_api::HostRuntimeContext',
        shared_value: 'Arc<dyn rustok_blog::CommentsThreadPort>',
        facade_reexport: 'pub use rustok_comments::CommentsThreadPort;',
        lookup: 'HostRuntimeContext::shared_get',
        selector: 'comment_service',
        injected_constructor: 'CommentService::with_comments_thread_port',
        fallback_constructor: 'CommentService::new',
        native_endpoint: 'blog/storefront-data',
        operation: 'list_public_comments_for_target',
      },
      availability: {
        states: ['AVAILABLE', 'UNAVAILABLE', 'TIMEOUT'],
        degraded_error_kinds: ['ExternalService', 'Timeout'],
        propagated_error_policy: 'all_other_blog_errors',
      },
      harness: {
        status: 'executable_no_run',
        runtime_status: 'not_run',
        source: nativeAdapterPath,
        test: harnessTest,
        command: harnessCommand,
      },
      registration: {
        standalone_verifier:
          'scripts/verify/verify-blog-comments-storefront-native-port-injection.mjs',
        focused_fixture:
          'scripts/verify/verify-blog-comments-storefront-native-port-injection.test.mjs',
        blog_fba_package_chain: registrationPromoted ? 'registered' : 'pending',
      },
    }),
  );

  write(
    root,
    planPath,
    planDrift
      ? ''
      : 'blog-comments-storefront-native-port-injection.json verify-blog-comments-storefront-native-port-injection.mjs verify-blog-comments-storefront-native-port-injection.test.mjs storefront native SSR Comments host selection is source-locked admin native SSR composition remains pending Blog FBA package-chain registration remains pending remote network transport remains pending Slice 63',
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
  assert.notEqual(rejects({ missingFacade: true }).status, 0);
});

test('rejects removal of the host shared-value lookup', () => {
  assert.notEqual(rejects({ missingLookup: true }).status, 0);
});

test('rejects removal of the injected selector branch', () => {
  assert.notEqual(rejects({ missingInjected: true }).status, 0);
});

test('rejects removal of the in-process fallback branch', () => {
  assert.notEqual(rejects({ missingFallback: true }).status, 0);
});

test('rejects direct provider construction in the storefront public read', () => {
  assert.notEqual(rejects({ directReadConstruction: true }).status, 0);
});

test('rejects removal of the storefront selector handoff', () => {
  assert.notEqual(rejects({ missingSelectorHandoff: true }).status, 0);
});

test('rejects removal of the approved-only public port operation', () => {
  assert.notEqual(rejects({ missingPublicRead: true }).status, 0);
});

test('rejects broad storefront degradation for every error', () => {
  assert.notEqual(rejects({ broadFallback: true }).status, 0);
});

test('rejects removal of typed unavailable and timeout states', () => {
  assert.notEqual(rejects({ missingAvailability: true }).status, 0);
});

test('rejects removal of the compile-only selector harness', () => {
  assert.notEqual(rejects({ missingHarness: true }).status, 0);
});

test('rejects runtime promotion without execution', () => {
  assert.notEqual(rejects({ runtimePromoted: true }).status, 0);
});

test('rejects admin native SSR promotion without implementation', () => {
  assert.notEqual(rejects({ adminPromoted: true }).status, 0);
});

test('rejects remote transport promotion without implementation', () => {
  assert.notEqual(rejects({ remotePromoted: true }).status, 0);
});

test('rejects unearned Blog FBA package-chain registration', () => {
  assert.notEqual(rejects({ registrationPromoted: true }).status, 0);
});

test('rejects canonical-plan drift', () => {
  assert.notEqual(rejects({ planDrift: true }).status, 0);
});
