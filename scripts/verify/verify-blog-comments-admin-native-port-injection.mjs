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

function segment(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) return '';
  const end = endMarker ? source.indexOf(endMarker, start + startMarker.length) : -1;
  return source.slice(start, end < 0 ? source.length : end);
}

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

const evidence = json(evidencePath);
const consumerMatrix = json(consumerMatrixPath);
const adminAdapter = read(adminAdapterPath);
const service = read(servicePath);
const normalizedPlan = read(planPath).replace(/\s+/g, ' ');

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_admin_native_port_injection' ||
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
    admin_adapter: adminAdapterPath,
    consumer_service: servicePath,
    consumer_matrix: consumerMatrixPath,
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
    composition.native_context !== 'NativeContext' ||
    composition.shared_value !== 'Arc<dyn rustok_blog::CommentsThreadPort>' ||
    composition.lookup !== 'HostRuntimeContext::shared_get' ||
    composition.selector !== 'comment_service' ||
    composition.injected_constructor !== 'CommentService::with_comments_thread_port' ||
    composition.fallback_constructor !== 'CommentService::new' ||
    !sameSet(composition.native_endpoints ?? [], [
      'blog/admin/moderation-comments',
      'blog/admin/moderate-comment',
    ]) ||
    !sameSet(composition.port_operations ?? [], [
      'list_comments_for_target',
      'get_comment',
      'set_comment_status',
    ])
  ) failures.push(`${evidencePath}: composition drift`);

  if (
    evidence.authorization?.tenant_binding !==
      'AuthContext.tenant_id == TenantContext.id' ||
    evidence.authorization?.permission !== 'blog_posts:manage' ||
    evidence.authorization?.checked_before_port_call !== true
  ) failures.push(`${evidencePath}: authorization drift`);

  if (
    evidence.moderation_policy?.read_pagination !==
      'page >= 1; 1 <= per_page <= 100' ||
    evidence.moderation_policy?.error_policy !== 'all_blog_errors_propagated' ||
    evidence.moderation_policy?.storefront_degradation_reused !== false
  ) failures.push(`${evidencePath}: moderation policy drift`);

  if (
    evidence.harness?.status !== 'executable_no_run' ||
    evidence.harness?.runtime_status !== 'not_run' ||
    evidence.harness?.source !== adminAdapterPath ||
    evidence.harness?.test !== harnessTest ||
    evidence.harness?.command !== harnessCommand
  ) failures.push(`${evidencePath}: harness drift`);

  if (
    evidence.registration?.standalone_verifier !==
      'scripts/verify/verify-blog-comments-admin-native-port-injection.mjs' ||
    evidence.registration?.focused_fixture !==
      'scripts/verify/verify-blog-comments-admin-native-port-injection.test.mjs' ||
    evidence.registration?.blog_fba_package_chain !== 'pending'
  ) failures.push(`${evidencePath}: registration drift`);
}

if (
  consumerMatrix?.schema_version !== 3 ||
  consumerMatrix?.status !== 'source_verified_no_compile' ||
  !sameSet(consumerMatrix?.profiles?.source_verified ?? [], ['in_process']) ||
  !sameSet(consumerMatrix?.profiles?.pending ?? [], ['remote_adapter_placeholder'])
) failures.push(`${consumerMatrixPath}: base consumer status drift`);

