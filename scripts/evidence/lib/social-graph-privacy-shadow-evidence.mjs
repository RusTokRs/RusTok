import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { isDeepStrictEqual } from 'node:util';

export const CAPTURE_CONTRACT = 'social_graph_index_privacy_shadow_window_capture_v1';
export const ADMISSION_CONTRACT = 'social_graph_index_privacy_shadow_window_admission_v1';
export const START_FILE = 'start.prom';
export const END_FILE = 'end.prom';
export const DESCRIPTOR_FILE = 'capture.json';
export const INVENTORY = [DESCRIPTOR_FILE, END_FILE, START_FILE];
export const MAX_SNAPSHOT_BYTES = 4 * 1024 * 1024;
export const MAX_DESCRIPTOR_BYTES = 512 * 1024;
export const MAX_WINDOW_SECONDS = 7 * 24 * 60 * 60;
export const TIMESTAMP_SKEW_SECONDS = 60;

export const METRICS = Object.freeze({
  collectorStarted: 'rustok_social_graph_index_privacy_shadow_collector_started_timestamp_seconds',
  observations: 'rustok_social_graph_index_privacy_shadow_observations_total',
  failures: 'rustok_social_graph_index_privacy_shadow_failures_total',
  duration: 'rustok_social_graph_index_privacy_shadow_comparison_duration_seconds',
  lastObservation: 'rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds',
});

export const OPERATIONS = Object.freeze([
  'blocks_between',
  'source_mutes_target',
  'source_follows_target',
  'source_follows_targets',
]);
export const OUTCOMES = Object.freeze([
  'match_positive',
  'match_negative',
  'false_negative',
  'false_positive',
  'match_batch_empty',
  'match_batch_nonempty',
  'batch_missing',
  'batch_extra',
  'batch_mixed',
  'error',
]);
export const ERROR_CODES = Object.freeze([
  'social_graph.index_privacy_unavailable',
  'social_graph.index_privacy_contract_invalid',
  'other',
]);
export const HISTOGRAM_BUCKETS = Object.freeze([
  '0.0005', '0.001', '0.0025', '0.005', '0.01', '0.025', '0.05', '0.1',
  '0.25', '0.5', '1', '2.5', '5', '+Inf',
]);

const METRIC_PREFIX = 'rustok_social_graph_index_privacy_shadow_';
const SAMPLE_PATTERN = /^([a-zA-Z_:][a-zA-Z0-9_:]*)(?:\{([^}]*)\})?\s+([^\s]+)$/;
const LABEL_PATTERN = /(?:^|,)([a-zA-Z_][a-zA-Z0-9_]*)="([^"\\]*)"/gy;

export function fail(message) {
  throw new Error(message);
}

export function ensure(condition, message) {
  if (!condition) fail(message);
}

export function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

export function artifactDescriptor(filePath, bytes) {
  return { path: filePath, bytes: bytes.length, sha256: sha256(bytes) };
}

export function runnerIdentity(keys, fallbackJob) {
  const first = keys.map((key) => process.env[key]?.trim()).find(Boolean);
  return {
    job: first || fallbackJob,
    runner_os: process.env.RUNNER_OS?.trim() || os.platform(),
    runner_arch: process.env.RUNNER_ARCH?.trim() || os.arch(),
  };
}

export function validateRepository(value, label) {
  const parts = value.split('/');
  ensure(
    parts.length === 2 && parts.every((part) => part.length > 0 && part.length <= 100),
    `${label} must use owner/repository form`,
  );
}

export function validateRunKey(value, label) {
  ensure(value.length >= 1 && value.length <= 128, `${label} must contain 1-128 bytes`);
  ensure(/^[A-Za-z0-9_.-]+$/.test(value), `${label} contains unsupported characters`);
  ensure(value !== '.' && value !== '..', `${label} must not be dot traversal`);
}

export function validateCommit(value, label) {
  ensure(/^[0-9a-f]{40}$/.test(value), `${label} must be a 40-character lowercase Git commit`);
}

export function parseUtc(value, label) {
  ensure(typeof value === 'string' && /(?:Z|[+-]00:00)$/.test(value), `${label} must be UTC RFC3339`);
  const timestamp = Date.parse(value);
  ensure(Number.isFinite(timestamp), `${label} must be a valid UTC RFC3339 timestamp`);
  return new Date(timestamp);
}

