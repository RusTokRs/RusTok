import {
  closeSync,
  fstatSync,
  lstatSync,
  openSync,
  readdirSync,
  readFileSync,
  realpathSync,
} from 'node:fs';
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
const fingerprintOf = (stat) => [
  identityOf(stat),
  stat.size,
  stat.mtimeNs,
  stat.ctimeNs,
].join(':');

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

const requireIdentity = (value, label) => {
  const identity = requireNonEmptyString(value, label);
  if (!/^\d+:\d+$/u.test(identity)) {
    throw new Error(`${label} must be a decimal device:inode identity`);
  }
  return identity;
};

const requireFingerprint = (value, label) => {
  const fingerprint = requireNonEmptyString(value, label);
  if (!/^\d+:\d+:\d+:\d+:\d+$/u.test(fingerprint)) {
    throw new Error(`${label} must be a decimal device:inode:size:mtimeNs:ctimeNs fingerprint`);
  }
  return fingerprint;
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

const ensureOutsideRoot = (root, filename, label) => {
  const relative = path.relative(root, filename);
  const inside = relative === ''
    || (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
  if (inside) throw new Error(`${label} must stay outside the retained bundle root`);
};

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
      fingerprint: fingerprintOf(descriptorStatAfter),
      canonical: realpathSync.native(filename),
    };
  } finally {
    closeSync(descriptor);
  }
};

const readRootSnapshot = (resolvedRoot) => {
  const stat = lstatSync(resolvedRoot, { bigint: true });
  if (stat.isSymbolicLink() || !stat.isDirectory()) {
    throw new Error('retained bundle root must be a regular non-symlink directory');
  }
  return {
    identity: identityOf(stat),
    fingerprint: fingerprintOf(stat),
    canonical: realpathSync.native(resolvedRoot),
  };
};

const assertRootUnchanged = ({ resolvedRoot, expected }) => {
  const current = readRootSnapshot(resolvedRoot);
  if (current.identity !== expected.identity
      || current.fingerprint !== expected.fingerprint
      || current.canonical !== expected.canonical) {
    throw new Error('retained bundle root changed after inspection');
  }
  return current;
};

const normalizeFiles = (files, { requireInspectionSnapshot = false } = {}) => {
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
    const normalized = { role, path: retainedPath, bytes, sha256 };
    if (requireInspectionSnapshot) {
      normalized.identity = requireIdentity(
        file.identity,
        `inspection.files[${index}].identity`,
      );
      normalized.fingerprint = requireFingerprint(
        file.fingerprint,
        `inspection.files[${index}].fingerprint`,
      );
    }
    return normalized;
  });
};

const requireDirectoryPath = (value, label) => {
  if (value === '') return value;
  const directoryPath = requireNonEmptyString(value, label);
  if (directoryPath.includes('\\')
      || directoryPath.startsWith('/')
      || directoryPath.endsWith('/')
      || path.posix.normalize(directoryPath) !== directoryPath
      || directoryPath === '.'
      || directoryPath === '..'
      || directoryPath.startsWith('../')) {
    throw new Error(`${label} must be a normalized portable relative directory path`);
  }
  return directoryPath;
};

