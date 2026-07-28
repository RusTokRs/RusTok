import { lstatSync, readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';

import {
  RAW_ARTIFACT_ROLES,
  assemblePartitionPacket,
} from './index-partition-evidence-assembly-core.mjs';
import {
  canonicalJson,
  sha256Hex,
  validatePartitionPacket,
} from './index-partition-evidence-core.mjs';

export const REVIEW_CONTRACT = 'index_partition_retained_bundle_review_v1';

const descriptorNames = Object.freeze({
  capture: 'capture.json',
  packet: 'partition-packet.json',
  admission: 'admission.json',
});

const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);
const identityOf = (stat) => `${stat.dev}:${stat.ino}`;
const portablePath = (value) => value.split(path.sep).join('/');

const requireObject = (value, label) => {
  if (!isObject(value)) throw new Error(`${label} must be an object`);
  return value;
};

const parseJsonBytes = (bytes, label) => {
  try {
    return JSON.parse(bytes.toString('utf8'));
  } catch (error) {
    throw new Error(`${label} must contain valid UTF-8 JSON: ${error.message}`);
  }
};

const ensureInsideRoot = (root, filename, label) => {
  const relative = path.relative(root, filename);
  if (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error(`${label} must stay inside the retained bundle root`);
  }
};

const readRegularFile = (filename, label) => {
  const stat = lstatSync(filename);
  if (stat.isSymbolicLink() || !stat.isFile()) {
    throw new Error(`${label} must be a regular non-symlink file`);
  }
  const bytes = readFileSync(filename);
  if (bytes.length === 0) throw new Error(`${label} must not be empty`);
  return {
    bytes,
    stat,
    identity: identityOf(stat),
    canonical: realpathSync.native(filename),
  };
};

const assertDistinctIdentity = (seen, identity, label) => {
  if (seen.has(identity)) throw new Error(`${label} aliases another retained bundle file`);
  seen.add(identity);
};

const assertCanonicalMatch = (actual, expected, label) => {
  if (canonicalJson(actual) !== canonicalJson(expected)) {
    throw new Error(`${label} does not match recalculated retained bundle content`);
  }
};

export const inspectRetainedPartitionBundle = ({ root, packetPath, admissionPath }) => {
  const resolvedRoot = path.resolve(root);
  const rootStat = lstatSync(resolvedRoot);
  if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
    throw new Error('retained bundle root must be a regular non-symlink directory');
  }
  const canonicalRoot = realpathSync.native(resolvedRoot);
  const capturePath = path.join(resolvedRoot, descriptorNames.capture);
  const resolvedPacketPath = path.resolve(packetPath ?? path.join(resolvedRoot, descriptorNames.packet));
  const resolvedAdmissionPath = path.resolve(
    admissionPath ?? path.join(resolvedRoot, descriptorNames.admission),
  );

  for (const [label, filename] of [
    ['capture file', capturePath],
    ['packet file', resolvedPacketPath],
    ['admission file', resolvedAdmissionPath],
  ]) {
    ensureInsideRoot(resolvedRoot, filename, label);
  }
  if (new Set([capturePath, resolvedPacketPath, resolvedAdmissionPath]).size !== 3) {
    throw new Error('capture, packet, and admission files must be distinct');
  }

  const captureFile = readRegularFile(capturePath, 'capture file');
  const packetFile = readRegularFile(resolvedPacketPath, 'packet file');
  const admissionFile = readRegularFile(resolvedAdmissionPath, 'admission file');
  for (const [label, file] of [
    ['capture file', captureFile],
    ['packet file', packetFile],
    ['admission file', admissionFile],
  ]) {
    ensureInsideRoot(canonicalRoot, file.canonical, label);
  }

  const capture = requireObject(parseJsonBytes(captureFile.bytes, 'capture file'), 'capture file');
  const packet = requireObject(parseJsonBytes(packetFile.bytes, 'packet file'), 'packet file');
  const admission = requireObject(parseJsonBytes(admissionFile.bytes, 'admission file'), 'admission file');
  const assembled = assemblePartitionPacket({
    manifest: packet.manifest,
    capturePath,
    capture,
  });
  assertCanonicalMatch(packet, assembled.packet, 'packet file');
  const recalculatedAdmission = validatePartitionPacket(packet);
  assertCanonicalMatch(admission, recalculatedAdmission, 'admission file');

  const seenIdentities = new Set();
  for (const [label, file] of [
    ['capture file', captureFile],
    ['packet file', packetFile],
    ['admission file', admissionFile],
  ]) {
    assertDistinctIdentity(seenIdentities, file.identity, label);
  }

  const files = [];
  for (const role of RAW_ARTIFACT_ROLES) {
    const canonical = assembled.resolvedPaths.get(role);
    const stat = lstatSync(canonical);
    const identity = assembled.identities.get(role);
    assertDistinctIdentity(seenIdentities, identity, `capture artifact ${role}`);
    files.push({
      role,
      path: portablePath(path.relative(canonicalRoot, canonical)),
      bytes: stat.size,
      sha256: packet.raw_artifacts[role],
    });
  }
  for (const [role, filename, file] of [
    ['capture', capturePath, captureFile],
    ['packet', resolvedPacketPath, packetFile],
    ['admission', resolvedAdmissionPath, admissionFile],
  ]) {
    files.push({
      role,
      path: portablePath(path.relative(resolvedRoot, filename)),
      bytes: file.bytes.length,
      sha256: sha256Hex(file.bytes),
    });
  }

  return {
    contract: REVIEW_CONTRACT,
    packet,
    admission: recalculatedAdmission,
    files,
  };
};

