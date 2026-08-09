#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];

const files = {
  evidence: 'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json',
  current: 'crates/rustok-blog/docs/implementation-plan-current.md',
  historical: 'crates/rustok-blog/docs/implementation-plan.md',
  slice97: 'crates/rustok-blog/docs/implementation-plan-slice-97.md',
  slice98: 'crates/rustok-blog/docs/implementation-plan-slice-98.md',
  slice99: 'crates/rustok-blog/docs/implementation-plan-slice-99.md',
  slice100: 'crates/rustok-blog/docs/implementation-plan-slice-100.md',
  slice101: 'crates/rustok-blog/docs/implementation-plan-slice-101.md',
  slice102: 'crates/rustok-blog/docs/implementation-plan-slice-102.md',
  slice103: 'crates/rustok-blog/docs/implementation-plan-slice-103.md',
  docsIndex: 'crates/rustok-blog/docs/README.md',
  tcpTransport: 'crates/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json',
  tcpServer: 'crates/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json',
  tcpListener: 'crates/rustok-blog/contracts/evidence/blog-comments-tcp-listener-lifecycle.json',
  translationPostgres: 'crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
  fallback: 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  writeSurface: 'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
  tagPagination: 'crates/rustok-blog/contracts/evidence/blog-tag-pagination-source.json',
  tagProjection: 'crates/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json',
};

function absolute(relativePath) { return path.join(repoRoot, relativePath); }
function read(relativePath) {
  const target = absolute(relativePath);
  if (!fs.existsSync(target)) {
    failures.push(`${relativePath}: missing file`);
    return '';
  }
  return fs.readFileSync(target, 'utf8');
}
function json(relativePath) {
  try { return JSON.parse(read(relativePath)); }
  catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}
