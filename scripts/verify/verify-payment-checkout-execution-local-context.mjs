#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const portImpl = read('crates/rustok-payment/src/checkout_execution/port_impl.rs');
const identity = read('crates/rustok-payment/src/checkout_execution/validation_identity.rs');
const errors = read('crates/rustok-payment/src/checkout_execution/validation_errors.rs');
const prepareAuthorize = read('crates/rustok-payment/src/checkout_execution/prepare_authorize.rs');
const capture = read('crates/rustok-payment/src/checkout_execution/capture_provider.rs');
const providerHelpers = read('crates/rustok-payment/src/checkout_execution/provider_helpers.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const publicImpl = between(
  portImpl,
  'impl CheckoutPaymentExecutionPort for InProcessCheckoutPaymentExecutionPort {',
  'impl InProcessCheckoutPaymentExecutionPort {',
  'checkout payment execution public implementation',
);
const prepare = between(
  publicImpl,
  'async fn prepare_checkout_collection(',
  'async fn authorize_checkout_collection(',
  'prepare operation',
);
const authorize = between(
  publicImpl,
  'async fn authorize_checkout_collection(',
  'async fn capture_checkout_collection(',
  'authorize operation',
);
const captureOperation = between(
  publicImpl,
  'async fn capture_checkout_collection(',
  'async fn read_checkout_collection(',
  'capture operation',
);
const readOperation = publicImpl.slice(publicImpl.indexOf('async fn read_checkout_collection('));
const readHelper = between(
  portImpl,
  'impl InProcessCheckoutPaymentExecutionPort {',
  'fn checkout_payment_execution_diagnostic_facts(',
  'read owner helper',
);
const facts = between(
  portImpl,
  'fn checkout_payment_execution_diagnostic_facts(',
  'fn map_checkout_payment_execution_local_port_error(',
  'safe diagnostic facts helper',
);
const mapper = portImpl.slice(
  portImpl.indexOf('fn map_checkout_payment_execution_local_port_error('),
);

for (const [block, config] of [
  [
    prepare,
    {
      policy: 'context.require_policy(PortCallPolicy::write())?;',
      semantics: 'context.require_write_semantics()?;',
      operation: 'PREPARE_CHECKOUT_COLLECTION_OPERATION',
      delegation: 'self.prepare(&context, owner_operation, tenant_id, request).await',
      collection: 'None,\n            None,\n            None,',
      label: 'prepare local routing',
    },
  ],
  [
    authorize,
    {
      policy: 'context.require_policy(PortCallPolicy::write())?;',
      semantics: 'context.require_write_semantics()?;',
      operation: 'AUTHORIZE_CHECKOUT_COLLECTION_OPERATION',
      delegation: '.authorize(&context, owner_operation, tenant_id, request)',
      collection: 'Some(request.collection_id),',
      label: 'authorize local routing',
    },
  ],
  [
    captureOperation,
    {
      policy: 'context.require_policy(PortCallPolicy::write())?;',
      semantics: 'context.require_write_semantics()?;',
      operation: 'CAPTURE_CHECKOUT_COLLECTION_OPERATION',
      delegation: 'self.capture(&context, owner_operation, tenant_id, request).await',
      collection: 'Some(request.collection_id),',
      label: 'capture local routing',
    },
  ],
  [
    readOperation,
    {
      policy: 'context.require_policy(PortCallPolicy::read())?;',
      semantics: null,
      operation: 'READ_CHECKOUT_COLLECTION_OPERATION',
      delegation: 'self.read(&context, owner_operation, tenant_id, request).await',
      collection: 'Some(request.collection_id),',
      label: 'read local routing',
    },
  ],
]) {
  for (const [value, detail] of [
    [config.policy, 'policy admission'],
    ['let tenant_id = parse_tenant_id(&context, owner_operation)?;', 'tenant validation'],
    ['require_operation_context(', 'causation validation'],
    ['let diagnostic_context = context.clone();', 'delegated context retention'],
    ['let diagnostic_facts = checkout_payment_execution_diagnostic_facts(', 'safe request retention'],
    [config.collection, 'collection/provider fact routing'],
    ['let result = self', 'owner result retention'],
    [config.delegation, 'unchanged owner delegation'],
    ['result.map_err(|error| {', 'post-delegation mapping'],
    ['map_checkout_payment_execution_local_port_error(', 'local mapper call'],
    ['&diagnostic_context,', 'retained context mapper argument'],
    ['owner_operation,', 'exact operation mapper argument'],
    ['&diagnostic_facts,', 'safe facts mapper argument'],
  ]) requireText(block, value, `${config.label} ${detail}`);
  if (config.semantics) requireText(block, config.semantics, `${config.label} write semantics`);
  else forbidText(block, 'require_write_semantics', `${config.label} must remain read-only`);

  const indexes = [
    block.indexOf(config.policy),
    block.indexOf('parse_tenant_id('),
    block.indexOf('require_operation_context('),
    block.indexOf('let diagnostic_context = context.clone();'),
    block.indexOf(config.delegation),
    block.indexOf('map_checkout_payment_execution_local_port_error('),
  ];
  if (!indexes.every((value, index) => index === 0 || indexes[index - 1] < value)) {
    failures.push(
      `${config.label}: expected admission -> tenant -> causation -> retention -> delegation -> mapping ordering`,
    );
  }
}

