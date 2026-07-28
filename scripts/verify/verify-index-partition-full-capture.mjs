#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const prefix = '[verify-index-partition-full-capture]';
const read = (filename) => readFileSync(filename, 'utf8');
const requireMarkers = (source, markers, label) => {
  for (const marker of markers) {
    if (!source.includes(marker)) throw new Error(`${label} is missing ${marker}`);
  }
};
const forbidMarkers = (source, markers, label) => {
  for (const marker of markers) {
    if (source.includes(marker)) throw new Error(`${label} must not contain ${marker}`);
  }
};
const requireExactlyOnce = (source, marker, label) => {
  const count = source.split(marker).length - 1;
  if (count !== 1) throw new Error(`${label} must contain ${marker} exactly once; found ${count}`);
};

try {
  const finalizer = read('ops/benches/src/index_storage/partition_capture.rs');
  const binary = read('ops/benches/src/bin/index_partition_capture_finalize.rs');
  const cargo = read('ops/benches/Cargo.toml');
  const module = read('ops/benches/src/index_storage/mod.rs');
  const orchestrator = read('scripts/verify/run-index-partition-evidence.mjs');
  const planTest = read('scripts/verify/index-partition-full-capture-plan.test.mjs');
  const reviewCore = read('scripts/verify/index-partition-review-core.mjs');
  const reviewCli = read('scripts/verify/render-index-partition-review.mjs');
  const archiveCore = read('scripts/verify/index-partition-archive-manifest-core.mjs');
  const archiveCli = read('scripts/verify/render-index-partition-archive-manifest.mjs');
  const archiveVerifyCli = read('scripts/verify/verify-index-partition-archive-manifest.mjs');
  const reviewTest = read('scripts/verify/index-partition-review.test.mjs');
  const tooling = read('scripts/verify/index-storage-tooling.mjs');
  const runbook = read('crates/rustok-index/docs/partition-full-capture.md');
  const plan = read('crates/rustok-index/docs/implementation-plan.md');
  const m3Start = plan.indexOf('### M3 - PostgreSQL storage engine');
  const retainedStart = plan.indexOf('#### Retained repository contract wording');
  if (m3Start < 0 || retainedStart <= m3Start) {
    throw new Error('implementation plan must contain a bounded primary M3 checklist');
  }
  const primaryM3Checklist = plan.slice(m3Start, retainedStart);

  requireMarkers(finalizer, [
    'INDEX_PARTITION_ALLOW_CAPTURE_FINALIZE',
    'index_partition_capture_v1',
    'pg_control_system()',
    'system_identifier',
    'capture.json',
    'baseline.json',
    'shadow.json',
    'query.json',
    'mutation.json',
    'maintenance.json',
    'cutover.json',
    'create_new(true)',
    'fs::hard_link',
    'refusing to overwrite',
  ], 'capture finalizer');
  requireMarkers(binary, [
    'PartitionCaptureFinalizeConfig::from_env()',
    'finalize_partition_capture',
  ], 'capture finalizer binary');
  requireMarkers(cargo, [
    'name = "index-partition-capture-finalize"',
    'path = "src/bin/index_partition_capture_finalize.rs"',
  ], 'benchmark Cargo targets');
  requireMarkers(module, [
    'mod partition_capture;',
    'PartitionCaptureFinalizeConfig',
    'finalize_partition_capture',
  ], 'index storage module exports');
  requireMarkers(orchestrator, [
    'INDEX_PARTITION_ALLOW_FULL_CAPTURE',
    "'--plan'",
    'index_partition_full_capture_plan_v1',
    'preflight_completed: true',
    'database_connection_attempted: false',
    'writes_performed: false',
    'baseEnvironmentOverrides',
    'INDEX_PARTITION_MANIFEST',
    'environment_overrides',
    'No Cargo or Node evidence stage is started.',
    'if (options.plan)',
    'JSON.stringify(plan, null, 2)',
    'for (const stage of stages)',
    'index-partition-snapshot-capture',
    'index-partition-query-evidence',
    'index-partition-mutation-evidence',
    'index-partition-maintenance-evidence',
    'index-partition-cutover-evidence',
    'index-partition-capture-finalize',
    'assemble-index-partition-evidence.mjs',
    'validate-index-partition-evidence.mjs',
    'refusing to reuse partial partition evidence output',
  ], 'full capture orchestrator');
  forbidMarkers(`${finalizer}\n${orchestrator}`, [
    'DROP TABLE',
    'TRUNCATE TABLE',
    'ALTER TABLE index_entities',
    'ALTER TABLE index_links',
    'dual-write',
  ], 'full capture tooling');
  requireMarkers(planTest, [
    'prints a no-write eight-stage full-capture plan',
    "CARGO: path.join(workspace, 'missing-cargo')",
    'database_connection_attempted',
    'writes_performed',
    'secret-value-must-not-be-printed',
    'plan.stages[0].environment_overrides',
    'INDEX_PARTITION_QUERY_AUDIT',
    'plan refuses partial output reuse without starting Cargo',
  ], 'full capture plan fixture');
  requireMarkers(reviewCore, [
    'index_partition_retained_bundle_review_v1',
    'RAW_ARTIFACT_ROLES',
    'assemblePartitionPacket',
    'assertCanonicalMatch(packet, assembled.packet',
    'validatePartitionPacket(packet)',
    'assertCanonicalMatch(admission, recalculatedAdmission',
    'aliases another retained bundle file',
    'sha256Hex(file.bytes)',
    'Exact-byte SHA-256',
    'does not create, edit, or admit evidence',
    'Production partition copy/replay',
  ], 'retained partition review core');
  requireMarkers(reviewCli, [
    "'--root'",
    "'--packet'",
    "'--admission'",
    'inspectRetainedPartitionBundle',
    'renderRetainedPartitionReview',
    'process.stdout.write',
    'It writes no files.',
  ], 'retained partition review CLI');
  requireMarkers(archiveCore, [
    'index_partition_retained_archive_manifest_v1',
    'canonical_json_without_manifest_digest_v1',
    'index_partition_retained_archive_verification_v1',
    'REVIEW_CONTRACT',
    "admission.outcome !== 'admitted'",
    'must contain exactly nine retained files',
    'total_bytes: totalBytes',
    "sha256Hex(Buffer.from(canonicalJson(payload), 'utf8'))",
    'must stay outside the retained bundle root',
    'saved archive manifest aliases a retained bundle file',
    'saved archive manifest digest does not match canonical payload',
    'saved archive manifest does not match recalculated retained bundle manifest',
    'production_lifecycle_authorized: false',
  ], 'retained partition archive manifest core');
  requireMarkers(archiveCli, [
    "'--root'",
    "'--packet'",
    "'--admission'",
    'inspectRetainedPartitionBundle',
    'buildRetainedPartitionArchiveManifest',
    'JSON.stringify(manifest, null, 2)',
    'process.stdout.write',
    'It writes no files.',
  ], 'retained partition archive manifest CLI');
  requireMarkers(archiveVerifyCli, [
    "'--root'",
    "'--manifest'",
    "'--packet'",
    "'--admission'",
    'inspectRetainedPartitionBundle',
    'verifySavedRetainedPartitionArchiveManifest',
    'JSON.stringify(receipt, null, 2)',
    'process.stdout.write',
    'outside the retained bundle',
    'The command writes no files.',
  ], 'retained partition archive verifier CLI');
  forbidMarkers(`${reviewCore}\n${reviewCli}\n${archiveCore}\n${archiveCli}\n${archiveVerifyCli}`, [
    'writeFileSync',
    'mkdirSync',
    'renameSync',
    'rmSync',
    'spawnSync',
    'DATABASE_URL',
    'INDEX_PARTITION_ALLOW',
  ], 'retained partition review and archive tooling');
  requireMarkers(reviewTest, [
    'renders a deterministic read-only nine-file retained bundle review',
    'Retained file count: `9`',
    'prints a deterministic read-only admitted archive manifest',
    'index_partition_retained_archive_manifest_v1',
    'canonical_json_without_manifest_digest_v1',
    "createHash('sha256').update(canonicalJson(payload)).digest('hex')",
    'verifies a saved admitted archive manifest without changing either input',
    'index_partition_retained_archive_verification_v1',
    'production_lifecycle_authorized',
    'rejects saved archive manifest digest drift',
    'rejects saved archive manifest semantic drift with a recalculated digest',
    'requires the saved archive manifest to stay outside the retained bundle',
    'refuses an archive manifest for a non-admitted retained bundle',
    'assert.deepEqual(snapshot(context.root), before)',
    'saved admission that does not match recalculated packet admission',
    'rejects raw artifact drift from the retained packet',
  ], 'retained partition review fixture');
  requireMarkers(tooling, [
    'partition-capture',
    'run-index-partition-evidence.mjs',
    'index-partition-full-capture-plan.test.mjs',
    'partition-report',
    'render-index-partition-review.mjs',
    'partition-archive-manifest',
    'render-index-partition-archive-manifest.mjs',
    'partition-archive-verify',
    'verify-index-partition-archive-manifest.mjs',
    'index-partition-review.test.mjs',
    'verify-index-partition-full-capture.mjs',
  ], 'index storage tooling router');
  requireMarkers(runbook, [
    'M3 partition cutover rehearsal evidence runner: `complete`',
    'M3 retained packet owner orchestration: `complete`',
    'Real retained PostgreSQL packet execution: `open`',
    'partition-capture --plan',
    'does not open a PostgreSQL connection',
    'does not start Cargo or Node evidence stages',
    'does not create the bundle directory or any output file',
    'does not print the `DATABASE_URL` value',
    'INDEX_PARTITION_ALLOW_FULL_CAPTURE=1',
    'index-partition-capture-finalize',
    'partition-report',
    'all nine retained files',
    'recalculates packet assembly and admission',
    'partition-archive-manifest',
    'refuses any outcome other than `admitted`',
    'canonical_json_without_manifest_digest_v1',
    'partition-archive-verify',
    'outside the immutable bundle',
    'index_partition_retained_archive_verification_v1',
    'production_lifecycle_authorized',
    'writes no files',
    'forbidden before one retained admitted packet',
  ], 'full partition capture runbook');
  requireMarkers(plan, [
    'M3 partition cutover rehearsal evidence runner: `complete`',
    'M3 retained packet owner orchestration: `complete`',
    'Real retained PostgreSQL packet execution: `open`',
    '- [x] Add owner-operated PostgreSQL cutover/rollback rehearsal evidence capture.',
    '- [x] Add owner-operated full retained packet orchestration and capture finalization.',
    '12. The cutover rehearsal runner validates production and retained shadow identities,',
    '13. The full-capture orchestrator requires one explicit owner opt-in, one immutable',
    'one retained admitted packet, query adapter, and production partition',
  ], 'Index implementation plan');
  requireExactlyOnce(
    primaryM3Checklist,
    '- [ ] Execute one fresh full PostgreSQL capture and retain all six raw artifacts,',
    'primary M3 checklist',
  );
  requireExactlyOnce(
    primaryM3Checklist,
    '- [ ] Review and archive one complete admitted real packet before production lifecycle',
    'primary M3 checklist',
  );
  forbidMarkers(primaryM3Checklist, [
    'Execute and retain PostgreSQL baseline/shadow, query, mutation, and maintenance',
    'Execute retained PostgreSQL maintenance and cutover evidence.',
    'Execute retained PostgreSQL cutover evidence.',
    'Assemble and validate one complete retained real packet.',
  ], 'primary M3 checklist');

  console.log(`${prefix} contract satisfied`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
