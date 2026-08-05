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
  wrapper: 'crates/rustok-inventory/src/reservation_port_context.rs',
  legacy: 'crates/rustok-inventory/src/ports.rs',
  service: 'crates/rustok-inventory/src/services/inventory.rs',
  evidence:
    'crates/rustok-inventory/contracts/evidence/availability-quantity-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-inventory/docs/availability-quantity-local-context.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return '';
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) {
      failures.push(`${label}: missing ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: ${marker} is out of order`);
      return;
    }
    previous = index;
  }
}

const wrapper = read(paths.wrapper);
const legacy = read(paths.legacy);
const service = read(paths.service);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const [functionName, markers, label] of [
  [
    'check_availability',
    [
      'require_inventory_reservation_read_admission(&context, AVAILABILITY_OPERATION)?;',
      'parse_inventory_reservation_tenant_id(&context, AVAILABILITY_OPERATION)?;',
      'let diagnostic_context = context.clone();',
      'let variant_id = request.variant_id;',
      'let quantity = request.requested_quantity;',
      'self.inner.check_availability(context, request).await',
      'map_inventory_reservation_local_port_error(',
    ],
    'availability routing',
  ],
  [
    'reserve_inventory',
    [
      'require_inventory_reservation_write_admission(&context, RESERVE_OPERATION)?;',
      'parse_inventory_reservation_tenant_id(&context, RESERVE_OPERATION)?;',
      'let diagnostic_context = context.clone();',
      'let variant_id = request.variant_id;',
      'let quantity = request.quantity;',
      'self.inner.reserve_inventory(context, request).await',
      'map_inventory_reservation_local_port_error(',
    ],
    'reserve routing',
  ],
  [
    'release_inventory_reservation',
    [
      'require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;',
      'parse_inventory_reservation_tenant_id(&context, RELEASE_OPERATION)?;',
      'let diagnostic_context = context.clone();',
      'let variant_id = request.variant_id;',
      'let quantity = request.quantity;',
      '.release_inventory_reservation(context, request)',
      'map_inventory_reservation_local_port_error(',
    ],
    'release routing',
  ],
]) {
  const body = functionBody(wrapper, functionName);
  for (const marker of markers) requireText(body, marker, label);
  requireOrder(body, markers, `${label} order`);
}

const mapper = functionBody(wrapper, 'map_inventory_reservation_local_port_error');
for (const marker of [
  '("inventory.validation", "inventory request is invalid")',
  '"validate_availability_request"',
  '"validate_reservation_request"',
  '"validate_reservation_release_request"',
  '("inventory.variant_not_found", "inventory variant was not found") => "load_variant"',
  '"inventory.insufficient_inventory"',
  '"inventory reservation conflicts with available stock"',
  ') if operation == RESERVE_OPERATION => "reserve_available_stock"',
  '"inventory.database_unavailable"',
  '"inventory storage is temporarily unavailable"',
  '"owner_storage"',
  '"inventory.invariant_violation"',
  '"inventory operation violated an owner invariant"',
  '"owner_invariant"',
  '_ => return error',
  'let technical_failure = inventory_reservation_error_is_technical(&error);',
  'log_inventory_reservation_local_outcome(',
  '\n    error\n}',
]) requireText(mapper, marker, `${paths.wrapper}: preserved local mapping`);
requireCount(mapper, 'return error', 2, `${paths.wrapper}: unknown local outcome pass-through`);
for (const forbidden of [
  'tracing::error!(',
  'tracing::warn!(',
  'error = ?error',
  'tenant_id = %context.tenant_id',
  'variant_id = %variant_id',
  'request_quantity = quantity',
]) forbidText(mapper, forbidden, `${paths.wrapper}: inline raw mapper diagnostics`);

