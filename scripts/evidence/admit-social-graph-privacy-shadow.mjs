#!/usr/bin/env node

import path from 'node:path';

import {
  ADMISSION_CONTRACT,
  DESCRIPTOR_FILE,
  END_FILE,
  INVENTORY,
  MAX_DESCRIPTOR_BYTES,
  MAX_SNAPSHOT_BYTES,
  START_FILE,
  TIMESTAMP_SKEW_SECONDS,
  absolutePath,
  artifactDescriptor,
  assessMetrics,
  ensure,
  ensureAbsent,
  ensureInventory,
  parseUtc,
  readStableRegularFile,
  resolveReceiptPath,
  runnerIdentity,
  validateBundleRoot,
  validateCaptureDescriptor,
  validateCommit,
  validateRepository,
  validateRunKey,
  writeJsonNew,
} from './lib/social-graph-privacy-shadow-evidence.mjs';

const ADMISSION_OPT_IN = 'SOCIAL_GRAPH_PRIVACY_SHADOW_ALLOW_ADMISSION';
const BUNDLE_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_BUNDLE';
const OUTPUT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_ADMISSION_OUTPUT';
const EXPECTED_COMMIT_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_EXPECTED_COMMIT';
const EXPECTED_RUN_KEY_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_EXPECTED_RUN_KEY';
const EXPECTED_REPOSITORY_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_EXPECTED_REPOSITORY';
const MIN_OBSERVATIONS_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_MIN_OBSERVATIONS';
const MAX_ERROR_RATE_BPS_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_MAX_ERROR_RATE_BPS';
const MAX_P95_SECONDS_ENV = 'SOCIAL_GRAPH_PRIVACY_SHADOW_MAX_P95_SECONDS';

function required(name) {
  const value = process.env[name];
  ensure(value !== undefined && value.trim() !== '', `${name} is required`);
  return value.trim();
}

function requiredInteger(name) {
  const value = Number(required(name));
  ensure(Number.isSafeInteger(value) && value >= 0, `${name} must be a non-negative safe integer`);
  return value;
}

function requiredPositiveNumber(name) {
  const value = Number(required(name));
  ensure(Number.isFinite(value) && value > 0, `${name} must be a positive finite number`);
  return value;
}

function validateRunner(runner, label) {
  ensure(runner && typeof runner === 'object' && !Array.isArray(runner), `${label} must be an object`);
  for (const key of ['job', 'runner_os', 'runner_arch']) {
    const value = runner[key];
    ensure(typeof value === 'string' && value.length >= 1 && value.length <= 128, `${label}.${key} must contain 1-128 characters`);
    ensure(/^[\x20-\x7E]+$/.test(value), `${label}.${key} must contain printable ASCII only`);
  }
}

function loadConfig() {
  ensure(process.env[ADMISSION_OPT_IN] === '1', `${ADMISSION_OPT_IN}=1 is required`);
  const bundleRoot = validateBundleRoot(absolutePath(required(BUNDLE_ENV)));
  const expectedRepository = process.env[EXPECTED_REPOSITORY_ENV]?.trim() || 'RusTokRs/RusTok';
  validateRepository(expectedRepository, EXPECTED_REPOSITORY_ENV);
  const expectedCommit = required(EXPECTED_COMMIT_ENV);
  validateCommit(expectedCommit, EXPECTED_COMMIT_ENV);
  const expectedRunKey = required(EXPECTED_RUN_KEY_ENV);
  validateRunKey(expectedRunKey, EXPECTED_RUN_KEY_ENV);

  const minimumObservations = requiredInteger(MIN_OBSERVATIONS_ENV);
  ensure(minimumObservations > 0, `${MIN_OBSERVATIONS_ENV} must be greater than zero`);
  const maximumErrorRateBasisPoints = requiredInteger(MAX_ERROR_RATE_BPS_ENV);
  ensure(maximumErrorRateBasisPoints <= 10_000, `${MAX_ERROR_RATE_BPS_ENV} must be between 0 and 10000`);
  const maximumP95Seconds = requiredPositiveNumber(MAX_P95_SECONDS_ENV);

  const requestedOutput = absolutePath(
    process.env[OUTPUT_ENV]?.trim()
      || path.join(path.dirname(bundleRoot), `${expectedRunKey}-admission.json`),
  );
  const outputPath = resolveReceiptPath(requestedOutput, bundleRoot);

  return {
    bundleRoot,
    outputPath,
    expected: {
      repository: expectedRepository,
      commit: expectedCommit,
      runKey: expectedRunKey,
    },
    policy: {
      minimum_observations: minimumObservations,
      maximum_error_rate_basis_points: maximumErrorRateBasisPoints,
      maximum_p95_seconds: maximumP95Seconds,
      require_notification_positive_and_negative_coverage: true,
      require_zero_mismatches: true,
    },
  };
}