const renderTable = (headers, rows) => [
  `| ${headers.join(' | ')} |`,
  `| ${headers.map(() => '---').join(' | ')} |`,
  ...rows.map((row) => `| ${row.join(' | ')} |`),
];

const code = (value) => {
  const escaped = String(value)
    .replaceAll('\\', '\\\\')
    .replaceAll('`', '\\`')
    .replaceAll('|', '\\|')
    .replaceAll('\r', ' ')
    .replaceAll('\n', ' ');
  return `\`${escaped}\``;
};

export const renderRetainedPartitionReview = (inspection) => {
  requireObject(inspection, 'inspection');
  if (inspection.contract !== REVIEW_CONTRACT) {
    throw new Error(`inspection.contract must be ${REVIEW_CONTRACT}`);
  }
  const { packet, admission, files } = inspection;
  const measurements = admission.measurements;
  const lines = [
    '# Index partition retained bundle review',
    '',
    `- Review contract: ${code(REVIEW_CONTRACT)}`,
    `- Evidence ID: ${code(admission.evidence_id)}`,
    `- Admission outcome: ${code(admission.outcome)}`,
    `- Completed at: ${code(admission.completed_at)}`,
    `- Packet canonical digest: ${code(admission.packet_digest)}`,
    `- Recalculated admission matches saved admission: ${code(true)}`,
    `- Retained file count: ${code(files.length)}`,
    '',
    '## Run provenance',
    '',
    ...renderTable(['Field', 'Value'], [
      ['Repository', code(admission.run_provenance.repository)],
      ['Commit', code(admission.run_provenance.commit)],
      ['Run key', code(admission.run_provenance.run_key)],
      ['Job', code(admission.run_provenance.job)],
      ['Runner OS', code(admission.run_provenance.runner_os)],
      ['Runner architecture', code(admission.run_provenance.runner_arch)],
    ]),
    '',
    '## PostgreSQL identity',
    '',
    ...renderTable(['Field', 'Value'], [
      ['Version', code(packet.database.version)],
      ['Server version number', code(packet.database.server_version_num)],
      ['JIT', code(packet.database.jit)],
      ['System identifier', code(packet.database.system_identifier)],
      ['Database name', code(packet.database.database_name)],
    ]),
    '',
    '## Calculated measurements',
    '',
    ...renderTable(['Measurement', 'Value'], [
      ['Total rows', code(measurements.total_rows)],
      ['Total bytes', code(measurements.total_bytes)],
      ['Distinct tenants', code(measurements.distinct_tenants)],
      ['Tenant predicate coverage (bps)', code(measurements.tenant_predicate_coverage_bps)],
      ['Query runs', code(measurements.query_runs)],
      ['Mutation runs', code(measurements.mutation_runs)],
      ['Maintenance runs', code(measurements.maintenance_runs)],
      ['Cutover rehearsals', code(measurements.cutover_rehearsals)],
      ['Query plan regressions', code(measurements.query_plan_regressions)],
      ['Maximum query p95 regression (bps)', code(measurements.maximum_query_p95_regression_bps)],
      ['Maximum mutation p95 regression (bps)', code(measurements.maximum_mutation_p95_regression_bps)],
      ['Maximum WAL amplification (bps)', code(measurements.maximum_wal_amplification_bps)],
      ['Maximum partition size/mean (bps)', code(measurements.maximum_partition_size_to_mean_bps)],
      ['Maximum cutover lock (ms)', code(measurements.maximum_cutover_lock_ms)],
      ['Entity digest matches', code(measurements.entity_digest_matches)],
      ['Link digest matches', code(measurements.link_digest_matches)],
      ['Shadow caught up', code(measurements.shadow_caught_up)],
      ['Foreign keys validated', code(measurements.foreign_keys_validated)],
      ['Orphan links', code(measurements.orphan_links)],
    ]),
    '',
    '## Retained file inventory',
    '',
    ...renderTable(
      ['Role', 'Relative path', 'Bytes', 'Exact-byte SHA-256'],
      files.map((file) => [code(file.role), code(file.path), code(file.bytes), code(file.sha256)]),
    ),
    '',
    '## Admission reasons',
    '',
  ];
  if (admission.reasons.length === 0) {
    lines.push('- None.');
  } else {
    for (const reason of admission.reasons) {
      lines.push(`- ${code(reason.code)}: ${code(canonicalJson(reason))}`);
    }
  }
  lines.push(
    '',
    '## Owner gate',
    '',
    '- This read-only report recalculates packet assembly and admission from the retained bundle; it does not create, edit, or admit evidence.',
    '- Archive the six raw artifacts, `capture.json`, `partition-packet.json`, and `admission.json` as the authoritative retained inputs.',
    '- Production partition copy/replay, dual-write, relation cutover, cleanup, and query-adapter work remain forbidden until an owner reviews and archives one complete admitted real bundle.',
    '',
  );
  return lines.join('\n');
};
