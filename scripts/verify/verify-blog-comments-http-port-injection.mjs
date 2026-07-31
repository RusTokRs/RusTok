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

function sameSet(actual, expected) {
  return [...actual].sort().join('|') === [...expected].sort().join('|');
}

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-http-port-injection.json';
const runtimePath = 'crates/rustok-blog/src/controllers/mod.rs';
const controllerPath = 'crates/rustok-blog/src/controllers/comments.rs';
const servicePath = 'crates/rustok-blog/src/services/comment.rs';
const matrixPath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const harnessTest = 'controllers::tests::blog_http_runtime_exposes_comments_port_selection';
const harnessCommand = `cargo test -p rustok-blog --lib ${harnessTest} -- --exact`;

const evidence = json(evidencePath);
const runtime = read(runtimePath);
const controller = read(controllerPath);
const service = read(servicePath);
const matrix = json(matrixPath);
const plan = read(planPath);

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_http_port_injection' ||
    evidence.role !== 'consumer' ||
    evidence.provider !== 'comments'
  ) failures.push(`${evidencePath}: identity drift`);
  if (
    evidence.status !== 'source_verified_no_compile' ||
    evidence.compile_policy !== 'not_run_by_request' ||
    evidence.runtime_status !== 'not_run'
  ) failures.push(`${evidencePath}: execution status drift`);
  if (
    evidence.source_contract?.http_runtime !== runtimePath ||
    evidence.source_contract?.moderation_controller !== controllerPath ||
    evidence.source_contract?.consumer_service !== servicePath ||
    evidence.source_contract?.consumer_matrix !== matrixPath
  ) failures.push(`${evidencePath}: source path drift`);
  if (!sameSet(evidence.profiles?.source_verified ?? [], [
    'in_process_fallback',
    'host_injected_port_selection',
  ])) failures.push(`${evidencePath}: source-verified profile drift`);
  if (!sameSet(evidence.profiles?.pending ?? [], ['remote_transport_implementation'])) {
    failures.push(`${evidencePath}: pending profile drift`);
  }
  const composition = evidence.composition ?? {};
  if (
    composition.host_context !== 'rustok_api::HostRuntimeContext' ||
    composition.shared_value !== 'Arc<dyn CommentsThreadPort>' ||
    composition.lookup !== 'HostRuntimeContext::shared_get' ||
    composition.selector !== 'BlogHttpRuntime::comment_service' ||
    composition.injected_constructor !== 'CommentService::with_comments_thread_port' ||
    composition.fallback_constructor !== 'CommentService::new' ||
    composition.http_operation !== 'moderate_comment'
  ) failures.push(`${evidencePath}: composition drift`);
  const harness = evidence.harness ?? {};
  if (
    harness.status !== 'executable_no_run' ||
    harness.runtime_status !== 'not_run' ||
    harness.source !== runtimePath ||
    harness.test !== harnessTest ||
    harness.command !== harnessCommand
  ) failures.push(`${evidencePath}: harness drift`);
}

if (
  matrix?.schema_version !== 3 ||
  matrix?.adapter_injection?.constructor !== 'CommentService::with_comments_thread_port' ||
  matrix?.adapter_injection?.runtime_status !== 'not_run' ||
  matrix?.adapter_injection?.remote_transport_implementation !== 'pending'
) failures.push(`${matrixPath}: base injection seam drift`);

for (const marker of [
  'use rustok_comments::CommentsThreadPort;',
  'use std::sync::Arc;',
  'comments_thread_port: Option<Arc<dyn CommentsThreadPort>>',
  'fn comment_service(&self) -> CommentService',
  'if let Some(comments_thread_port) = self.comments_thread_port.clone()',
  'CommentService::with_comments_thread_port(self.db_clone(), comments_thread_port)',
  'CommentService::new(self.db_clone(), self.event_bus())',
  'comments_thread_port: runtime.shared_get::<Arc<dyn CommentsThreadPort>>()',
  'mod tests',
  'fn blog_http_runtime_exposes_comments_port_selection()',
  'let selector: fn(&BlogHttpRuntime) -> CommentService = BlogHttpRuntime::comment_service;',
]) requireMarker(runtime, marker, runtimePath);

requireMarker(controller, 'let service = runtime.comment_service();', controllerPath);
requireNoMarker(controller, 'CommentService::new(', controllerPath);
requireNoMarker(controller, 'CommentService::with_comments_thread_port(', controllerPath);

for (const marker of [
  'pub fn with_comments_thread_port(',
  'comments_thread_port: Arc<dyn CommentsThreadPort>',
]) requireMarker(service, marker, servicePath);

for (const marker of [
  'blog-comments-http-port-injection.json',
  'verify-blog-comments-http-port-injection.mjs',
  'verify-blog-comments-http-port-injection.test.mjs',
  'BlogHttpRuntime::comment_service',
  'HTTP moderation',
  'remote network transport remains pending',
  'Slice 59',
]) requireMarker(plan, marker, planPath);

if (failures.length > 0) {
  console.error('Blog Comments HTTP port injection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog Comments HTTP host-injected port composition is source-consistent');
