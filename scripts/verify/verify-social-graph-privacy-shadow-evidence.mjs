#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-social-graph-privacy-shadow-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};
const requireOrder = (relative, source, markers) => {
  let previous = -1;
  for (const marker of markers) {
    const current = source.indexOf(marker, previous + 1);
    if (current < 0 || current <= previous) fail(`${relative} is missing or reorders ${marker}`);
    previous = current;
  }
};

const metricsPath = 'crates/rustok-telemetry/src/social_graph_index_privacy_shadow_metrics.rs';
const metrics = requireMarkers(metricsPath, [
  'collector_started_timestamp_seconds: IntGauge',
  'rustok_social_graph_index_privacy_shadow_collector_started_timestamp_seconds',
  'collector_started_timestamp_seconds.set(unix_timestamp_seconds())',
  'descriptions.extend(self.collector_started_timestamp_seconds.desc())',
  'families.extend(self.collector_started_timestamp_seconds.collect())',
  'rustok_social_graph_index_privacy_shadow_observations_total',
  'rustok_social_graph_index_privacy_shadow_failures_total',
  'rustok_social_graph_index_privacy_shadow_comparison_duration_seconds',
  'rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds',
]);
forbidMarkers(metricsPath, metrics, [
  'tenant_id',
  'source_user_id',
  'target_user_id',
  'relation_id',
  'entity_id',
  'payload',
  'storage_error',
]);

