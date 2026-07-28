import {
  closeSync,
  fstatSync,
  lstatSync,
  openSync,
  readFileSync,
  realpathSync,
} from 'node:fs';
import path from 'node:path';

import {
  PACKET_CONTRACT,
  sha256Hex,
  validatePartitionPacket,
  validatePreparedManifest,
} from './index-partition-evidence-core.mjs';

export const CAPTURE_CONTRACT = 'index_partition_capture_v1';
export const RAW_ARTIFACT_ROLES = Object.freeze([
  'baseline',
  'shadow',
  'query',
  'mutation',
  'maintenance',
  'cutover',
]);

const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);

const requireObject = (value, label) => {
  if (!isObject(value)) throw new Error(`${label} must be an object`);
  return value;
};

const requireArray = (value, label) => {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value;
};

const requireNonEmptyString = (value, label) => {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
};

const requireExactKeys = (value, expected, label) => {
  const actual = Object.keys(requireObject(value, label)).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(`${label} must contain exactly: ${wanted.join(', ')}`);
  }
};

const parseJsonBytes = (bytes, label) => {
  try {
    return JSON.parse(bytes.toString('utf8'));
  } catch (error) {
    throw new Error(`${label} must contain valid UTF-8 JSON: ${error.message}`);
  }
};

const identityOf = (stat) => `${stat.dev}:${stat.ino}`;
const fingerprintOf = (stat) => [
  identityOf(stat),
  stat.size,
  stat.mtimeNs,
  stat.ctimeNs,
].join(':');

const readStableRegularFile = (filename, label) => {
  const pathStatBefore = lstatSync(filename, { bigint: true });
  if (pathStatBefore.isSymbolicLink() || !pathStatBefore.isFile()) {
    throw new Error(`${label} must be a regular non-symlink file`);
  }

  const descriptor = openSync(filename, 'r');
  try {
    const descriptorStatBefore = fstatSync(descriptor, { bigint: true });
    if (!descriptorStatBefore.isFile()
        || identityOf(descriptorStatBefore) !== identityOf(pathStatBefore)) {
      throw new Error(`${label} changed before it could be read`);
    }

    const bytes = readFileSync(descriptor);
    const descriptorStatAfter = fstatSync(descriptor, { bigint: true });
    const pathStatAfter = lstatSync(filename, { bigint: true });
    if (pathStatAfter.isSymbolicLink()
        || !pathStatAfter.isFile()
        || identityOf(pathStatAfter) !== identityOf(descriptorStatAfter)
        || fingerprintOf(descriptorStatBefore) !== fingerprintOf(descriptorStatAfter)
        || fingerprintOf(descriptorStatAfter) !== fingerprintOf(pathStatAfter)) {
      throw new Error(`${label} changed while it was being read`);
    }
    if (bytes.length === 0) throw new Error(`${label} must not be empty`);

    return {
      bytes,
      identity: identityOf(descriptorStatAfter),
      canonical: realpathSync.native(filename),
    };
  } finally {
    closeSync(descriptor);
  }
};

const resolveArtifactPath = (bundleRoot, canonicalBundleRoot, relative, label) => {
  requireNonEmptyString(relative, label);
  if (path.isAbsolute(relative)) throw new Error(`${label} must be relative to the capture file`);
  const normalized = path.normalize(relative);
  if (normalized === '.' || normalized === '..' || normalized.startsWith(`..${path.sep}`)) {
    throw new Error(`${label} must stay inside the capture bundle`);
  }
  const resolved = path.resolve(bundleRoot, normalized);
  const relativeToRoot = path.relative(bundleRoot, resolved);
  if (relativeToRoot === '..' || relativeToRoot.startsWith(`..${path.sep}`) || path.isAbsolute(relativeToRoot)) {
    throw new Error(`${label} must stay inside the capture bundle`);
  }
  const canonical = realpathSync.native(resolved);
  const canonicalRelative = path.relative(canonicalBundleRoot, canonical);
  if (canonicalRelative === '..'
      || canonicalRelative.startsWith(`..${path.sep}`)
      || path.isAbsolute(canonicalRelative)) {
    throw new Error(`${label} must stay inside the canonical capture bundle`);
  }
  return { resolved, canonical };
};

