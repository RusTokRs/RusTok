#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve('scripts/verify/verify-blog-comments-http-port-injection.mjs');
const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-http-port-injection.json';
const runtimePath = 'crates/rustok-blog/src/controllers/mod.rs';
const controllerPath = 'crates/rustok-blog/src/controllers/comments.rs';
const servicePath = 'crates/rustok-blog/src/services/comment.rs';
const matrixPath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const harnessTest = 'controllers::tests::blog_http_runtime_exposes_comments_port_selection';
const harnessCommand = `cargo test -p rustok-blog --lib ${harnessTest} -- --exact`;

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingSharedLookup = false,
  missingInjectedBranch = false,
  missingFallback = false,
  directControllerConstruction = false,
  missingHarness = false,
  runtimePromoted = false,
  remotePromoted = false,
  missingPlanMarker = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-comments-http-port-'));

  write(
    root,
    runtimePath,
    `
use rustok_comments::CommentsThreadPort;
use std::sync::Arc;
comments_thread_port: Option<Arc<dyn CommentsThreadPort>>
fn comment_service(&self) -> CommentService {
${missingInjectedBranch ? '' : `
if let Some(comments_thread_port) = self.comments_thread_port.clone() {
CommentService::with_comments_thread_port(self.db_clone(), comments_thread_port)
}
`}
${missingFallback ? '' : 'CommentService::new(self.db_clone(), self.event_bus())'}
}
${missingSharedLookup ? '' : 'comments_thread_port: runtime.shared_get::<Arc<dyn CommentsThreadPort>>()'}
${missingHarness ? '' : `
mod tests
fn blog_http_runtime_exposes_comments_port_selection()
let selector: fn(&BlogHttpRuntime) -> CommentService = BlogHttpRuntime::comment_service;
`}
`,
  );

  write(
    root,
    controllerPath,
    directControllerConstruction
      ? 'let service = CommentService::new(runtime.db_clone(), runtime.event_bus());'
      : 'let service = runtime.comment_service();',
  );

  write(
    root,
    servicePath,
    `
pub fn with_comments_thread_port(
comments_thread_port: Arc<dyn CommentsThreadPort>
`,
  );

  write(
    root,
    matrixPath,
    JSON.stringify({
      schema_version: 3,
      adapter_injection: {
        constructor: 'CommentService::with_comments_thread_port',
        runtime_status: 'not_run',
        remote_transport_implementation: 'pending',
      },
    }),
  );

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'comments_http_port_injection',
      role: 'consumer',
      provider: 'comments',
      status: 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: runtimePromoted ? 'passed' : 'not_run',
      source_contract: {
        http_runtime: runtimePath,
        moderation_controller: controllerPath,
        consumer_service: servicePath,
        consumer_matrix: matrixPath,
      },
      profiles: {
        source_verified: [
          'in_process_fallback',
          'host_injected_port_selection',
          ...(remotePromoted ? ['remote_transport_implementation'] : []),
        ],
        pending: remotePromoted ? [] : ['remote_transport_implementation'],
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
        runtime_status: runtimePromoted ? 'passed' : 'not_run',
        source: runtimePath,
        test: harnessTest,
        command: harnessCommand,
      },
      non_claims: [],
    }),
  );

  write(
    root,
    planPath,
    missingPlanMarker
      ? 'Slice 59 remote network transport remains pending'
      : 'blog-comments-http-port-injection.json verify-blog-comments-http-port-injection.mjs verify-blog-comments-http-port-injection.test.mjs BlogHttpRuntime::comment_service HTTP moderation remote network transport remains pending Slice 59',
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

test('accepts the canonical Blog Comments HTTP port injection boundary', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects removal of the HostRuntimeContext shared port lookup', () => {
  assert.notEqual(rejects({ missingSharedLookup: true }).status, 0);
});

test('rejects removal of the injected Comments port branch', () => {
  assert.notEqual(rejects({ missingInjectedBranch: true }).status, 0);
});

test('rejects removal of the in-process fallback', () => {
  assert.notEqual(rejects({ missingFallback: true }).status, 0);
});

test('rejects controller-level Comments service construction', () => {
  const result = rejects({ directControllerConstruction: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forbidden CommentService::new/);
});

test('rejects removal of the compile-only HTTP selection harness', () => {
  assert.notEqual(rejects({ missingHarness: true }).status, 0);
});

test('rejects runtime promotion without execution', () => {
  const result = rejects({ runtimePromoted: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /execution status drift|harness drift/);
});

test('rejects promotion of the unimplemented remote transport', () => {
  const result = rejects({ remotePromoted: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /profile drift/);
});

test('rejects removal of canonical plan bindings', () => {
  assert.notEqual(rejects({ missingPlanMarker: true }).status, 0);
});
