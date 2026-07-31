#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve('.');
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return '';
  }
  return readFileSync(target, 'utf8');
}

function json(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function requireNoMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function countMarker(source, marker) {
  return source.split(marker).length - 1;
}

function sameSet(actual, expected) {
  return [...actual].sort().join('|') === [...expected].sort().join('|');
}

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
const serverCodegenPath = 'apps/server/build.rs';
const serverSchemaPath = 'apps/server/src/graphql/schema.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const harnessTest =
  'graphql::runtime_data::tests::graphql_runtime_data_exposes_comments_port_selection';
const harnessCommand = `cargo test -p rustok-blog --lib ${harnessTest} -- --exact`;
const expectedOperations = ['public_comments', 'moderation_comments', 'moderate_comment'];

const evidence = json(evidencePath);
const manifest = read(manifestPath);
const graphqlModule = read(graphqlModulePath);
const runtimeData = read(runtimeDataPath);
const commentReads = read(commentReadsPath);
const commentMutation = read(commentMutationPath);
const service = read(servicePath);
const serverCodegen = read(serverCodegenPath);
const serverSchema = read(serverSchemaPath);
const plan = read(planPath);

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_graphql_port_injection' ||
    evidence.role !== 'consumer' ||
    evidence.provider !== 'comments'
  ) failures.push(`${evidencePath}: identity drift`);
  if (evidence.status !== 'source_verified_no_compile') {
    failures.push(`${evidencePath}: status drift`);
  }
  if (
    evidence.compile_policy !== 'not_run_by_request' ||
    evidence.runtime_status !== 'not_run'
  ) failures.push(`${evidencePath}: execution policy drift`);

  const expectedSources = {
    module_manifest: manifestPath,
    graphql_module: graphqlModulePath,
    runtime_data: runtimeDataPath,
    comment_reads: commentReadsPath,
    comment_mutation: commentMutationPath,
    consumer_service: servicePath,
    consumer_matrix: consumerMatrixPath,
    server_codegen: serverCodegenPath,
    server_schema: serverSchemaPath,
  };
  for (const [key, expected] of Object.entries(expectedSources)) {
    if (evidence.source_contract?.[key] !== expected) {
      failures.push(`${evidencePath}: ${key} source path drift`);
    }
  }

  if (
    !sameSet(evidence.profiles?.source_verified ?? [], [
      'in_process_fallback',
      'host_injected_port_selection',
    ])
  ) failures.push(`${evidencePath}: source-verified profile drift`);
  if (!sameSet(evidence.profiles?.pending ?? [], ['remote_transport_implementation'])) {
    failures.push(`${evidencePath}: pending profile drift`);
  }

  const composition = evidence.composition ?? {};
  if (
    composition.host_inputs !== 'rustok_api::graphql::GraphqlRuntimeInputs' ||
    composition.manifest_factory !== 'graphql::attach_schema_data' ||
    composition.schema_attachment !== 'schema_codegen::attach_module_graphql_data' ||
    composition.schema_data !== 'BlogGraphqlRuntimeData' ||
    composition.shared_value !== 'Arc<dyn CommentsThreadPort>' ||
    composition.lookup !== 'GraphqlRuntimeInputs::shared_get' ||
    composition.selector !== 'BlogGraphqlRuntimeData::comment_service' ||
    composition.injected_constructor !== 'CommentService::with_comments_thread_port' ||
    composition.fallback_constructor !== 'CommentService::new' ||
    !sameSet(composition.graphql_operations ?? [], expectedOperations)
  ) failures.push(`${evidencePath}: composition drift`);

  if (
    evidence.harness?.status !== 'executable_no_run' ||
    evidence.harness?.runtime_status !== 'not_run' ||
    evidence.harness?.source !== runtimeDataPath ||
    evidence.harness?.test !== harnessTest ||
    evidence.harness?.command !== harnessCommand
  ) failures.push(`${evidencePath}: harness drift`);

  if (
    evidence.registration?.standalone_verifier !==
      'scripts/verify/verify-blog-comments-graphql-port-injection.mjs' ||
    evidence.registration?.focused_fixture !==
      'scripts/verify/verify-blog-comments-graphql-port-injection.test.mjs' ||
    evidence.registration?.blog_fba_package_chain !== 'pending'
  ) failures.push(`${evidencePath}: registration drift`);
}

