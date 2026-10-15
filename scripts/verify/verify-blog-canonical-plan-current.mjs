#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];

const files = {
  evidence: 'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json',
  current: 'crates/modules/rustok-blog/docs/implementation-plan-current.md',
  historical: 'crates/modules/rustok-blog/docs/implementation-plan.md',
  slice97: 'crates/modules/rustok-blog/docs/implementation-plan-slice-97.md',
  slice98: 'crates/modules/rustok-blog/docs/implementation-plan-slice-98.md',
  slice99: 'crates/modules/rustok-blog/docs/implementation-plan-slice-99.md',
  slice100: 'crates/modules/rustok-blog/docs/implementation-plan-slice-100.md',
  slice101: 'crates/modules/rustok-blog/docs/implementation-plan-slice-101.md',
  slice102: 'crates/modules/rustok-blog/docs/implementation-plan-slice-102.md',
  slice103: 'crates/modules/rustok-blog/docs/implementation-plan-slice-103.md',
  slice104: 'crates/modules/rustok-blog/docs/implementation-plan-slice-104.md',
  slice105: 'crates/modules/rustok-blog/docs/implementation-plan-slice-105.md',
  docsIndex: 'crates/modules/rustok-blog/docs/README.md',
  tcpTransport: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json',
  tcpServer: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json',
  tcpListener: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-listener-lifecycle.json',
  retiredTranslationPostgres: 'crates/modules/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
  fallback: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  writeSurface: 'crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
  tagPagination: 'crates/modules/rustok-blog/contracts/evidence/blog-tag-pagination-source.json',
  tagProjection: 'crates/modules/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json',
  tagMutation: 'crates/modules/rustok-blog/contracts/evidence/blog-tag-mutation-reindex-source.json',
  postCategoryName: 'crates/modules/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json',
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
const slice104 = read(files.slice104);
const slice105 = read(files.slice105);
const docsIndex = read(files.docsIndex);
const tcpTransport = json(files.tcpTransport);
const tcpServer = json(files.tcpServer);
const tcpListener = json(files.tcpListener);
const retiredTranslationPostgresPresent = fs.existsSync(absolute(files.retiredTranslationPostgres));
const fallback = json(files.fallback);
const writeSurface = json(files.writeSurface);
const tagPagination = json(files.tagPagination);
const tagProjection = json(files.tagProjection);
const tagMutation = json(files.tagMutation);
const postCategoryName = json(files.postCategoryName);

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
    evidence.latest_behavior_slice !== 105 ||
    evidence.current_recorded_slice !== 105 ||
    evidence.documentation_cursor_actualization !== 'owner_scoped_actualized_through_cat17' ||
    !Number.isInteger(evidence.historical_baseline_last_embedded_slice)
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
    tracks.category_translation_postgres?.status !== 'superseded_by_taxonomy_cutover' ||
    tracks.category_translation_postgres?.retired_slice !== files.slice98 ||
    tracks.category_translation_postgres?.retired_evidence !== files.retiredTranslationPostgres ||
    tracks.category_translation_postgres?.must_remain_absent !== true ||
    tracks.category_translation_postgres?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: retired category Translation track drift`);
  if (
    tracks.cached_public_comments_snapshot?.status !== 'source_implemented_maintainer_execution_pending' ||
    tracks.cached_public_comments_snapshot?.slice !== files.slice99 ||
    tracks.cached_public_comments_snapshot?.evidence !== files.fallback ||
    tracks.cached_public_comments_snapshot?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: cached Comments track drift`);
  if (
    tracks.comment_form_policy?.status !== 'source_verified_hide_on_provider_absence' ||
    tracks.comment_form_policy?.slice !== files.slice100 ||
    tracks.comment_form_policy?.evidence !== files.writeSurface ||
    tracks.comment_form_policy?.write_surface_evidence !== files.fallback ||
    tracks.comment_form_policy?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: comment-form policy track drift`);
  if (
    tracks.tag_list_pagination?.status !== 'source_complete_maintainer_execution_pending' ||
    tracks.tag_list_pagination?.slice !== files.slice102 ||
    tracks.tag_list_pagination?.evidence !== files.tagPagination ||
    tracks.tag_list_pagination?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: tag pagination track drift`);
  // The pagination track restates the owner evidence, so it is compared field by field with
  // it: a track can no longer keep the old offset-era claim while the evidence moved on.
  for (const key of [
    'maximum_page_size',
    'page_index_uses_saturating_arithmetic',
    'extreme_page_skips_database_fetch_after_total_count_guard',
    'database_side_pagination_claimed',
  ]) {
    const trackValue = tracks.tag_list_pagination?.[key];
    const evidenceValue = tagPagination?.source_contract?.[key];
    // Both sides have to state it: two absent fields would otherwise compare equal and the
    // cross-check would pass while saying nothing.
    if (trackValue === undefined || evidenceValue === undefined) {
      failures.push(
        `${files.evidence}: tag pagination track is missing ${key} (${files.tagPagination} states it as ${evidenceValue})`,
      );
    } else if (trackValue !== evidenceValue) {
      failures.push(
        `${files.evidence}: tag pagination track ${key} disagrees with ${files.tagPagination}`,
      );
    }
  }
  if (
    tracks.tag_canonical_projection?.status !== 'source_complete_maintainer_execution_pending' ||
    tracks.tag_canonical_projection?.slice !== files.slice103 ||
    tracks.tag_canonical_projection?.evidence !== files.tagProjection ||
    tracks.tag_canonical_projection?.attachment_source !== 'blog_post_tags' ||
    tracks.tag_canonical_projection?.dictionary_source !== 'rustok-taxonomy' ||
    tracks.tag_canonical_projection?.metadata_tags_are_canonical !== false ||
    tracks.tag_canonical_projection?.legacy_data_audit_pending !== true ||
    tracks.tag_canonical_projection?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: canonical tag projection track drift`);
  if (
    tracks.tag_mutation_atomic_reindex?.status !== 'source_complete_maintainer_execution_pending' ||
    tracks.tag_mutation_atomic_reindex?.slice !== files.slice104 ||
    tracks.tag_mutation_atomic_reindex?.evidence !== files.tagMutation ||
    tracks.tag_mutation_atomic_reindex?.canonical_source_defined !== true ||
    tracks.tag_mutation_atomic_reindex?.taxonomy_owner_transaction_api_present !== true ||
    tracks.tag_mutation_atomic_reindex?.blog_scope_reindex_same_transaction !== true ||
    tracks.tag_mutation_atomic_reindex?.manual_predelete_relation_cleanup_removed !== true ||
    tracks.tag_mutation_atomic_reindex?.declared_fk_cascade_used !== true ||
    tracks.tag_mutation_atomic_reindex?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: tag mutation atomic-reindex track drift`);
  if (
    tracks.post_category_name_projection?.status !== 'source_complete_canonical_taxonomy_read' ||
    tracks.post_category_name_projection?.slice !== files.slice105 ||
    tracks.post_category_name_projection?.evidence !== files.postCategoryName ||
    tracks.post_category_name_projection?.category_identity_source !== 'blog_posts.category_id' ||
    tracks.post_category_name_projection?.category_name_source !== 'taxonomy_owner_category.name' ||
    tracks.post_category_name_projection?.category_identity_path !==
      'blog_posts.category_id -> taxonomy_owner_category.id' ||
    tracks.post_category_name_projection?.taxonomy_scope !== 'module/blog' ||
    tracks.post_category_name_projection?.detail_projection_present !== true ||
    tracks.post_category_name_projection?.authenticated_list_projection_present !== true ||
    tracks.post_category_name_projection?.public_list_projection_present !== true ||
    tracks.post_category_name_projection?.list_batch_projection !== true ||
    tracks.post_category_name_projection?.legacy_category_translation_read_removed !== true ||
    tracks.post_category_name_projection?.category_translation_readiness_promoted !== false ||
    tracks.post_category_name_projection?.must_not_reopen_as_source_gap !== true
  ) failures.push(`${files.evidence}: post category-name projection track drift`);

  if (
    evidence.planning_result?.independent_production_source_gap_identified !== false ||
    evidence.planning_result?.next_source_gap !== null ||
    evidence.planning_result?.future_autonomous_source_work_requires_fresh_audit !== true ||
    evidence.planning_result?.historical_stale_phrases_are_live_instructions !== false ||
    evidence.planning_result?.production_behavior_changed_since_slice_101 !== true ||
    evidence.planning_result?.ffa_or_fba_promoted !== false ||
    !Array.isArray(evidence.execution) || evidence.execution.length !== 0
  ) failures.push(`${files.evidence}: planning/execution claim drift`);
}

