#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-locale-checkpoint-worker] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const applicationPath = 'crates/rustok-index/src/application/source_replay.rs';
const application = requireMarkers(applicationPath, [
  'locale: Option<LocaleKey>',
  'pub(crate) fn for_locale(',
  'IndexSourceScanRequest::for_locale(',
  'IndexReplayCheckpointKey::for_locale(',
  'registered.schema.locale_mode == LocaleMode::None',
  'SchemaNotRegistered(SchemaRef)',
  'LocaleScopeUnsupported(SchemaRef)',
  'Some(locale) => IndexSourceScanRequest::for_locale(',
  'None => IndexSourceScanRequest::new(',
]);
const schemaLookup = application.indexOf('.schema_registry\n            .get(request.schema())'.replace('\\n', '\n'));
const checkpointLoad = application.indexOf('.load_replay_checkpoint(&checkpoint_key)', schemaLookup);
const sourceScan = application.indexOf('.sources\n            .scan(scan_request)'.replace('\\n', '\n'), checkpointLoad);
if (schemaLookup < 0 || checkpointLoad <= schemaLookup || sourceScan <= checkpointLoad) {
  fail('locale schema admission must occur before checkpoint read and source scan');
}
for (const forbidden of ['partition_key', 'scope_kind = \'partition\'', 'targeted_rebuild', 'shadow_rebuild']) {
  if (application.includes(forbidden)) {
    fail(`${applicationPath} must not absorb partition or rebuild-mode semantics: ${forbidden}`);
  }
}

const adapterPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay.rs';
const adapter = requireMarkers(adapterPath, [
  'key.locale() != lease.locale()',
  'checkpoint_lease_identity_mismatch',
  'key.locale()\n            .map(|locale| locale.as_str().to_owned())'.replace('\\n', '\n'),
  '.unwrap_or_default()',
  '"".into()',
  'locale_key = {prefix}7',
  'partition_key = {prefix}8',
]);
if (adapter.includes('partition_key()') || adapter.includes('partition: Option')) {
  fail(`${adapterPath} must keep partition scope empty in this slice`);
}

const packetPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_locale_job_tests.rs';
requireMarkers(packetPath, [
  'locale_jobs_are_distinct_from_schema_and_other_locales',
  'IndexReplayCheckpointKey::new(',
  'IndexReplayCheckpointKey::for_locale(',
  'LocaleKey::new("EN-us")',
  'fixture.jobs.succeed(&schema_lease).await.unwrap();',
  'Err(IndexReplayJobError::CheckpointMissing)',
  'fixture.jobs.succeed(&en_lease).await.unwrap();',
  'fixture.jobs.succeed(&de_lease).await',
  '"en-US"',
  'checkpoint_rows.len(), 2',
]);

requireMarkers('crates/rustok-index/docs/m6-locale-replay-checkpoint-worker.md', [
  'Status: `checkpoint_worker_source_complete_runner_pending`.',
  '`SchemaNotRegistered`',
  '`LocaleScopeUnsupported`',
  '`IndexSourceScanRequest::for_locale(...)`',
  '`IndexReplayJobLease.locale`',
  '`partition_key` remains the empty string',
  '`IndexReplayRunRequest` or multi-page runner scope',
  'optional GraphQL `locale`',
]);

console.log('[verify-index-replay-locale-checkpoint-worker] one-page locale admission, source scope, checkpoint identity, and durable completion remain exact while runner/GraphQL/partition modes stay separate');
