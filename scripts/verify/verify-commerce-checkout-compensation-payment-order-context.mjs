#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const services = read('crates/rustok-commerce/src/services/mod.rs');
const facade = read('crates/rustok-commerce/src/services/checkout_compensation_error_safe.rs');
const retained = read('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs');

requireText(
  services,
  '#[path = "checkout_compensation_error_safe.rs"]\nmod checkout_compensation;',
  'mounted compensation facade',
);

for (const marker of [
  'use super::rustok_payment_shim as rustok_payment;',
  'use super::rustok_order_shim as rustok_order;',
  'use super::rustok_inventory_shim as rustok_inventory;',
  'use super::rustok_cart_shim as rustok_cart;',
  'include!("checkout_compensation_owner_ports.rs");',
  'BoundaryFacts::payment(&request)',
  'BoundaryFacts::order(&request)',
  'BoundaryFacts::inventory(&request)',
  'BoundaryFacts::cart_snapshot(&request)',
  'BoundaryFacts::cart_release(&request)',
  'operation_id_shape = facts.operation_id_shape',
  'primary_id_shape = facts.primary_id_shape',
  'secondary_id_shape = facts.secondary_id_shape',
  'opaque_text_shape = facts.opaque_text_shape',
  'opaque_text_len = ?facts.opaque_text_len',
  'payload_kind = facts.payload_kind',
  'payload_entry_count = ?facts.payload_entry_count',
  'owner_message_present',
  'owner_message_len',
  'formatter.write_str("redacted")',
]) requireText(facade, marker, 'combined bounded context facade');

for (const marker of [
  'error = ?error',
  'internal_message = %error.message',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(facade, marker, 'raw mounted context diagnostics');

for (const marker of [
  'self.compensate_payment(tenant_id, actor_id, operation)',
  'self.compensate_order(tenant_id, actor_id, operation)',
  'self.release_remaining_reservations(tenant_id, operation)',
  'self.release_cart(tenant_id, operation)',
  'payment_context(',
  'order_context(',
  'inventory_context(',
  'cart_context(',
  'with_causation_id(',
  'with_idempotency_key(',
  'with_deadline(',
]) requireText(retained, marker, 'retained context and ordering');

for (const owner of ['rustok_payment', 'rustok_order', 'rustok_inventory', 'rustok_cart']) {
  const count = facade.split(`$owner != "${owner}"`).length - 1;
  if (count !== 2) failures.push(`expected two retained-log suppressions for ${owner}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Checkout compensation context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Checkout compensation retains bounded payment, order, inventory, and cart context');
