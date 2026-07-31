#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve(
  'scripts/verify/verify-blog-comments-graphql-port-injection.mjs',
);
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-graphql-port-injection.json';
const manifestPath = 'crates/rustok-blog/rustok-module.toml';
const graphqlModulePath = 'crates/rustok-blog/src/graphql/mod.rs';
const runtimeDataPath = 'crates/rustok-blog/src/graphql/runtime_data.rs';
const commentReadsPath = 'crates/rustok-blog/src/graphql/types.rs';
const commentMutationPath = 'crates/rustok-blog/src/graphql/mutation.rs';
const servicePath = 'crates/rustok-blog/src/services/comment.rs';
const consumerMatrixPath =
  'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const harnessTest =
  'graphql::runtime_data::tests::graphql_runtime_data_exposes_comments_port_selection';
const harnessCommand = `cargo test -p rustok-blog --lib ${harnessTest} -- --exact`;

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingManifestFactory = false,
  missingHostLookup = false,
  missingInjectedBranch = false,
  missingFallback = false,
  directReadConstruction = false,
  directMutationConstruction = false,
  missingHarness = false,
  runtimePromoted = false,
  remotePromoted = false,
  registrationPromoted = false,
  planDrift = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-graphql-comments-port-'));

  write(
    root,
    manifestPath,
    `[provides.graphql]\nquery = "graphql::BlogQuery"\nmutation = "graphql::BlogMutation"\n${
      missingManifestFactory ? '' : 'runtime_data_factory = "graphql::attach_schema_data"'
    }`,
  );
  write(
    root,
    graphqlModulePath,
    'mod runtime_data;\npub use runtime_data::{BlogGraphqlRuntimeData, attach_schema_data};',
  );

  write(
    root,
    runtimeDataPath,
    `
use rustok_api::graphql::GraphqlRuntimeInputs;
use rustok_comments::CommentsThreadPort;
comments_thread_port: Option<Arc<dyn CommentsThreadPort>>
pub fn attach_schema_data(
${missingHostLookup ? '' : 'inputs.shared_get::<Arc<dyn CommentsThreadPort>>()'}
pub(crate) fn comment_service(
match self.comments_thread_port.clone()
Some(comments_thread_port)
${
  missingInjectedBranch
    ? ''
    : 'CommentService::with_comments_thread_port(db, comments_thread_port)'
}
${missingFallback ? '' : 'None => CommentService::new(db, event_bus)'}
${
  missingHarness
    ? ''
    : `
fn graphql_runtime_data_exposes_comments_port_selection()
let factory: fn(&GraphqlRuntimeInputs) -> Result<BlogGraphqlRuntimeData, String>
BlogGraphqlRuntimeData::comment_service;
`
}
`,
  );

  write(
    root,
    commentReadsPath,
    `
use super::runtime_data::BlogGraphqlRuntimeData;
async fn public_comments(
let runtime = ctx.data::<BlogGraphqlRuntimeData>()?;
let service = runtime.comment_service(db.clone(), event_bus.clone());
async fn moderation_comments(
let runtime = ctx.data::<BlogGraphqlRuntimeData>()?;
let service = runtime.comment_service(db.clone(), event_bus.clone());
${directReadConstruction ? 'CommentService::new(db.clone(), event_bus.clone())' : ''}
`,
  );
  write(
    root,
    commentMutationPath,
    `
use super::runtime_data::BlogGraphqlRuntimeData;
async fn moderate_comment(
let runtime = ctx.data::<BlogGraphqlRuntimeData>()?;
runtime.comment_service(db.clone(), event_bus.clone())
${
  directMutationConstruction
    ? 'CommentService::with_comments_thread_port(db.clone(), comments_thread_port)'
    : ''
}
`,
  );
  write(
    root,
    servicePath,
    'pub fn with_comments_thread_port(\ncomments_thread_port: Arc<dyn CommentsThreadPort>,',
  );
  write(root, consumerMatrixPath, '{}');

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'comments_graphql_port_injection',
      role: 'consumer',
      provider: 'comments',
      status: 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: runtimePromoted ? 'passed' : 'not_run',
      source_contract: {
        module_manifest: manifestPath,
        graphql_module: graphqlModulePath,
        runtime_data: runtimeDataPath,
        comment_reads: commentReadsPath,
        comment_mutation: commentMutationPath,
        consumer_service: servicePath,
        consumer_matrix: consumerMatrixPath,
      },
      profiles: {
        source_verified: remotePromoted
          ? ['in_process_fallback', 'host_injected_port_selection', 'remote_transport']
          : ['in_process_fallback', 'host_injected_port_selection'],
        pending: remotePromoted ? [] : ['remote_transport_implementation'],
      },
      composition: {
        host_inputs: 'rustok_api::graphql::GraphqlRuntimeInputs',
        manifest_factory: 'graphql::attach_schema_data',
        schema_data: 'BlogGraphqlRuntimeData',
        shared_value: 'Arc<dyn CommentsThreadPort>',
        lookup: 'GraphqlRuntimeInputs::shared_get',
        selector: 'BlogGraphqlRuntimeData::comment_service',
        injected_constructor: 'CommentService::with_comments_thread_port',
        fallback_constructor: 'CommentService::new',
        graphql_operations: ['public_comments', 'moderation_comments', 'moderate_comment'],
      },
      harness: {
        status: 'executable_no_run',
        runtime_status: 'not_run',
        source: runtimeDataPath,
        test: harnessTest,
        command: harnessCommand,
      },
      registration: {
        standalone_verifier:
          'scripts/verify/verify-blog-comments-graphql-port-injection.mjs',
        focused_fixture:
          'scripts/verify/verify-blog-comments-graphql-port-injection.test.mjs',
        blog_fba_package_chain: registrationPromoted ? 'registered' : 'pending',
      },
    }),
  );

  write(
    root,
    planPath,
    planDrift
      ? ''
      : 'blog-comments-graphql-port-injection.json verify-blog-comments-graphql-port-injection.mjs verify-blog-comments-graphql-port-injection.test.mjs BlogGraphqlRuntimeData graphql::attach_schema_data GraphQL Comments host selection is source-locked Blog FBA package-chain registration remains pending remote network transport remains pending Slice 61',
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

test('accepts canonical Blog GraphQL Comments port injection source', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects removal of the manifest runtime-data factory', () => {
  assert.notEqual(rejects({ missingManifestFactory: true }).status, 0);
});

