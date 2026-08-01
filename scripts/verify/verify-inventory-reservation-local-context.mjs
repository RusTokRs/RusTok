#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  wrapper: 'crates/rustok-inventory/src/reservation_owner_context.rs',
  legacy: 'crates/rustok-inventory/src/ports.rs',
  evidence:
    'crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source.json',
  review:
    'crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source-review.json',
  document: 'crates/rustok-inventory/docs/reservation-local-context.md',
};
const wrapper = read(paths.wrapper);
const legacy = read(paths.legacy);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
function functionBody(content, name) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${name}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(content);
  if (!match) {
    failures.push(`missing function ${name}`);
    return '';
  }
  const open = content.indexOf('{', match.index);
  let depth = 0;
  for (let index = open; index < content.length; index += 1) {
    if (content[index] === '{') depth += 1;
    if (content[index] === '}') {
      depth -= 1;
      if (depth === 0) return content.slice(open, index + 1);
    }
  }
  failures.push(`unterminated function ${name}`);
  return '';
}

const mapper = functionBody(
  wrapper,
  'map_inventory_reservation_identity_local_port_error',
);
const logger = functionBody(wrapper, 'log_inventory_reservation_local_outcome');
const reserveFacts = functionBody(wrapper, 'reserve');
const releaseFacts = functionBody(wrapper, 'release');
const diagnosticScope = [logger, reserveFacts, releaseFacts].join('\n');

for (const [value, label] of [
  ['let diagnostic_context = context.clone();', 'delegated context retention'],
  [
    'let identity = InventoryReservationIdentityDiagnostic::reserve(&request);',
    'reserve request shape',
  ],
  [
    'let identity = InventoryReservationIdentityDiagnostic::release(&request);',
    'release request shape',
  ],
  ['.reserve_inventory_by_identity(context, request)', 'reserve delegation'],
  ['.release_inventory_by_identity(context, request)', 'release delegation'],
  ['result.map_err(|error| {', 'post-delegation mapping'],
  ['&diagnostic_context,', 'retained context argument'],
  ['&identity,', 'request-shape argument'],
]) requireText(wrapper, value, label);

for (const [value, label] of [
  ['reservation_id_present: true', 'reservation-id presence'],
  ['reservation_id_non_nil: !request.reservation_id.is_nil()', 'reservation-id shape'],
  ['variant_id_present: true', 'variant-id presence'],
  ['variant_id_non_nil: !request.variant_id.is_nil()', 'variant-id shape'],
  ['quantity_present: true', 'quantity presence'],
  ['quantity_nonzero: request.quantity != 0', 'quantity non-zero shape'],
  ['quantity_negative: request.quantity < 0', 'quantity sign shape'],
  ['line_item_id_present: request.line_item_id.is_some()', 'line-item presence'],
  ['external_id_length: request.external_id.chars().count()', 'external-id length'],
]) requireText(reserveFacts, value, label);
for (const [value, label] of [
  ['reservation_id_present: true', 'release reservation-id presence'],
  ['reservation_id_non_nil: !request.reservation_id.is_nil()', 'release reservation-id shape'],
  ['variant_id_present: false', 'release variant absence'],
  ['quantity_present: false', 'release quantity absence'],
  ['line_item_id_present: false', 'release line-item absence'],
  ['external_id_length: request.external_id.chars().count()', 'release external-id length'],
]) requireText(releaseFacts, value, label);

