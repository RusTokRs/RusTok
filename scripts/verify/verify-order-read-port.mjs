#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const owner = read('crates/rustok-order/src/order_read.rs');
const exports = read('crates/rustok-order/src/lib.rs');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/order-read-port-source.json'),
);
const note = read('crates/rustok-order/docs/order-read-port.md');
const orderPlan = read('crates/rustok-order/docs/implementation-plan.md');
const commercePlan = read('crates/rustok-commerce/docs/implementation-plan.md');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [owner, 'pub trait OrderReadPort: Send + Sync {', 'owner read trait'],
  [owner, 'async fn read_order_projection(', 'detail operation'],
  [owner, 'async fn list_order_projections(', 'list operation'],
  [owner, 'pub struct ReadOrderProjectionRequest {', 'detail request'],
  [owner, 'pub struct ListOrderProjectionsRequest {', 'list request'],
  [owner, 'pub struct OrderProjectionPage {', 'page projection'],
  [owner, 'pub struct InProcessOrderReadPort {', 'in-process adapter'],
  [owner, 'pub fn in_process_order_read_port(', 'root factory'],
  [owner, 'context.require_policy(PortCallPolicy::read())?', 'read policy'],
  [owner, 'Uuid::parse_str(&context.tenant_id)', 'tenant parsing'],
  [owner, '.get_order_with_locale_fallback(', 'owner detail delegation'],
  [owner, '.list_orders_with_locale_fallback(', 'owner list delegation'],
  [owner, 'context.locale.as_str()', 'requested locale propagation'],
  [owner, 'request.tenant_default_locale.as_deref()', 'fallback locale propagation'],
  [owner, 'OrderError::Validation(_)', 'validation mapping'],
  [owner, 'OrderError::OrderNotFound(_)', 'order not-found mapping'],
  [owner, 'OrderError::OrderReturnNotFound(_)', 'return not-found mapping'],
  [owner, 'OrderError::OrderChangeNotFound(_)', 'change not-found mapping'],
  [owner, 'OrderError::InvalidTransition { .. }', 'transition mapping'],
  [owner, 'OrderError::Database(_)', 'database mapping'],
  [owner, 'OrderError::Core(_)', 'core mapping'],
  [owner, 'PortErrorKind::InvariantViolation', 'core fail-closed kind'],
  [owner, 'PortError::new(kind, code, message, retryable)', 'stable port error'],
  [owner, 'boundary = "order_read_port"', 'owner diagnostic boundary'],
  [exports, 'mod order_read;', 'private owner module'],
  [exports, 'OrderReadPort,', 'root trait export'],
  [exports, 'InProcessOrderReadPort,', 'root adapter export'],
  [exports, 'in_process_order_read_port,', 'root factory export'],
  [note, 'Status: owner port published, unvalidated', 'owner note status'],
  [orderPlan, 'OrderReadPort', 'order plan checkpoint'],
  [commercePlan, 'OrderReadPort', 'commerce plan checkpoint'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['error.message', 'owner message control flow'],
  ['error.to_string()', 'owner error string control flow'],
  ['format!("{error}")', 'formatted owner error control flow'],
  ['PortError::new(kind, code, error', 'raw owner error publication'],
]) forbidText(owner, value, label);

if (evidence.status !== 'owner_port_published_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (evidence.owner?.port !== 'OrderReadPort') {
  failures.push('evidence owner port must be OrderReadPort');
}
if (evidence.owner?.adapter !== 'InProcessOrderReadPort') {
  failures.push('evidence adapter must be InProcessOrderReadPort');
}
if (evidence.operations?.map((operation) => operation.name).join(',') !==
    'read_order_projection,list_order_projections') {
  failures.push('evidence operation inventory mismatch');
}
if (evidence.errors?.owner_message_control_flow !== false) {
  failures.push('evidence must forbid owner-message control flow');
}
if (evidence.errors?.all_current_order_error_variants_mapped !== true) {
  failures.push('evidence must record complete current OrderError mapping');
}
if (evidence.consumer_inventory?.runtime_composition_published !== false) {
  failures.push('runtime composition must remain unpublished in this wave');
}
if (evidence.consumer_inventory?.consumer_cutover_completed !== false) {
  failures.push('Commerce consumer cutover must remain incomplete in this wave');
}
if (evidence.consumer_inventory?.cutover_required !== true) {
  failures.push('evidence must retain the pending Commerce cutover');
}
if (evidence.decision?.status_promotion !== false) {
  failures.push('source publication must not promote status');
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'runtime_parity_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error('Order read port source verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order detail/list projections are published through a typed owner read port while Commerce runtime composition and consumer cutover remain explicitly pending',
);
