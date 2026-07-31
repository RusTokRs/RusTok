#!/usr/bin/env node

import './verify-blog-comments-http-port-injection.test.mjs';
import './verify-blog-comments-graphql-port-injection.test.mjs';
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
const injectionConstructor = 'CommentService::with_comments_thread_port';
const injectionSignature = 'fn(DatabaseConnection, Arc<dyn CommentsThreadPort>) -> CommentService';
const injectionTest =
  'services::comment::port_injection_tests::comment_service_accepts_an_injected_comments_thread_port';
const injectionCommand =
  `cargo test -p rustok-blog --lib ${injectionTest} -- --exact`;
const httpHarnessTest = 'controllers::tests::blog_http_runtime_exposes_comments_port_selection';
const httpHarnessCommand = `cargo test -p rustok-blog --lib ${httpHarnessTest} -- --exact`;

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
  missingAvailabilityField = false,
  broadNativeFallback = false,
  missingGraphqlAvailability = false,
  missingUiState = false,
  missingInjectionConstructor = false,
  missingInjectionHarness = false,
  injectionRuntimePromoted = false,
  statusDrift = false,
  fallbackPromoted = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-comments-port-'));
  const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
  const fallbackEvidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json';
  const httpEvidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-http-port-injection.json';
  const servicePath = 'crates/rustok-blog/src/services/comment.rs';
  const httpRuntimePath = 'crates/rustok-blog/src/controllers/mod.rs';
  const httpControllerPath = 'crates/rustok-blog/src/controllers/comments.rs';
  const graphqlOwnerPath = 'crates/rustok-blog/src/graphql/types.rs';
  const storefrontModelPath = 'crates/rustok-blog/storefront/src/model.rs';
  const storefrontGraphqlPath = 'crates/rustok-blog/storefront/src/transport/graphql_adapter.rs';
  const storefrontNativePath = 'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs';
  const storefrontUiPath = 'crates/rustok-blog/storefront/src/ui/leptos.rs';
  const providerRegistryPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
  const consumerRegistryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';

  write(
    root,
    servicePath,
    `
comments_thread_port: Arc<dyn CommentsThreadPort>
${missingInjectionConstructor ? '' : `
let comments_thread_port = in_process_comments_thread_port(db.clone(), event_bus);
Self::with_comments_thread_port(db, comments_thread_port)
pub fn with_comments_thread_port(
comments_thread_port: Arc<dyn CommentsThreadPort>,
Self {
            db,
            comments_thread_port,
        }
`}
${missingInjectionHarness ? '' : `
mod port_injection_tests
fn comment_service_accepts_an_injected_comments_thread_port()
) -> CommentService = CommentService::with_comments_thread_port;
`}
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

  write(
    root,
    httpRuntimePath,
    `
use rustok_comments::CommentsThreadPort;
use std::sync::Arc;
comments_thread_port: Option<Arc<dyn CommentsThreadPort>>
fn comment_service(&self) -> CommentService {
if let Some(comments_thread_port) = self.comments_thread_port.clone() {
CommentService::with_comments_thread_port(self.db_clone(), comments_thread_port)
} else {
CommentService::new(self.db_clone(), self.event_bus())
}
}
comments_thread_port: runtime.shared_get::<Arc<dyn CommentsThreadPort>>()
mod tests
fn blog_http_runtime_exposes_comments_port_selection()
let selector: fn(&BlogHttpRuntime) -> CommentService = BlogHttpRuntime::comment_service;
`,
  );
  write(root, httpControllerPath, 'let service = runtime.comment_service();');

  write(
    root,
    storefrontModelPath,
    `
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlogCommentsAvailability {
Available,
Unavailable,
Timeout,
}
${missingAvailabilityField ? '' : 'pub availability: BlogCommentsAvailability'}
`,
  );
  write(
    root,
    storefrontNativePath,
    `
fn comments_read_availability(
rustok_core::error::ErrorKind::ExternalService
Some(BlogCommentsAvailability::Unavailable)
rustok_core::error::ErrorKind::Timeout
Some(BlogCommentsAvailability::Timeout)
let Some(availability) = comments_read_availability(&error) else
return Err(ServerFnError::new(error));
availability: BlogCommentsAvailability::Available
items: Vec::new()
total: 0
${broadNativeFallback ? 'Err(_) => BlogCommentList' : ''}
`,
  );
  write(
    root,
    graphqlOwnerPath,
    `
pub enum GqlBlogCommentsAvailability
pub availability: GqlBlogCommentsAvailability
fn graphql_comments_read_availability(error: &BlogError)
ErrorKind::ExternalService => Some(GqlBlogCommentsAvailability::Unavailable)
ErrorKind::Timeout => Some(GqlBlogCommentsAvailability::Timeout)
${missingGraphqlAvailability ? '' : 'let Some(availability) = graphql_comments_read_availability(&error) else'}
return Err(async_graphql::Error::new(error.to_string()));
GqlBlogCommentsAvailability::Available
`,
  );
  write(
    root,
    storefrontGraphqlPath,
    'publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage) { availability total items',
  );
  write(
    root,
    storefrontUiPath,
    missingUiState
      ? ''
      : `
comments.availability != BlogCommentsAvailability::Available
BlogCommentsAvailability::Unavailable
BlogCommentsAvailability::Timeout
Comments are temporarily unavailable. The article is still available.
Comments took too long to load. The article is still available.
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
      schema_version: 3,
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
        injection_constructor: injectionConstructor,
      },
      profiles: {
        source_verified: ['in_process'],
        pending: ['remote_adapter_placeholder'],
      },
      adapter_injection: {
        status: 'executable_no_run',
        runtime_status: injectionRuntimePromoted ? 'passed' : 'not_run',
        source: servicePath,
        constructor: injectionConstructor,
        signature: injectionSignature,
        test: injectionTest,
        command: injectionCommand,
        default_profile: 'in_process',
        remote_transport_implementation: 'pending',
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
    httpEvidencePath,
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'comments_http_port_injection',
      role: 'consumer',
      provider: 'comments',
      status: 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: 'not_run',
      source_contract: {
        http_runtime: httpRuntimePath,
        moderation_controller: httpControllerPath,
        consumer_service: servicePath,
        consumer_matrix: evidencePath,
      },
      profiles: {
        source_verified: ['in_process_fallback', 'host_injected_port_selection'],
        pending: ['remote_transport_implementation'],
      },
      composition: {
        host_context: 'rustok_api::HostRuntimeContext',
        shared_value: 'Arc<dyn CommentsThreadPort>',
        lookup: 'HostRuntimeContext::shared_get',
        selector: 'BlogHttpRuntime::comment_service',
        injected_constructor: 'CommentService::with_comments_thread_port',
        fallback_constructor: 'CommentService::new',
        http_operation: 'moderate_comment',
      },
      harness: {
        status: 'executable_no_run',
        runtime_status: 'not_run',
        source: httpRuntimePath,
        test: httpHarnessTest,
        command: httpHarnessCommand,
      },
      non_claims: [],
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
        graphql_owner: graphqlOwnerPath,
        storefront_model: storefrontModelPath,
        storefront_graphql: storefrontGraphqlPath,
        storefront_native: storefrontNativePath,
        storefront_ui: storefrontUiPath,
      },
      storefront_read_degradation: {
        status: 'source_verified_no_compile',
        runtime_status: 'not_run',
        operation: 'list_public_comments_for_target',
        transports: ['graphql', 'native_ssr'],
        availability_states: ['AVAILABLE', 'UNAVAILABLE', 'TIMEOUT'],
        degraded_error_kinds: ['ExternalService', 'Timeout'],
        propagated_error_policy: 'all_other_blog_errors',
        degraded_payload: { items: [], total: 0 },
        cached_thread_snapshot: 'planned',
        comment_form_fallback: 'planned',
        runtime_evidence: 'pending',
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

  write(root, providerRegistryPath, JSON.stringify({ ports: [{ name: 'CommentsThreadPort', operations }] }));
  write(
    root,
    consumerRegistryPath,
    JSON.stringify({
      provider_dependencies: [{ module: 'comments', port: 'CommentsThreadPort', operations }],
      contract_tests: {
        status: 'source_verified_no_compile',
        runtime_status: 'pending',
        adapter_injection: {
          status: 'executable_no_run',
          runtime_status: injectionRuntimePromoted ? 'passed' : 'not_run',
          source: servicePath,
          constructor: injectionConstructor,
          signature: injectionSignature,
          test: injectionTest,
          command: injectionCommand,
          remote_transport_implementation: 'pending',
        },
        cases: operations.map((operation) => ({ operation })),
        fallback_smoke: { status: fallbackPromoted ? 'source_verified_no_compile' : 'planned' },
      },
    }),
  );
  write(
    root,
    'crates/rustok-blog/docs/implementation-plan.md',
    'blog-comments-consumer-static-matrix.json blog-comments-runtime-fallback-smoke.json blog-comments-http-port-injection.json verify-blog-comments-http-port-injection.mjs verify-blog-comments-http-port-injection.test.mjs verify:blog:comments-port-boundary test:verify:blog:comments-port-boundary source_verified_no_compile typed storefront comments availability CommentService::with_comments_thread_port BlogHttpRuntime::comment_service HTTP moderation remote transport remains pending remote network transport remains pending cached snapshot and comment-form fallback remain planned Slice 59 Slice 60',
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

test('rejects removal of the Comments port injection constructor', () => {
  const result = rejects({ missingInjectionConstructor: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /with_comments_thread_port/);
});

test('rejects removal of the compile-only injection harness', () => {
  const result = rejects({ missingInjectionHarness: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /port_injection_tests|injected_comments_thread_port/);
});

test('rejects runtime promotion of the unexecuted injection harness', () => {
  const result = rejects({ injectionRuntimePromoted: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /adapter injection drift/);
});

test('rejects a storefront model without typed availability', () => {
  assert.notEqual(rejects({ missingAvailabilityField: true }).status, 0);
});

test('rejects broad native fallback for every error', () => {
  const result = rejects({ broadNativeFallback: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forbidden Err\(_\) => BlogCommentList/);
});

test('rejects GraphQL degradation without typed classification', () => {
  assert.notEqual(rejects({ missingGraphqlAvailability: true }).status, 0);
});

test('rejects removal of the storefront unavailable and timeout state', () => {
  assert.notEqual(rejects({ missingUiState: true }).status, 0);
});

test('rejects runtime status promotion without execution', () => {
  const result = rejects({ statusDrift: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /status drift/);
});

test('rejects source promotion of planned cached snapshot and form fallback', () => {
  const result = rejects({ fallbackPromoted: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /contract-test status drift/);
});