function need(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}
function forbid(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden live cursor marker ${marker}`);
}
function sameSet(actual, expected) {
  return [...(actual ?? [])].sort().join('|') === [...expected].sort().join('|');
}
function between(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) return '';
  const end = source.indexOf(endMarker, start + startMarker.length);
  return source.slice(start, end < 0 ? source.length : end);
}

const evidence = json(files.evidence);
const current = read(files.current);
const historical = read(files.historical);
const slice97 = read(files.slice97);
const slice98 = read(files.slice98);
const slice99 = read(files.slice99);
const slice100 = read(files.slice100);
const slice101 = read(files.slice101);
const slice102 = read(files.slice102);
const slice103 = read(files.slice103);
const docsIndex = read(files.docsIndex);
const tcpTransport = json(files.tcpTransport);
const tcpServer = json(files.tcpServer);
const tcpListener = json(files.tcpListener);
const translationPostgres = json(files.translationPostgres);
const fallback = json(files.fallback);
const writeSurface = json(files.writeSurface);
const tagPagination = json(files.tagPagination);
const tagProjection = json(files.tagProjection);

if (evidence) {
  if (
    evidence.schema_version !== 2 ||
    evidence.module !== 'blog' ||
    evidence.surface !== 'canonical_implementation_cursor' ||
    evidence.status !== 'source_verified_no_compile' ||
    evidence.compile_policy !== 'not_run_by_request' ||
    evidence.runtime_status !== 'not_run'
  ) failures.push(`${files.evidence}: identity/status drift`);

  if (
    evidence.historical_baseline !== files.historical ||
    evidence.canonical_current_cursor !== files.current ||
    evidence.actualization_slice !== files.slice101 ||
    evidence.historical_baseline_last_embedded_slice !== 67 ||
    evidence.latest_behavior_slice !== 103 ||
    evidence.current_recorded_slice !== 103
  ) failures.push(`${files.evidence}: cursor path/version drift`);

  const tracks = evidence.source_tracks ?? {};
  if (
    tracks.remote_comments_transport?.status !== 'source_implemented_maintainer_execution_pending' ||
    tracks.remote_comments_transport?.transport_evidence !== files.tcpTransport ||
    tracks.remote_comments_transport?.latest_audit_relay_slice !== files.slice97 ||
    tracks.remote_comments_transport?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: remote Comments track drift`);

  if (
    tracks.comments_source_retention?.status !== 'blocked_until_slices_95_97_execution' ||
    tracks.comments_source_retention?.blocking_slice !== files.slice97 ||
    tracks.comments_source_retention?.must_not_advance_before_execution !== true
  ) failures.push(`${files.evidence}: Comments source-retention gate drift`);

  if (
    tracks.category_translation_postgres?.status !== 'source_ready_maintainer_execution_pending' ||
    tracks.category_translation_postgres?.slice !== files.slice98 ||
    tracks.category_translation_postgres?.evidence !== files.translationPostgres ||
    tracks.category_translation_postgres?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: category Translation track drift`);

  if (
    tracks.cached_public_comments_snapshot?.status !== 'source_ready_maintainer_execution_pending' ||
    tracks.cached_public_comments_snapshot?.slice !== files.slice99 ||
    tracks.cached_public_comments_snapshot?.evidence !== files.fallback ||
    tracks.cached_public_comments_snapshot?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: cached Comments track drift`);

  if (
    tracks.storefront_comment_form_fallback?.status !== 'not_applicable_no_storefront_write_surface' ||
    tracks.storefront_comment_form_fallback?.slice !== files.slice100 ||
    tracks.storefront_comment_form_fallback?.evidence !== files.writeSurface ||
    tracks.storefront_comment_form_fallback?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: storefront write-surface track drift`);

  if (
    tracks.tag_list_pagination?.status !== 'source_ready_maintainer_execution_pending' ||
    tracks.tag_list_pagination?.slice !== files.slice102 ||
    tracks.tag_list_pagination?.evidence !== files.tagPagination ||
    tracks.tag_list_pagination?.maximum_page_size !== 100 ||
    tracks.tag_list_pagination?.offset_overflow_safe !== true ||
    tracks.tag_list_pagination?.database_side_pagination_claimed !== false ||
    tracks.tag_list_pagination?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: tag pagination track drift`);

  if (
    tracks.tag_canonical_projection?.status !== 'source_ready_maintainer_execution_pending' ||
    tracks.tag_canonical_projection?.slice !== files.slice103 ||
    tracks.tag_canonical_projection?.evidence !== files.tagProjection ||
    tracks.tag_canonical_projection?.attachment_source !== 'blog_post_tags' ||
    tracks.tag_canonical_projection?.dictionary_source !== 'rustok-taxonomy' ||
    tracks.tag_canonical_projection?.metadata_tags_are_canonical !== false ||
    tracks.tag_canonical_projection?.legacy_data_audit_pending !== true ||
    tracks.tag_canonical_projection?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: canonical tag projection track drift`);

  if (
    tracks.tag_mutation_atomic_reindex?.status !== 'next_source_gap' ||
    tracks.tag_mutation_atomic_reindex?.canonical_source_defined !== true ||
    tracks.tag_mutation_atomic_reindex?.taxonomy_supplied_transaction_api_required !== true ||
    tracks.tag_mutation_atomic_reindex?.blog_scope_reindex_same_transaction_required !== true ||
    tracks.tag_mutation_atomic_reindex?.manual_predelete_relation_cleanup_must_be_removed !== true
  ) failures.push(`${files.evidence}: tag mutation atomic-reindex cursor drift`);

  if (
    evidence.planning_result?.independent_production_source_gap_identified !== true ||
    evidence.planning_result?.next_source_gap !== 'tag_mutation_atomic_reindex' ||
    evidence.planning_result?.future_autonomous_source_work_requires_fresh_audit !== false ||
    evidence.planning_result?.historical_stale_phrases_are_live_instructions !== false ||
    evidence.planning_result?.production_behavior_changed_since_slice_101 !== true ||
    evidence.planning_result?.ffa_or_fba_promoted !== false ||
    !Array.isArray(evidence.execution) ||
    evidence.execution.length !== 0
  ) failures.push(`${files.evidence}: planning/execution claim drift`);
}

for (const [label, artifact, expectedSurface] of [
  [files.tcpTransport, tcpTransport, 'comments_tcp_json_transport'],
  [files.tcpServer, tcpServer, 'comments_tcp_server_adapter'],
  [files.tcpListener, tcpListener, 'comments_tcp_listener_lifecycle'],
]) {
  if (
    artifact?.module !== 'blog' ||
    artifact?.provider !== 'comments' ||
    artifact?.surface !== expectedSurface ||
    artifact?.status !== 'source_verified_no_compile' ||
    artifact?.runtime_status !== 'not_run'
  ) failures.push(`${label}: remote transport source evidence drift`);
}

if (!sameSet(tcpServer?.operations, [
  'create_comment',
  'get_comment',
  'list_comments_for_target',
  'list_public_comments_for_target',
  'update_comment',
  'set_comment_status',
  'delete_comment',
])) failures.push(`${files.tcpServer}: seven-operation transport parity drift`);

if (
  translationPostgres?.status !== 'blog_category_translation_postgres_source_unvalidated' ||
  translationPostgres?.source_contract?.translation_target_migration_up_down_up_covered !== true ||
  translationPostgres?.source_contract?.concurrent_apply_requires_exactly_one_success !== true ||
  translationPostgres?.source_contract?.cursor_recovery_drains_after_latest_cursor !== true ||
  translationPostgres?.source_contract?.postgres_execution_observed !== false ||
  !Array.isArray(translationPostgres?.execution) || translationPostgres.execution.length !== 0
) failures.push(`${files.translationPostgres}: Translation PostgreSQL source status drift`);

if (
  fallback?.storefront_read_degradation?.cached_thread_snapshot !== 'source_verified_no_compile' ||
  fallback?.storefront_read_degradation?.runtime_evidence !== 'pending' ||
  !sameSet(fallback?.storefront_read_degradation?.transports, ['graphql', 'native_ssr']) ||
  fallback?.storefront_write_surface?.comment_form_fallback !== 'not_applicable_no_storefront_write_surface'
) failures.push(`${files.fallback}: storefront fallback current-state drift`);

if (
  writeSurface?.status !== 'source_verified_absent' ||
  writeSurface?.actualization !== 'comment_form_fallback_not_applicable_no_storefront_write_surface' ||
  writeSurface?.source_contract?.create_comment_surface_present !== false ||
  writeSurface?.source_contract?.comment_form_present !== false ||
  writeSurface?.planning_effect?.new_storefront_write_surface_authorized !== false ||
  !Array.isArray(writeSurface?.execution) || writeSurface.execution.length !== 0
) failures.push(`${files.writeSurface}: storefront write-surface evidence drift`);

if (
  tagPagination?.status !== 'source_verified_no_compile' ||
  tagPagination?.runtime_status !== 'not_run' ||
  tagPagination?.source_contract?.maximum_page_size !== 100 ||
  tagPagination?.source_contract?.owner_service_clamps_page_size !== true ||
  tagPagination?.source_contract?.extreme_page_cannot_overflow_offset_arithmetic !== true ||
  tagPagination?.source_contract?.database_side_pagination_claimed !== false ||
  !Array.isArray(tagPagination?.execution) || tagPagination.execution.length !== 0
) failures.push(`${files.tagPagination}: tag pagination source evidence drift`);

if (
  tagProjection?.status !== 'source_verified_no_compile' ||
  tagProjection?.runtime_status !== 'not_run' ||
  tagProjection?.ownership?.dictionary_owner !== 'rustok-taxonomy' ||
  tagProjection?.ownership?.attachment_table !== 'blog_post_tags' ||
  tagProjection?.ownership?.metadata_tags_is_canonical_read_source !== false ||
  tagProjection?.ownership?.metadata_tags_is_search_projection_source !== false ||
  tagProjection?.source_contract?.blog_read_harness_rejects_metadata_resurrection !== true ||
  tagProjection?.source_contract?.stale_metadata_tags_are_ignored_by_search_harness !== true ||
  tagProjection?.source_contract?.tag_mutation_semantics_changed !== false ||
  tagProjection?.source_contract?.tag_mutation_atomic_reindex_implemented !== false ||
  tagProjection?.next_source_gap?.status !== 'tag_mutation_atomic_reindex_next' ||
  !Array.isArray(tagProjection?.execution) || tagProjection.execution.length !== 0
) failures.push(`${files.tagProjection}: canonical tag projection source evidence drift`);

for (const [source, marker, label] of [
  [slice97, 'Status: `canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending`.', files.slice97],
  [slice98, 'Status: `category_translation_postgres_evidence_source_ready_maintainer_execution_pending`.', files.slice98],
  [slice99, 'Status: `storefront_cached_public_comments_snapshot_source_ready_maintainer_execution_pending`.', files.slice99],
  [slice100, 'Status: `storefront_comment_form_fallback_not_applicable_source_verified`.', files.slice100],
  [slice102, 'Status: `tag_list_pagination_owner_bound_source_ready_maintainer_execution_pending`.', files.slice102],
  [slice103, 'Status: `tag_canonical_read_search_projection_source_ready_maintainer_execution_pending`.', files.slice103],
]) need(source, marker, label);

need(slice97, 'After retained maintainer execution of slices 95–97, define bounded lifecycle and', files.slice97);
need(slice98, 'The Comments audit track remains independently blocked on maintainer execution', files.slice98);
need(slice99, 'comment-form fallback is still a separate unfinished result', files.slice99);
need(slice100, 'comment_form_fallback = not_applicable_no_storefront_write_surface', files.slice100);
need(slice102, 'Re-audit Blog tag mutation/projection consistency as a separate ownership slice.', files.slice102);
need(slice103, 'Implement `tag_mutation_atomic_reindex` as slice 104', files.slice103);
need(slice103, 'metadata-only rows', files.slice103);

for (const marker of [
  'Status: `canonical_source_cursor_actualized_through_slice_103`.',
  '`remote_comments_transport = source_implemented_maintainer_execution_pending`',
  '`category_translation_postgres = source_ready_maintainer_execution_pending`',
  '`cached_public_comments_snapshot = source_ready_maintainer_execution_pending`',
  '`comment_form_fallback = not_applicable_no_storefront_write_surface`',
  '`tag_list_pagination = source_ready_maintainer_execution_pending`',
  '`tag_canonical_projection = source_ready_maintainer_execution_pending`',
  '`tag_mutation_atomic_reindex = next_source_gap`',
  'do not advance that source work before retained maintainer execution of slices 95–97',
  'This does **not** claim database-side pagination.',
  'metadata-only legacy rows',
  'Implement slice 104: `tag_mutation_atomic_reindex`.',
  '## Superseded historical cursor phrases',
  'No tests, Cargo commands, Node verifiers',
]) need(current, marker, files.current);

const supersededSection = between(current, '## Superseded historical cursor phrases', '## Validation boundary');
const liveCurrent = current.replace(supersededSection, '');
for (const marker of [
  'remote transport remains pending',
  'cached snapshot and comment-form fallback remain planned',
  'PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are still required before production inventory enablement',
  'then implement the remote network transport',
]) forbid(liveCurrent, marker, files.current);

const completed = between(historical, '## Completed implementation slices', '## Next results');
need(completed, '67. Executed all four Blog Comments projection PostgreSQL targets locally', files.historical);
if (/\n68\.\s/.test(completed)) {
  failures.push(`${files.historical}: historical embedded completed-slice list unexpectedly advanced without current-cursor evidence update`);
}

for (const marker of [
  '## Planning cursor',
  '[Current Implementation Cursor](./implementation-plan-current.md)',
  '[Historical Implementation Plan](./implementation-plan.md)',
  'must not be treated as the live cursor',
]) need(docsIndex, marker, files.docsIndex);
const currentLink = docsIndex.indexOf('[Current Implementation Cursor](./implementation-plan-current.md)');
const historicalLink = docsIndex.indexOf('[Historical Implementation Plan](./implementation-plan.md)');
if (currentLink < 0 || historicalLink < 0 || currentLink > historicalLink) {
  failures.push(`${files.docsIndex}: canonical current cursor must be listed before the historical plan`);
}

for (const marker of [
  'Status: `canonical_plan_current_cursor_source_ready_no_runtime_promotion`.',
  'crates/rustok-blog/docs/implementation-plan-current.md',
  'historical root implementation plan fell behind the continuation series',
  'remote_comments_transport = source_implemented_maintainer_execution_pending',
  'source_row_and_recovery_audit_retention = blocked_until_slices_95_97_execution',
  'The next autonomous source slice must be justified by a fresh repository audit',
]) need(slice101, marker, files.slice101);

if (failures.length > 0) {
  console.error('[verify-blog-canonical-plan-current] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('[verify-blog-canonical-plan-current] PASS cursor=slice-103 behavior-through=slice-103 execution=not-run');
