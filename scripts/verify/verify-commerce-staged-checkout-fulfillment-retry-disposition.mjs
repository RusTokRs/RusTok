#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (file) => readFileSync(new URL(file, root), 'utf8');
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
    failures.push(`${label}: unable to isolate block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const staged = read('crates/rustok-commerce/src/services/staged_checkout.rs');
const fulfillmentFacade = read('crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs');
const fulfillmentLegacy = read(
  'crates/rustok-commerce/src/services/checkout_fulfillment_stages_legacy.rs',
);
const fulfillment = `${fulfillmentFacade}\n${fulfillmentLegacy}`;
const operation = read('crates/rustok-commerce/src/services/checkout_operation.rs');
const recovery = read('crates/rustok-commerce/src/services/recovering_staged_checkout.rs');
const doc = read('crates/rustok-commerce/docs/checkout-fulfillment-stage-context.md');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-fulfillment-retry-disposition-source-review.json',
));

const disposition = between(
  staged,
  'fn pipeline_failure_disposition(',
  'fn checkout_error_code(',
  'pipeline failure disposition',
);
const persistence = between(
  staged,
  '    async fn persist_pipeline_failure(',
  '\n}\n\n#[derive(Clone, Copy)]',
  'pipeline failure persistence',
);
const claimExecution = between(
  operation,
  '    pub async fn claim_execution(',
  '    pub async fn checkpoint(',
  'execution claim',
);
const claimCompensation = between(
  operation,
  '    pub async fn claim_compensation(',
  '    pub async fn mark_compensation_retryable(',
  'compensation claim',
);

for (const [source, value, label] of [
  [staged, 'CheckoutFulfillmentStageError', 'typed fulfillment stage import'],
  [
    disposition,
    'CheckoutStagePipelineError::FulfillmentStage(\n            CheckoutFulfillmentStageError::Boundary {\n                retryable: true,',
    'retryable fulfillment boundary disposition',
  ],
  [disposition, '=> FailureDisposition::Retryable', 'retryable disposition target'],
  [disposition, '_ => FailureDisposition::CompensationRequired', 'fail-closed fallback'],
  [persistence, '.mark_retryable_error(', 'retryable journal transition'],
  [persistence, '.mark_compensation_required(', 'compensation journal transition'],
  [
    staged,
    'fn retryable_fulfillment_stage_boundary_does_not_force_compensation()',
    'retryable fulfillment source test',
  ],
  [
    staged,
    'fn retryable_order_settlement_boundary_does_not_force_compensation()',
    'retryable order settlement source test',
  ],
  [
    staged,
    'fn non_retryable_fulfillment_stage_boundary_requires_compensation()',
    'non-retryable fulfillment source test',
  ],
  [claimExecution, 'CheckoutOperationStatus::RetryableError.as_str()', 'retryable execution claim'],
  [
    claimCompensation,
    'CheckoutOperationStatus::CompensationRequired.as_str()',
    'compensation-required claim admission',
  ],
  [
    recovery,
    'if operation.status != CheckoutOperationStatus::CompensationRequired.as_str()',
    'synchronous compensation admission',
  ],
  [fulfillment, 'retryable: error.retryable', 'owner retryability propagation'],
  [
    fulfillment,
    '.with_idempotency_key(format!("checkout:{operation_id}:fulfillment-set"))',
    'fulfillment ensure idempotency',
  ],
  [
    fulfillment,
    '.with_idempotency_key(format!("checkout:{operation_id}:order:payment-settlement"))',
    'order settlement idempotency',
  ],
  [fulfillment, 'read_checkout_fulfillments(', 'side-effect-free fulfillment read'],
  [
    fulfillment,
    'next_stage: CheckoutOperationStage::FulfillmentCreated',
    'fulfillment checkpoint preservation',
  ],
  [
    fulfillmentFacade,
    'include!("checkout_fulfillment_stages_legacy.rs");',
    'safe facade preserves retained implementation',
  ],
]) requireText(source, value, label);

const fulfillmentArmIndex = disposition.indexOf(
  'CheckoutStagePipelineError::FulfillmentStage(',
);
const fallbackIndex = disposition.indexOf('_ => FailureDisposition::CompensationRequired');
if (!(fulfillmentArmIndex >= 0 && fulfillmentArmIndex < fallbackIndex)) {
  failures.push('retryable fulfillment boundary arm must precede the compensation fallback');
}

for (const value of [
  'CheckoutFulfillmentStageError::Boundary {\n                retryable: false',
  'CheckoutFulfillmentStageError::Conflict(_) => FailureDisposition::Retryable',
  'CheckoutFulfillmentStageError::Operation(_) => FailureDisposition::Retryable',
]) forbidText(disposition, value, 'unsafe fulfillment retry broadening');

for (const stage of [
  '"ensure_fulfillments"',
  '"read_fulfillments"',
  '"settle_order_payment"',
]) requireText(fulfillment, stage, `${stage} retained boundary stage`);

for (const [key, expected] of Object.entries({
  fulfillment_stage_boundary_retryability_preserved: true,
  retryable_fulfillment_boundary_disposition_retryable: true,
  retryable_order_settlement_boundary_disposition_retryable: true,
  non_retryable_fulfillment_boundary_disposition_compensation_required: true,
  retryable_fulfillment_boundary_synchronous_compensation: false,
  retryable_error_execution_claimable: true,
  compensation_claim_requires_compensation_status: true,
  fulfillment_stage_execution_changed: false,
  operation_journal_implementation_changed: false,
  recovery_service_implementation_changed: false,
  source_tests_added: true,
  focused_static_guard_added: true,
  broad_fulfillment_mapper_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
}

for (const [value, label] of [
  ['Status: **source-reviewed / unvalidated**', 'truthful document status'],
  ['retryable fulfillment ensure/read boundary → `retryable_error`', 'documented fulfillment retry'],
  ['retryable order-payment settlement boundary → `retryable_error`', 'documented order retry'],
  ['No tests, Node verifiers, Cargo commands', 'validation disclosure'],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error('Staged checkout fulfillment retry-disposition verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Retryable fulfillment-stage boundaries remain resumable without synchronous compensation');