export function validateWindow(startedAt, endedAt) {
  const durationSeconds = Math.floor((endedAt.getTime() - startedAt.getTime()) / 1000);
  ensure(
    durationSeconds > 0 && durationSeconds <= MAX_WINDOW_SECONDS,
    `privacy-shadow window must be between 1 and ${MAX_WINDOW_SECONDS} seconds`,
  );
  return durationSeconds;
}

export function absolutePath(value, base = process.cwd()) {
  return path.isAbsolute(value) ? value : path.join(base, value);
}

export function canonicalRegularFile(value, label) {
  const resolved = fs.realpathSync(value);
  const metadata = fs.lstatSync(resolved);
  ensure(metadata.isFile() && !metadata.isSymbolicLink(), `${label} must be a regular non-symlink file`);
  return resolved;
}

export function readStableRegularFile(filePath, maxBytes, label) {
  const before = fs.lstatSync(filePath);
  ensure(before.isFile() && !before.isSymbolicLink(), `${label} must be a regular non-symlink file`);
  ensure(before.size <= maxBytes, `${label} exceeds the retained size limit`);
  const first = fs.readFileSync(filePath);
  const second = fs.readFileSync(filePath);
  const after = fs.lstatSync(filePath);
  ensure(
    after.isFile() && !after.isSymbolicLink() && before.size === after.size && first.equals(second),
    `${label} changed while it was being read`,
  );
  return first;
}

export function ensureAbsent(target, label) {
  try {
    fs.lstatSync(target);
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw error;
  }
  fail(`${label} already exists: ${target}`);
}

export function writeNewFile(target, bytes, label) {
  const handle = fs.openSync(target, 'wx');
  try {
    fs.writeFileSync(handle, bytes);
    fs.fsyncSync(handle);
  } finally {
    fs.closeSync(handle);
  }
  const metadata = fs.lstatSync(target);
  ensure(metadata.isFile() && !metadata.isSymbolicLink(), `${label} is not a regular file`);
}

export function writeJsonNew(target, value, label) {
  ensureAbsent(target, label);
  writeNewFile(target, Buffer.from(`${JSON.stringify(value, null, 2)}\n`, 'utf8'), label);
}

export function validateBundleRoot(value) {
  const metadata = fs.lstatSync(value);
  ensure(metadata.isDirectory() && !metadata.isSymbolicLink(), 'bundle must be a regular non-symlink directory');
  return fs.realpathSync(value);
}

export function readInventory(root) {
  return fs.readdirSync(root).map((name) => {
    const target = path.join(root, name);
    const metadata = fs.lstatSync(target);
    ensure(metadata.isFile() && !metadata.isSymbolicLink(), `bundle entry must be a regular file: ${name}`);
    return name;
  }).sort();
}

export function ensureInventory(root, expected) {
  const actual = readInventory(root);
  ensure(isDeepStrictEqual(actual, [...expected].sort()), `bundle inventory mismatch: ${JSON.stringify(actual)}`);
  return actual;
}

export function resolveReceiptPath(value, forbiddenRoot) {
  const parent = path.dirname(value);
  const metadata = fs.lstatSync(parent);
  ensure(metadata.isDirectory() && !metadata.isSymbolicLink(), 'receipt parent must be a regular non-symlink directory');
  const resolved = path.join(fs.realpathSync(parent), path.basename(value));
  ensure(!resolved.startsWith(`${forbiddenRoot}${path.sep}`), 'receipt must be outside the immutable bundle');
  return resolved;
}

