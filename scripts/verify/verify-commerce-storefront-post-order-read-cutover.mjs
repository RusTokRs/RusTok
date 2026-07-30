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
const storefront = read('crates/rustok-commerce/src/controllers/store/orders.rs');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/order-read-port-source.json'),
);
const note = read('crates/rustok-order/docs/order-read-port.md');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

for (const [source, value, label] of [
  [owner, 'async fn read_order_return_projection(', 'return detail operation'],
  [owner, 'async fn list_order_return_projections(', 'return list operation'],
  [owner, 'async fn read_order_change_projection(', 'change detail operation'],
  [owner, 'async fn list_order_change_projections(', 'change list operation'],
  [owner, 'pub struct ReadOrderReturnProjectionRequest {', 'return detail request'],
  [owner, 'pub struct ListOrderReturnProjectionsRequest {', 'return list request'],
  [owner, 'pub struct OrderReturnProjectionPage {', 'return page'],
  [owner, 'pub struct ReadOrderChangeProjectionRequest {', 'change detail request'],
  [owner, 'pub struct ListOrderChangeProjectionsRequest {', 'change list request'],
  [owner, 'pub struct OrderChangeProjectionPage {', 'change page'],
  [owner, '.get_return(tenant_id, request.return_id)', 'owner return detail delegation'],
  [owner, '.list_returns(', 'owner return list delegation'],
  [owner, '.get_order_change(tenant_id, request.change_id)', 'owner change detail delegation'],
  [owner, '.list_order_changes(', 'owner change list delegation'],
  [exports, 'ListOrderReturnProjectionsRequest', 'return request export'],
  [exports, 'ListOrderChangeProjectionsRequest', 'change request export'],
  [storefront, '.list_order_return_projections(', 'storefront return port call'],
  [storefront, 'ListOrderReturnProjectionsRequest {', 'storefront return request'],
  [storefront, '.list_order_change_projections(', 'storefront change port call'],
  [storefront, 'ListOrderChangeProjectionsRequest {', 'storefront change request'],
  [storefront, 'data: page.items,', 'typed page items'],
  [storefront, 'page.total', 'typed page total'],
  [note, '## Storefront post-order read cutover', 'owner note section'],
]) requireText(source, value, label);

const returnRoute = between(
  storefront,
  'pub async fn list_order_returns(',
  '/// List refunds',
  'storefront return list route',
);
const changeRoute = between(
  storefront,
  'pub async fn list_order_changes(',
  '\n}',
  'storefront change list route',
);
for (const [route, label, call] of [
  [returnRoute, 'storefront return list route', '.list_order_return_projections('],
  [changeRoute, 'storefront change list route', '.list_order_change_projections('],
]) {
  requireText(route, call, `${label} typed owner port`);
  forbidText(route, 'OrderService::new', `${label} concrete owner construction`);
  forbidText(route, '.list_returns(', `${label} concrete return list`);
  forbidText(route, '.list_order_changes(', `${label} concrete change list`);
}

for (const [value, label] of [
  ['.create_return(tenant.id, id, input)', 'return mutation remains owner service'],
  ['PaymentService::new(runtime.db_clone())', 'refund list remains payment service'],
]) requireText(storefront, value, label);

if (evidence.status !== 'mounted_consumer_cutover_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (evidence.operations?.map((operation) => operation.name).join(',') !==
    'read_order_projection,list_order_projections,read_order_return_projection,list_order_return_projections,read_order_change_projection,list_order_change_projections') {
  failures.push('owner operation inventory mismatch');
}
if (evidence.consumer_inventory?.commerce_storefront_return_list !==
    'order_read_port_host_runtime') {
  failures.push('storefront return list must use the host-selected owner port');
}
if (evidence.consumer_inventory?.commerce_storefront_order_change_list !==
    'order_read_port_host_runtime') {
  failures.push('storefront order-change list must use the host-selected owner port');
}
for (const [key, expected] of [
  ['commerce_graphql_return_and_order_change_reads_cutover_completed', true],
  ['commerce_admin_return_and_order_change_reads_cutover_completed', true],
  ['post_order_consumer_cutover_completed', true],
  ['all_mounted_consumer_cutover_completed', true],
  ['all_consumer_cutover_completed', true],
  ['cutover_required', false],
]) {
  if (evidence.consumer_inventory?.[key] !== expected) {
    failures.push(`evidence consumer_inventory.${key} must be ${expected}`);
  }
}
if (evidence.decision?.status_promotion !== false) {
  failures.push('source cutover must not promote status');
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
  console.error('Commerce storefront post-order read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront return and order-change lists use typed owner projections while return mutation and refunds remain unchanged; all mounted post-order read consumers are cut over and execution evidence remains open',
);
