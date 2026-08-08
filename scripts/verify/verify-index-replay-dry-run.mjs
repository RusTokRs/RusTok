#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-dry-run] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const dryRunPath = 'crates/rustok-index/src/replay_dry_run.rs';
const dryRun = requireMarkers(dryRunPath, [
  'const MAX_DRY_RUN_PAGES: usize = 1_024;',
  'pub struct IndexReplayDryRunRequest',
  'locale: Option<LocaleKey>',
  'pub fn for_locale(',
  'pub fn locale(&self) -> Option<&LocaleKey>',
  'pub enum IndexReplayDryRunStatus',
  'pub struct IndexReplayDryRunOutcome',
  'pub struct SharedIndexReplayDryRunRuntime',
  'pub async fn run(',
  'source_for_schema(request.schema())',
  'registered.schema.locale_mode == LocaleMode::None',
  'IndexReplayDryRunError::LocaleScopeUnsupported',
  'IndexSourceScanRequest::for_locale(',
  '.scan(scan_request)',
  'let mut event_ids = BTreeSet::new();',
  'event_id.is_nil()',
  'DuplicateEventId',
  'self.schemas.validate_mutation(mutation)',
  'IndexMutation::Upsert',
  'IndexMutation::Delete',
  'next_cursor: cursor',
  'exact_locale_dry_run_uses_the_same_canonical_scope_for_every_page',
  'exact_locale_dry_run_rejects_a_non_localized_schema_before_source_scan',
  'pub fn materialize_index_replay_dry_run_runtime(',
  'extensions.get::<SharedIndexSourceRegistry>().cloned()',
  '.get::<SharedIndexSchemaRegistry>()',
  'extensions.insert(runtime.clone())',
]);
for (const forbidden of [
  'DatabaseConnection',
  'PostgresMutationStore',
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'IndexSourceCatalog::register',
  'index_entities',
  'index_links',
  'index_inbox',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'tokio::time::sleep',
  '.execute(',
  '.begin()',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
]) {
  if (dryRun.includes(forbidden)) {
    fail(`${dryRunPath} contains forbidden persistence/scheduler marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/tests/replay_dry_run_contract.rs', [
  'dry_run_rejects_nil_event_id_before_accepting_the_page',
  'IndexReplayDryRunError::NilEventId',
  'dry_run_rejects_page_local_duplicate_event_id',
  'IndexReplayDryRunError::DuplicateEventId',
  'valid_event_identity_page_completes_without_a_resume_cursor',
  'dry_run_materialization_requires_the_complete_registry_pair',
  'IndexReplayDryRunRuntimeCompositionError::MissingSchemaRegistry',
  'dry_run_runtime_is_single_assignment',
  'IndexReplayDryRunRuntimeCompositionError::AlreadyMaterialized',
]);

const replayRuntimePath = 'crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs';
const replayRuntime = requireMarkers(replayRuntimePath, [
  'DryRun(#[from] IndexReplayDryRunRuntimeCompositionError)',
  'materialize_index_replay_dry_run_runtime(extensions)?;',
  'extensions.contains::<SharedIndexReplayDryRunRuntime>()',
  'complete_registries_materialize_replay_and_module_work_registration',
]);
for (const forbidden of ['tokio::spawn', '.execute(', '.begin()']) {
  if (replayRuntime.includes(forbidden)) {
    fail(`${replayRuntimePath} contains forbidden IO/task marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/lib.rs', [
  'bounded side-effect-free',
  'pub mod replay_dry_run;',
  'pub use replay_dry_run::*;',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod source_timeout;',
  'pub use source_timeout::register_index_source;',
]);
requireMarkers('crates/rustok-index/src/application/source_timeout.rs', [
  'const DEFAULT_INDEX_SOURCE_CALL_TIMEOUT: Duration = Duration::from_secs(30);',
  'const INDEX_SOURCE_SCAN_TIMEOUT_CODE: &str = "index_source_scan_timeout";',
  'TimedIndexSource::new(source, DEFAULT_INDEX_SOURCE_CALL_TIMEOUT)',
]);
requireMarkers('crates/rustok-index/src/application/source_continuation.rs', [
  'locale: Option<LocaleKey>',
  'pub fn for_locale(',
  'IndexSourceContinuationError::LocaleScopeMismatch',
  'schema_wide_and_exact_locale_continuations_cannot_cross_scopes',
]);

requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  'shadow: rustok_index::SharedIndexReplayDryRunRuntime',
  'pub async fn run_shadow(',
  'context.authorize_for(request.tenant_id())?;',
  'self.shadow.run(request).await.map_err(Into::into)',
  '.get::<rustok_index::SharedIndexReplayDryRunRuntime>()',
  'IndexReplayOperatorRuntime::new(runtime, shadow)',
  'IndexReplayShadowTransportRuntime',
]);
const graphql = read('apps/server/src/graphql/index_replay.rs').split('\n#[cfg(test)]')[0];
for (const forbidden of ['SharedIndexReplayDryRunRuntime', 'IndexReplayDryRunRequest', '.run_shadow(']) {
  if (graphql.includes(forbidden)) {
    fail(`GraphQL must use only the sealed Shadow transport adapter: ${forbidden}`);
  }
}
requireMarkers('apps/server/src/graphql/index_replay.rs', [
  'async fn run_index_replay_shadow(',
  '.get::<IndexReplayShadowTransportRuntime>()',
  'pub locale: Option<String>',
  '.run(',
]);

requireMarkers('crates/rustok-index/docs/m6-bounded-replay-dry-run.md', [
  'Status: `source_complete_locale_transport_execution_pending`',
  '`IndexReplayDryRunRequest::for_locale`',
  '`SharedIndexReplayDryRunRuntime::run`',
  '`LocaleScopeUnsupported`',
  'one invocation budget from 1 through 1024 pages',
  'complete `SchemaRegistry::validate_mutation` validity',
  'Product, ProductVariant, SalesChannel',
  '30-second source-call timeout',
  'Direct low-level `IndexSourceCatalog::register` usage',
  'No-write boundary',
  'Server-owned host guard',
  '`IndexReplayOperatorRuntime::run_shadow`',
  '`runIndexReplayShadow`',
  'schema-wide or exact-locale invocation',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Bounded Replay Dry-run](./m6-bounded-replay-dry-run.md)',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add in-page interruption/timeouts, dry-run, and targeted/full/shadow rebuild modes.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-dry-run.mjs'",
  "'verify-index-replay-shadow-host-dispatch.mjs'",
  "'verify-index-replay-shadow-graphql-transport.mjs'",
]);

console.log('[verify-index-replay-dry-run] bounded no-write validation carries one canonical schema-wide or exact-locale scope without durable ownership');
