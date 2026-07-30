#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-partition-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const corePath = 'scripts/verify/index-partition-evidence-core.mjs';
const core = requireMarkers(corePath, [
  "MANIFEST_CONTRACT = 'index_partition_evidence_manifest_v1'",
  "PACKET_CONTRACT = 'index_partition_evidence_packet_v1'",
  "ADMISSION_CONTRACT = 'index_partition_admission_v1'",
  "SHADOW_PLAN_VERSION = 'tenant_hash_shadow_v1'",
  "PLAN_DIGEST_CONTRACT = 'normalized_partition_plan_v1'",
  'export const computeEvidenceId',
  'export const deriveShadowRelations',
  'export const renderShadowBootstrapSql',
  'export const prepareManifest',
  'export const validatePreparedManifest',
  'export const validatePartitionPacket',
  "manifest.repository must be RusTokRs/RusTok",
  'manifest.run_key must be a bounded stable run identifier',
  "manifest.postgres_image must be postgres:16",
  "manifest.strategy must be tenant_hash",
  'manifest.modulus must be a power of two between 2 and 128',
  'packet.manifest.evidence_id does not match canonical manifest bytes',
  'packet.run_provenance.commit must match the manifest',
  'packet.raw_artifacts must contain exactly',
  'packet.database.system_identifier must contain only digits',
  'packet timestamps must satisfy baseline <= shadow <= completed',
  'packet_digest: sha256Hex(canonicalJson(packet))',
  'tenant_predicate_coverage_bps',
  'query_plan_regressions',
  'maximum_query_p95_regression_bps',
  'maximum_mutation_p95_regression_bps',
  'maximum_wal_amplification_bps',
  'maximum_partition_size_to_mean_bps',
  'maximum_cutover_lock_ms',
  "outcome: reasons.length === 0 ? 'admitted' : 'keep_unpartitioned'",
  "code: 'rollback_not_verified'",
  "code: 'production_relations_changed'",
]);

for (const forbidden of [
  'ALTER TABLE "index_entities"',
  'ALTER TABLE "index_links"',
  'DROP TABLE "index_entities"',
  'DROP TABLE "index_links"',
  'RENAME TO index_entities',
  'RENAME TO index_links',
  'VACUUM FULL',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
]) {
  if (core.includes(forbidden)) fail(`${corePath} contains forbidden marker ${forbidden}`);
}

const prepare = requireMarkers('scripts/verify/prepare-index-partition-evidence.mjs', [
  '--input, --manifest, and --bootstrap must reference distinct paths',
  'refusing to overwrite existing file',
  'writeFileSync(temporary, content',
  'linkSync(temporary, filename)',
  'writeAtomicNew(options.manifest',
  'writeAtomicNew(options.bootstrap',
  'rmSync(options.manifest, { force: true })',
]);
if (prepare.includes("writeFileSync(options.manifest")) {
  fail('manifest preparer must publish through the atomic new-file helper');
}

const assemblyCore = requireMarkers('scripts/verify/index-partition-evidence-assembly-core.mjs', [
  "CAPTURE_CONTRACT = 'index_partition_capture_v1'",
  'RAW_ARTIFACT_ROLES = Object.freeze',
  "['contract', 'completed_at', 'run_provenance', 'database', 'artifacts']",
  "requireExactKeys(capture.artifacts, RAW_ARTIFACT_ROLES, 'capture.artifacts')",
  'realpathSync.native(bundleRoot)',
  'must stay inside the canonical capture bundle',
  'must be a regular non-symlink file',
  'const identityOf = (stat) => `${stat.dev}:${stat.ino}`',
  'const fingerprintOf = (stat)',
  'readStableRegularFile(resolved, label)',
  'capture artifact files must be unique',
  'rawArtifacts[role] = sha256Hex(file.bytes)',
  'fingerprints.set(role, file.fingerprint)',
  'byteLengths.set(role, file.bytes.length)',
  'export const readCaptureArtifacts',
  'export const assemblePartitionPacket',
  'const {',
  'rawArtifacts,',
  'parsed,',
  'resolvedPaths,',
  'identities,',
  'fingerprints,',
  'byteLengths,',
  '} = readCaptureArtifacts({',
  'validatePartitionPacket(packet)',
  'return {',
  'packet,',
  'resolvedPaths,',
  'identities,',
  'fingerprints,',
  'byteLengths,',
]);
for (const forbidden of ['readlinkSync(', 'followSymlink', 'artifactBundle =']) {
  if (assemblyCore.includes(forbidden)) {
    fail(`partition assembly core contains forbidden bypass behavior: ${forbidden}`);
  }
}

requireMarkers('scripts/verify/assemble-index-partition-evidence.mjs', [
  '--manifest, --capture, and --output must reference distinct paths',
  '--manifest and --capture must not alias the same file',
  '--output must not alias a raw artifact path',
  'manifest, capture, and raw artifacts must be distinct files',
  'refusing to overwrite existing file',
  'writeFileSync(temporary, content',
  'linkSync(temporary, filename)',
  'const { packet, resolvedPaths, identities } = assemblePartitionPacket({',
]);

