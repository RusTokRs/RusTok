#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/services/checkout_order_stages.rs', root),
  'utf8',
);
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

const completionBlock = between(
  source,
  '                stage if stage == CheckoutOperationStage::InventoryReserved.as_str() => {',
  '                stage if stage == CheckoutOperationStage::OrderCreated.as_str() => {',
  'inventory-reserved order completion block',
);
const orderCreatedBlock = between(
  source,
  '                stage if stage == CheckoutOperationStage::OrderCreated.as_str() => {',
  '                stage if stage == CheckoutOperationStage::PaymentReady.as_str() => {',
  'order-created checkpoint block',
);
const readProjectionBlock = between(
  source,
  '    async fn read_order_projection(',
  '    pub fn plan_journal(',
  'order projection read block',
);
const contextBlock = between(
  source,
  'fn completion_context(',
  'fn order_boundary_error(',
  'order context helper',
);
const ownerMapper = between(
  source,
  'fn order_boundary_error(',
  'fn boundary_error(',
  'order owner boundary mapper',
);
const boundaryMapper = between(
  source,
  'fn boundary_error(',
  'fn canonicalize_json(',
  'generic order boundary mapper',
);

for (const [value, label] of [
  ['PortErrorKind', 'typed port error classification import'],
  [
    'const CHECKOUT_ORDER_STAGE_BOUNDARY: &str = "commerce_checkout_order_stage";',
    'stable order stage boundary',
  ],
  ['const ORDER_STAGE_OWNER: &str = "rustok_order";', 'truthful order owner'],
  [
    'const RECOVER_EXISTING_OPERATION: &str = "recover_existing_checkout";',
    'exact recovery operation',
  ],
  [
    'const COMPLETE_CHECKOUT_OPERATION: &str = "complete_checkout";',
    'exact completion operation',
  ],
  [
    'const READ_ORDER_OPERATION: &str = "read_checkout_order";',
    'exact read operation',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  ['let write_context = completion_context(', 'retained write context'],
  ['recover_existing_checkout(', 'mounted recovery call'],
  ['write_context.clone()', 'write context delegation clone'],
  ['&write_context', 'write context mapper input'],
  ['RECOVER_EXISTING_OPERATION', 'recovery mapper operation'],
  ['"recover_existing"', 'recovery commerce stage'],
  ['complete_checkout(write_context.clone(), request)', 'mounted completion clone'],
  ['COMPLETE_CHECKOUT_OPERATION', 'completion mapper operation'],
  ['"complete"', 'completion commerce stage'],
  ['RecoverExistingCheckoutOrderRequest {', 'recovery request'],
  ['completion: request.clone()', 'immutable completion request reuse'],
  ['legacy_snapshot_hash', 'legacy snapshot hash'],
  ['legacy_request_hash', 'legacy request hash'],
  ['completion.order_id != order.id', 'completion projection identity validation'],
  ['validate_order_projection(&operation, &order, &[OrderStatusKind::Confirmed])', 'confirmed order validation'],
  ['.adopt_and_checkpoint(tenant_id, operation_id, lease_owner.clone(), &order)', 'inventory adoption checkpoint'],
]) requireText(completionBlock, value, label);

const writeBindings = completionBlock.match(/let write_context\s*=/g) ?? [];
const writeClones = completionBlock.match(/write_context\.clone\(\)/g) ?? [];
const writeMapperInputs = completionBlock.match(/&write_context/g) ?? [];
if (writeBindings.length !== 1 || writeClones.length !== 2 || writeMapperInputs.length !== 2) {
  failures.push(
    `expected one retained write context, two clones, and two mapper inputs, found ${writeBindings.length}/${writeClones.length}/${writeMapperInputs.length}`,
  );
}

for (const [value, label] of [
  ['let read_context = completion_context(', 'retained read context'],
  ['PortActor::service("rustok-commerce.checkout-order-stage")', 'read service actor'],
  ['"read-order"', 'read correlation action'],
  ['false,', 'read semantics'],
  ['read_checkout_order(', 'mounted order projection read'],
  ['read_context.clone()', 'read context delegation clone'],
  ['&read_context', 'read context mapper input'],
  ['READ_ORDER_OPERATION', 'read mapper operation'],
  ['"read_order"', 'read commerce stage'],
  ['ReadCheckoutOrderProjectionRequest {', 'read projection request'],
  ['checkout_operation_id: operation_id', 'read checkout identity'],
  ['locale: Some(plan.payload.context.locale.clone())', 'read locale'],
  ['fallback_locale: Some(plan.payload.context.default_locale.clone())', 'read fallback locale'],
]) requireText(readProjectionBlock, value, label);

const readBindings = readProjectionBlock.match(/let read_context\s*=/g) ?? [];
const readClones = readProjectionBlock.match(/read_context\.clone\(\)/g) ?? [];
const readMapperInputs = readProjectionBlock.match(/&read_context/g) ?? [];
if (readBindings.length !== 1 || readClones.length !== 1 || readMapperInputs.length !== 1) {
  failures.push(
    `expected one retained read context, one clone, and one mapper input, found ${readBindings.length}/${readClones.length}/${readMapperInputs.length}`,
  );
}

for (const [value, label] of [
  ['PortContext::new(', 'port context construction'],
  ['PLATFORM_FALLBACK_LOCALE', 'locale fallback'],
  ['checkout:{operation_id}:order:{action}', 'order correlation identity'],
  ['.with_causation_id(operation_id.to_string())', 'order causation'],
  ['.with_deadline(deadline)', 'order deadline'],
  ['if write {', 'write semantics branch'],
  ['checkout:{operation_id}:order:complete', 'completion idempotency key'],
]) requireText(contextBlock, value, label);

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'owner operation input'],
  ["stage: &'static str", 'commerce stage input'],
  ['error: PortError', 'original port error input'],
  [
    'log_checkout_order_boundary_failure(context, owner_operation, stage, &error);',
    'diagnostics before mapping',
  ],
  ['boundary_error(stage, error)', 'unchanged boundary delegation'],
  ['fn log_checkout_order_boundary_failure(', 'structured diagnostic helper'],
  ['error = ?boundary_error', 'original port error'],
  ['owner = ORDER_STAGE_OWNER', 'truthful owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation field'],
  ['stage,', 'commerce stage field'],
  ['code = %boundary_error.code', 'original port code'],
  ['internal_message = %boundary_error.message', 'public-safe port message'],
  ['error_kind = ?boundary_error.kind', 'typed port error kind'],
  ['retryable = boundary_error.retryable', 'original retryability'],
  ['boundary = CHECKOUT_ORDER_STAGE_BOUNDARY', 'boundary identity'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical error severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['"checkout order owner boundary failed"', 'technical diagnostic event'],
  ['"checkout order owner boundary was rejected"', 'rejection diagnostic event'],
]) requireText(ownerMapper, value, label);

