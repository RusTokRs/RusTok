#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];
const files = {
  evidence: 'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
  fallback: 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  blogRegistry: 'crates/rustok-blog/contracts/blog-fba-registry.json',
  commentsRegistry: 'crates/rustok-comments/contracts/comments-fba-registry.json',
  readme: 'crates/rustok-blog/storefront/README.md',
  ui: 'crates/rustok-blog/storefront/src/ui/leptos.rs',
  graphql: 'crates/rustok-blog/storefront/src/transport/graphql_adapter.rs',
  native: 'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs',
  facade: 'crates/rustok-blog/storefront/src/transport/mod.rs',
  model: 'crates/rustok-blog/storefront/src/model.rs',
  plan: 'crates/rustok-blog/docs/implementation-plan-slice-100.md',
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => {
  const target = absolute(relativePath);
  if (!fs.existsSync(target)) {
    failures.push(`${relativePath}: missing file`);
    return '';
  }
  return fs.readFileSync(target, 'utf8');
};
const parse = (relativePath) => {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
};
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

const evidence = parse(files.evidence);
const fallback = parse(files.fallback);
const blogRegistry = parse(files.blogRegistry);
const commentsRegistry = parse(files.commentsRegistry);
const readme = read(files.readme);
const ui = read(files.ui);
const graphql = read(files.graphql);
const native = read(files.native);
const facade = read(files.facade);
const model = read(files.model);
const plan = read(files.plan);

if (evidence) {
  if (
    evidence.schema_version !== 1 ||
    evidence.module !== 'blog' ||
    evidence.surface !== 'storefront_comments_write_surface' ||
    evidence.owner !== 'rustok-blog-storefront' ||
    evidence.status !== 'source_verified_absent' ||
    evidence.actualization !== 'comment_form_fallback_not_applicable_no_storefront_write_surface' ||
    evidence.legacy_degraded_mode !== 'hide_comment_form' ||
    evidence.legacy_registry_semantics !== 'compatibility_vocabulary_not_active_storefront_surface'
  ) failures.push(`${files.evidence}: identity/status drift`);

  const expectedInventory = {
    readme: files.readme,
    ui: files.ui,
    graphql_transport: files.graphql,
    native_transport: files.native,
    transport_facade: files.facade,
    model: files.model,
  };
  for (const [key, expected] of Object.entries(expectedInventory)) {
    if (evidence.source_inventory?.[key] !== expected) {
      failures.push(`${files.evidence}: source_inventory.${key} drift`);
    }
  }
  for (const key of [
    'storefront_responsibility_is_dual_path_read',
    'approved_public_comments_are_read_only_projection',
    'graphql_transport_contains_storefront_query_only',
    'native_transport_contains_storefront_data_read_only',
    'leptos_ui_renders_comments_without_submit_surface',
  ]) {
    if (evidence.source_contract?.[key] !== true) {
      failures.push(`${files.evidence}: source_contract.${key} must be true`);
    }
  }
  for (const key of [
    'create_comment_surface_present',
    'comment_form_present',
    'textarea_present',
    'submit_handler_present',
    'production_behavior_changed',
    'runtime_execution_observed',
    'browser_execution_observed',
  ]) {
    if (evidence.source_contract?.[key] !== false) {
      failures.push(`${files.evidence}: source_contract.${key} must be false`);
    }
  }
  if (
    evidence.planning_effect?.comment_form_fallback !== 'not_applicable_no_storefront_write_surface' ||
    evidence.planning_effect?.cached_thread_snapshot !== 'source_ready_maintainer_execution_pending' ||
    evidence.planning_effect?.fallback_smoke_status !== 'planned_runtime_execution_only' ||
    evidence.planning_effect?.new_storefront_write_surface_authorized !== false ||
    !Array.isArray(evidence.execution) ||
    evidence.execution.length !== 0
  ) failures.push(`${files.evidence}: planning/execution drift`);
}

if (fallback) {
  if (fallback.source_contract?.storefront_write_surface_inventory !== files.evidence) {
    failures.push(`${files.fallback}: write-surface inventory path drift`);
  }
  if (
    fallback.storefront_read_degradation?.comment_form_fallback !== 'planned' ||
    fallback.storefront_read_degradation?.comment_form_fallback_interpretation !==
      'legacy_registry_compatibility_only_see_storefront_write_surface'
  ) failures.push(`${files.fallback}: legacy compatibility marker drift`);
  const writeSurface = fallback.storefront_write_surface ?? {};
  if (
    writeSurface.status !== 'source_verified_absent' ||
    writeSurface.inventory !== files.evidence ||
    writeSurface.active_comment_form !== false ||
    writeSurface.active_create_comment_transport !== false ||
    writeSurface.legacy_degraded_mode !== 'hide_comment_form' ||
    writeSurface.legacy_registry_semantics !==
      'compatibility_vocabulary_not_active_storefront_surface' ||
    writeSurface.comment_form_fallback !== 'not_applicable_no_storefront_write_surface'
  ) failures.push(`${files.fallback}: write-surface actualization drift`);
  if (
    fallback.fallback_smoke?.status !== 'planned' ||
    fallback.fallback_smoke?.status_scope !== 'cached_read_runtime_execution_only' ||
    fallback.fallback_smoke?.runtime_evidence !== 'pending'
  ) failures.push(`${files.fallback}: fallback runtime scope drift`);
  const createCase = fallback.fallback_smoke?.cases?.find(
    (entry) => entry.operation === 'create_comment',
  );
  if (
    !createCase ||
    createCase.degraded_mode !== 'hide_comment_form' ||
    createCase.mode_status !== 'legacy_not_applicable_no_storefront_write_surface' ||
    !createCase.expected_consumer_behavior?.includes('not an implementation target')
  ) failures.push(`${files.fallback}: create-comment legacy mode drift`);
}

const blogDependency = blogRegistry?.provider_dependencies?.find(
  (entry) => entry.module === 'comments',
);
const commentsConsumer = commentsRegistry?.consumers?.find(
  (entry) => entry.module === 'blog',
);
for (const [label, value] of [
  ['Blog registry', blogDependency],
  ['Comments registry', commentsConsumer],
]) {
  if (!value?.degraded_modes?.includes('hide_comment_form')) {
    failures.push(`${label}: legacy hide_comment_form vocabulary disappeared without schema migration`);
  }
}

for (const marker of [
  'Owns dual-path read access for published posts',
  "Renders the selected post's approved public comments",
  'Owns public comment pagination',
]) need(readme, marker, files.readme);

for (const [label, source] of [
  [files.ui, ui],
  [files.graphql, graphql],
  [files.native, native],
  [files.facade, facade],
  [files.model, model],
]) {
  for (const marker of [
    '<form',
    '<textarea',
    'on:submit',
    'CreateCommentInput',
    'create_comment(',
    'createComment',
    'submit_comment',
  ]) forbid(source, marker, label);
}

need(graphql, 'query StorefrontBlog', files.graphql);
forbid(graphql, 'mutation StorefrontBlog', files.graphql);
need(native, 'endpoint = "blog/storefront-data"', files.native);
need(native, 'list_public_comments_with_snapshot(', files.native);
need(ui, 'fn PublicCommentsList(', files.ui);

for (const marker of [
  'storefront_comment_form_fallback_not_applicable_source_verified',
  'active storefront package is read-only',
  'hide_comment_form',
  'compatibility vocabulary',
  'not_applicable_no_storefront_write_surface',
  'cached read fallback runtime evidence',
  'No tests, Cargo commands, Node verifiers',
]) need(plan, marker, files.plan);

if (failures.length) {
  console.error('[verify-blog-comments-storefront-write-surface] FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '[verify-blog-comments-storefront-write-surface] PASS write_surface=absent comment_form_fallback=not_applicable execution=not_run',
);
