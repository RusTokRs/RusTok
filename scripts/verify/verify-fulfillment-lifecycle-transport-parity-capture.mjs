#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const contractPath =
  'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-transport-parity-execution-contract.json';
const runnerPath = 'scripts/evidence/capture-fulfillment-lifecycle-transport-parity.mjs';
const verifierPath =
  'scripts/verify/verify-fulfillment-lifecycle-transport-parity-capture.mjs';
const evidencePath =
  'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-transport-parity-execution.json';
const sourceEvidencePath =
  'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json';
const runbookPath =
  'crates/rustok-fulfillment/docs/fulfillment-lifecycle-transport-parity-capture.md';
const planPath = 'crates/rustok-fulfillment/docs/implementation-plan.md';

const contract = JSON.parse(read(contractPath));
const runner = read(runnerPath);
const sourceEvidence = JSON.parse(read(sourceEvidencePath));
const runbook = read(runbookPath);
const plan = read(planPath);
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const sameRecord = (left, right) => JSON.stringify(left) === JSON.stringify(right);

const expectedSourceFiles = [
  'apps/server/src/controllers/graphql.rs',
  'crates/rustok-commerce/src/graphql/query.rs',
  'crates/rustok-commerce/src/graphql/safe_query.rs',
  'crates/rustok-commerce/src/graphql_runtime.rs',
  'crates/rustok-commerce/src/controllers/admin/fulfillments.rs',
  'crates/rustok-fulfillment/src/fulfillment_read.rs',
  'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json',
];
const expectedRequiredEnvironment = [
  'RUSTOK_FULFILLMENT_PARITY_GRAPHQL_URL',
  'RUSTOK_FULFILLMENT_PARITY_REST_BASE_URL',
  'RUSTOK_FULFILLMENT_PARITY_TENANT_ID',
  'RUSTOK_FULFILLMENT_PARITY_AUTH_TOKEN',
  'RUSTOK_FULFILLMENT_PARITY_DETAIL_ID',
  'RUSTOK_FULFILLMENT_PARITY_ORDER_ID',
  'RUSTOK_FULFILLMENT_PARITY_STATUS',
  'RUSTOK_FULFILLMENT_PARITY_LATEST_ID',
  'RUSTOK_FULFILLMENT_PARITY_MISSING_ID',
  'RUSTOK_FULFILLMENT_PARITY_SOURCE_REVISION',
  'RUSTOK_FULFILLMENT_PARITY_ADAPTER_PROFILE',
];
const expectedScenarios = [
  'lookup_rest_detail_projection_parity',
  'filtered_list_projection_parity',
  'latest_by_order_projection_parity',
  'optional_not_found_transport_policy',
];

if (
  contract.schema_version !== 1 ||
  contract.module !== 'fulfillment' ||
  contract.packet !== 'fulfillment-lifecycle-transport-parity-execution-contract' ||
  contract.status !== 'runtime_execution_contract_locked'
) {
  failures.push('execution contract identity mismatch');
}
if (
  contract.runner !== runnerPath ||
  contract.verifier !== verifierPath ||
  contract.evidence_path !== evidencePath ||
  contract.evidence_status !== 'runtime_execution_pending'
) {
  failures.push('execution contract tooling or output boundary mismatch');
}
if (!sameRecord(contract.source_files, expectedSourceFiles)) {
  failures.push('execution contract source allowlist mismatch');
}
if (!sameRecord(contract.required_environment, expectedRequiredEnvironment)) {
  failures.push('execution contract required environment allowlist mismatch');
}
if (!sameRecord(contract.scenarios?.map((scenario) => scenario.id), expectedScenarios)) {
  failures.push('execution contract scenario allowlist mismatch');
}
if (
  contract.request_policy?.graphql_method !== 'POST' ||
  contract.request_policy?.rest_method !== 'GET' ||
  contract.request_policy?.graphql_mounted_path !== '/api/graphql' ||
  contract.request_policy?.rest_list_path !== '/admin/fulfillments' ||
  contract.request_policy?.maximum_response_bytes !== 1048576 ||
  contract.request_policy?.allow_http_for_local_capture !== true
) {
  failures.push('execution contract request policy mismatch');
}
for (const [value, label] of [
  [contract.retained_boundary?.bearer_token_retained, 'bearer token retention'],
  [contract.retained_boundary?.raw_response_bodies_retained, 'raw response retention'],
  [contract.retained_boundary?.fulfillment_metadata_retained, 'metadata retention'],
]) {
  if (value !== false) failures.push(`execution contract must forbid ${label}`);
}
for (const [value, label] of [
  [contract.retained_boundary?.normalized_projection_hashes_retained, 'normalized projection hashes'],
  [contract.retained_boundary?.source_hashes_retained, 'source hashes'],
  [
    contract.retained_boundary?.transport_projection_parity_requires_successful_capture,
    'successful projection-parity capture',
  ],
  [
    contract.retained_boundary?.restart_deadline_failure_and_remote_adapter_evidence_separate,
    'separate wider runtime evidence',
  ],
]) {
  if (value !== true) failures.push(`execution contract must require ${label}`);
}
if ('runtime_parity_requires_successful_capture' in contract.retained_boundary) {
  failures.push('execution contract must not alias bounded projection parity as runtime parity');
}