const contextFacts = functionBody(wrapper, 'inventory_reservation_context_facts');
for (const marker of [
  'tenant_id_length: context.tenant_id.chars().count()',
  'rustok_api::PortActorKind::User => "user"',
  'rustok_api::PortActorKind::Service => "service"',
  'rustok_api::PortActorKind::System => "system"',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
]) requireText(contextFacts, marker, `${paths.wrapper}: bounded context facts`);

const technical = functionBody(wrapper, 'inventory_reservation_error_is_technical');
requireText(
  technical,
  'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
  `${paths.wrapper}: technical severity classification`,
);

const logger = functionBody(wrapper, 'log_inventory_reservation_local_outcome');
for (const marker of [
  'let context_facts = inventory_reservation_context_facts(context);',
  'if technical_failure',
  'tracing::error!(',
  'tracing::warn!(',
  'owner = INVENTORY_OWNER',
  'operation,',
  'local_operation,',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'channel_length = ?context_facts.channel_length',
  'locale_length = context_facts.locale_length',
  'causation_id_present = context_facts.causation_id_present',
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'deadline_ms = ?context_facts.deadline_ms',
  'variant_id_non_nil = !variant_id.is_nil()',
  'request_quantity_zero = quantity == 0',
  'request_quantity_negative = quantity < 0',
  'code = error.code.as_str()',
  'error_message_length = error.message.chars().count()',
  'retryable = error.retryable',
  'technical_failure,',
  'boundary = INVENTORY_RESERVATION_BOUNDARY',
]) requireText(logger, marker, `${paths.wrapper}: bounded local logger`);
requireCount(logger, 'tracing::error!(', 1, `${paths.wrapper}: technical local event count`);
requireCount(logger, 'tracing::warn!(', 1, `${paths.wrapper}: ordinary local event count`);
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'variant_id = %variant_id',
  'request_quantity = quantity',
]) forbidText(logger, forbidden, `${paths.wrapper}: raw local payload`);

for (const marker of [
  'PortError::unavailable(',
  '"inventory.database_unavailable"',
  '"inventory storage is temporarily unavailable"',
  '"inventory.variant_not_found"',
  '"inventory variant was not found"',
  '"inventory.insufficient_inventory"',
  '"inventory reservation conflicts with available stock"',
  'PortError::validation("inventory.validation", "inventory request is invalid")',
  '"inventory.invariant_violation"',
  '"inventory operation violated an owner invariant"',
]) requireText(legacy, marker, `${paths.legacy}: preserved public envelopes`);
for (const marker of [
  'validate_availability_request_quantity(requested_quantity)?;',
  'validate_reservation_quantity(quantity)?;',
  'validate_release_quantity(quantity)?;',
  'let variant = self.load_variant',
  'CommerceError::InsufficientInventory {',
]) requireText(service, marker, `${paths.service}: preserved owner behavior`);

if (
  evidence.status !==
  'inventory_availability_quantity_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  local_outcome_diagnostics_sanitized: true,
  complete_port_error_logged: false,
  raw_context_logged: false,
  raw_variant_uuid_logged: false,
  exact_quantity_logged: false,
  raw_error_message_logged: false,
  debug_error_kind_logged: false,
  exact_code_message_classification_preserved: true,
  technical_severity_preserved: true,
  ordinary_severity_preserved: true,
  same_port_error_return_preserved: true,
  owner_delegation_changed: false,
  public_envelopes_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.review_findings?.[key] !== expected) {
    failures.push(`${paths.evidence}: review_findings.${key} must be ${expected}`);
  }
}

requireText(doc, 'Status: **source-ready / unvalidated**', `${paths.doc}: status`);
requireText(doc, 'They no longer record the complete delegated `PortError`', `${paths.doc}: payload policy`);
requireText(
  doc,
  'This closes the identified raw diagnostic payloads in',
  `${paths.doc}: local source closure`,
);
requireText(
  plan,
  'Finish correlation-safe mapper cleanup',
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error('Inventory availability and quantity local diagnostic verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Inventory availability and quantity local outcomes preserve exact routing and same-error return with bounded diagnostics',
);