requireMarkers('scripts/verify/validate-index-partition-evidence.mjs', [
  '--input and --output must reference distinct paths',
  'rmSync(options.output, { force: true })',
  'validatePartitionPacket(packet)',
  'writeAtomic(options.output',
]);

requireMarkers('scripts/verify/index-partition-evidence.test.mjs', [
  'manifest identity and shadow bootstrap are deterministic and non-destructive',
  'complete measured packet is admitted with calculated metrics',
  'regressed packet stays unpartitioned with typed calculated reasons',
  'manifest tampering, provenance drift, and incomplete groups fail validation',
  "run_key: 'fixture-run-1'",
  "plan_digest_contract: 'normalized_partition_plan_v1'",
  'raw_artifacts',
  'system_identifier',
  'admission.packet_digest.length',
  'run_provenance.commit must match the manifest',
  'tenant_predicate_coverage',
  'link_digest_mismatch',
  'query_plan_regressions',
  'wal_amplification',
  'partition_size_skew',
  'rollback_not_verified',
  'production_relations_changed',
  'assert.doesNotMatch(sql, /DROP TABLE/u)',
  'assert.doesNotMatch(sql, /RENAME TO/u)',
]);

requireMarkers('scripts/verify/index-partition-evidence-assembly.test.mjs', [
  'assembly hashes exact raw bytes and emits a structurally validated packet',
  'capture artifact paths fail closed on traversal and duplicate file identity',
  'assembler CLI publishes once and never aliases or overwrites retained inputs',
  'whitespace changes exact-byte identity',
  'must stay inside the capture bundle',
  'artifact files must be unique',
  'raw-shadow-hardlink.json',
  'refusing to overwrite existing file',
  'must not alias a raw artifact path',
  'must not alias the same file',
]);

requireMarkers('scripts/verify/index-storage-tooling.mjs', [
  'partition-prepare',
  'partition-assemble',
  'partition-validate',
  "'verify-index-partition-evidence.mjs'",
  "scriptPath('index-partition-evidence.test.mjs')",
  "scriptPath('index-partition-evidence-assembly.test.mjs')",
  "runScript('prepare-index-partition-evidence.mjs', args)",
  "runScript('assemble-index-partition-evidence.mjs', args)",
  "runScript('validate-index-partition-evidence.mjs', args)",
]);

requireMarkers('.github/workflows/index-storage-smoke.yml', [
  'scripts/verify/index-partition-evidence-assembly-core.mjs',
  'scripts/verify/assemble-index-partition-evidence.mjs',
  'scripts/verify/index-partition-evidence-assembly.test.mjs',
  'node --check scripts/verify/index-partition-evidence-assembly-core.mjs',
  'node --check scripts/verify/assemble-index-partition-evidence.mjs',
  'node --test scripts/verify/index-partition-evidence-assembly.test.mjs',
]);

requireMarkers('crates/rustok-index/tests/module.rs', [
  'module_registers_canonical_storage_migrations',
  'let migrations = module.migrations();',
  'm20260727_000001_create_index_records',
  'm20260727_000002_create_index_delivery_state',
  'm20260727_000003_create_index_operations',
  'migration_dependencies',
]);

requireMarkers('crates/rustok-index/docs/partition-evidence-runbook.md', [
  'index_partition_evidence_manifest_v1',
  'index_partition_capture_v1',
  'index_partition_evidence_packet_v1',
  'index_partition_admission_v1',
  'normalized_partition_plan_v1',
  'run_key',
  'partition-prepare',
  'partition-assemble',
  'partition-validate',
  'six raw JSON artifacts',
  'regular files, never symbolic links',
  'hard-link aliases',
  'PostgreSQL system identifier',
  'SHA-256 digests for the retained raw baseline',
  'Tenant-predicate coverage is calculated by the validator',
  'The packet cannot supply a precomputed pass/fail value',
  'packet_digest',
  'It must not contain production `ALTER TABLE`, `DROP TABLE`, `RENAME TO`',
]);

requireMarkers('crates/rustok-index/docs/README.md', [
  'M3 partition evidence packet tooling: `complete`',
  'M3 partition evidence capture/assembly: `complete`',
  'The repository owner still executes and',
  'retains the PostgreSQL packet.',
  '[M3 partition evidence runbook](./partition-evidence-runbook.md)',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M3 partition evidence packet tooling: `complete`',
  '- M3 partition evidence capture/assembly: `complete`',
  '- [x] Add immutable partition evidence manifest, measured packet validator, and',
  '- [x] Add exact-byte raw-artifact capture assembly with bundle confinement and',
  '- [ ] Execute retained PostgreSQL partition baseline/shadow evidence.',
  'The seventh M3 slice adds fail-closed raw-artifact packet assembly.',
  'node --test scripts/verify/index-partition-evidence-assembly.test.mjs',
  'node scripts/verify/verify-index-partition-evidence.mjs',
]);

console.log('[verify-index-partition-evidence] OK');
