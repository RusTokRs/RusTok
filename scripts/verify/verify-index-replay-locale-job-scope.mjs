#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-locale-job-scope] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const migrationPath = 'crates/rustok-index/src/migrations/m20260808_000009_add_index_job_locale_scope.rs';
const migration = requireMarkers(migrationPath, [
  "scope_kind = 'locale'",
  "locale_key IS NOT NULL",
  "length(locale_key) BETWEEN 1 AND 32",
  'replace_postgres_scope_constraint',
  'rebuild_sqlite_table',
  'DROP INDEX IF EXISTS idx_index_jobs_scope',
  'schema_version{locale}, state',
  'STRICT_SCOPE_CHECK',
]);
if (migration.includes('partition_key')) {
  fail(`${migrationPath} must not add partition scope to index_jobs`);
}

requireMarkers('crates/rustok-index/src/migrations/mod.rs', [
  'mod m20260808_000009_add_index_job_locale_scope;',
  'Box::new(m20260808_000009_add_index_job_locale_scope::Migration)',
  '"m20260808_000009_add_index_job_locale_scope"',
  '"m20260806_000008_add_index_finding_repair_recovery"',
]);

const jobPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_job.rs';
const job = requireMarkers(jobPath, [
  'REPLAY_JOB_REQUEST_CONTRACT_V1: &str = "index_replay_job_v1"',
  'REPLAY_JOB_REQUEST_CONTRACT_V2: &str = "index_replay_job_v2"',
  'locale: Option<LocaleKey>',
  'pub(crate) fn for_locale(',
  'fn scope_kind(&self) ->',
  'if self.locale.is_some() { "locale" } else { "schema" }',
  'LocaleScopeUnsupported(SchemaRef)',
  '"locale": locale.as_str()',
  'request.locale does not match locale_key',
  'schema.locale_mode == LocaleMode::None',
  'request.scope_kind().to_owned().into()',
  'locale_key IS NOT DISTINCT FROM',
  'locale_key IS {prefix}6',
  'lease.locale',
  "partition_key = ''",
]);
for (const forbidden of ['partition_key: Option', 'scope_kind = \'partition\'', 'targeted_rebuild', 'shadow_rebuild']) {
  if (job.includes(forbidden)) fail(`${jobPath} must not absorb partition/rebuild-mode semantics: ${forbidden}`);
}

const packetPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_locale_job_tests.rs';
requireMarkers(packetPath, [
  'locale_jobs_are_distinct_from_schema_and_other_locales',
  'LocaleKey::new(locale)',
  '"EN-us"',
  'Some("en-US")',
  '"index_replay_job_v1"',
  '"index_replay_job_v2"',
  'IndexReplayJobAcquireOutcome::Busy',
  'Err(IndexReplayJobError::CheckpointMissing)',
  'locale_job_scope_rejects_nonlocalized_schema',
  'LocaleMode::None',
  'LocaleScopeUnsupported',
  'assert_eq!(count, 0);',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  '#[cfg(test)]\nmod source_replay_locale_job_tests;'.replace('\\n', '\n'),
]);

requireMarkers('crates/rustok-index/docs/m6-locale-replay-job-scope.md', [
  'Status: `job_scope_source_complete_checkpoint_worker_pending`.',
  '`scope_kind = \'locale\'`',
  '`index_replay_job_v1`',
  '`index_replay_job_v2`',
  '`LocaleMode::None`',
  '`CheckpointMissing`',
  'constructor remains crate-private',
  '`partition_key` source/job/checkpoint semantics',
]);

console.log('[verify-index-replay-locale-job-scope] durable schema/locale replay jobs are isolated and locale completion remains fail-closed until checkpoint/worker support lands');