for (const [value, label] of [
  ['async fn read(', 'private read helper'],
  ['validate_identity(&request.identity)?;', 'read identity validation'],
  ['.get_collection(tenant_id, request.collection_id)', 'read collection lookup'],
  ['payment_error_to_port_error(context, owner_operation, error)', 'read owner error mapping'],
  ['validate_collection(&collection, tenant_id, &request.identity)?;', 'read collection validation'],
  ['Ok(collection)', 'read response preservation'],
]) requireText(readHelper, value, label);
const readIndexes = [
  readHelper.indexOf('validate_identity(&request.identity)?;'),
  readHelper.indexOf('.get_collection(tenant_id, request.collection_id)'),
  readHelper.indexOf('validate_collection(&collection, tenant_id, &request.identity)?;'),
  readHelper.indexOf('Ok(collection)'),
];
if (!readIndexes.every((value, index) => index === 0 || readIndexes[index - 1] < value)) {
  failures.push('read helper must preserve validate identity -> load collection -> validate collection ordering');
}

for (const [value, label] of [
  ['checkout_operation_id: identity.checkout_operation_id', 'checkout operation fact'],
  ['cart_id: identity.cart_id', 'cart fact'],
  ['order_id: identity.order_id', 'order fact'],
  ['customer_id: identity.customer_id', 'customer fact'],
  ['collection_id,', 'collection fact'],
  ['amount: identity.amount', 'amount fact'],
  ['currency_code_length: identity.currency_code.chars().count()', 'currency length fact'],
  ['order_plan_hash_length: identity.order_plan_hash.chars().count()', 'plan hash length fact'],
  ['requested_provider_id_length: requested_provider_id.map(|value| value.chars().count())', 'provider id length fact'],
  ['provider_payment_id_length: provider_payment_id.map(|value| value.chars().count())', 'provider payment id length fact'],
]) requireText(facts, value, label);
for (const value of [
  'currency_code.clone()',
  'order_plan_hash.clone()',
  'requested_provider_id.map(str::to_string)',
  'provider_payment_id.map(str::to_string)',
]) forbidText(facts, value, 'raw unvalidated payment identity retention');

