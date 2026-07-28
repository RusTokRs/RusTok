import {
  canonicalJson,
  sha256Hex,
} from './index-partition-evidence-core.mjs';
import { REVIEW_CONTRACT } from './index-partition-review-core.mjs';

export const ARCHIVE_MANIFEST_CONTRACT = 'index_partition_retained_archive_manifest_v1';
export const ARCHIVE_MANIFEST_DIGEST_CONTRACT = 'canonical_json_without_manifest_digest_v1';

const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);

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
    const sha256 = requireNonEmptyString(file.sha256, `inspection.files[${index}].sha256`);
    if (!/^[0-9a-f]{64}$/u.test(sha256)) {
      throw new Error(`inspection.files[${index}].sha256 must be a lowercase SHA-256 digest`);
    }
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
