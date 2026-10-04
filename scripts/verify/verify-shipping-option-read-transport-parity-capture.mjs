#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const contractPath =
  'crates/modules/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-execution-contract.json';
const runnerPath = 'scripts/evidence/capture-shipping-option-read-transport-parity.mjs';
const verifierPath =
  'scripts/verify/verify-shipping-option-read-transport-parity-capture.mjs';
const evidencePath =
  'crates/modules/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-execution.json';
const sourceEvidencePath =
  'crates/modules/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-source.json';
const runbookPath =
  'crates/modules/rustok-fulfillment/docs/shipping-option-read-transport-parity-capture.md';
const planPath = 'crates/modules/rustok-fulfillment/docs/implementation-plan.md';

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
  'crates/modules/rustok-commerce/src/graphql/query.rs',
  'crates/modules/rustok-commerce/src/graphql/safe_query.rs',
  'crates/modules/rustok-commerce/src/graphql_runtime.rs',
  'crates/modules/rustok-commerce/src/controllers/store/products.rs',
  'crates/modules/rustok-commerce/src/controllers/admin/shipping.rs',
  'crates/modules/rustok-fulfillment/src/shipping_option_read.rs',
  'crates/modules/rustok-commerce/src/graphql/safe_query/query_error_boundary.rs',
  'crates/modules/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-source.json',
];

const expectedRequiredEnvironment = [
  'RUSTOK_SHIPPING_PARITY_GRAPHQL_URL',
  'RUSTOK_SHIPPING_PARITY_REST_BASE_URL',
  'RUSTOK_SHIPPING_PARITY_TENANT_ID',
  'RUSTOK_SHIPPING_PARITY_AUTH_TOKEN',
  'RUSTOK_SHIPPING_PARITY_DETAIL_ID',
  'RUSTOK_SHIPPING_PARITY_MISSING_ID',
];

const expectedScenarios = [
  'storefront_active_list_projection_parity',
  'admin_lookup_projection_parity',
  'admin_list_projection_parity',
  'optional_not_found_transport_policy',
];

if (
  contract.schema_version !== 1 ||
  contract.module !== 'fulfillment' ||
  contract.packet !== 'shipping-option-read-transport-parity-execution-contract' ||
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
  contract.request_policy?.storefront_rest_path !== '/store/shipping-options' ||
  contract.request_policy?.admin_rest_path !== '/admin/shipping-options' ||
  contract.request_policy?.maximum_response_bytes !== 1048576 ||
  contract.request_policy?.allow_http_for_local_capture !== true ||
  contract.request_policy?.forbid_url_credentials_query_and_fragment !== true
) {
  failures.push('execution contract request policy mismatch');
}

for (const [value, label] of [
  [contract.retained_boundary?.bearer_token_retained, 'bearer token retention'],
  [contract.retained_boundary?.raw_response_bodies_retained, 'raw response retention'],
  [contract.retained_boundary?.shipping_option_metadata_retained, 'shipping-option metadata retention'],
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
    contract.retained_boundary?.runtime_context_failure_restart_and_remote_adapter_evidence_separate,
    'separate wider runtime evidence',
  ],
]) {
  if (value !== true) failures.push(`execution contract must require ${label}`);
}

for (const [value, label] of [
  ['const projectionSelection =', 'GraphQL projection selection'],
  ['function repositoryPath(relativePath)', 'repository path boundary'],
  ['repository path escapes capture root', 'source path traversal rejection'],
  ['function sourceHashes()', 'source hashing'],
  ['function normalizeOption(value, flavor, field)', 'option projection normalization'],
  ['function normalizeTranslations(value, flavor, field)', 'translation normalization'],
  ['function normalizeTimestampFields(option, field)', 'timestamp normalization'],
  ['function projectionHash(value)', 'projection hashing'],
  ['function endpoint(value, field)', 'URL validation'],
  ['must not contain credentials, query, or fragment', 'URL credential/query rejection'],
  ['must use https unless the mounted endpoint is localhost or loopback', 'remote HTTP rejection'],
  ["redirect: 'error'", 'redirect rejection'],
  ['function authorizationHeader(value)', 'authorization construction'],
  ['function tenantHeaderName(value)', 'tenant header validation'],
  ['function ensureOutputBoundary()', 'immutable output boundary'],
  ['parity evidence already exists; remove it explicitly before a new capture', 'immutable output'],
  ['writeFileSync(temporaryPath', 'atomic temporary write'],
  ['renameSync(temporaryPath, outputPath)', 'atomic evidence publish'],
  ['storefrontShippingOptions(tenantId: $tenantId, filter: $filter)', 'GraphQL storefront operation'],
  ['shippingOption(tenantId: $tenantId, id: $id)', 'GraphQL lookup operation'],
  ['shippingOptions(tenantId: $tenantId, filter: $filter)', 'GraphQL admin list operation'],
  ["restUrl(restBaseUrl, '/store/shipping-options'", 'REST storefront operation'],
  ['admin/shipping-options/', 'REST admin lookup operation'],
  ["restUrl(restBaseUrl, '/admin/shipping-options'", 'REST admin list operation'],
  ['GraphQL missing shipping-option lookup must return null', 'optional not-found policy'],
  ["restMissing.status !== 404 || restMissingCode !== 'commerce_admin_not_found'", 'REST optional not-found policy'],
  ['GraphQL storefront shipping-option list contains inactive options', 'storefront active-only assertion'],
  ["status: 'transport_projection_parity_captured_unreviewed'", 'bounded evidence status'],
  ['shipping_option_metadata_retained: false', 'packet metadata boundary'],
  ['transport_projection_parity_proven: true', 'bounded projection result'],
  ['runtime_parity_proven: false', 'wider runtime parity remains open'],
  ['runtime_context_failure_injection_proven: false', 'runtime context/failure evidence remains open'],
  ['process_restart_proven: false', 'restart evidence remains open'],
  ['remote_adapter_identity_proven: false', 'remote identity evidence remains open'],
  ['remote_adapter_behavior_proven: false', 'remote adapter evidence remains open'],
]) {
  requireText(runner, value, label);
}

