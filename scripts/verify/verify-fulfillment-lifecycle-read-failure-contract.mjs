#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { readCommerceSafeQuerySource } from './lib/commerce-safe-query-source.mjs';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const safeQuery = readCommerceSafeQuerySource(read);
const cargo = read('crates/rustok-commerce/Cargo.toml');
const harness = read(
  'crates/rustok-commerce/tests/fulfillment_read_port_failure_contract.rs',
);
const contract = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-failure-execution-contract.json',
  ),
);
const evidence = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json',
  ),
);
const plan = read('crates/rustok-fulfillment/docs/implementation-plan.md');
const ownerNote = read(
  'crates/rustok-fulfillment/docs/fulfillment-lifecycle-read-failure-contract.md',
);
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [safeQuery, 'pub enum FulfillmentError {', 'typed GraphQL shim error'],
  [safeQuery, 'ShippingOptionNotFound(Uuid)', 'shipping optional not-found variant'],
  [safeQuery, 'FulfillmentNotFound(Uuid)', 'fulfillment optional not-found variant'],
  [safeQuery, 'Public(BoundaryError)', 'typed public boundary variant'],
  [safeQuery, 'pub(crate) fn to_string(self) -> BoundaryError', 'boundary-preserving conversion'],
  [safeQuery, 'Self::Public(error) => error', 'typed public conversion'],
  [
    safeQuery,
    'FulfillmentError::Public(BoundaryError::Public {',
    'owner error to public boundary conversion',
  ],
  [safeQuery, '"FULFILLMENT_ACCESS_DENIED"', 'GraphQL forbidden code'],
  [safeQuery, '"FULFILLMENT_OPERATION_FAILED"', 'GraphQL invariant code'],
  [safeQuery, 'if optional_not_found {', 'optional shipping not-found branch'],
  [
    safeQuery,
    'if matches!(&error.kind, PortErrorKind::NotFound)',
    'optional fulfillment not-found branch',
  ],
  [cargo, 'name = "fulfillment_read_port_failure_contract"', 'test target name'],
  [
    cargo,
    'path = "tests/fulfillment_read_port_failure_contract.rs"',
    'test target path',
  ],
  [harness, 'impl FulfillmentReadPort for ScriptedFulfillmentReadPort', 'scripted port'],
  [harness, 'Some(2_000)', 'two-second deadline assertion'],
  [harness, 'PortActorKind::Service', 'GraphQL service actor assertion'],
  [harness, 'PortActorKind::User', 'REST user actor assertion'],
  [harness, '"ru-RU"', 'REST locale assertion'],
  [harness, 'OWNER_SENTINEL', 'owner-message sentinel'],
  [
    harness,
    'graphql_fulfillment_lookup_preserves_typed_port_errors_and_redacts_owner_messages',
    'GraphQL failure matrix',
  ],
  [
    harness,
    'graphql_fulfillment_lookup_keeps_not_found_optional',
    'GraphQL optional not-found',
  ],
  [
    harness,
    'graphql_list_and_latest_by_order_apply_the_same_deadline_contract',
    'GraphQL operation deadline matrix',
  ],
  [
    harness,
    'admin_rest_fulfillment_detail_preserves_typed_errors_and_request_context',
    'REST failure matrix',
  ],
  [ownerNote, 'Typed GraphQL boundary', 'owner note section'],
  [plan, 'deterministic lifecycle read deadline and typed-failure harness', 'plan checkpoint'],
]) requireText(source, value, label);

for (const [value, label] of [
  [
    'FulfillmentError::Validation("fulfillment query is not permitted".to_string())',
    'forbidden downgrade',
  ],
  [
    'FulfillmentError::Validation(\n                    "fulfillment query could not be completed safely"',
    'invariant downgrade',
  ],
  [
    'DbErr::Custom("fulfillment storage is temporarily unavailable".to_string())',
    'technical error downgrade',
  ],
]) forbidText(safeQuery, value, label);

if (contract.status !== 'source_ready_unexecuted') {
  failures.push(`contract status mismatch: ${contract.status}`);
}
if (contract.owner_port !== 'FulfillmentReadPort') {
  failures.push('contract owner port must be FulfillmentReadPort');
}
if (contract.deadline?.milliseconds !== 2000) {
  failures.push('contract deadline must be 2000 milliseconds');
}
if (contract.test_target?.name !== 'fulfillment_read_port_failure_contract') {
  failures.push('contract test target mismatch');
}
if (contract.source_fix?.owner_message_control_flow !== false) {
  failures.push('contract must forbid owner-message control flow');
}
if (contract.source_fix?.query_source_changed !== false) {
  failures.push('contract must record unchanged query.rs source');
}
if (contract.source_fix?.optional_not_found_preserved !== true) {
  failures.push('contract must preserve optional not-found');
}
if (contract.context_assertions?.owner_message_redacted !== true) {
  failures.push('contract must require owner-message redaction');
}
if (contract.validation?.deadline_failure_proven !== false) {
  failures.push('contract must not claim executed deadline/failure proof');
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
]) {
  if (contract.validation?.[key] !== false) {
    failures.push(`contract validation.${key} must be false`);
  }
}

if (evidence.runtime_capture?.failure_contract_published !== true) {
  failures.push('source evidence must record published failure contract');
}
if (evidence.runtime_capture?.failure_harness_published !== true) {
  failures.push('source evidence must record published failure harness');
}
if (evidence.runtime_capture?.failure_harness_executed !== false) {
  failures.push('source evidence must retain failure harness as unexecuted');
}
if (evidence.runtime_capture?.deadline_failure_proven !== false) {
  failures.push('source evidence must retain deadline/failure proof as false');
}
if (evidence.graphql?.typed_port_error_extensions_preserved !== true) {
  failures.push('source evidence must record typed GraphQL error extensions');
}

if (failures.length > 0) {
  console.error('Fulfillment lifecycle failure-contract source verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ fulfillment lifecycle reads retain typed GraphQL/REST failures, optional not-found, two-second context deadlines, and owner-message redaction in the deterministic source harness',
);