const normalizeDirectories = (directories, inspectedRoot) => {
  if (!Array.isArray(directories) || directories.length === 0) {
    throw new Error('inspection.directories must contain the retained bundle directory snapshot');
  }
  const paths = new Set();
  const normalized = directories.map((directory, index) => {
    requireObject(directory, `inspection.directories[${index}]`);
    const directoryPath = requireDirectoryPath(
      directory.path,
      `inspection.directories[${index}].path`,
    );
    if (paths.has(directoryPath)) {
      throw new Error(`inspection.directories contains duplicate path ${directoryPath || '.'}`);
    }
    paths.add(directoryPath);
    if (!Array.isArray(directory.entries)) {
      throw new Error(`inspection.directories[${index}].entries must be an array`);
    }
    const entries = directory.entries.map((entry, entryIndex) => {
      const name = requireNonEmptyString(
        entry,
        `inspection.directories[${index}].entries[${entryIndex}]`,
      );
      if (name === '.' || name === '..' || name.includes('/') || name.includes('\\')) {
        throw new Error(`inspection.directories[${index}].entries must contain direct entry names`);
      }
      return name;
    });
    if (new Set(entries).size !== entries.length
        || canonicalJson(entries) !== canonicalJson([...entries].sort())) {
      throw new Error(`inspection.directories[${index}].entries must be sorted and distinct`);
    }
    return {
      path: directoryPath,
      identity: requireIdentity(directory.identity, `inspection.directories[${index}].identity`),
      fingerprint: requireFingerprint(
        directory.fingerprint,
        `inspection.directories[${index}].fingerprint`,
      ),
      entries,
    };
  }).sort((left, right) => left.path.localeCompare(right.path));
  const rootDirectory = normalized.find((directory) => directory.path === '');
  if (!rootDirectory) throw new Error('inspection.directories must include the retained bundle root');
  if (rootDirectory.identity !== inspectedRoot.identity
      || rootDirectory.fingerprint !== inspectedRoot.fingerprint) {
    throw new Error('inspection root and directory snapshots must agree');
  }
  const byPath = new Map(normalized.map((directory) => [directory.path, directory]));
  for (const directory of normalized) {
    if (directory.path === '') continue;
    const parentPath = path.posix.dirname(directory.path);
    const normalizedParent = parentPath === '.' ? '' : parentPath;
    const parent = byPath.get(normalizedParent);
    if (!parent || !parent.entries.includes(path.posix.basename(directory.path))) {
      throw new Error(`inspection.directories is missing parent inventory for ${directory.path}`);
    }
  }
  return normalized;
};

const readCurrentDirectorySnapshot = ({ resolvedRoot, canonicalRoot, directory }) => {
  const filename = path.resolve(resolvedRoot, directory.path.split('/').join(path.sep));
  ensureInsideRoot(resolvedRoot, filename, `retained bundle directory ${directory.path || '.'}`);
  const before = lstatSync(filename, { bigint: true });
  if (before.isSymbolicLink() || !before.isDirectory()) {
    throw new Error(`retained bundle directory ${directory.path || '.'} changed after inspection`);
  }
  const canonical = realpathSync.native(filename);
  ensureInsideRoot(canonicalRoot, canonical, `retained bundle directory ${directory.path || '.'}`);
  const entries = readdirSync(filename).sort();
  const after = lstatSync(filename, { bigint: true });
  if (after.isSymbolicLink()
      || !after.isDirectory()
      || identityOf(before) !== identityOf(after)
      || fingerprintOf(before) !== fingerprintOf(after)) {
    throw new Error(`retained bundle directory ${directory.path || '.'} changed while it was being read`);
  }
  return {
    identity: identityOf(after),
    fingerprint: fingerprintOf(after),
    entries,
  };
};

const assertDirectoryInventoryUnchanged = ({ directories, resolvedRoot, canonicalRoot }) => {
  for (const directory of directories) {
    const current = readCurrentDirectorySnapshot({ resolvedRoot, canonicalRoot, directory });
    const label = directory.path || '.';
    if (current.identity !== directory.identity) {
      throw new Error(`retained bundle directory ${label} identity changed after inspection`);
    }
    if (current.fingerprint !== directory.fingerprint) {
      throw new Error(`retained bundle directory ${label} metadata changed after inspection`);
    }
    if (canonicalJson(current.entries) !== canonicalJson(directory.entries)) {
      throw new Error(`retained bundle directory ${label} inventory changed after inspection`);
    }
  }
};

