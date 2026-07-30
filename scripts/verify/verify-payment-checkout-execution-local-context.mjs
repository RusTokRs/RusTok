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
const admission = between(
  errors,
  'fn require_checkout_payment_read_admission(',
  'fn require_operation_context(',
  'checkout payment execution admission helpers',
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
      admission: 'require_checkout_payment_write_admission(&context, owner_operation)?;',
      operation: 'PREPARE_CHECKOUT_COLLECTION_OPERATION',
      delegation: 'self.prepare(&context, owner_operation, tenant_id, request).await',
      collection: 'None,\n            None,\n            None,',
      label: 'prepare local routing',
    },
  ],
  [
    authorize,
    {
      admission: 'require_checkout_payment_write_admission(&context, owner_operation)?;',
      operation: 'AUTHORIZE_CHECKOUT_COLLECTION_OPERATION',
      delegation: '.authorize(&context, owner_operation, tenant_id, request)',
      collection: 'Some(request.collection_id),',
      label: 'authorize local routing',
    },
  ],
  [
    captureOperation,
    {
      admission: 'require_checkout_payment_write_admission(&context, owner_operation)?;',
      operation: 'CAPTURE_CHECKOUT_COLLECTION_OPERATION',
      delegation: 'self.capture(&context, owner_operation, tenant_id, request).await',
      collection: 'Some(request.collection_id),',
      label: 'capture local routing',
    },
  ],
  [
    readOperation,
    {
      admission: 'require_checkout_payment_read_admission(&context, owner_operation)?;',
      operation: 'READ_CHECKOUT_COLLECTION_OPERATION',
      delegation: 'self.read(&context, owner_operation, tenant_id, request).await',
      collection: 'Some(request.collection_id),',
      label: 'read local routing',
    },
  ],
]) {
  for (const [value, detail] of [
    [`let owner_operation = ${config.operation};`, 'owner operation'],
    [config.admission, 'correlation-safe admission'],
    ['let tenant_id = parse_tenant_id(&context, owner_operation)?;', 'tenant validation'],
    ['require_operation_context(', 'causation validation'],
    ['let diagnostic_context = context.clone();', 'delegated context retention'],
    ['let diagnostic_facts = checkout_payment_execution_diagnostic_facts(', 'safe request retention'],
    [config.collection, 'collection/provider fact routing'],
    [config.delegation, 'unchanged owner delegation'],
    ['result.map_err(|error| {', 'post-delegation mapping'],
    ['map_checkout_payment_execution_local_port_error(', 'local mapper call'],
  ]) requireText(block, value, `${config.label} ${detail}`);

  const indexes = [
    block.indexOf(config.admission),
    block.indexOf('parse_tenant_id('),
    block.indexOf('require_operation_context('),
    block.indexOf('let diagnostic_context = context.clone();'),
    block.indexOf(config.delegation),
    block.indexOf('map_checkout_payment_execution_local_port_error('),
  ];
  if (!indexes.every((value, index) => value >= 0 && (index === 0 || indexes[index - 1] < value))) {
    failures.push(
      `${config.label}: expected admission -> tenant -> causation -> retention -> delegation -> mapping ordering`,
    );
  }
}

for (const value of [
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_policy(PortCallPolicy::read())?;',
  'context.require_write_semantics()?;',
]) forbidText(publicImpl, value, 'public execution admission must use contextual helpers');

for (const [value, label] of [
  ['fn require_checkout_payment_read_admission(', 'read admission helper'],
  ['fn require_checkout_payment_write_admission(', 'write admission helper'],
  ['.require_policy(PortCallPolicy::read())', 'read policy'],
  ['.require_policy(PortCallPolicy::write())', 'write policy'],
  ['context.require_write_semantics().inspect_err', 'write semantics retention'],
  ['"policy"', 'policy admission kind'],
  ['"write_semantics"', 'write semantics admission kind'],
  ['fn log_checkout_payment_execution_admission_rejection(', 'admission logger'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity'],
  ['tracing::error!(', 'technical event'],
  ['tracing::warn!(', 'ordinary event'],
  ['error = ?error', 'original PortError'],
  ['owner = "rustok_payment"', 'truthful owner'],
  ['operation = owner_operation', 'owner operation context'],
  ['admission,', 'admission classification'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'stable code'],
  ['internal_message = %error.message', 'stable message'],
  ['error_kind = ?error.kind', 'typed kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = "checkout_payment_execution_port"', 'owner boundary'],
  ['"payment checkout execution admission failed"', 'technical message'],
  ['"payment checkout execution admission was rejected"', 'ordinary message'],
]) requireText(admission, value, label);

const inspectErrCount = admission.match(/\.inspect_err\(\|error\|/g)?.length ?? 0;
if (inspectErrCount !== 3) {
  failures.push(`admission helpers must retain the original PortError through three inspect_err calls; found ${inspectErrCount}`);
}
for (const value of [
  'PortError::new(',
  'PortError::validation(',
  'PortError::unavailable(',
  'PortError::conflict(',
  'map_err(',
]) forbidText(admission, value, 'admission helpers must not remap public errors');

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
  ['"payment.checkout_plan_hash_invalid"', 'plan hash code'],
  ['"payment.checkout_collection_identity_missing"', 'missing collection identity code'],
  ['"payment.database_unavailable"', 'storage code'],
  ['"payment.checkout_execution_validation"', 'owner validation code'],
  ['"payment.checkout_execution_state_conflict"', 'owner lifecycle code'],
  ['"payment.provider_unavailable"', 'provider unavailable code'],
  ['"payment.provider_rejected"', 'provider rejected code'],
  ['"payment.checkout_execution_manual_reconciliation"', 'manual reconciliation code'],
  ['"payment.provider_not_configured"', 'provider configuration code'],
  ['tracing::error!(', 'technical local event'],
  ['tracing::warn!(', 'ordinary local event'],
  ['owner = "rustok_payment"', 'truthful local owner'],
  ['correlation_id = %context.correlation_id', 'local correlation context'],
  ['boundary = "checkout_payment_execution_port"', 'local owner boundary'],
  ['\n    error\n}', 'same delegated error return'],
]) requireText(mapper, value, label);

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
      '"payment.checkout_collection_identity_missing"',
    ],
    'identity source',
  ],
  [
    prepareAuthorize,
    [
      'async fn prepare(',
      'validate_identity(&request.identity)?;',
      '"payment.checkout_authorize_state_conflict"',
      '"payment.checkout_authorize_request_invalid"',
    ],
    'prepare and authorize source',
  ],
  [
    capture,
    [
      'validate_identity(&request.identity)?;',
      '"payment.checkout_capture_state_conflict"',
      '"payment.provider_idempotency_key_required"',
      '"payment.provider_request_encoding_failed"',
    ],
    'capture source',
  ],
  [providerHelpers, ['insert_metadata_string(', '"provider_payment_id"', 'manual_reconciliation('], 'provider helper source'],
  [
    errors,
    [
      '"payment.provider_metadata_invalid"',
      '"payment.provider_identity_conflict"',
      '"payment.checkout_execution_manual_reconciliation"',
      '"payment.database_unavailable"',
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
  '✔ Payment checkout execution admission and delegated outcomes retain correlation-safe owner context without remapping PortError results or exposing raw caller strings',
);