for (const [label, artifact, expectedSurface] of [
  [files.tcpTransport, tcpTransport, 'comments_tcp_json_transport'],
  [files.tcpServer, tcpServer, 'comments_tcp_server_adapter'],
  [files.tcpListener, tcpListener, 'comments_tcp_listener_lifecycle'],
]) {
  if (
    artifact?.module !== 'blog' ||
    artifact?.surface !== expectedSurface || artifact?.status !== 'source_verified_no_compile' ||
    artifact?.runtime_status !== 'not_run'
  ) failures.push(`${label}: remote transport source evidence drift`);
}
// The transport and server-adapter artifacts name the provider as a string; the listener
// artifact names the host-side composition instead, so the check is on the composition.
for (const [label, artifact] of [[files.tcpTransport, tcpTransport], [files.tcpServer, tcpServer]]) {
  if (artifact?.provider !== 'comments') failures.push(`${label}: provider identity drift`);
}
if (
  tcpListener?.provider?.override_wrapper !== 'SharedCommentsTcpServerProvider' ||
  tcpListener?.provider?.consumer_port_reused_as_server_provider !== false ||
  tcpListener?.provider?.default_provider !== 'owner_managed_in_process' ||
  !sameSet(tcpListener?.provider?.default_provider_inputs, [
    'server_database', 'transactional_event_bus',
  ])
) failures.push(`${files.tcpListener}: host provider composition drift`);