for (const marker of [
  'use std::sync::Arc;',
  'struct NativeContext {',
  'comments_thread_port: Option<Arc<dyn rustok_blog::CommentsThreadPort>>',
  'let runtime = expect_context::<HostRuntimeContext>();',
  'if auth.tenant_id != tenant.id',
  'runtime.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>()',
  'fn comment_service(context: &NativeContext) -> rustok_blog::CommentService',
  'context.comments_thread_port.clone()',
  'rustok_blog::CommentService::with_comments_thread_port(',
  'rustok_blog::CommentService::new(context.db.clone(), context.event_bus.clone())',
  '#[server(prefix = "/api/fn", endpoint = "blog/admin/moderation-comments")]',
  '#[server(prefix = "/api/fn", endpoint = "blog/admin/moderate-comment")]',
  'require_manage_permission(&context.auth)?;',
  '&[rustok_api::Permission::BLOG_POSTS_MANAGE]',
  '"Permission denied: blog_posts:manage required"',
  'page: page.max(1)',
  'per_page: per_page.clamp(1, 100)',
  '.list_for_post_with_locale_fallback(',
  '.moderate_comment(',
  '.map_err(ServerFnError::new)?;',
  'fn admin_native_runtime_exposes_comments_port_selection()',
  'let selector: fn(&NativeContext) -> rustok_blog::CommentService = comment_service;',
]) requireMarker(adminAdapter, marker, adminAdapterPath);

if (countMarker(adminAdapter, 'CommentService::with_comments_thread_port(') !== 1) {
  failures.push(`${adminAdapterPath}: expected one injected constructor branch`);
}
if (countMarker(adminAdapter, 'CommentService::new(') !== 1) {
  failures.push(`${adminAdapterPath}: expected one in-process fallback branch`);
}
if (countMarker(adminAdapter, 'comment_service(&context)') !== 2) {
  failures.push(`${adminAdapterPath}: expected two moderation selector handoffs`);
}
requireNoMarker(adminAdapter, 'use rustok_blog::CommentService;', adminAdapterPath);
requireNoMarker(adminAdapter, 'use rustok_blog::{CommentService,', adminAdapterPath);
requireNoMarker(adminAdapter, 'Err(_) => BlogModerationCommentList', adminAdapterPath);
requireNoMarker(adminAdapter, 'unwrap_or_default()', adminAdapterPath);

const listSegment = segment(
  adminAdapter,
  '#[server(prefix = "/api/fn", endpoint = "blog/admin/moderation-comments")]',
  '#[server(prefix = "/api/fn", endpoint = "blog/admin/moderate-comment")]',
);
const mutationSegment = segment(
  adminAdapter,
  '#[server(prefix = "/api/fn", endpoint = "blog/admin/moderate-comment")]',
  '#[cfg(feature = "ssr")]\nfn optional_text',
);
for (const [label, source] of [
  ['moderation list', listSegment],
  ['moderation mutation', mutationSegment],
]) {
  const permissionIndex = source.indexOf('require_manage_permission(&context.auth)?;');
  const selectorIndex = source.indexOf('comment_service(&context)');
  if (permissionIndex < 0 || selectorIndex < permissionIndex) {
    failures.push(`${adminAdapterPath}: ${label} authorization/selector order drift`);
  }
  if (!source.includes('.map_err(ServerFnError::new)?;')) {
    failures.push(`${adminAdapterPath}: ${label} error propagation drift`);
  }
}

for (const marker of [
  'pub fn with_comments_thread_port(',
  '.list_comments_for_target(',
  '.get_comment(',
  '.set_comment_status(',
  'comments_read_port_context(',
  'comments_write_port_context(',
  'PortErrorKind::Unavailable => rustok_core::error::ErrorKind::ExternalService',
  'PortErrorKind::Timeout => rustok_core::error::ErrorKind::Timeout',
]) requireMarker(service, marker, servicePath);

for (const marker of [
  'blog-comments-admin-native-port-injection.json',
  'verify-blog-comments-admin-native-port-injection.mjs',
  'verify-blog-comments-admin-native-port-injection.test.mjs',
  'admin native SSR Comments host selection is source-locked',
  'Blog FBA package-chain registration remains pending',
  'remote network transport remains pending',
  'Slice 65',
]) requireMarker(normalizedPlan, marker, planPath);

if (failures.length > 0) {
  console.error('Blog admin native Comments port injection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog admin native Comments host selection and fail-closed moderation source boundary is consistent');
