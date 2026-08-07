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
const paymentStage = read('crates/rustok-commerce/src/services/checkout_payment_stages_legacy.rs');
const operation = read('crates/rustok-commerce/src/services/checkout_operation.rs');
const recovery = read('crates/rustok-commerce/src/services/recovering_staged_checkout.rs');
const doc = read('crates/rustok-commerce/docs/checkout-payment-stage-context.md');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-payment-stage-error-safety-source-review.json',
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
  [staged, 'CheckoutPaymentStageError', 'typed payment stage import'],
  [
    disposition,
    'CheckoutStagePipelineError::PaymentStage(CheckoutPaymentStageError::Boundary {\n            retryable: true,',
    'retryable payment boundary disposition',
  ],
  [disposition, '=> FailureDisposition::Retryable', 'retryable disposition target'],
  [disposition, '_ => FailureDisposition::CompensationRequired', 'fail-closed fallback'],
  [persistence, 'FailureDisposition::Retryable', 'retryable persistence branch'],
  [persistence, '.mark_retryable_error(', 'retryable journal transition'],
  [persistence, 'FailureDisposition::CompensationRequired', 'compensation persistence branch'],
  [persistence, '.mark_compensation_required(', 'compensation journal transition'],
  [
    staged,
    'fn retryable_payment_stage_boundary_does_not_force_compensation()',
    'retryable payment disposition source test',
  ],
  [
    staged,
    'fn non_retryable_payment_stage_boundary_requires_compensation()',
    'non-retryable payment disposition source test',
  ],
  [claimExecution, 'CheckoutOperationStatus::RetryableError.as_str()', 'retryable execution claim'],
  [
    claimCompensation,
    'CheckoutOperationStatus::CompensationRequired.as_str()',
    'compensation-only claim admission',
  ],
  [
    recovery,
    'if operation.status != CheckoutOperationStatus::CompensationRequired.as_str()',
    'synchronous compensation admission',
  ],
  [paymentStage, 'retryable: error.retryable', 'payment owner retryability propagation'],
]) requireText(source, value, label);

const retryableTest = between(
  staged,
  '    fn retryable_payment_stage_boundary_does_not_force_compensation()',
  '    #[test]\n    fn non_retryable_payment_stage_boundary_requires_compensation()',
  'retryable payment source test',
);
const nonRetryableTest = between(
  staged,
  '    fn non_retryable_payment_stage_boundary_requires_compensation()',
  '    #[test]\n    fn retryable_marketplace_boundary_does_not_force_compensation()',
  'non-retryable payment source test',
);
requireText(retryableTest, 'retryable: true', 'retryable payment fixture');
requireText(retryableTest, 'FailureDisposition::Retryable', 'retryable payment assertion');
requireText(nonRetryableTest, 'retryable: false', 'non-retryable payment fixture');
requireText(
  nonRetryableTest,
  'FailureDisposition::CompensationRequired',
  'non-retryable payment assertion',
);

const paymentArmIndex = disposition.indexOf('CheckoutStagePipelineError::PaymentStage(');
const fallbackIndex = disposition.indexOf('_ => FailureDisposition::CompensationRequired');
if (!(paymentArmIndex >= 0 && paymentArmIndex < fallbackIndex)) {
  failures.push('retryable payment boundary arm must precede the compensation fallback');
}

for (const value of [
  'CheckoutPaymentStageError::Boundary {\n            retryable: false',
  'CheckoutPaymentStageError::Conflict(_) => FailureDisposition::Retryable',
  'CheckoutPaymentStageError::Operation(_) => FailureDisposition::Retryable',
]) forbidText(disposition, value, 'unsafe payment retry broadening');

for (const operationName of [
  'prepare_checkout_collection',
  'authorize_checkout_collection',
  'capture_checkout_collection',
  'read_checkout_collection',
]) requireText(paymentStage, operationName, `${operationName} retained payment path`);

for (const [key, expected] of Object.entries({
  failure_disposition_changed: true,
  retryable_payment_boundary_disposition_retryable: true,
  non_retryable_payment_boundary_disposition_compensation_required: true,
  retryable_payment_boundary_synchronous_compensation: false,
  operation_journal_implementation_changed: false,
  recovery_service_implementation_changed: false,
  payment_stage_execution_changed: false,
  source_tests_added: true,
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
  ['retryable payment-stage owner boundaries', 'documented retryable scope'],
  ['Non-retryable payment-stage failures still require compensation', 'documented fail-closed scope'],
  ['No tests, Node verifiers, Cargo commands', 'validation disclosure'],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error('Staged checkout payment retry-disposition verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Retryable payment-stage boundaries remain resumable without synchronous compensation');