for (const [value, label] of [
  ['const expectedSourceFiles = [', 'runner source allowlist'],
  ['function repositoryPath(relativePath)', 'repository path boundary'],
  ['repository path escapes capture root', 'source path traversal rejection'],
  ['const maximumResponseBytes = contract.request_policy.maximum_response_bytes;', 'bounded responses'],
  ['function tenantHeaderName(value, field)', 'tenant header boundary'],
  ['must not override a reserved HTTP header', 'reserved header rejection'],
  ['function isLocalCaptureHost(hostname)', 'local HTTP host boundary'],
  ["['localhost', '127.0.0.1', '[::1]', '::1']", 'local HTTP host allowlist'],
  ['function endpoint(value, field)', 'URL validation'],
  ['parsed.username || parsed.password || parsed.search || parsed.hash', 'URL credential/query rejection'],
  ["parsed.protocol === 'http:' && !isLocalCaptureHost(parsed.hostname)", 'remote HTTPS requirement'],
  ['must use https unless the mounted endpoint is localhost or loopback', 'remote HTTP rejection'],
  ["redirect: 'error'", 'redirect rejection'],
  ['function authorizationHeader(value)', 'authorization construction'],
  ['function optionalString(value, field)', 'optional string normalization'],
  ['function timestamp(value, field)', 'timestamp normalization'],
  ['must be an RFC3339 timestamp', 'timestamp format boundary'],
  ['new Date(milliseconds).toISOString()', 'UTC timestamp canonicalization'],
  ['function normalizeProjection(value, flavor, field)', 'projection normalization'],
  ['function normalizeItems(items, flavor, field)', 'item normalization'],
  ['.sort((left, right) => left.id.localeCompare(right.id))', 'stable item ordering'],
  ['function projectionHash(value)', 'projection hashing'],
  ['function sourceHashes()', 'source hashing'],
  ['parity evidence already exists; remove it explicitly before a new capture', 'immutable output'],
  ['writeFileSync(temporaryPath', 'atomic temporary write'],
  ['renameSync(temporaryPath, outputPath)', 'atomic publish'],
  ['lookup: fulfillment(tenantId: $tenantId, id: $id)', 'GraphQL lookup'],
  ['list: fulfillments(tenantId: $tenantId, filter: $filter)', 'GraphQL filtered list'],
  ['order(tenantId: $tenantId, id: $id)', 'GraphQL latest by order'],
  ["restUrl(restBaseUrl, '/admin/fulfillments')", 'REST list'],
  ['`/admin/fulfillments/${detailId}`', 'REST detail'],
  ["restMissing.status !== 404 || restMissingCode !== 'commerce_admin_not_found'", 'REST optional not-found'],
  ['GraphQL missing fulfillment lookup must return null', 'GraphQL optional not-found'],
  ['lookup/detail projection parity', 'detail projection equality'],
  ['filtered list projection parity', 'list projection equality'],
  ['latest-by-order projection parity', 'latest projection equality'],
  ["status: 'transport_projection_parity_captured_unreviewed'", 'bounded packet status'],
  ['claimed_source_revision: sourceRevision', 'claimed source revision'],
  ['claimed_adapter_profile: adapterProfile', 'claimed adapter profile'],
  ['claims_verified_by_runner: false', 'unverified runtime claims'],
  ['bearer_token_retained: false', 'packet token boundary'],
  ['raw_response_bodies_retained: false', 'packet response boundary'],
  ['fulfillment_metadata_retained: false', 'packet metadata boundary'],
  ['transport_projection_parity_proven: true', 'bounded projection parity result'],
  ['runtime_parity_proven: false', 'wider runtime parity remains open'],
  ['owner_deadline_failure_injection_proven: false', 'deadline/failure remains open'],
  ['process_restart_proven: false', 'restart remains open'],
  ['external_adapter_identity_proven: false', 'external identity remains open'],
  ['remote_adapter_behavior_proven: false', 'remote adapter remains open'],
]) {
  requireText(runner, value, label);
}
for (const value of [
  'auth_token:',
  'raw_response_body',
  'metadata: source.metadata',
  'runtime_parity_proven: true',
  "external_adapter_identity_proven: adapterProfile !== 'in_process'",
]) {
  forbidText(runner, value, 'capture runner must not over-retain or overclaim');
}

