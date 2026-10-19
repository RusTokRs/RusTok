#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const wrapper = read('crates/modules/rustok-seo/src/services/diagnostics.rs');
const batch = read('crates/modules/rustok-seo/src/services/diagnostics_batch.rs');
const routing = read('crates/modules/rustok-seo/src/services/routing.rs');
const targets = read('crates/modules/rustok-seo/src/services/targets.rs');

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

if (wrapper.trim() !== 'include!("diagnostics_batch.rs");') {
  failures.push('diagnostics wrapper must activate only the batched implementation');
}

const legacyPath = new URL('crates/modules/rustok-seo/src/services/diagnostics_legacy.rs', root);
if (existsSync(legacyPath)) {
  failures.push('diagnostics_legacy.rs must stay deleted; the batched implementation is the only one');
}
const servicesDir = new URL('crates/modules/rustok-seo/src/services/', root);
for (const file of readdirSync(servicesDir)) {
  if (!file.endsWith('.rs')) continue;
  const content = read(`crates/modules/rustok-seo/src/services/${file}`);
  if (content.includes('diagnostics_legacy')) {
    failures.push(`${file}: forbidden reference to diagnostics_legacy`);
  }
}

for (const [value, label] of [
  ['const DIAGNOSTICS_META_BATCH_SIZE: usize = 256;', 'bounded batch size'],
  ['target_ids.chunks(DIAGNOSTICS_META_BATCH_SIZE)', 'bounded target chunks'],
  ['seo_meta::Column::TargetId.is_in(', 'metadata batch predicate'],
  ['meta_translation::Column::MetaId.is_in(', 'translation batch predicate'],
  ['let settings = if enabled {', 'single settings snapshot'],
  ['load_target_states_batch(', 'batched provider state loads'],
  ['SeoTargetLoadScope::Authoring', 'authoring scope batch'],
  ['SeoTargetLoadScope::PublicRoute', 'public route scope batch'],
  ['build_diagnostics_meta_record(', 'metadata projection'],
  ['self.merge_page_context(', 'shared page-context projection'],
  ['duplicate_hreflang_locales(', 'unique-href hreflang check'],
  ['"duplicate_hreflang_href"', 'duplicate hreflang issue code'],
  ['fn readiness_score(', 'defect-only readiness scoring'],
  ['let weighted_issues = (error_count * 3) + (warning_count * 2);', 'readiness excludes Info signals'],
]) {
  requireText(batch, value, label);
}

// The per-target authoring/route loads are what made diagnostics fan out the provider
// detail projection twice per target; scans must go through the batch loader instead.
const stateBatchCalls = batch.match(/load_target_states_batch\(/g) ?? [];
if (stateBatchCalls.length !== 2) {
  failures.push(
    `expected exactly the authoring and route batch calls in diagnostics, found ${stateBatchCalls.length}`,
  );
}
for (const [value, label] of [
  ['pub(super) async fn load_target_states_batch(', 'batch state loader declaration'],
  ['SeoTargetLoadTargetsRequest {', 'provider batch request'],
  ['.load_targets(', 'provider batch invocation'],
]) {
  requireText(targets, value, `targets.rs ${label}`);
}

// The diagnostics page context must be the shared render formula itself, not a
// parallel copy: canonical normalization, suffix/limits, OG settings, and
// hreflang/rich-snippet gates live only in routing's merge_page_context.
requireText(
  routing,
  'pub(super) async fn merge_page_context(',
  'shared render formula projection',
);

for (const [value, label] of [
  ['.seo_meta(', 'per-target metadata service call'],
  ['.resolve_page_context(', 'per-target route resolver call'],
  ['load_explicit_meta(', 'per-target explicit metadata call'],
  ['load_target_state(', 'per-target authoring state load'],
  ['load_route_target_state(', 'per-target route state load'],
  ['meta_translation::Column::MetaId.eq(', 'per-meta translation query'],
]) {
  forbidText(batch, value, label);
}

const settingsLoads = batch.match(/load_settings\(tenant\.id\)/g) ?? [];
if (settingsLoads.length !== 1) {
  failures.push(`expected one settings load, found ${settingsLoads.length}`);
}

const metadataBatchQueries = batch.match(/seo_meta::Entity::find\(\)/g) ?? [];
if (metadataBatchQueries.length !== 1) {
  failures.push(`expected one metadata batch query definition, found ${metadataBatchQueries.length}`);
}

const translationBatchQueries = batch.match(/meta_translation::Entity::find\(\)/g) ?? [];
if (translationBatchQueries.length !== 1) {
  failures.push(
    `expected one translation batch query definition, found ${translationBatchQueries.length}`,
  );
}

if (failures.length > 0) {
  console.error('SEO diagnostics batch-read verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ SEO diagnostics uses bounded metadata batches and direct shared-snapshot projections',
);