if (!sameSet(tcpServer?.operations, [
  'create_comment', 'get_comment', 'list_comments_for_target',
  'list_public_comments_for_target', 'update_comment', 'set_comment_status', 'delete_comment',
])) failures.push(`${files.tcpServer}: seven-operation transport parity drift`);

// The Blog Category Translation provider and its PostgreSQL harness were retired by the
// Taxonomy cutover; the cursor and the evidence it cites agree only if the file stays gone.
if (retiredTranslationPostgresPresent) {
  failures.push(`${files.retiredTranslationPostgres}: retired provider evidence must remain absent`);
}

if (
  fallback?.storefront_read_degradation?.cached_thread_snapshot !== 'source_verified_no_compile' ||
  fallback?.storefront_read_degradation?.runtime_evidence !== 'pending' ||
  !sameSet(fallback?.storefront_read_degradation?.transports, ['graphql', 'native_ssr']) ||
  fallback?.storefront_write_surface?.comment_form_fallback !== 'source_verified'
) failures.push(`${files.fallback}: storefront fallback current-state drift`);

if (
  writeSurface?.status !== 'source_verified_present' ||
  writeSurface?.actualization !== 'active_storefront_write_surface_source_verified' ||
  writeSurface?.source_contract?.create_comment_surface_present !== true ||
  writeSurface?.source_contract?.comment_form_present !== true ||
  writeSurface?.source_contract?.submit_handler_present !== true ||
  writeSurface?.planning_effect?.new_storefront_write_surface_authorized !== false ||
  !Array.isArray(writeSurface?.execution) || writeSurface.execution.length !== 0
) failures.push(`${files.writeSurface}: storefront write-surface evidence drift`);