requireMarker(
  manifest,
  'runtime_data_factory = "graphql::attach_schema_data"',
  manifestPath,
);
for (const marker of [
  'mod runtime_data;',
  'pub use runtime_data::{BlogGraphqlRuntimeData, attach_schema_data};',
]) requireMarker(graphqlModule, marker, graphqlModulePath);

for (const marker of [
  'graphql_runtime_data_factory: Option<String>',
  'graphql_runtime_data_factory_expr',
  'builder = builder.data({factory}(inputs)?);',
]) requireMarker(serverCodegen, marker, serverCodegenPath);
requireMarker(
  serverSchema,
  'schema_codegen::attach_module_graphql_data(builder, &graphql_runtime_inputs)',
  serverSchemaPath,
);

for (const marker of [
  'use rustok_api::graphql::GraphqlRuntimeInputs;',
  'use rustok_comments::CommentsThreadPort;',
  'comments_thread_port: Option<Arc<dyn CommentsThreadPort>>',
  'pub fn attach_schema_data(',
  'inputs.shared_get::<Arc<dyn CommentsThreadPort>>()',
  'pub(crate) fn comment_service(',
  'match self.comments_thread_port.clone()',
  'Some(comments_thread_port)',
  'CommentService::with_comments_thread_port(db, comments_thread_port)',
  'None => CommentService::new(db, event_bus)',
  'fn graphql_runtime_data_exposes_comments_port_selection()',
  'let factory: fn(&GraphqlRuntimeInputs) -> Result<BlogGraphqlRuntimeData, String>',
  'BlogGraphqlRuntimeData::comment_service;',
]) requireMarker(runtimeData, marker, runtimeDataPath);

for (const marker of [
  'use super::runtime_data::BlogGraphqlRuntimeData;',
  'async fn public_comments(',
  'async fn moderation_comments(',
  'let service = runtime.comment_service(db.clone(), event_bus.clone());',
]) requireMarker(commentReads, marker, commentReadsPath);
if (countMarker(commentReads, 'ctx.data::<BlogGraphqlRuntimeData>()?;') !== 2) {
  failures.push(`${commentReadsPath}: expected two GraphQL runtime-data lookups`);
}
if (countMarker(commentReads, 'runtime.comment_service(db.clone(), event_bus.clone())') !== 2) {
  failures.push(`${commentReadsPath}: expected two runtime selector calls`);
}

for (const marker of [
  'use super::runtime_data::BlogGraphqlRuntimeData;',
  'async fn moderate_comment(',
  'let runtime = ctx.data::<BlogGraphqlRuntimeData>()?;',
  '.comment_service(db.clone(), event_bus.clone())',
]) requireMarker(commentMutation, marker, commentMutationPath);

for (const source of [commentReads, commentMutation]) {
  requireNoMarker(source, 'CommentService::new(', 'GraphQL resolver source');
  requireNoMarker(
    source,
    'CommentService::with_comments_thread_port(',
    'GraphQL resolver source',
  );
}

requireMarker(service, 'pub fn with_comments_thread_port(', servicePath);
requireMarker(
  service,
  'comments_thread_port: Arc<dyn CommentsThreadPort>,',
  servicePath,
);

for (const marker of [
  'blog-comments-graphql-port-injection.json',
  'verify-blog-comments-graphql-port-injection.mjs',
  'verify-blog-comments-graphql-port-injection.test.mjs',
  'BlogGraphqlRuntimeData',
  'graphql::attach_schema_data',
  'schema_codegen::attach_module_graphql_data',
  'GraphQL Comments host selection is source-locked',
  'Blog FBA package-chain registration remains pending',
  'remote network transport remains pending',
  'Slice 61',
]) requireMarker(plan, marker, planPath);

if (failures.length > 0) {
  console.error('Blog GraphQL Comments port injection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog GraphQL Comments manifest runtime-data and port selection source boundary is consistent');