for (const [value, label] of [
  ['"inventory.reservation_external_id_invalid"', 'external-id validation code'],
  ['"reservation external_id must contain 1 to 191 characters"', 'external-id exact classification'],
  ['"normalize_external_id"', 'external-id local operation'],
  ['"inventory.reservation_quantity_invalid"', 'quantity validation code'],
  ['"validate_reservation_quantity"', 'quantity local operation'],
  ['"inventory.variant_not_found"', 'variant code'],
  ['"load_variant"', 'variant local operation'],
  ['"inventory.state_not_found"', 'state code'],
  ['"load_inventory_state"', 'state local operation'],
  ['"validate_existing_reservation_identity"', 'reserve replay local operation'],
  ['"reserve_available_stock"', 'stock local operation'],
  ['"load_reservation"', 'release lookup local operation'],
  ['"validate_release_external_identity"', 'release identity local operation'],
  ['"load_reservation_inventory_item"', 'missing item local operation'],
  ['"revalidate_release_identity"', 'locked identity local operation'],
  ['"release_reserved_quantity"', 'ledger local operation'],
  ['"calculate_available_quantity"', 'availability local operation'],
  ['"owner_storage"', 'storage local operation'],
  ['_ => return error,', 'unknown pass-through'],
  ['log_inventory_reservation_local_outcome(', 'bounded logger call'],
  ['error\n}', 'same delegated error return'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['tracing::error!(', 'technical event'],
  ['tracing::warn!(', 'ordinary event'],
  ['owner = INVENTORY_OWNER', 'truthful owner'],
  ['operation,', 'public operation'],
  ['local_operation,', 'local operation'],
  ['correlation_id = %context.correlation_id', 'correlation id'],
  ['tenant_id_length = facts.tenant_id_length', 'tenant shape'],
  ['actor_kind = facts.actor_kind', 'actor kind'],
  ['reservation_id_present = identity.reservation_id_present', 'reservation presence'],
  ['reservation_id_non_nil = identity.reservation_id_non_nil', 'reservation shape'],
  ['variant_id_present = identity.variant_id_present', 'variant presence'],
  ['variant_id_non_nil = identity.variant_id_non_nil', 'variant shape'],
  ['quantity_present = identity.quantity_present', 'quantity presence'],
  ['quantity_nonzero = identity.quantity_nonzero', 'quantity non-zero'],
  ['quantity_negative = identity.quantity_negative', 'quantity sign'],
  ['line_item_id_present = identity.line_item_id_present', 'line-item presence'],
  ['line_item_id_non_nil = identity.line_item_id_non_nil', 'line-item shape'],
  ['external_id_length = identity.external_id_length', 'external-id length'],
  ['code = %error.code', 'stable code'],
  ['error_message_present = !error.message.is_empty()', 'message presence'],
  ['error_message_length = error.message.chars().count()', 'message length'],
  [
    'error_kind = inventory_reservation_port_error_kind(&error.kind)',
    'closed error kind',
  ],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'boundary'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'severity classification',
  ],
]) requireText(logger, value, label);

for (const [value, label] of [
  ['error = ?error', 'complete delegated error'],
  ['error = %error', 'display delegated error'],
  ['internal_message = %error.message', 'raw message'],
  ['error_kind = ?error.kind', 'debug error kind'],
  ['tenant_id = %context.tenant_id', 'raw tenant'],
  ['actor = ?context.actor', 'raw actor'],
  ['channel = ?context.channel', 'raw channel'],
  ['locale = %context.locale', 'raw locale'],
  ['reservation_id = %', 'raw reservation id'],
  ['variant_id = ?', 'raw variant id'],
  ['request_quantity = ?', 'exact quantity'],
  ['line_item_id = ?', 'raw line-item id'],
  ['external_id =', 'raw external id'],
]) forbidText(diagnosticScope, value, label);

for (const marker of [
  'request.external_id = normalize_external_id(request.external_id)?;',
  '"inventory.reservation_quantity_invalid"',
  '"inventory.state_not_found"',
  '"inventory.insufficient_inventory"',
  '"inventory.reservation_not_found"',
  '"inventory.reservation_item_missing"',
  '"inventory.reservation_ledger_inconsistent"',
]) requireText(legacy, marker, 'unchanged persistent reservation behavior');

for (const [key, expected] of Object.entries({
  local_outcome_diagnostic_cleanup_closed: true,
  complete_port_error_logged: false,
  raw_request_uuid_logged: false,
  exact_request_quantity_logged: false,
  raw_external_id_logged: false,
  bounded_request_shape_logged: true,
  stable_code_logged: true,
  error_message_shape_logged: true,
  closed_error_kind_logged: true,
  delegated_error_return_changed: false,
  exact_code_message_classification_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  exact_local_classification_preserved: true,
  unknown_error_pass_through_preserved: true,
  complete_local_error_removed: true,
  raw_request_identifiers_removed: true,
  exact_quantity_removed: true,
  bounded_request_shape_retained: true,
  same_delegated_error_returned: true,
  persistent_owner_behavior_preserved: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  '# Inventory durable reservation local diagnostic safety',
  'Status: **source-ready / unvalidated**',
  'exact stable `code + message` pairs',
  'bounded request shape',
  'complete delegated `PortError` is not logged',
  'The same delegated error is returned unchanged.',
  'The ecommerce correlation-safe mapper task remains open',
]) requireText(document, marker, 'truthful local diagnostic document');

if (failures.length > 0) {
  console.error('Inventory durable reservation local diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Durable inventory reservation local outcomes retain exact classification and bounded request/context shape without logging complete delegated errors',
);