if (sourceEvidence.status !== 'source_cutover_unvalidated') {
  failures.push('source evidence status must remain source_cutover_unvalidated');
}
if (sourceEvidence.validation?.runtime_parity_proven !== false) {
  failures.push('source evidence runtime parity must remain false');
}
if (sourceEvidence.runtime_capture?.contract !== contractPath) {
  failures.push('source evidence must reference the execution contract');
}
if (sourceEvidence.runtime_capture?.runner !== runnerPath) {
  failures.push('source evidence must reference the capture runner');
}
if (sourceEvidence.runtime_capture?.verifier !== verifierPath) {
  failures.push('source evidence must reference the capture verifier');
}
if (sourceEvidence.runtime_capture?.contract_published !== true) {
  failures.push('source evidence must record the published capture contract');
}
for (const [value, label] of [
  [sourceEvidence.runtime_capture?.capture_executed, 'capture execution'],
  [sourceEvidence.runtime_capture?.transport_projection_parity_proven, 'projection parity'],
  [sourceEvidence.runtime_capture?.deadline_failure_proven, 'deadline/failure evidence'],
  [sourceEvidence.runtime_capture?.restart_proven, 'restart evidence'],
  [sourceEvidence.runtime_capture?.remote_adapter_proven, 'remote adapter evidence'],
]) {
  if (value !== false) failures.push(`source evidence must retain ${label} as false`);
}

for (const [value, label] of [
  ['Status: capture contract published, execution pending.', 'runbook status'],
  [contractPath, 'runbook contract path'],
  [runnerPath, 'runbook runner path'],
  [verifierPath, 'runbook verifier path'],
  ['RUSTOK_FULFILLMENT_PARITY_GRAPHQL_URL', 'runbook GraphQL input'],
  ['RUSTOK_FULFILLMENT_PARITY_REST_BASE_URL', 'runbook REST input'],
  ['Remote mounted endpoints must use HTTPS.', 'runbook transport security'],
  ['timestamps are normalized to UTC millisecond form', 'runbook timestamp normalization'],
  ['transport_projection_parity_proven', 'runbook bounded result'],
  ['runtime_parity_proven` remains `false`', 'runbook non-promotion rule'],
  ['does not retain the bearer token', 'runbook secret boundary'],
]) {
  requireText(runbook, value, label);
}
for (const [value, label] of [
  ['Publish the mounted lifecycle projection-parity execution contract and capture runner.', 'plan contract checklist'],
  ['Execute the mounted GraphQL/REST projection-parity capture and retain its immutable packet.', 'plan execution checklist'],
  ['Prove deadline/failure injection, process restart, and remote-adapter behavior separately.', 'plan wider runtime checklist'],
]) {
  requireText(plan, value, label);
}

if (failures.length > 0) {
  console.error('Fulfillment lifecycle transport parity capture verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment lifecycle mounted projection-parity capture is contract-locked, fail-closed, secret-safe, and does not claim wider runtime parity',
);
