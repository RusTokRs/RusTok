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

const evidence = json(evidencePath);
const consumerMatrix = json(consumerMatrixPath);
const fallbackEvidence = json(fallbackEvidencePath);
const facade = read(facadePath);
const nativeAdapter = read(nativeAdapterPath);
const service = read(servicePath);
const plan = read(planPath);
const normalizedPlan = plan.replace(/\s+/g, ' ');

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_storefront_native_port_injection' ||
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
    blog_facade: facadePath,
    native_adapter: nativeAdapterPath,
    consumer_service: servicePath,
    consumer_matrix: consumerMatrixPath,
    fallback_evidence: fallbackEvidencePath,
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
  if (
    !sameSet(evidence.profiles?.pending ?? [], ['remote_transport_implementation'])
  ) failures.push(`${evidencePath}: pending profile drift`);

  const composition = evidence.composition ?? {};
  if (
    composition.host_context !== 'rustok_api::HostRuntimeContext' ||
    composition.shared_value !== 'Arc<dyn rustok_blog::CommentsThreadPort>' ||
    composition.facade_reexport !== 'pub use rustok_comments::CommentsThreadPort;' ||
    composition.lookup !== 'HostRuntimeContext::shared_get' ||
    composition.selector !== 'comment_service' ||
    composition.injected_constructor !== 'CommentService::with_comments_thread_port' ||
    composition.fallback_constructor !== 'CommentService::new' ||
    composition.native_endpoint !== 'blog/storefront-data' ||
    composition.operation !== 'list_public_comments_for_target'
  ) failures.push(`${evidencePath}: composition drift`);

  if (
    !sameSet(evidence.availability?.states ?? [], ['AVAILABLE', 'UNAVAILABLE', 'TIMEOUT']) ||
    !sameSet(evidence.availability?.degraded_error_kinds ?? [], [
      'ExternalService',
      'Timeout',
    ]) ||
    evidence.availability?.propagated_error_policy !== 'all_other_blog_errors'
  ) failures.push(`${evidencePath}: availability drift`);

  if (
    evidence.harness?.status !== 'executable_no_run' ||
    evidence.harness?.runtime_status !== 'not_run' ||
    evidence.harness?.source !== nativeAdapterPath ||
    evidence.harness?.test !== harnessTest ||
    evidence.harness?.command !== harnessCommand
  ) failures.push(`${evidencePath}: harness drift`);

  if (
    evidence.registration?.standalone_verifier !==
      'scripts/verify/verify-blog-comments-storefront-native-port-injection.mjs' ||
    evidence.registration?.focused_fixture !==
      'scripts/verify/verify-blog-comments-storefront-native-port-injection.test.mjs' ||
    evidence.registration?.blog_fba_package_chain !== 'pending'
  ) failures.push(`${evidencePath}: registration drift`);
}

if (
  consumerMatrix?.schema_version !== 3 ||
  consumerMatrix?.status !== 'source_verified_no_compile' ||
  !sameSet(consumerMatrix?.profiles?.source_verified ?? [], ['in_process']) ||
  !sameSet(consumerMatrix?.profiles?.pending ?? [], ['remote_adapter_placeholder'])
) failures.push(`${consumerMatrixPath}: base consumer status drift`);

if (
  fallbackEvidence?.schema_version !== 2 ||
  fallbackEvidence?.status !== 'source_verified_no_compile' ||
  fallbackEvidence?.runtime_status !== 'not_run' ||
  fallbackEvidence?.storefront_read_degradation?.cached_thread_snapshot !==
    'source_verified_no_compile' ||
  !sameSet(
    fallbackEvidence?.storefront_read_degradation?.availability_states ?? [],
    ['AVAILABLE', 'UNAVAILABLE', 'TIMEOUT'],
  ) ||
  !sameSet(
    fallbackEvidence?.storefront_read_degradation?.degraded_error_kinds ?? [],
    ['ExternalService', 'Timeout'],
  ) ||
  fallbackEvidence?.storefront_read_degradation?.propagated_error_policy !==
    'all_other_blog_errors'
) failures.push(`${fallbackEvidencePath}: fallback evidence drift`);