test('rejects removal of the host shared-value lookup', () => {
  assert.notEqual(rejects({ missingHostLookup: true }).status, 0);
});

test('rejects removal of the injected selector branch', () => {
  assert.notEqual(rejects({ missingInjectedBranch: true }).status, 0);
});

test('rejects removal of the in-process fallback branch', () => {
  assert.notEqual(rejects({ missingFallback: true }).status, 0);
});

test('rejects direct provider construction in GraphQL comment reads', () => {
  assert.notEqual(rejects({ directReadConstruction: true }).status, 0);
});

test('rejects direct provider construction in the GraphQL comment mutation', () => {
  assert.notEqual(rejects({ directMutationConstruction: true }).status, 0);
});

test('rejects removal of the compile-only runtime-data harness', () => {
  assert.notEqual(rejects({ missingHarness: true }).status, 0);
});

test('rejects runtime promotion without execution', () => {
  assert.notEqual(rejects({ runtimePromoted: true }).status, 0);
});

test('rejects remote transport promotion without an implementation', () => {
  assert.notEqual(rejects({ remotePromoted: true }).status, 0);
});

test('rejects unearned Blog FBA package-chain registration', () => {
  assert.notEqual(rejects({ registrationPromoted: true }).status, 0);
});

test('rejects canonical-plan drift', () => {
  assert.notEqual(rejects({ planDrift: true }).status, 0);
});
