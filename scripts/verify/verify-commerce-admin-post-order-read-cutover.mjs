#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const router = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const reads = read('crates/rustok-commerce/src/controllers/admin/post_order_reads.rs');
const returns = read('crates/rustok-commerce/src/controllers/admin/returns.rs');
const changes = read('crates/rustok-commerce/src/controllers/admin/changes.rs');
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
const count = (source, value) => source.split(value).length - 1;

for (const [source, value, label] of [
  [router, 'pub mod post_order_reads;', 'mounted admin read module'],
  [router, 'pub mod post_order_commands;', 'mounted admin command module'],
  [router, 'axum::routing::get(post_order_reads::list_order_returns)', 'mounted return list'],
  [router, 'axum::routing::get(post_order_reads::show_order_return)', 'mounted return detail'],
  [router, 'axum::routing::get(post_order_reads::list_order_changes)', 'mounted change list'],
  [router, 'axum::routing::get(post_order_reads::show_order_change)', 'mounted change detail'],
  [router, 'axum::routing::post(post_order_commands::create_order_return)', 'return creation owner mutation'],
  [router, 'axum::routing::post(returns::create_order_return_decision)', 'return decision remains mounted orchestration'],
  [router, 'axum::routing::post(returns::complete_order_return)', 'return completion remains mounted orchestration'],
  [router, 'axum::routing::post(post_order_commands::cancel_order_return)', 'return cancel owner mutation'],
  [router, 'axum::routing::post(post_order_commands::create_order_change)', 'change creation owner mutation'],
  [router, 'axum::routing::post(changes::apply_order_change)', 'change apply remains mounted orchestration'],
  [router, 'axum::routing::post(post_order_commands::cancel_order_change)', 'change cancel owner mutation'],
  [reads, 'ListOrderReturnProjectionsRequest {', 'typed return list request'],
  [reads, 'ReadOrderReturnProjectionRequest { return_id: id }', 'typed return detail request'],
  [reads, 'ListOrderChangeProjectionsRequest {', 'typed change list request'],
  [reads, 'ReadOrderChangeProjectionRequest { change_id: id }', 'typed change detail request'],
  [reads, '.list_order_return_projections(', 'return list owner port call'],
  [reads, '.read_order_return_projection(', 'return detail owner port call'],
  [reads, '.list_order_change_projections(', 'change list owner port call'],
  [reads, '.read_order_change_projection(', 'change detail owner port call'],
  [reads, 'PortActor::user(auth.user_id.to_string())', 'validated user actor'],
  [reads, 'request_context.locale.as_str()', 'resolved request locale'],
  [reads, 'request_context.channel_slug.as_deref()', 'resolved request channel'],
  [reads, '.with_deadline(std::time::Duration::from_secs(2))', 'two-second deadline'],
  [reads, '&[Permission::ORDERS_READ]', 'preserved orders-read permission'],
  [reads, '"Permission denied: orders:read required"', 'preserved permission message'],
  [reads, 'per_page: pagination.limit()', 'clamped owner page size'],
  [reads, 'data: page.items,', 'typed page items'],
  [reads, 'page.total', 'owner pagination total'],
  [returns, '.create_return(tenant.id, id, input)', 'legacy return mutation compatibility source'],
  [returns, '.complete_return(tenant.id, auth.user_id, id, command)', 'return completion remains orchestration'],
  [returns, '.cancel_return(tenant.id, id, input)', 'legacy return cancel compatibility source'],
  [changes, '.create_order_change(tenant.id, actor_id, id, input)', 'legacy change mutation compatibility source'],
  [changes, '.apply_order_change(tenant.id, id, input.difference_refund, input.metadata)', 'change apply remains orchestration'],
  [changes, '.cancel_order_change(tenant.id, id, input)', 'legacy change cancel compatibility source'],
  [note, '## Admin post-order read cutover', 'focused admin cutover note'],
  [note, 'all mounted complete/post-order reads cut over, unvalidated', 'focused status'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['OrderService::new', 'concrete owner construction'],
  ['.list_returns(', 'concrete return list'],
  ['.get_return(', 'concrete return detail'],
  ['.list_order_changes(', 'concrete change list'],
  ['.get_order_change(', 'concrete change detail'],
  ['PermissionExtractor', 'changed granular permission extractor'],
]) forbidText(reads, value, label);

if (count(reads, '&[Permission::ORDERS_READ]') !== 4) {
  failures.push('all four mounted handlers must preserve ORDERS_READ');
}
if (count(reads, 'per_page: pagination.limit()') !== 2) {
  failures.push('both mounted list handlers must clamp page size');
}

if (evidence.status !== 'mounted_consumer_cutover_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of [
  ['commerce_admin_return_and_order_change_reads_cutover_completed', true],
  ['admin_post_order_consumer_cutover_completed', true],
  ['post_order_consumer_cutover_completed', true],
  ['all_mounted_consumer_cutover_completed', true],
  ['all_consumer_cutover_completed', true],
  ['cutover_required', false],
]) {
  if (evidence.consumer_inventory?.[key] !== expected) {
    failures.push(`evidence consumer_inventory.${key} must be ${expected}`);
  }
}
if (evidence.consumer_inventory?.commerce_admin_return_and_order_change_reads !==
    'order_read_port_host_runtime_with_request_context') {
  failures.push('mounted admin post-order reads must use the host-selected owner port');
}
if (evidence.unchanged_scope?.unmounted_admin_compatibility_handlers !==
    'concrete_order_service_source_only_not_routed') {
  failures.push('unmounted compatibility handlers must remain explicit source debt');
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
  console.error('Commerce admin post-order read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted admin return/order-change reads use typed owner projections while create/cancel writes use the owner command module and payment-coupled orchestration remains unchanged',
);