for (const value of [
  'auth_token:',
  'raw_response_body',
  'runtime_parity_proven: true',
  'remote_adapter_behavior_proven: true',
]) {
  forbidText(runner, value, 'capture runner must not over-retain or overclaim');
}

if (sourceEvidence.status !== 'source_cutover_ready_unvalidated') {
  failures.push('source evidence status must remain source_cutover_ready_unvalidated');
}
if (sourceEvidence.runtime_parity_proven !== false) {
  failures.push('source evidence runtime parity must remain false');
}
if (sourceEvidence.runtime_capture?.contract !== contractPath) {
  failures.push('source evidence must reference the shipping-option capture contract');
}
if (sourceEvidence.runtime_capture?.runner !== runnerPath) {
  failures.push('source evidence must reference the shipping-option capture runner');
}
if (sourceEvidence.runtime_capture?.verifier !== verifierPath) {
  failures.push('source evidence must reference the shipping-option capture verifier');
}
if (sourceEvidence.runtime_capture?.contract_published !== true) {
  failures.push('source evidence must record the published shipping-option capture contract');
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

requireText(runbook, 'Status: capture contract published, execution pending.', 'runbook status');
requireText(runbook, contractPath, 'runbook contract path');
requireText(runbook, runnerPath, 'runbook runner path');
requireText(runbook, verifierPath, 'runbook verifier path');
requireText(runbook, 'RUSTOK_SHIPPING_PARITY_GRAPHQL_URL', 'runbook GraphQL input');
requireText(runbook, 'RUSTOK_SHIPPING_PARITY_REST_BASE_URL', 'runbook REST input');
requireText(runbook, 'Remote mounted endpoints must use HTTPS.', 'runbook transport security');
requireText(runbook, 'does not retain the bearer token', 'runbook secret boundary');
requireText(runbook, 'runtime_parity_proven remains false', 'runbook non-promotion rule');

for (const [value, label] of [
  ['Publish the mounted shipping-option projection-parity execution contract and capture runner.', 'plan contract checklist'],
  ['Execute the mounted GraphQL/REST shipping-option projection-parity capture and retain its immutable packet.', 'plan execution checklist'],
  ['Prove deadline/failure injection, process restart, and remote-adapter behavior separately.', 'plan wider runtime checklist'],
]) {
  requireText(plan, value, label);
}

if (existsSync(new URL(evidencePath, root))) {
  const packet = JSON.parse(read(evidencePath));
  if (packet.schema_version !== 1) failures.push('captured shipping-option evidence schema mismatch');
  if (packet.status !== 'transport_projection_parity_captured_unreviewed') {
    failures.push('captured shipping-option evidence status mismatch');
  }
  if (packet.runtime_parity_proven !== false) {
    failures.push('captured shipping-option evidence must keep runtime parity false');
  }
  if (packet.review?.production_promotion_authorized !== false) {
    failures.push('captured shipping-option evidence cannot authorize promotion');
  }
  if (packet.retained_boundary?.bearer_token_retained !== false) {
    failures.push('captured shipping-option evidence must not retain bearer token');
  }
  if (packet.retained_boundary?.raw_response_bodies_retained !== false) {
    failures.push('captured shipping-option evidence must not retain raw bodies');
  }
  if (packet.retained_boundary?.shipping_option_metadata_retained !== false) {
    failures.push('captured shipping-option evidence must not retain shipping-option metadata');
  }
}

if (failures.length > 0) {
  console.error('Shipping-option transport parity capture verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Shipping-option GraphQL/REST projection parity capture is contract-locked, secret-safe, immutable, and cannot promote unproven runtime parity',
);