const libraryPath = 'scripts/evidence/lib/social-graph-privacy-shadow-evidence.mjs';
const library = requireMarkers(libraryPath, [
  "CAPTURE_CONTRACT = 'social_graph_index_privacy_shadow_window_capture_v1'",
  "ADMISSION_CONTRACT = 'social_graph_index_privacy_shadow_window_admission_v1'",
  "START_FILE = 'start.prom'",
  "END_FILE = 'end.prom'",
  "DESCRIPTOR_FILE = 'capture.json'",
  'collector_started_timestamp_seconds',
  'unknown privacy-shadow metric',
  'collector epoch changed between snapshots',
  'reset detected',
  'window +Inf bucket does not equal observation delta',
  'window error observation/failure mismatch',
  'evidence window contains no observations',
  'p95_upper_bound_seconds',
  'descriptor metrics do not match retained snapshots',
  'zero_negative_safety_misses',
  'require_zero_mismatches',
  'policy_passed',
]);
forbidMarkers(libraryPath, library, [
  'fetch(',
  'http.request',
  'https.request',
  'curl',
  'reqwest',
  'DatabaseConnection',
  'postgres://',
  'cargo test',
  'cargo run',
]);
if ((library.match(/execFileSync\(/g) ?? []).length !== 2) {
  fail(`${libraryPath} may invoke subprocesses only for the two Git identity reads`);
}
requireMarkers(libraryPath, [
  "execFileSync('git', ['-C', workspaceRoot, 'rev-parse', 'HEAD']",
  "['-C', workspaceRoot, 'status', '--porcelain=v1', '--untracked-files=all']",
]);

const canonicalPath = 'scripts/evidence/lib/social-graph-privacy-shadow-canonical.mjs';
const canonical = requireMarkers(canonicalPath, [
  'export function canonicalizeSnapshot(snapshot)',
  'canonical metric value is missing',
  'METRICS.collectorStarted',
  'METRICS.observations',
  'METRICS.failures',
  '`${METRICS.duration}_bucket`',
  '`${METRICS.duration}_sum`',
  '`${METRICS.duration}_count`',
  'METRICS.lastObservation',
]);
forbidMarkers(canonicalPath, canonical, [
  'http_requests_total',
  'tenant_id',
  'source_user_id',
  'target_user_id',
  'payload',
]);

const capturePath = 'scripts/evidence/capture-social-graph-privacy-shadow.mjs';
const capture = requireMarkers(capturePath, [
  'SOCIAL_GRAPH_PRIVACY_SHADOW_ALLOW_CAPTURE',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_START_PROM',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_END_PROM',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_WINDOW_STARTED_AT',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_WINDOW_ENDED_AT',
  'verifySourceIdentity(config.workspaceRoot, config.commit)',
  'const startSnapshot = parseSnapshot(rawStart)',
  'const endSnapshot = parseSnapshot(rawEnd)',
  'const startBytes = canonicalizeSnapshot(startSnapshot)',
  'const endBytes = canonicalizeSnapshot(endSnapshot)',
  'collector epoch is later than the declared evidence-window start',
  'writeNewFile(path.join(finalRoot, START_FILE)',
  'writeNewFile(path.join(finalRoot, END_FILE)',
  'writeJsonNew(path.join(finalRoot, DESCRIPTOR_FILE)',
  'authoritative_cutover_authorized: false',
]);
requireOrder(capturePath, capture, [
  'verifySourceIdentity(config.workspaceRoot, config.commit)',
  'const rawStart = readStableRegularFile',
  'const rawEnd = readStableRegularFile',
  'verifySourceIdentity(config.workspaceRoot, config.commit)',
  'const startSnapshot = parseSnapshot(rawStart)',
  'const endSnapshot = parseSnapshot(rawEnd)',
  'const startBytes = canonicalizeSnapshot(startSnapshot)',
  'const endBytes = canonicalizeSnapshot(endSnapshot)',
  'const metrics = analyzeWindow',
  'writeNewFile(path.join(finalRoot, START_FILE)',
  'writeNewFile(path.join(finalRoot, END_FILE)',
  'writeJsonNew(path.join(finalRoot, DESCRIPTOR_FILE)',
]);
forbidMarkers(capturePath, capture, [
  'canonicalRegularFile',
  'fetch(',
  'http://',
  'https://',
  'curl',
  'spawn',
  'execFile',
  'DatabaseConnection',
  'postgres://',
  'cargo',
  'tenant_id',
  'source_user_id',
  'target_user_id',
]);

const admissionPath = 'scripts/evidence/admit-social-graph-privacy-shadow.mjs';
const admission = requireMarkers(admissionPath, [
  'SOCIAL_GRAPH_PRIVACY_SHADOW_ALLOW_ADMISSION',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_EXPECTED_COMMIT',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_EXPECTED_RUN_KEY',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_MIN_OBSERVATIONS',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_MAX_ERROR_RATE_BPS',
  'SOCIAL_GRAPH_PRIVACY_SHADOW_MAX_P95_SECONDS',
  'const parsedStart = parseSnapshot(startBytes)',
  'const parsedEnd = parseSnapshot(endBytes)',
  'canonicalizeSnapshot(parsedStart).equals(startBytes)',
  'canonicalizeSnapshot(parsedEnd).equals(endBytes)',
  'retained start snapshot is not the canonical shadow-only export',
  'retained end snapshot is not the canonical shadow-only export',
  'validateCaptureDescriptor(',
  'const assessment = assessMetrics(metrics, config.policy)',
  'capture completed_at precedes window end',
  'capture completed_at is implausibly in the future',
  'collector epoch is later than the declared evidence-window start',
  'admitted: true',
  'policy_passed: assessment.policy_passed',
  'authoritative_cutover_authorized: false',
  'writeJsonNew(config.outputPath, receipt',
]);
requireOrder(admissionPath, admission, [
  'const inventoryBefore = ensureInventory',
  'const descriptorBytes = readStableRegularFile',
  'const startBytes = readStableRegularFile',
  'const endBytes = readStableRegularFile',
  'const parsedStart = parseSnapshot(startBytes)',
  'const parsedEnd = parseSnapshot(endBytes)',
  'canonicalizeSnapshot(parsedStart).equals(startBytes)',
  'canonicalizeSnapshot(parsedEnd).equals(endBytes)',
  'const metrics = validateCaptureDescriptor',
  'const assessment = assessMetrics',
  'const inventoryAfter = ensureInventory',
  'writeJsonNew(config.outputPath, receipt',
]);
forbidMarkers(admissionPath, admission, [
  'fetch(',
  'http://',
  'https://',
  'curl',
  'spawn',
  'execFile',
  'DatabaseConnection',
  'postgres://',
  'cargo',
  'tenant_id',
  'source_user_id',
  'target_user_id',
]);

const testPath = 'scripts/evidence/social-graph-privacy-shadow-evidence.test.mjs';
requireMarkers(testPath, [
  'canonical retained snapshot excludes unrelated process metrics',
  'collector epoch change rejects a restarted observation window',
  'counter decrease rejects a reset inside one collector epoch',
  'unknown labels under the shadow prefix fail closed',
  'false negative evidence is reviewable but cannot pass policy',
]);

const contractPath = 'crates/rustok-social-graph/contracts/social-graph-index-privacy-shadow-evidence.json';
const contract = JSON.parse(read(contractPath));
if (contract.schema_version !== 1) fail(`${contractPath} must use schema_version 1`);
if (contract.status !== 'source_complete_owner_execution_pending') {
  fail(`${contractPath} must remain owner-execution pending`);
}
if (contract.authority?.authoritative_cutover_authorized !== false) {
  fail(`${contractPath} must not authorize authoritative cutover`);
}
if (contract.bundle?.descriptor_last !== true || contract.bundle?.full_process_scrape_is_not_retained !== true) {
  fail(`${contractPath} must retain descriptor-last canonical shadow-only bundles`);
}
if (JSON.stringify(contract.bundle?.inventory) !== JSON.stringify(['capture.json', 'end.prom', 'start.prom'])) {
  fail(`${contractPath} bundle inventory drifted`);
}
if (contract.capture?.offline_only !== true
  || contract.capture?.executes_runtime !== false
  || contract.capture?.executes_cargo !== false
  || contract.capture?.connects_database !== false
  || contract.capture?.performs_http_scrape !== false) {
  fail(`${contractPath} capture must remain offline and non-executing`);
}
if (contract.admission?.recomputes_metrics_from_retained_snapshots !== true
  || contract.admission?.authoritative_cutover_authorized !== false) {
  fail(`${contractPath} admission must recompute evidence without authorizing cutover`);
}
if (contract.metrics?.collector_epoch !== 'rustok_social_graph_index_privacy_shadow_collector_started_timestamp_seconds') {
  fail(`${contractPath} must bind the restart-detection metric`);
}
for (const label of ['tenant_id', 'source_user_id', 'target_user_id', 'relation_id', 'entity_id']) {
  if (!contract.metrics?.identity_labels_forbidden?.includes(label)) {
    fail(`${contractPath} must forbid identity label ${label}`);
  }
}

const notificationContractPath = 'crates/rustok-social-graph/contracts/social-graph-notification-policy.json';
const notificationContract = JSON.parse(read(notificationContractPath));
if (notificationContract.index_privacy_shadow_evidence !== contractPath) {
  fail(`${notificationContractPath} must bind the privacy-shadow evidence contract`);
}
if (notificationContract.telemetry?.collector_started_metric !== contract.metrics.collector_epoch) {
  fail(`${notificationContractPath} must bind the collector epoch metric`);
}
if (notificationContract.verification?.evidence_verifier !== 'scripts/verify/verify-social-graph-privacy-shadow-evidence.mjs') {
  fail(`${notificationContractPath} must register the evidence verifier`);
}

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-social-graph-privacy-shadow-evidence.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-social-graph-privacy-consumer.md', [
  'Status: `source_complete_metrics_evidence_tooling_execution_pending`',
  '`start.prom`',
  '`end.prom`',
  '`capture.json`',
  '`social_graph_index_privacy_shadow_window_capture_v1`',
  '`social_graph_index_privacy_shadow_window_admission_v1`',
  '`policy_passed`',
  '`authoritative_cutover_authorized: false`',
  'Authoritative cutover remains blocked',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-telemetry/CRATE_API.md', [
  '`rustok_social_graph_index_privacy_shadow_collector_started_timestamp_seconds`',
  'restart detection',
]);

console.log('[verify-social-graph-privacy-shadow-evidence] OK');
