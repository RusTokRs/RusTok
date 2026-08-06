#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  source: 'crates/rustok-index/src/application/drift_candidates.rs',
  applicationMod: 'crates/rustok-index/src/application/mod.rs',
  doc: 'crates/rustok-index/docs/m6-bounded-drift-candidates.md',
  plan: 'crates/rustok-index/docs/implementation-plan-current-2026-08-03.md',
  recheck: 'crates/rustok-index/docs/implementation-recheck-2026-08-06-bounded-drift-candidates.md',
  aggregate: 'scripts/verify/verify-index-query-contract.mjs',
};

const content = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, 'utf8')]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!content[name].includes(marker)) {
      throw new Error(`${files[name]} missing ${marker}`);
    }
  }
}

requireMarkers('applicationMod', [
  'mod drift_candidates;',
  'IndexDriftCandidateReader',
  'IndexDriftCandidateRequest',
  'IndexDriftStaleEntityCandidate',
  'IndexDriftOrphanLinkCandidate',
]);

requireMarkers('source', [
  'const MAX_CANDIDATE_PAGE_SIZE: usize = 32;',
  'const MAX_CANDIDATE_CURSOR_BYTES: usize = 4 * 1024;',
  'const MAX_CANDIDATE_FENCE_BYTES: usize = 512;',
  'pub struct IndexDriftCandidateScope',
  'tenant_id.is_nil()',
  'schema.version.get() == 0',
  'pub struct IndexDriftCandidateCursor(String);',
  'pub struct IndexDriftCandidateFence(String);',
  'pub struct IndexDriftCandidateRequest',
  'if fence.is_some() != cursor.is_some()',
  'pub struct IndexDriftStaleEntityCandidate',
  'pub struct IndexDriftOrphanLinkCandidate',
  'pub enum IndexDriftCandidate',
  'StaleEntity(IndexDriftStaleEntityCandidate)',
  'OrphanLink(IndexDriftOrphanLinkCandidate)',
  'enum IndexDriftCandidateOrderKey',
  'pub struct IndexDriftCandidatePage',
  'if candidates.len() > request.limit',
  'IndexDriftCandidateError::FenceChanged',
  'IndexDriftCandidateError::EmptyPageContinuation',
  'IndexDriftCandidateError::CursorDidNotAdvance',
  'IndexDriftCandidateError::CandidateScopeMismatch',
  'IndexDriftCandidateError::UnstableCandidateOrder',
  'pub trait IndexDriftCandidateReader: Send + Sync',
  'async fn read_candidate_page(',
  'continuation_requires_fence_and_cursor_together',
  'page_rejects_scope_escape_and_unstable_order',
  'page_requires_fence_stability_and_cursor_progress',
  'orphan_candidate_keeps_only_typed_identity',
  'opaque_debug_does_not_reveal_values',
]);

const production = content.source.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'sea_orm',
  'DatabaseConnection',
  'DatabaseTransaction',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
  'async_graphql',
  'Router::new',
  'tokio::spawn',
  'spawn_blocking',
  'std::env',
  'SecretResolverRegistry',
  'resolve_finding',
  'ignore_finding',
  'repair_finding',
]) {
  if (production.includes(forbidden)) {
    throw new Error(`drift candidate contract contains forbidden capability: ${forbidden}`);
  }
}

const pageStart = production.indexOf('    pub fn new(\n        request: &IndexDriftCandidateRequest,');
const pageEnd = production.indexOf('\n    pub fn fence(&self)', pageStart);
if (pageStart < 0 || pageEnd <= pageStart) {
  throw new Error('candidate page constructor segment is incomplete');
}
const page = production.slice(pageStart, pageEnd);
const size = page.indexOf('if candidates.len() > request.limit');
const fence = page.indexOf('IndexDriftCandidateError::FenceChanged', size);
const empty = page.indexOf('IndexDriftCandidateError::EmptyPageContinuation', fence);
const progress = page.indexOf('IndexDriftCandidateError::CursorDidNotAdvance', empty);
const scope = page.indexOf('IndexDriftCandidateError::CandidateScopeMismatch', progress);
const order = page.indexOf('IndexDriftCandidateError::UnstableCandidateOrder', scope);
if (size < 0 || fence <= size || empty <= fence || progress <= empty || scope <= progress || order <= scope) {
  throw new Error('candidate page must bound, fence, advance, scope, then order candidates');
}

for (const leak of [
  '.field("value"',
  '.field("cursor"',
  '.field("fence"',
  'derive(Debug, Clone, PartialEq, Eq)\npub struct IndexDriftCandidateCursor',
  'derive(Debug, Clone, PartialEq, Eq)\npub struct IndexDriftCandidateFence',
]) {
  if (production.includes(leak)) {
    throw new Error(`opaque candidate continuation leaks through Debug: ${leak}`);
  }
}

requireMarkers('doc', [
  'Status: `source_complete_downstream_confirmation_and_persistence_complete`.',
  'page limit in `1..=32`',
  'bounded to 512 bytes',
  'bounded to 4 KiB',
  'Stale entity',
  'Orphan link',
  'strict and deterministic',
  'PostgresIndexDriftConfirmedCandidateWriter',
  'The candidate contract itself still does not add:',
]);
requireMarkers('plan', [
  'M6 - add drift finding lifecycle commands',
  'M6 bounded stale-entity and orphan-link candidate contract',
  'source_complete_lifecycle_pending',
  '[x] Add a database-neutral bounded candidate contract',
]);
requireMarkers('recheck', [
  'Audited baseline: `main@53aeddfbf05ceccea27f6c2f639af904c3ace6b2`.',
  'The only main delta after the baseline is Pages storefront Navigation/SEO ETag composition.',
  'This recheck does not claim:',
  'did not run tests, JavaScript verifiers',
]);
requireMarkers('aggregate', [
  "'verify-index-drift-candidate-contract.mjs'",
]);

console.log('Index bounded drift candidate contract verified');