if (
  tagPagination?.status !== 'source_verified_no_compile' || tagPagination?.runtime_status !== 'not_run' ||
  tagPagination?.source_contract?.maximum_page_size !== 100 ||
  tagPagination?.source_contract?.owner_service_clamps_page_size !== true ||
  tagPagination?.source_contract?.page_index_uses_saturating_arithmetic !== true ||
  tagPagination?.source_contract?.extreme_page_skips_database_fetch_after_total_count_guard !== true ||
  tagPagination?.source_contract?.database_side_pagination_claimed !== true ||
  !Array.isArray(tagPagination?.execution) || tagPagination.execution.length !== 0
) failures.push(`${files.tagPagination}: tag pagination source evidence drift`);

if (
  tagProjection?.status !== 'source_verified_no_compile' || tagProjection?.runtime_status !== 'not_run' ||
  tagProjection?.ownership?.dictionary_owner !== 'rustok-taxonomy' ||
  tagProjection?.ownership?.attachment_table !== 'blog_post_tags' ||
  tagProjection?.ownership?.metadata_tags_is_canonical_read_source !== false ||
  tagProjection?.ownership?.metadata_tags_is_search_projection_source !== false ||
  tagProjection?.source_contract?.blog_read_harness_rejects_metadata_resurrection !== true ||
  tagProjection?.source_contract?.stale_metadata_tags_are_ignored_by_search_harness !== true ||
  !Array.isArray(tagProjection?.execution) || tagProjection.execution.length !== 0
) failures.push(`${files.tagProjection}: canonical tag projection source evidence drift`);

if (
  tagMutation?.status !== 'source_verified_no_compile' || tagMutation?.runtime_status !== 'not_run' ||
  tagMutation?.production_contract?.update_uses_supplied_transaction !== true ||
  tagMutation?.production_contract?.delete_uses_supplied_transaction !== true ||
  tagMutation?.production_contract?.blog_reindex_same_transaction !== true ||
  tagMutation?.production_contract?.manual_predelete_relation_cleanup_removed !== true ||
  tagMutation?.production_contract?.relation_cleanup_uses_declared_fk_cascade !== true ||
  tagMutation?.planning_result?.tag_mutation_source_complete !== true ||
  tagMutation?.planning_result?.additional_tag_mutation_scaffolding_required !== false ||
  !Array.isArray(tagMutation?.execution) || tagMutation.execution.length !== 0
) failures.push(`${files.tagMutation}: tag mutation atomic-reindex source evidence drift`);

if (
  postCategoryName?.status !== 'source_verified_no_compile' ||
  postCategoryName?.runtime_status !== 'not_run' ||
  postCategoryName?.source_contract?.category_identity_source !== 'blog_posts.category_id' ||
  postCategoryName?.source_contract?.category_name_source !== 'taxonomy_owner_category.name' ||
  postCategoryName?.source_contract?.category_identity_path !==
    'blog_posts.category_id -> taxonomy_owner_category.id' ||
  postCategoryName?.source_contract?.taxonomy_scope !== 'module/blog' ||
  postCategoryName?.source_contract?.tenant_bound_category_read !== true ||
  postCategoryName?.source_contract?.detail_projection_present !== true ||
  postCategoryName?.source_contract?.authenticated_list_projection_present !== true ||
  postCategoryName?.source_contract?.public_list_projection_present !== true ||
  postCategoryName?.source_contract?.list_projection_uses_batch_category_query !== true ||
  postCategoryName?.source_contract?.legacy_category_translation_read_removed !== true ||
  postCategoryName?.source_contract?.category_translation_readiness_promoted !== false ||
  postCategoryName?.planning_result?.post_category_name_source_complete !== true ||
  postCategoryName?.planning_result?.legacy_blog_category_translation_projection_retired !== true ||
  postCategoryName?.planning_result?.additional_category_name_projection_scaffolding_required !== false ||
  !Array.isArray(postCategoryName?.execution) || postCategoryName.execution.length !== 0
) failures.push(`${files.postCategoryName}: post category-name projection source evidence drift`);