requireMarker(facade, 'pub use rustok_comments::CommentsThreadPort;', facadePath);

for (const marker of [
  'use std::sync::Arc;',
  '#[server(prefix = "/api/fn", endpoint = "blog/storefront-data")]',
  'let runtime_ctx = expect_context::<HostRuntimeContext>();',
  'fn comment_service(',
  'runtime_ctx: &rustok_api::HostRuntimeContext,',
  'runtime_ctx.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>()',
  'rustok_blog::CommentService::with_comments_thread_port(',
  'rustok_blog::CommentService::new(runtime_ctx.db_clone(), event_bus)',
  'let comments = comment_service(&runtime_ctx, event_bus.clone());',
  'runtime_ctx.shared_get::<Arc<dyn PublicCommentsSnapshotStore>>()',
  'list_public_comments_with_snapshot(',
  'availability: map_comments_availability(public_comments.availability)',
  'cached_snapshot: public_comments.cached_snapshot',
  'fn map_comments_availability(',
  'PublicCommentsAvailability::Unavailable',
  'BlogCommentsAvailability::Unavailable',
  'PublicCommentsAvailability::Timeout',
  'BlogCommentsAvailability::Timeout',
  'fn storefront_native_runtime_exposes_comments_port_selection()',
  ') -> rustok_blog::CommentService = comment_service;',
]) requireMarker(nativeAdapter, marker, nativeAdapterPath);

if (countMarker(nativeAdapter, 'rustok_blog::CommentService::with_comments_thread_port(') !== 1) {
  failures.push(`${nativeAdapterPath}: expected one injected constructor branch`);
}
if (countMarker(nativeAdapter, 'rustok_blog::CommentService::new(') !== 1) {
  failures.push(`${nativeAdapterPath}: expected one in-process fallback branch`);
}
if (countMarker(nativeAdapter, 'comment_service(&runtime_ctx, event_bus.clone())') !== 1) {
  failures.push(`${nativeAdapterPath}: expected one public-read selector handoff`);
}
requireNoMarker(nativeAdapter, 'rustok_comments::CommentsThreadPort', nativeAdapterPath);
requireNoMarker(nativeAdapter, 'Err(_) => BlogCommentList', nativeAdapterPath);
requireNoMarker(nativeAdapter, 'fn comments_read_availability(', nativeAdapterPath);

const lookupIndex = nativeAdapter.indexOf(
  'runtime_ctx.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>()',
);
const injectedIndex = nativeAdapter.indexOf(
  'rustok_blog::CommentService::with_comments_thread_port(',
);
const fallbackIndex = nativeAdapter.indexOf('rustok_blog::CommentService::new(');
if (
  lookupIndex < 0 ||
  injectedIndex < lookupIndex ||
  fallbackIndex < injectedIndex
) failures.push(`${nativeAdapterPath}: selector branch order drift`);

for (const marker of [
  'pub fn with_comments_thread_port(',
  '.list_public_comments_for_target(',
  'comments_public_read_port_context(',
  'PortActor::service(PUBLIC_COMMENTS_PORT_ACTOR)',
]) requireMarker(service, marker, servicePath);

for (const marker of [
  'blog-comments-storefront-native-port-injection.json',
  'verify-blog-comments-storefront-native-port-injection.mjs',
  'verify-blog-comments-storefront-native-port-injection.test.mjs',
  'storefront native SSR Comments host selection is source-locked',
  'Blog FBA package-chain registration remains pending',
  'remote network transport remains pending',
  'Slice 63',
]) requireMarker(normalizedPlan, marker, planPath);

if (failures.length > 0) {
  console.error('Blog storefront native Comments port injection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog storefront native Comments host selection and shared cached-snapshot degradation boundary is consistent');
