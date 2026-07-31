#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve(
  'scripts/verify/verify-blog-comments-admin-native-port-injection.mjs',
);
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-admin-native-port-injection.json';
const adminAdapterPath =
  'crates/rustok-blog/admin/src/transport/native_server_adapter.rs';
const servicePath = 'crates/rustok-blog/src/services/comment.rs';
const consumerMatrixPath =
  'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const harnessTest =
  'transport::native_server_adapter::tests::admin_native_runtime_exposes_comments_port_selection';
const harnessCommand =
  `cargo test -p rustok-blog-admin --features ssr ${harnessTest} -- --exact`;

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingTenantBinding = false,
  missingLookup = false,
  missingInjected = false,
  missingFallback = false,
  missingListHandoff = false,
  missingMutationHandoff = false,
  directConstruction = false,
  missingPermission = false,
  permissionAfterSelector = false,
  broadListFallback = false,
  swallowedMutation = false,
  missingHarness = false,
  runtimePromoted = false,
  remotePromoted = false,
  registrationPromoted = false,
  planDrift = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-admin-native-comments-'));

  const permission = missingPermission
    ? ''
    : 'require_manage_permission(&context.auth)?;';
  const listSelector = missingListHandoff ? '' : 'comment_service(&context)';
  const mutationSelector = missingMutationHandoff ? '' : 'comment_service(&context)';
  const listAuthorization = permissionAfterSelector ? `${listSelector}\n${permission}` : `${permission}\n${listSelector}`;
  const mutationAuthorization = permissionAfterSelector
    ? `${mutationSelector}\n${permission}`
    : `${permission}\n${mutationSelector}`;

  write(
    root,
    adminAdapterPath,
    `
use std::sync::Arc;
struct NativeContext {
comments_thread_port: Option<Arc<dyn rustok_blog::CommentsThreadPort>>
}
let runtime = expect_context::<HostRuntimeContext>();
${missingTenantBinding ? '' : 'if auth.tenant_id != tenant.id'}
${missingLookup ? '' : 'runtime.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>()'}
fn comment_service(context: &NativeContext) -> rustok_blog::CommentService {
context.comments_thread_port.clone()
${missingInjected ? '' : 'rustok_blog::CommentService::with_comments_thread_port('}
${missingFallback ? '' : 'rustok_blog::CommentService::new(context.db.clone(), context.event_bus.clone())'}
}
${directConstruction ? 'rustok_blog::CommentService::new(context.db.clone(), context.event_bus.clone())' : ''}
fn require_manage_permission(
&[rustok_api::Permission::BLOG_POSTS_MANAGE]
"Permission denied: blog_posts:manage required"
#[server(prefix = "/api/fn", endpoint = "blog/admin/moderation-comments")]
${listAuthorization}
page: page.max(1)
per_page: per_page.clamp(1, 100)
.list_for_post_with_locale_fallback(
${broadListFallback ? 'Err(_) => BlogModerationCommentList' : '.map_err(ServerFnError::new)?;'}
#[server(prefix = "/api/fn", endpoint = "blog/admin/moderate-comment")]
${mutationAuthorization}
.moderate_comment(
${swallowedMutation ? 'unwrap_or_default()' : '.map_err(ServerFnError::new)?;'}
#[cfg(feature = "ssr")]
fn optional_text
${
  missingHarness
    ? ''
    : `
fn admin_native_runtime_exposes_comments_port_selection()
let selector: fn(&NativeContext) -> rustok_blog::CommentService = comment_service;
`
}
`,
  );

  write(
    root,
    servicePath,
    `
pub fn with_comments_thread_port(
.list_comments_for_target(
.get_comment(
.set_comment_status(
comments_read_port_context(
comments_write_port_context(
PortErrorKind::Unavailable => rustok_core::error::ErrorKind::ExternalService
PortErrorKind::Timeout => rustok_core::error::ErrorKind::Timeout
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

  const sourceVerified = ['in_process_fallback', 'host_injected_port_selection'];
  const pending = ['remote_transport_implementation'];
  if (remotePromoted) {
    sourceVerified.push('remote_transport_implementation');
    pending.splice(0, 1);
  }

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'comments_admin_native_port_injection',
      role: 'consumer',
      provider: 'comments',
      status: 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: runtimePromoted ? 'passed' : 'not_run',
      source_contract: {
        admin_adapter: adminAdapterPath,
        consumer_service: servicePath,
        consumer_matrix: consumerMatrixPath,
      },
      profiles: { source_verified: sourceVerified, pending },
      composition: {
        host_context: 'rustok_api::HostRuntimeContext',
        native_context: 'NativeContext',
        shared_value: 'Arc<dyn rustok_blog::CommentsThreadPort>',
        lookup: 'HostRuntimeContext::shared_get',
        selector: 'comment_service',
        injected_constructor: 'CommentService::with_comments_thread_port',
        fallback_constructor: 'CommentService::new',
        native_endpoints: [
          'blog/admin/moderation-comments',
          'blog/admin/moderate-comment',
        ],
        port_operations: [
          'list_comments_for_target',
          'get_comment',
          'set_comment_status',
        ],
      },
      authorization: {
        tenant_binding: 'AuthContext.tenant_id == TenantContext.id',
        permission: 'blog_posts:manage',
        checked_before_port_call: true,
      },
      moderation_policy: {
        read_pagination: 'page >= 1; 1 <= per_page <= 100',
        error_policy: 'all_blog_errors_propagated',
        storefront_degradation_reused: false,
      },
      harness: {
        status: 'executable_no_run',
        runtime_status: 'not_run',
        source: adminAdapterPath,
        test: harnessTest,
        command: harnessCommand,
      },
      registration: {
        standalone_verifier:
          'scripts/verify/verify-blog-comments-admin-native-port-injection.mjs',
        focused_fixture:
          'scripts/verify/verify-blog-comments-admin-native-port-injection.test.mjs',
        blog_fba_package_chain: registrationPromoted ? 'registered' : 'pending',
      },
    }),
  );

  write(
    root,
    planPath,
    planDrift
      ? ''
      : 'blog-comments-admin-native-port-injection.json verify-blog-comments-admin-native-port-injection.mjs verify-blog-comments-admin-native-port-injection.test.mjs admin native SSR Comments host selection is source-locked Blog FBA package-chain registration remains pending remote network transport remains pending Slice 65',
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

test('accepts the canonical admin native Comments composition boundary', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects removal of tenant binding', () => {
  assert.notEqual(rejects({ missingTenantBinding: true }).status, 0);
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

test('rejects direct provider construction outside the selector', () => {
  assert.notEqual(rejects({ directConstruction: true }).status, 0);
});

test('rejects removal of the moderation-list selector handoff', () => {
  assert.notEqual(rejects({ missingListHandoff: true }).status, 0);
});

test('rejects removal of the moderation-mutation selector handoff', () => {
  assert.notEqual(rejects({ missingMutationHandoff: true }).status, 0);
});

test('rejects removal of manage permission checks', () => {
  assert.notEqual(rejects({ missingPermission: true }).status, 0);
});

test('rejects permission checks after port selection', () => {
  assert.notEqual(rejects({ permissionAfterSelector: true }).status, 0);
});

test('rejects broad empty-success fallback for moderation lists', () => {
  assert.notEqual(rejects({ broadListFallback: true }).status, 0);
});

test('rejects swallowed moderation mutation errors', () => {
  assert.notEqual(rejects({ swallowedMutation: true }).status, 0);
});

test('rejects removal of the compile-only selector harness', () => {
  assert.notEqual(rejects({ missingHarness: true }).status, 0);
});

test('rejects runtime promotion without execution', () => {
  assert.notEqual(rejects({ runtimePromoted: true }).status, 0);
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