for (const [source, marker, label] of [
  [slice97, 'Status: `canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending`.', files.slice97],
  [slice98, 'Status: `category_translation_postgres_evidence_source_ready_maintainer_execution_pending`.', files.slice98],
  [slice99, 'Status: `storefront_cached_public_comments_snapshot_source_ready_maintainer_execution_pending`.', files.slice99],
  [slice100, 'Status: `storefront_comment_form_active_source_verified`.', files.slice100],
  [slice102, 'Status: `tag_list_pagination_owner_bound_source_ready_maintainer_execution_pending`.', files.slice102],
  [slice103, 'Status: `tag_canonical_read_search_projection_source_ready_maintainer_execution_pending`.', files.slice103],
  [slice104, 'Status: `tag_mutation_atomic_reindex_source_ready_maintainer_execution_pending`.', files.slice104],
  [slice105, 'Status: `post_category_name_projection_source_ready_maintainer_execution_pending`.', files.slice105],
]) need(source, marker, label);

need(slice97, 'After retained maintainer execution of slices 95–97, define bounded lifecycle and', files.slice97);
need(slice98, 'The Comments audit track remains independently blocked on maintainer execution', files.slice98);
need(slice99, 'comment-form fallback is still a separate unfinished result', files.slice99);
need(slice100, 'comment_form_fallback = planned', files.slice100);
need(slice102, 'Re-audit Blog tag mutation/projection consistency as a separate ownership slice.', files.slice102);
need(slice103, 'Implement `tag_mutation_atomic_reindex` as slice 104', files.slice103);
need(slice103, 'metadata-only rows', files.slice103);
need(slice104, 'Do not add another tag mutation scaffolding slice without new evidence.', files.slice104);
need(slice105, 'No additional category-name projection scaffolding is required after this slice.', files.slice105);

for (const marker of [
  'Status: `canonical_reference_v1_source_certified_execution_evidence_pending`.',
  '`blog_category_taxonomy_cutover = source_complete_through_cat12`',
  '`blog_category_documentation_cursor = owner_scoped_actualized_through_cat17`',
  'blog_category_translation_provider = retired',
  'blog_category_translation_postgres_evidence = superseded_by_taxonomy_cutover',
  '`remote_comments_transport = source_implemented_maintainer_execution_pending`',
  '`cached_public_comments_snapshot = source_implemented_maintainer_execution_pending`',
  '`comment_form_policy = source_verified_hide_on_provider_absence`',
  '`tag_list_pagination = source_complete_maintainer_execution_pending`',
  '`tag_canonical_projection = source_complete_maintainer_execution_pending`',
  '`tag_mutation_atomic_reindex = source_complete_maintainer_execution_pending`',
  '`tag_delete_relation_cleanup = declared_fk_cascade`',
  '`post_category_name_projection = source_complete_canonical_taxonomy_read`',
  'source-complete through slice 104',
  'metadata-only legacy tag rows',
  // The storefront write surface exists now, so the cursor states the bounded claim the
  // keyset path actually makes instead of the retired offset-era non-claim.
  'bounded by the owner-clamped page size and by the saturating page index',
  // The Comments source-retention gate is still live; it is stated by tracked result, not by
  // the slice numbers the historical records used.
  'Execute the retained Comments transport/composition, restart/ambiguity',
  '## Superseded historical cursor phrases',
  '## Validation boundary',
  'No tests, Cargo commands',
]) need(current, marker, files.current);

