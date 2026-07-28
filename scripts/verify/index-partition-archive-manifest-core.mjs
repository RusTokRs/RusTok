import { lstatSync, readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';

import {
  canonicalJson,
  sha256Hex,
} from './index-partition-evidence-core.mjs';
import { REVIEW_CONTRACT } from './index-partition-review-core.mjs';

export const ARCHIVE_MANIFEST_CONTRACT = 'index_partition_retained_archive_manifest_v1';
export const ARCHIVE_MANIFEST_DIGEST_CONTRACT = 'canonical_json_without_manifest_digest_v1';
export const ARCHIVE_VERIFICATION_CONTRACT = 'index_partition_retained_archive_verification_v1';

const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);
const identityOf = (stat) => `${stat.dev}:${stat.ino}`;

const requireObject = (value, label) => {
  if (!isObject(value)) throw new Error(`${label} must be an object`);
  return value;
};

const requireNonEmptyString = (value, label) => {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
};

const requireInteger = (value, label) => {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative safe integer`);
  }
  return value;
};

const requireDigest = (value, label) => {
  const digest = requireNonEmptyString(value, label);
  if (!/^[0-9a-f]{64}$/u.test(digest)) {
    throw new Error(`${label} must be a lowercase SHA-256 digest`);
  }
  return digest;
};

const parseJsonBytes = (bytes, label) => {
  try {
    return JSON.parse(bytes.toString('utf8'));
  } catch (error) {
    throw new Error(`${label} must contain valid UTF-8 JSON: ${error.message}`);
  }
};

const ensureOutsideRoot = (root, filename, label) => {
  const relative = path.relative(root, filename);
  const inside = relative === ''
    || (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
  if (inside) throw new Error(`${label} must stay outside the retained bundle root`);
};

const normalizeFiles = (files) => {
  if (!Array.isArray(files) || files.length !== 9) {
    throw new Error('inspection.files must contain exactly nine retained files');
  }
  const roles = new Set();
  const paths = new Set();
  return files.map((file, index) => {
    requireObject(file, `inspection.files[${index}]`);
    const role = requireNonEmptyString(file.role, `inspection.files[${index}].role`);
    const retainedPath = requireNonEmptyString(file.path, `inspection.files[${index}].path`);
    const bytes = requireInteger(file.bytes, `inspection.files[${index}].bytes`);
    const sha256 = requireDigest(file.sha256, `inspection.files[${index}].sha256`);
    if (roles.has(role)) throw new Error(`inspection.files contains duplicate role ${role}`);
    if (paths.has(retainedPath)) throw new Error(`inspection.files contains duplicate path ${retainedPath}`);
    roles.add(role);
    paths.add(retainedPath);
    return { role, path: retainedPath, bytes, sha256 };
  });
};

export const buildRetainedPartitionArchiveManifest = (inspection) => {
  requireObject(inspection, 'inspection');
  if (inspection.contract !== REVIEW_CONTRACT) {
    throw new Error(`inspection.contract must be ${REVIEW_CONTRACT}`);
  }
  const packet = requireObject(inspection.packet, 'inspection.packet');
  const admission = requireObject(inspection.admission, 'inspection.admission');
  if (admission.outcome !== 'admitted') {
    throw new Error('retained archive manifest requires admission outcome admitted');
  }
  const files = normalizeFiles(inspection.files);
  const totalBytes = files.reduce((total, file) => total + file.bytes, 0);
  requireInteger(totalBytes, 'archive manifest total bytes');
  const payload = {
    contract: ARCHIVE_MANIFEST_CONTRACT,
    digest_contract: ARCHIVE_MANIFEST_DIGEST_CONTRACT,
    source_review_contract: REVIEW_CONTRACT,
    evidence_id: requireNonEmptyString(admission.evidence_id, 'admission.evidence_id'),
    completed_at: requireNonEmptyString(admission.completed_at, 'admission.completed_at'),
    admission_outcome: admission.outcome,
    packet_digest: requireNonEmptyString(admission.packet_digest, 'admission.packet_digest'),
    run_provenance: structuredClone(requireObject(admission.run_provenance, 'admission.run_provenance')),
    database: structuredClone(requireObject(packet.database, 'packet.database')),
    file_count: files.length,
    total_bytes: totalBytes,
    files,
  };
  return {
    ...payload,
    manifest_digest: sha256Hex(Buffer.from(canonicalJson(payload), 'utf8')),
  };
};

export const verifySavedRetainedPartitionArchiveManifest = ({
  inspection,
  root,
  manifestPath,
}) => {
  const expected = buildRetainedPartitionArchiveManifest(inspection);
  const resolvedRoot = path.resolve(requireNonEmptyString(root, 'root'));
  const resolvedManifestPath = path.resolve(requireNonEmptyString(manifestPath, 'manifestPath'));
  ensureOutsideRoot(resolvedRoot, resolvedManifestPath, 'saved archive manifest');

  const manifestStat = lstatSync(resolvedManifestPath);
  if (manifestStat.isSymbolicLink() || !manifestStat.isFile()) {
    throw new Error('saved archive manifest must be a regular non-symlink file');
  }
  const manifestBytes = readFileSync(resolvedManifestPath);
  if (manifestBytes.length === 0) throw new Error('saved archive manifest must not be empty');

  const canonicalRoot = realpathSync.native(resolvedRoot);
  const canonicalManifestPath = realpathSync.native(resolvedManifestPath);
  ensureOutsideRoot(canonicalRoot, canonicalManifestPath, 'saved archive manifest');

  const manifestIdentity = identityOf(manifestStat);
  for (const file of inspection.files) {
    const retainedStat = lstatSync(path.resolve(resolvedRoot, file.path));
    if (identityOf(retainedStat) === manifestIdentity) {
      throw new Error('saved archive manifest aliases a retained bundle file');
    }
  }

  const saved = requireObject(
    parseJsonBytes(manifestBytes, 'saved archive manifest'),
    'saved archive manifest',
  );
  const savedDigest = requireDigest(saved.manifest_digest, 'saved archive manifest.manifest_digest');
  const { manifest_digest: ignoredManifestDigest, ...savedPayload } = saved;
  void ignoredManifestDigest;
  const calculatedDigest = sha256Hex(Buffer.from(canonicalJson(savedPayload), 'utf8'));
  if (savedDigest !== calculatedDigest) {
    throw new Error('saved archive manifest digest does not match canonical payload');
  }
  if (canonicalJson(saved) !== canonicalJson(expected)) {
    throw new Error('saved archive manifest does not match recalculated retained bundle manifest');
  }

  return {
    contract: ARCHIVE_VERIFICATION_CONTRACT,
    verified: true,
    source_manifest_contract: ARCHIVE_MANIFEST_CONTRACT,
    source_digest_contract: ARCHIVE_MANIFEST_DIGEST_CONTRACT,
    evidence_id: expected.evidence_id,
    packet_digest: expected.packet_digest,
    manifest_digest: expected.manifest_digest,
    saved_manifest_sha256: sha256Hex(manifestBytes),
    file_count: expected.file_count,
    total_bytes: expected.total_bytes,
    production_lifecycle_authorized: false,
  };
};