for (const [value, label] of [
  ['"payment.checkout_identity_invalid"', 'checkout identity code'],
  ['"validate_checkout_identity"', 'checkout identity local operation'],
  ['"payment.checkout_currency_invalid"', 'currency code'],
  ['"validate_checkout_currency"', 'currency local operation'],
  ['"payment.checkout_plan_hash_invalid"', 'plan hash code'],
  ['"validate_checkout_plan_hash"', 'plan hash local operation'],
  ['"payment.checkout_collection_id_invalid"', 'collection id code'],
  ['"validate_collection_id"', 'collection id local operation'],
  ['"payment.checkout_collection_operation_conflict"', 'operation conflict code'],
  ['"validate_collection_operation"', 'operation conflict local operation'],
  ['"payment.checkout_collection_plan_conflict"', 'plan conflict code'],
  ['"validate_collection_plan"', 'plan conflict local operation'],
  ['"payment.checkout_collection_identity_missing"', 'missing collection identity code'],
  ['"require_collection_identity"', 'missing identity local operation'],
  ['"payment.checkout_collection_identity_conflict"', 'collection identity conflict code'],
  ['"validate_collection_identity"', 'collection identity local operation'],
  ['"payment.checkout_authorize_state_conflict"', 'authorize lifecycle code'],
  ['"validate_authorize_lifecycle"', 'authorize lifecycle local operation'],
  ['"payment.checkout_capture_state_conflict"', 'capture lifecycle code'],
  ['"validate_capture_lifecycle"', 'capture lifecycle local operation'],
  ['"payment.checkout_authorize_request_invalid"', 'authorize request code'],
  ['"validate_authorize_request"', 'authorize request local operation'],
  ['"payment.provider_metadata_invalid"', 'provider metadata code'],
  ['"validate_provider_metadata"', 'provider metadata local operation'],
  ['"payment.provider_identity_conflict"', 'provider identity code'],
  ['"validate_provider_identity"', 'provider identity local operation'],
  ['"payment.provider_idempotency_key_required"', 'provider idempotency code'],
  ['"require_provider_idempotency_key"', 'provider idempotency local operation'],
  ['"payment.provider_request_encoding_failed"', 'provider encoding code'],
  ['"encode_provider_request"', 'provider encoding local operation'],
  ['"payment.database_unavailable"', 'storage code'],
  ['"owner_storage"', 'storage local operation'],
  ['"payment.checkout_execution_validation"', 'owner validation code'],
  ['"validate_owner_request"', 'owner validation local operation'],
  ['"payment.collection_not_found"', 'collection not-found code'],
  ['"load_collection"', 'collection not-found local operation'],
  ['"payment.checkout_execution_state_conflict"', 'owner lifecycle code'],
  ['"apply_payment_lifecycle"', 'owner lifecycle local operation'],
  ['"payment.provider_unavailable"', 'provider unavailable code'],
  ['"payment.provider_rejected"', 'provider rejected code'],
  ['"execute_provider_operation"', 'provider execution local operation'],
  ['"payment.checkout_execution_manual_reconciliation"', 'manual reconciliation code'],
  ['"require_manual_reconciliation"', 'manual reconciliation local operation'],
  ['"payment.provider_not_configured"', 'provider configuration code'],
  ['"resolve_provider"', 'provider configuration local operation'],
  ['"require_collection_identity" | "require_manual_reconciliation"', 'integrity severity classification'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['tracing::error!(', 'technical local event'],
  ['tracing::warn!(', 'ordinary local event'],
  ['error = ?error', 'original delegated error'],
  ['owner = "rustok_payment"', 'truthful owner'],
  ['operation,', 'public operation'],
  ['local_operation,', 'local operation'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['checkout_operation_id = %facts.checkout_operation_id', 'checkout operation context'],
  ['cart_id = %facts.cart_id', 'cart context'],
  ['order_id = %facts.order_id', 'order context'],
  ['customer_id = ?facts.customer_id', 'customer context'],
  ['collection_id = ?facts.collection_id', 'collection context'],
  ['request_amount = %facts.amount', 'amount context'],
  ['currency_code_length = facts.currency_code_length', 'currency length context'],
  ['order_plan_hash_length = facts.order_plan_hash_length', 'plan hash length context'],
  ['requested_provider_id_length = ?facts.requested_provider_id_length', 'provider id length context'],
  ['provider_payment_id_length = ?facts.provider_payment_id_length', 'provider payment id length context'],
  ['internal_code = %error.code', 'stable code'],
  ['internal_message = %error.message', 'stable message'],
  ['error_kind = ?error.kind', 'typed kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = "checkout_payment_execution_port"', 'owner boundary'],
  ['\n    error\n}', 'same delegated error return'],
]) requireText(mapper, value, label);

const unknownReturns = mapper.match(/_ => return error,/g)?.length ?? 0;
if (unknownReturns !== 1) {
  failures.push(`unknown local outcome pass-through count: expected 1, found ${unknownReturns}`);
}
for (const value of [
  'payment.tenant_id_invalid',
  'payment.checkout_operation_id_invalid',
]) forbidText(mapper, value, 'admission and context errors must not be remapped locally');
for (const value of [
  'currency_code =',
  'order_plan_hash =',
  'requested_provider_id =',
  'provider_payment_id =',
  'metadata =',
]) forbidText(mapper, value, 'raw caller payment identity diagnostics');

for (const [content, values, label] of [
  [
    identity,
    [
      '"payment.checkout_identity_invalid"',
      '"payment.checkout_currency_invalid"',
      '"payment.checkout_plan_hash_invalid"',
      '"payment.checkout_collection_operation_conflict"',
      '"payment.checkout_collection_plan_conflict"',
      '"payment.checkout_collection_identity_conflict"',
      '"payment.checkout_collection_identity_missing"',
    ],
    'identity source',
  ],
  [
    prepareAuthorize,
    [
      'async fn prepare(',
      'validate_identity(&request.identity)?;',
      '"payment.checkout_collection_id_invalid"',
      '"payment.checkout_authorize_state_conflict"',
      '"payment.checkout_authorize_request_invalid"',
    ],
    'prepare and authorize source',
  ],
  [
    capture,
    [
      'validate_identity(&request.identity)?;',
      '"payment.checkout_collection_id_invalid"',
      '"payment.checkout_capture_state_conflict"',
      '"payment.provider_idempotency_key_required"',
      '"payment.provider_request_encoding_failed"',
    ],
    'capture source',
  ],
  [
    providerHelpers,
    [
      'insert_metadata_string(',
      '"provider_payment_id"',
      'manual_reconciliation(',
    ],
    'provider helper source',
  ],
  [
    errors,
    [
      '"payment.provider_metadata_invalid"',
      '"payment.provider_identity_conflict"',
      '"payment.checkout_execution_manual_reconciliation"',
      '"payment.database_unavailable"',
      '"payment.checkout_execution_validation"',
      '"payment.checkout_execution_state_conflict"',
      '"payment.provider_unavailable"',
      '"payment.provider_rejected"',
      '"payment.provider_not_configured"',
    ],
    'stable owner envelope source',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

if (failures.length > 0) {
  console.error('Payment checkout execution local-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment checkout execution operations retain delegated context and safe request facts for stable local outcomes without exposing raw caller strings or changing PortError results',
);