const supersededSection = between(current, '## Superseded historical cursor phrases', '## Validation boundary');
if (supersededSection === '') {
  failures.push(`${files.current}: missing the superseded-phrase archive section`);
}
// The slice-105-era claims are archived verbatim: still readable as history, and forbidden as
// live claims (which the `liveCurrent` checks below enforce).
for (const marker of [
  'Status: `canonical_source_cursor_actualized_through_slice_105`.',
  '`category_translation_postgres = source_ready_maintainer_execution_pending`',
  '`cached_public_comments_snapshot = source_ready_maintainer_execution_pending`',
  '`comment_form_fallback = not_applicable_no_storefront_write_surface`',
  '`tag_list_pagination = source_ready_maintainer_execution_pending`',
  '`tag_canonical_projection = source_ready_maintainer_execution_pending`',
  '`tag_mutation_atomic_reindex = source_ready_maintainer_execution_pending`',
  '`post_category_name_projection = source_ready_maintainer_execution_pending`',
  'It does **not** claim database-side pagination.',
  'source-complete through slice 105',
  'No independent production source gap is claimed after slice 105.',
  'do not advance that source work before retained maintainer execution of slices 95–97',
]) need(supersededSection, marker, `${files.current} archive`);
const liveCurrent = current.replace(supersededSection, '');
for (const marker of [
  'remote transport remains pending',
  'cached snapshot and comment-form fallback remain planned',
  'PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are still required before production inventory enablement',
  'then implement the remote network transport',
  // Retired slice-97-105 vocabulary: readable in the archive, never a live claim here. The
  // `= source_ready_maintainer_execution_pending` forms are the tokens the cursor used to
  // state for these tracks; today they are `source_complete` or retired outright.
  'does **not** claim database-side pagination',
  '`category_translation_postgres = source_ready_maintainer_execution_pending`',
  '`cached_public_comments_snapshot = source_ready_maintainer_execution_pending`',
  '`comment_form_fallback = not_applicable_no_storefront_write_surface`',
  '`tag_list_pagination = source_ready_maintainer_execution_pending`',
  '`tag_canonical_projection = source_ready_maintainer_execution_pending`',
  '`tag_mutation_atomic_reindex = source_ready_maintainer_execution_pending`',
  '`post_category_name_projection = source_ready_maintainer_execution_pending`',
]) forbid(liveCurrent, marker, files.current);

const completed = between(historical, '## Completed implementation slices', '## Next results');
need(completed, '67. Executed all four Blog Comments projection PostgreSQL targets locally', files.historical);
// The historical plan is frozen evidence, so its last embedded slice is recorded in the
// cursor evidence and checked here: the archive may not advance silently, and the recorded
// boundary may not go stale behind a later entry.
const embeddedSlices = [...completed.matchAll(/^(\d+)\. /gmu)].map((match) => Number(match[1]));
const lastEmbeddedSlice = embeddedSlices.length === 0 ? null : Math.max(...embeddedSlices);
if (lastEmbeddedSlice !== evidence?.historical_baseline_last_embedded_slice) {
  failures.push(
    `${files.historical}: last embedded slice ${lastEmbeddedSlice} does not match the recorded boundary ${evidence?.historical_baseline_last_embedded_slice}`,
  );
}

for (const marker of [
  '## Planning cursor',
  '[Current Implementation Cursor](./implementation-plan-current.md)',
  '[Historical Implementation Plan](./implementation-plan.md)',
  'historical implementation records',
  'must not override the current cursor',
]) need(docsIndex, marker, files.docsIndex);
const currentLink = docsIndex.indexOf('[Current Implementation Cursor](./implementation-plan-current.md)');
const historicalLink = docsIndex.indexOf('[Historical Implementation Plan](./implementation-plan.md)');
if (currentLink < 0 || historicalLink < 0 || currentLink > historicalLink) {
  failures.push(`${files.docsIndex}: canonical current cursor must be listed before the historical plan`);
}

for (const marker of [
  'Status: `canonical_plan_current_cursor_source_ready_no_runtime_promotion`.',
  'crates/modules/rustok-blog/docs/implementation-plan-current.md',
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

console.log(
  '[verify-blog-canonical-plan-current] PASS cursor=cat17 behavior-through=slice-105 execution=not-run',
);