export function verifySourceIdentity(workspaceRoot, expectedCommit) {
  const head = execFileSync('git', ['-C', workspaceRoot, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  ensure(head === expectedCommit, `checkout commit mismatch: expected ${expectedCommit}, got ${head}`);
  const status = execFileSync(
    'git',
    ['-C', workspaceRoot, 'status', '--porcelain=v1', '--untracked-files=all'],
    { encoding: 'utf8' },
  ).trim();
  ensure(status.length === 0, 'evidence capture requires a clean worktree');
}

function seriesKey(...parts) {
  return JSON.stringify(parts);
}

function decodeSeriesKey(key) {
  return JSON.parse(key);
}

function parseLabels(raw, lineNumber) {
  if (raw === undefined) return {};
  if (raw === '') return {};
  const labels = {};
  let consumed = 0;
  LABEL_PATTERN.lastIndex = 0;
  for (const match of raw.matchAll(LABEL_PATTERN)) {
    ensure(match.index === consumed, `malformed label list at line ${lineNumber}`);
    const [, key, value] = match;
    ensure(!(key in labels), `duplicate label ${key} at line ${lineNumber}`);
    labels[key] = value;
    consumed = match.index + match[0].length;
  }
  ensure(consumed === raw.length, `escaped or malformed label value at line ${lineNumber}`);
  return labels;
}

function exactLabelValues(labels, expected, lineNumber) {
  const actual = Object.keys(labels).sort();
  ensure(isDeepStrictEqual(actual, [...expected].sort()), `labels at line ${lineNumber} must be exactly ${expected.join(',')}`);
}

function parseCounter(value, lineNumber) {
  const parsed = Number(value);
  ensure(Number.isSafeInteger(parsed) && parsed >= 0, `counter at line ${lineNumber} must be a non-negative safe integer`);
  return parsed;
}

function parseNonNegativeNumber(value, lineNumber) {
  const parsed = Number(value);
  ensure(Number.isFinite(parsed) && parsed >= 0, `metric at line ${lineNumber} must be finite and non-negative`);
  return parsed;
}

function insertUnique(map, key, value, lineNumber) {
  ensure(!map.has(key), `duplicate metric series at line ${lineNumber}`);
  map.set(key, value);
}

export function parseSnapshot(bytes) {
  const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  const snapshot = {
    collectorStarted: null,
    observations: new Map(),
    failures: new Map(),
    durationCounts: new Map(),
    durationSums: new Map(),
    durationBuckets: new Map(),
    lastObservations: new Map(),
  };

  text.split(/\r?\n/).forEach((rawLine, index) => {
    const lineNumber = index + 1;
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) return;
    const metricName = line.split(/[\s{]/, 1)[0];
    if (!metricName.startsWith('rustok_social_graph_index_privacy_shadow_')) return;

    const match = SAMPLE_PATTERN.exec(line);
    ensure(match, `malformed Prometheus sample at line ${lineNumber}`);
    const [, name, rawLabels, rawValue] = match;
    const labels = parseLabels(rawLabels, lineNumber);

    if (name === METRICS.collectorStarted) {
      exactLabelValues(labels, [], lineNumber);
      ensure(snapshot.collectorStarted === null, `duplicate collector epoch at line ${lineNumber}`);
      snapshot.collectorStarted = parseCounter(rawValue, lineNumber);
      return;
    }
    if (name === METRICS.observations) {
      exactLabelValues(labels, ['operation', 'outcome'], lineNumber);
      validateOperationOutcome(labels.operation, labels.outcome, lineNumber);
      insertUnique(snapshot.observations, seriesKey(labels.operation, labels.outcome), parseCounter(rawValue, lineNumber), lineNumber);
      return;
    }
    if (name === METRICS.failures) {
      exactLabelValues(labels, ['operation', 'error_code', 'retryable'], lineNumber);
      ensure(OPERATIONS.includes(labels.operation), `unknown operation at line ${lineNumber}`);
      ensure(ERROR_CODES.includes(labels.error_code), `unknown error code at line ${lineNumber}`);
      ensure(['true', 'false'].includes(labels.retryable), `retryable must be true/false at line ${lineNumber}`);
      insertUnique(
        snapshot.failures,
        seriesKey(labels.operation, labels.error_code, labels.retryable === 'true'),
        parseCounter(rawValue, lineNumber),
        lineNumber,
      );
      return;
    }
    if (name === `${METRICS.duration}_count`) {
      exactLabelValues(labels, ['operation', 'outcome'], lineNumber);
      validateOperationOutcome(labels.operation, labels.outcome, lineNumber);
      insertUnique(snapshot.durationCounts, seriesKey(labels.operation, labels.outcome), parseCounter(rawValue, lineNumber), lineNumber);
      return;
    }
    if (name === `${METRICS.duration}_sum`) {
      exactLabelValues(labels, ['operation', 'outcome'], lineNumber);
      validateOperationOutcome(labels.operation, labels.outcome, lineNumber);
      insertUnique(snapshot.durationSums, seriesKey(labels.operation, labels.outcome), parseNonNegativeNumber(rawValue, lineNumber), lineNumber);
      return;
    }
    if (name === `${METRICS.duration}_bucket`) {
      exactLabelValues(labels, ['operation', 'outcome', 'le'], lineNumber);
      validateOperationOutcome(labels.operation, labels.outcome, lineNumber);
      ensure(HISTOGRAM_BUCKETS.includes(labels.le), `unknown histogram bucket at line ${lineNumber}`);
      insertUnique(
        snapshot.durationBuckets,
        seriesKey(labels.operation, labels.outcome, labels.le),
        parseCounter(rawValue, lineNumber),
        lineNumber,
      );
      return;
    }
    if (name === METRICS.lastObservation) {
      exactLabelValues(labels, ['operation', 'outcome'], lineNumber);
      validateOperationOutcome(labels.operation, labels.outcome, lineNumber);
      insertUnique(snapshot.lastObservations, seriesKey(labels.operation, labels.outcome), parseCounter(rawValue, lineNumber), lineNumber);
      return;
    }
    fail(`unknown privacy-shadow metric at line ${lineNumber}: ${name}`);
  });

  validateSnapshot(snapshot);
  return snapshot;
}

function validateOperationOutcome(operation, outcome, lineNumber) {
  ensure(OPERATIONS.includes(operation), `unknown operation at line ${lineNumber}: ${operation}`);
  ensure(OUTCOMES.includes(outcome), `unknown outcome at line ${lineNumber}: ${outcome}`);
}

function validateSnapshot(snapshot) {
  ensure(Number.isSafeInteger(snapshot.collectorStarted) && snapshot.collectorStarted > 0, 'collector epoch metric is required');
  for (const [key, count] of snapshot.observations) {
    ensure(snapshot.durationCounts.get(key) === count, `observation/duration count mismatch for ${key}`);
    ensure(snapshot.durationSums.has(key), `duration sum missing for ${key}`);
    ensure((snapshot.lastObservations.get(key) || 0) > 0 || count === 0, `last observation missing for ${key}`);
    let previous = 0;
    const [operation, outcome] = decodeSeriesKey(key);
    for (const le of HISTOGRAM_BUCKETS) {
      const bucket = snapshot.durationBuckets.get(seriesKey(operation, outcome, le));
      ensure(bucket !== undefined, `histogram bucket ${le} missing for ${key}`);
      ensure(bucket >= previous, `histogram buckets are not cumulative for ${key}`);
      previous = bucket;
    }
    ensure(previous === count, `+Inf histogram bucket does not equal count for ${key}`);
  }
  for (const key of snapshot.durationCounts.keys()) ensure(snapshot.observations.has(key), `duration count without observation: ${key}`);
  for (const key of snapshot.durationSums.keys()) ensure(snapshot.observations.has(key), `duration sum without observation: ${key}`);
  for (const key of snapshot.lastObservations.keys()) ensure(snapshot.observations.has(key), `timestamp without observation: ${key}`);

  for (const operation of OPERATIONS) {
    let failures = 0;
    for (const [key, count] of snapshot.failures) {
      if (decodeSeriesKey(key)[0] === operation) failures += count;
    }
    const errors = snapshot.observations.get(seriesKey(operation, 'error')) || 0;
    ensure(failures === errors, `error observation/failure mismatch for ${operation}`);
  }
}

function deltaMap(start, end, label, floating = false) {
  const keys = [...new Set([...start.keys(), ...end.keys()])].sort();
  const result = new Map();
  for (const key of keys) {
    const before = start.get(key) || 0;
    const after = end.get(key) || 0;
    ensure(after + (floating ? Number.EPSILON : 0) >= before, `${label} reset detected for ${key}`);
    result.set(key, Math.max(0, after - before));
  }
  return result;
}

function p95UpperBound(count, buckets) {
  const rank = Math.ceil(count * 0.95);
  for (const bucket of buckets) {
    if (bucket.count >= rank) return bucket.le === '+Inf' ? null : Number(bucket.le);
  }
  fail('histogram does not contain the p95 rank');
}

export function analyzeWindow(start, end, window) {
  ensure(start.collectorStarted === end.collectorStarted, 'collector epoch changed between snapshots');
  const observationsDelta = deltaMap(start.observations, end.observations, 'observation counter');
  const failuresDelta = deltaMap(start.failures, end.failures, 'failure counter');
  const durationCountDelta = deltaMap(start.durationCounts, end.durationCounts, 'duration count');
  const durationSumDelta = deltaMap(start.durationSums, end.durationSums, 'duration sum', true);
  const durationBucketDelta = deltaMap(start.durationBuckets, end.durationBuckets, 'duration bucket');

  const observations = [];
  const durations = [];
  const lastObservations = [];
  const earliest = Math.floor(Date.parse(window.started_at) / 1000);
  const latest = Math.floor(Date.parse(window.ended_at) / 1000) + TIMESTAMP_SKEW_SECONDS;

  for (const [key, count] of observationsDelta) {
    if (count === 0) continue;
    const [operation, outcome] = decodeSeriesKey(key);
    ensure(durationCountDelta.get(key) === count, `window observation/duration count mismatch for ${key}`);
    const totalSeconds = durationSumDelta.get(key) || 0;
    const buckets = [];
    let previous = 0;
    for (const le of HISTOGRAM_BUCKETS) {
      const bucketCount = durationBucketDelta.get(seriesKey(operation, outcome, le)) || 0;
      ensure(bucketCount >= previous, `window histogram is not cumulative for ${key}`);
      previous = bucketCount;
      buckets.push({ le, count: bucketCount });
    }
    ensure(previous === count, `window +Inf bucket does not equal observation delta for ${key}`);
    observations.push({ operation, outcome, count });
    durations.push({
      operation,
      outcome,
      count,
      total_seconds: totalSeconds,
      average_seconds: totalSeconds / count,
      p95_upper_bound_seconds: p95UpperBound(count, buckets),
      buckets,
    });
    const timestamp = end.lastObservations.get(key);
    ensure(Number.isSafeInteger(timestamp), `end timestamp missing for ${key}`);
    ensure(timestamp >= earliest && timestamp <= latest, `last observation is outside the declared window for ${key}`);
    lastObservations.push({ operation, outcome, timestamp_seconds: timestamp });
  }

  for (const [key, count] of durationCountDelta) {
    ensure((observationsDelta.get(key) || 0) === count, `duration count exists without matching observation delta: ${key}`);
  }

  const failures = [];
  for (const [key, count] of failuresDelta) {
    if (count === 0) continue;
    const [operation, errorCode, retryable] = decodeSeriesKey(key);
    failures.push({ operation, error_code: errorCode, retryable, count });
  }
  for (const operation of OPERATIONS) {
    const failureCount = failures.filter((entry) => entry.operation === operation).reduce((sum, entry) => sum + entry.count, 0);
    const errorCount = observationsDelta.get(seriesKey(operation, 'error')) || 0;
    ensure(failureCount === errorCount, `window error observation/failure mismatch for ${operation}`);
  }

  observations.sort(compareObservation);
  failures.sort((a, b) => `${a.operation}\0${a.error_code}\0${a.retryable}`.localeCompare(`${b.operation}\0${b.error_code}\0${b.retryable}`));
  durations.sort(compareObservation);
  lastObservations.sort(compareObservation);

  const totals = totalsFrom(observations, failures);
  ensure(totals.observations > 0, 'evidence window contains no observations');
  return {
    collector_started_timestamp_seconds: start.collectorStarted,
    observations,
    failures,
    durations,
    last_observations: lastObservations,
    totals,
    restart_detected: false,
    counter_reset_detected: false,
  };
}

function compareObservation(a, b) {
  return `${a.operation}\0${a.outcome}`.localeCompare(`${b.operation}\0${b.outcome}`);
}

function totalsFrom(observations, failures) {
  const count = (outcome) => observations.filter((entry) => entry.outcome === outcome).reduce((sum, entry) => sum + entry.count, 0);
  return {
    observations: observations.reduce((sum, entry) => sum + entry.count, 0),
    failures: failures.reduce((sum, entry) => sum + entry.count, 0),
    false_negative: count('false_negative'),
    false_positive: count('false_positive'),
    batch_missing: count('batch_missing'),
    batch_extra: count('batch_extra'),
    batch_mixed: count('batch_mixed'),
    errors: count('error'),
  };
}

export function validateCaptureDescriptor(descriptor, expected, startBytes, endBytes) {
  assertExactKeys(descriptor, ['contract', 'completed_at', 'source', 'runner', 'window', 'start', 'end', 'metrics', 'authority'], 'capture descriptor');
  ensure(descriptor.contract === CAPTURE_CONTRACT, `unexpected capture contract: ${descriptor.contract}`);
  assertExactKeys(descriptor.source, ['repository', 'commit', 'run_key', 'clean_worktree'], 'source identity');
  assertExactKeys(descriptor.runner, ['job', 'runner_os', 'runner_arch'], 'capture runner');
  assertExactKeys(descriptor.window, ['started_at', 'ended_at', 'duration_seconds'], 'capture window');
  assertExactKeys(descriptor.start, ['path', 'bytes', 'sha256'], 'start artifact');
  assertExactKeys(descriptor.end, ['path', 'bytes', 'sha256'], 'end artifact');
  assertExactKeys(descriptor.authority, ['measurement_only', 'owner_result_authoritative', 'authoritative_cutover_authorized'], 'authority boundary');

  ensure(descriptor.source.repository === expected.repository, 'repository mismatch');
  ensure(descriptor.source.commit === expected.commit, 'commit mismatch');
  ensure(descriptor.source.run_key === expected.runKey, 'run key mismatch');
  ensure(descriptor.source.clean_worktree === true, 'capture was not bound to a clean worktree');
  validateCommit(descriptor.source.commit, 'captured commit');
  ensure(descriptor.runner.job && descriptor.runner.runner_os && descriptor.runner.runner_arch, 'capture runner identity is incomplete');
  const startedAt = parseUtc(descriptor.window.started_at, 'window.started_at');
  const endedAt = parseUtc(descriptor.window.ended_at, 'window.ended_at');
  ensure(descriptor.window.duration_seconds === validateWindow(startedAt, endedAt), 'window duration mismatch');
  validateArtifact(descriptor.start, START_FILE, startBytes);
  validateArtifact(descriptor.end, END_FILE, endBytes);
  ensure(
    isDeepStrictEqual(descriptor.authority, {
      measurement_only: true,
      owner_result_authoritative: true,
      authoritative_cutover_authorized: false,
    }),
    'authority boundary drifted',
  );

  const recomputed = analyzeWindow(parseSnapshot(startBytes), parseSnapshot(endBytes), descriptor.window);
  ensure(isDeepStrictEqual(recomputed, descriptor.metrics), 'descriptor metrics do not match retained snapshots');
  return recomputed;
}

function validateArtifact(descriptor, expectedPath, bytes) {
  ensure(descriptor.path === expectedPath, `artifact path mismatch: ${descriptor.path}`);
  ensure(descriptor.bytes === bytes.length, `artifact byte count mismatch: ${expectedPath}`);
  ensure(descriptor.sha256 === sha256(bytes), `artifact hash mismatch: ${expectedPath}`);
}

export function assessMetrics(metrics, policy) {
  const sampleCountSufficient = metrics.totals.observations >= policy.minimum_observations;
  const coverage = ['blocks_between', 'source_mutes_target'].every((operation) =>
    ['match_positive', 'match_negative'].every((outcome) =>
      metrics.observations.some((entry) => entry.operation === operation && entry.outcome === outcome && entry.count > 0),
    ),
  );
  const zeroNegativeSafetyMisses = metrics.totals.false_negative === 0
    && metrics.totals.batch_missing === 0
    && metrics.totals.batch_mixed === 0;
  const mismatchTotal = metrics.totals.false_negative + metrics.totals.false_positive
    + metrics.totals.batch_missing + metrics.totals.batch_extra + metrics.totals.batch_mixed;
  const zeroMismatches = mismatchTotal === 0;
  const errorRateBasisPoints = Math.ceil((metrics.totals.errors * 10_000) / metrics.totals.observations);
  const errorRateWithinLimit = errorRateBasisPoints <= policy.maximum_error_rate_basis_points;
  const latencyWithinLimit = metrics.durations.every((entry) =>
    entry.p95_upper_bound_seconds !== null && entry.p95_upper_bound_seconds <= policy.maximum_p95_seconds,
  );
  const policyPassed = sampleCountSufficient
    && (!policy.require_notification_positive_and_negative_coverage || coverage)
    && zeroNegativeSafetyMisses
    && (!policy.require_zero_mismatches || zeroMismatches)
    && errorRateWithinLimit
    && latencyWithinLimit;
  return {
    sample_count_sufficient: sampleCountSufficient,
    notification_positive_and_negative_coverage: coverage,
    zero_negative_safety_misses: zeroNegativeSafetyMisses,
    zero_mismatches: zeroMismatches,
    error_rate_basis_points: errorRateBasisPoints,
    error_rate_within_limit: errorRateWithinLimit,
    latency_within_limit: latencyWithinLimit,
    policy_passed: policyPassed,
  };
}

export function assertExactKeys(value, keys, label) {
  ensure(value && typeof value === 'object' && !Array.isArray(value), `${label} must be an object`);
  const actual = Object.keys(value).sort();
  ensure(isDeepStrictEqual(actual, [...keys].sort()), `${label} fields mismatch: ${actual.join(',')}`);
}