const assertRetainedFilesUnchanged = ({
  files,
  resolvedRoot,
  canonicalRoot,
  manifestIdentity,
}) => {
  const retainedIdentities = new Set();
  for (const file of files) {
    const filename = path.resolve(resolvedRoot, file.path);
    ensureInsideRoot(resolvedRoot, filename, `retained bundle file ${file.role}`);
    const current = readStableRegularFile(filename, `retained bundle file ${file.role}`);
    ensureInsideRoot(canonicalRoot, current.canonical, `retained bundle file ${file.role}`);
    if (current.identity === manifestIdentity) {
      throw new Error('saved archive manifest aliases a retained bundle file');
    }
    if (retainedIdentities.has(current.identity)) {
      throw new Error(`retained bundle file ${file.role} aliases another retained bundle file after inspection`);
    }
    retainedIdentities.add(current.identity);
    if (current.identity !== file.identity) {
      throw new Error(`retained bundle file ${file.role} identity changed after inspection`);
    }
    if (current.bytes.length !== file.bytes || sha256Hex(current.bytes) !== file.sha256) {
      throw new Error(`retained bundle file ${file.role} changed after inspection`);
    }
    if (current.fingerprint !== file.fingerprint) {
      throw new Error(`retained bundle file ${file.role} metadata changed after inspection`);
    }
  }
};

const assertSavedManifestUnchanged = ({
  resolvedManifestPath,
  canonicalRoot,
  initial,
}) => {
  const current = readStableRegularFile(resolvedManifestPath, 'saved archive manifest');
  ensureOutsideRoot(canonicalRoot, current.canonical, 'saved archive manifest');
  if (current.identity !== initial.identity
      || current.fingerprint !== initial.fingerprint
      || current.canonical !== initial.canonical
      || !current.bytes.equals(initial.bytes)) {
    throw new Error('saved archive manifest changed after it was verified');
  }
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
  const inspectedFiles = normalizeFiles(inspection.files, { requireInspectionSnapshot: true });
  const inspectedRoot = {
    identity: requireIdentity(inspection.rootIdentity, 'inspection.rootIdentity'),
    fingerprint: requireFingerprint(inspection.rootFingerprint, 'inspection.rootFingerprint'),
    canonical: requireNonEmptyString(inspection.rootCanonical, 'inspection.rootCanonical'),
  };
  const inspectedDirectories = normalizeDirectories(inspection.directories, inspectedRoot);
  const resolvedRoot = path.resolve(requireNonEmptyString(root, 'root'));
  const resolvedManifestPath = path.resolve(requireNonEmptyString(manifestPath, 'manifestPath'));
  ensureOutsideRoot(resolvedRoot, resolvedManifestPath, 'saved archive manifest');

  const currentRoot = assertRootUnchanged({ resolvedRoot, expected: inspectedRoot });
  const canonicalRoot = currentRoot.canonical;
  assertDirectoryInventoryUnchanged({
    directories: inspectedDirectories,
    resolvedRoot,
    canonicalRoot,
  });
  const manifestFile = readStableRegularFile(resolvedManifestPath, 'saved archive manifest');
  ensureOutsideRoot(canonicalRoot, manifestFile.canonical, 'saved archive manifest');

  const saved = requireObject(
    parseJsonBytes(manifestFile.bytes, 'saved archive manifest'),
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

  assertRetainedFilesUnchanged({
    files: inspectedFiles,
    resolvedRoot,
    canonicalRoot,
    manifestIdentity: manifestFile.identity,
  });
  assertSavedManifestUnchanged({
    resolvedManifestPath,
    canonicalRoot,
    initial: manifestFile,
  });
  assertDirectoryInventoryUnchanged({
    directories: inspectedDirectories,
    resolvedRoot,
    canonicalRoot,
  });
  assertRootUnchanged({ resolvedRoot, expected: inspectedRoot });

  return {
    contract: ARCHIVE_VERIFICATION_CONTRACT,
    verified: true,
    retained_files_rechecked: true,
    source_manifest_contract: ARCHIVE_MANIFEST_CONTRACT,
    source_digest_contract: ARCHIVE_MANIFEST_DIGEST_CONTRACT,
    evidence_id: expected.evidence_id,
    packet_digest: expected.packet_digest,
    manifest_digest: expected.manifest_digest,
    saved_manifest_sha256: sha256Hex(manifestFile.bytes),
    file_count: expected.file_count,
    total_bytes: expected.total_bytes,
    production_lifecycle_authorized: false,
  };
};