export const validateCaptureDescriptor = (capture) => {
  requireExactKeys(
    capture,
    ['contract', 'completed_at', 'run_provenance', 'database', 'artifacts'],
    'capture',
  );
  if (capture.contract !== CAPTURE_CONTRACT) {
    throw new Error(`capture.contract must be ${CAPTURE_CONTRACT}`);
  }
  requireNonEmptyString(capture.completed_at, 'capture.completed_at');
  requireObject(capture.run_provenance, 'capture.run_provenance');
  requireObject(capture.database, 'capture.database');
  requireExactKeys(capture.artifacts, RAW_ARTIFACT_ROLES, 'capture.artifacts');
  for (const role of RAW_ARTIFACT_ROLES) {
    requireNonEmptyString(capture.artifacts[role], `capture.artifacts.${role}`);
  }
  return capture;
};

export const readCaptureArtifacts = ({ capturePath, capture }) => {
  validateCaptureDescriptor(capture);
  const bundleRoot = path.dirname(path.resolve(capturePath));
  const canonicalBundleRoot = realpathSync.native(bundleRoot);
  const resolvedPaths = new Map();
  const identities = new Map();
  const byteLengths = new Map();
  const seenIdentities = new Set();
  const rawArtifacts = {};
  const parsed = {};

  for (const role of RAW_ARTIFACT_ROLES) {
    const label = `capture artifact ${role}`;
    const { resolved, canonical } = resolveArtifactPath(
      bundleRoot,
      canonicalBundleRoot,
      capture.artifacts[role],
      `capture.artifacts.${role}`,
    );
    const file = readStableRegularFile(resolved, label);
    if (file.canonical !== canonical) {
      throw new Error(`${label} changed before it could be read`);
    }
    if (seenIdentities.has(file.identity)) {
      throw new Error(`capture artifact files must be unique; ${role} aliases another role`);
    }
    seenIdentities.add(file.identity);
    resolvedPaths.set(role, file.canonical);
    identities.set(role, file.identity);
    byteLengths.set(role, file.bytes.length);
    rawArtifacts[role] = sha256Hex(file.bytes);
    parsed[role] = parseJsonBytes(file.bytes, label);
  }

  requireObject(parsed.baseline, 'baseline artifact');
  requireObject(parsed.shadow, 'shadow artifact');
  requireArray(parsed.query, 'query artifact');
  requireArray(parsed.mutation, 'mutation artifact');
  requireArray(parsed.maintenance, 'maintenance artifact');
  requireArray(parsed.cutover, 'cutover artifact');

  return {
    rawArtifacts,
    parsed,
    resolvedPaths,
    identities,
    byteLengths,
  };
};

export const assemblePartitionPacket = ({ manifest, capturePath, capture }) => {
  validatePreparedManifest(manifest);
  const {
    rawArtifacts,
    parsed,
    resolvedPaths,
    identities,
    byteLengths,
  } = readCaptureArtifacts({
    capturePath,
    capture,
  });
  const packet = {
    contract: PACKET_CONTRACT,
    completed_at: capture.completed_at,
    manifest: structuredClone(manifest),
    run_provenance: structuredClone(capture.run_provenance),
    raw_artifacts: rawArtifacts,
    database: structuredClone(capture.database),
    baseline: parsed.baseline,
    shadow: parsed.shadow,
    query_runs: parsed.query,
    mutation_runs: parsed.mutation,
    maintenance_runs: parsed.maintenance,
    cutover_rehearsals: parsed.cutover,
  };
  validatePartitionPacket(packet);
  return {
    packet,
    resolvedPaths,
    identities,
    byteLengths,
  };
};