const diagnosticIndex = ownerMapper.indexOf(
  'log_checkout_order_boundary_failure(context, owner_operation, stage, &error);',
);
const boundaryIndex = ownerMapper.indexOf('boundary_error(stage, error)');
if (!(diagnosticIndex >= 0 && diagnosticIndex < boundaryIndex)) {
  failures.push('order diagnostics must run before public boundary mapping');
}

for (const [value, label] of [
  ['CheckoutOrderStageError::Boundary {', 'stable boundary envelope'],
  ['stage,', 'stable boundary stage'],
  ['code: error.code', 'stable boundary code'],
  ['message: error.message', 'stable boundary message'],
  ['retryable: error.retryable', 'stable boundary retryability'],
]) requireText(boundaryMapper, value, label);

for (const [value, label] of [
  ['expected_stage: CheckoutOperationStage::OrderCreated', 'order-created checkpoint source'],
  ['next_stage: CheckoutOperationStage::PaymentReady', 'payment-ready checkpoint target'],
  ['order_id: Some(order.id)', 'order checkpoint identity'],
]) requireText(orderCreatedBlock, value, label);

for (const [value, label] of [
  ['.map_err(|error| boundary_error("recover_existing", error))', 'context-dropping recovery mapper'],
  ['.map_err(|error| boundary_error("complete", error))', 'context-dropping completion mapper'],
  ['.map_err(|error| boundary_error("read_order", error))', 'context-dropping read mapper'],
  ['complete_checkout(write_context, request)', 'moved completion context'],
  [
    'read_checkout_order(\n                completion_context(',
    'inline read context delegation',
  ],
  ['order_boundary_error("recover_existing", error)', 'legacy recovery owner mapper'],
  ['order_boundary_error("complete", error)', 'legacy completion owner mapper'],
  ['order_boundary_error("read_order", error)', 'legacy read owner mapper'],
]) forbidText(source, value, label);

if (failures.length > 0) {
  console.error('Checkout order stage context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout order recovery, completion, and projection read retain owner context without changing public envelopes',
);