function main() {
  const config = loadConfig();
  ensureAbsent(config.outputPath, 'privacy-shadow admission output');
  const inventoryBefore = ensureInventory(config.bundleRoot, INVENTORY);
  const descriptorBytes = readStableRegularFile(
    path.join(config.bundleRoot, DESCRIPTOR_FILE),
    MAX_DESCRIPTOR_BYTES,
    'privacy-shadow capture descriptor',
  );
  const startBytes = readStableRegularFile(
    path.join(config.bundleRoot, START_FILE),
    MAX_SNAPSHOT_BYTES,
    'privacy-shadow start snapshot',
  );
  const endBytes = readStableRegularFile(
    path.join(config.bundleRoot, END_FILE),
    MAX_SNAPSHOT_BYTES,
    'privacy-shadow end snapshot',
  );
  let descriptor;
  try {
    descriptor = JSON.parse(descriptorBytes.toString('utf8'));
  } catch (error) {
    throw new Error(`failed to decode privacy-shadow capture descriptor: ${error instanceof Error ? error.message : String(error)}`);
  }

  const completedAt = parseUtc(descriptor.completed_at, 'completed_at');
  const endedAt = parseUtc(descriptor.window?.ended_at, 'window.ended_at');
  ensure(completedAt.getTime() >= endedAt.getTime(), 'capture completed_at precedes window end');
  validateRunner(descriptor.runner, 'capture_runner');
  const metrics = validateCaptureDescriptor(
    descriptor,
    config.expected,
    startBytes,
    endBytes,
  );
  const startedAtSeconds = Math.floor(parseUtc(descriptor.window.started_at, 'window.started_at').getTime() / 1000);
  ensure(
    metrics.collector_started_timestamp_seconds <= startedAtSeconds + TIMESTAMP_SKEW_SECONDS,
    'collector epoch is later than the declared evidence-window start',
  );
  const assessment = assessMetrics(metrics, config.policy);

  const inventoryAfter = ensureInventory(config.bundleRoot, INVENTORY);
  ensure(JSON.stringify(inventoryAfter) === JSON.stringify(inventoryBefore), 'bundle inventory changed during review');
  ensure(
    readStableRegularFile(path.join(config.bundleRoot, DESCRIPTOR_FILE), MAX_DESCRIPTOR_BYTES, 'descriptor reread').equals(descriptorBytes),
    'capture descriptor changed during admission review',
  );
  ensure(
    readStableRegularFile(path.join(config.bundleRoot, START_FILE), MAX_SNAPSHOT_BYTES, 'start snapshot reread').equals(startBytes),
    'start snapshot changed during admission review',
  );
  ensure(
    readStableRegularFile(path.join(config.bundleRoot, END_FILE), MAX_SNAPSHOT_BYTES, 'end snapshot reread').equals(endBytes),
    'end snapshot changed during admission review',
  );

  const reviewer = runnerIdentity(
    ['SOCIAL_GRAPH_PRIVACY_SHADOW_ADMISSION_JOB', 'GITHUB_JOB'],
    'social-graph-privacy-shadow-admission',
  );
  validateRunner(reviewer, 'reviewer');
  const receipt = {
    contract: ADMISSION_CONTRACT,
    reviewed_at: new Date().toISOString(),
    admitted: true,
    policy_passed: assessment.policy_passed,
    authoritative_cutover_authorized: false,
    source: descriptor.source,
    capture_runner: descriptor.runner,
    reviewer,
    window: descriptor.window,
    policy: config.policy,
    assessment,
    metrics,
    bundle: {
      inventory: INVENTORY,
      descriptor: artifactDescriptor(DESCRIPTOR_FILE, descriptorBytes),
      start: artifactDescriptor(START_FILE, startBytes),
      end: artifactDescriptor(END_FILE, endBytes),
    },
  };
  writeJsonNew(config.outputPath, receipt, 'privacy-shadow admission receipt');
  console.log(
    `privacy shadow evidence admitted: commit=${descriptor.source.commit} run_key=${descriptor.source.run_key} policy_passed=${assessment.policy_passed} output=${config.outputPath}`,
  );
}

try {
  main();
} catch (error) {
  console.error(`[admit-social-graph-privacy-shadow] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
