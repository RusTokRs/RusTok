#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const failures = [];
const read = (relativePath) => {
  try {
    return readFileSync(new URL(relativePath, root), 'utf8');
  } catch {
    failures.push(`${relativePath}: source file is missing`);
    return '';
  }
};

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

// --- checkout-owned admission record (commerce) ---------------------------
// The contract has two halves: the checkout journal owns the level and its
// generation plus the park-time invalidation, the payment claim gate enforces
// them in one conditional write and records bounded refusals.
const migration = read(
  'crates/modules/rustok-commerce/src/migrations/m20261007_000012_add_checkout_operation_admission.rs',
);
requireText(
  migration,
  'ck_checkout_operations_execution_admission',
  'admission level constraint',
);
requireText(
  migration,
  "CHECK (execution_admission IN ('open', 'settling', 'closed'))",
  'bounded admission vocabulary',
);
requireText(migration, 'CHECK (admission_epoch > 0)', 'positive admission epoch');
requireText(migration, 'SET execution_admission = CASE', 'pre-existing row backfill');
requireText(migration, "'compensation_required',", 'settling status vocabulary');
requireText(migration, 'THEN \'settling\'', 'settling backfill level');
requireText(migration, "THEN 'closed'", 'closed backfill level');

const entity = read('crates/modules/rustok-commerce/src/entities/checkout_operation.rs');
requireText(entity, '    pub execution_admission: String,', 'checkout operation entity level');
requireText(entity, '    pub admission_epoch: i64,', 'checkout operation entity epoch');

const journal = read('crates/modules/rustok-commerce/src/services/checkout_operation.rs');
requireText(journal, 'pub enum CheckoutExecutionAdmission {', 'typed admission level');
requireText(
  journal,
  'Self::Open => CHECKOUT_OPERATION_ADMISSION_OPEN,',
  'open level from the event vocabulary',
);
requireText(
  journal,
  'Self::Settling => CHECKOUT_OPERATION_ADMISSION_SETTLING,',
  'settling level from the event vocabulary',
);
requireText(
  journal,
  'Self::Closed => CHECKOUT_OPERATION_ADMISSION_CLOSED,',
  'closed level from the event vocabulary',
);
requireText(journal, 'pub const fn for_status(', 'status to level derivation');
requireText(journal, '| CheckoutOperationStatus::RetryableError => Self::Open,', 'open status set');
requireText(
  journal,
  '| CheckoutOperationStatus::ReconciliationRequired => Self::Settling,',
  'settling status set',
);
requireText(journal, '| CheckoutOperationStatus::Failed => Self::Closed,', 'closed status set');
requireText(journal, 'fn unchanged(level: CheckoutExecutionAdmission) -> Self {', 'lease writes');
requireText(journal, 'fn epoch_delta(self) -> i64 {', 'epoch delta');
requireText(
  journal,
  'Expr::col(checkout_operation::Column::AdmissionEpoch),',
  'in-statement epoch bump',
);

// Every status write also writes the admission level and its generation, so the
// level can never drift away from the status the row carries.
const admissionWriters = countText(journal, 'checkout_operation::Column::ExecutionAdmission,');
const epochWriters = countText(journal, 'checkout_operation::Column::AdmissionEpoch,');
if (admissionWriters !== 7 || epochWriters !== 7) {
  failures.push(
    `expected 7 status writers to carry the admission columns, found ${admissionWriters} and ${epochWriters}`,
  );
}
const statusWriters = [
  'pub async fn claim_execution(',
  'pub async fn claim_compensation(',
  'pub async fn park_exhausted_compensation(',
  'pub async fn request_compensation_retry(',
  'pub async fn resolve_reconciliation_required(',
  'async fn release_lease_with_error(',
  'async fn mark_terminal(',
];
for (const writer of statusWriters) {
  const start = journal.indexOf(writer);
  if (start < 0) {
    failures.push(`status writer ${writer} not found`);
    continue;
  }
  const body = journal.slice(start, start + 9000);
  if (!body.includes('checkout_operation::Column::ExecutionAdmission,')) {
    failures.push(`${writer} writes a status without the admission level`);
  }
  if (!body.includes('checkout_operation::Column::AdmissionEpoch,')) {
    failures.push(`${writer} writes a status without the admission generation`);
  }
}

// The level vocabulary is owned by the event contract, not re-declared here.
for (const literal of ['"open"', '"settling"', '"closed"']) {
  forbidText(journal, `= ${literal};`, `duplicated admission vocabulary ${literal}`);
}

// --- admission event (rustok-events) --------------------------------------
const events = read('crates/libs/rustok-events/src/checkout_operation.rs');
requireText(
  events,
  'CHECKOUT_OPERATION_ADMISSION_CHANGED_EVENT_TYPE: &str =\n    "checkout.operation.admission_changed";',
  'admission changed event type',
);
requireText(events, 'pub const CHECKOUT_OPERATION_ADMISSIONS: &[&str]', 'bounded levels');
requireText(events, 'AdmissionChanged {', 'admission changed variant');
requireText(events, 'previous_admission: Option<String>,', 'previous level payload');
requireText(events, 'admission_epoch: i64,', 'generation payload');
requireText(
  events,
  'Self::AdmissionChanged { .. } => CHECKOUT_OPERATION_ADMISSION_CHANGED_EVENT_TYPE,',
  'admission changed event routing',
);
requireText(
  events,
  'validators::validate_range("admission_epoch", *admission_epoch, 1, i64::MAX)?;',
  'positive generation validation',
);
requireText(
  events,
  'validate_bounded_label("admission", admission, CHECKOUT_OPERATION_ADMISSIONS)?;',
  'bounded level validation',
);

const eventsLib = read('crates/libs/rustok-events/src/lib.rs');
requireText(
  eventsLib,
  '    CHECKOUT_OPERATION_ADMISSIONS, CHECKOUT_OPERATION_ADMISSION_CHANGED_EVENT_TYPE,',
  'admission vocabulary re-export',
);

// --- in-transaction publication -------------------------------------------
requireText(
  journal,
  'self.event_bus\n            .publish_contract_in_tx(\n                txn,\n                operation.tenant_id,\n                None,\n                CheckoutOperationEvent::AdmissionChanged {',
  'admission event written inside the writer transaction',
);

const writers = [
  'pub async fn begin(',
  'async fn release_lease_with_error(',
  'async fn mark_terminal(',
  'pub async fn resolve_reconciliation_required(',
];
for (const writer of writers) {
  const start = journal.indexOf(writer);
  if (start < 0) {
    failures.push(`writer ${writer} not found`);
    continue;
  }
  const body = journal.slice(start, start + 9000);
  const publish = body.indexOf('self.publish_admission_changed(');
  const commit = body.indexOf('txn.commit().await?');
  if (publish < 0) {
    failures.push(`${writer} must publish the admission change`);
  } else if (commit >= 0 && publish > commit) {
    failures.push(`${writer} publishes the admission change after the commit`);
  }
}

// --- payment half: the claim gate that replaced the trigger ------------------
const paymentMigration = read(
  'crates/modules/rustok-payment/src/migrations/m20261007_000122_add_provider_operation_admission.rs',
);
requireText(
  paymentMigration,
  'ck_payment_provider_operations_admission_epoch',
  'payment admission epoch constraint',
);
requireText(
  paymentMigration,
  'ck_payment_provider_operations_admission_refusal',
  'payment refusal code constraint',
);
requireText(
  paymentMigration,
  'admission_refusal_code IN (',
  'bounded refusal vocabulary constraint',
);
for (const code of [
  'checkout_admission_settling',
  'checkout_admission_closed',
  'checkout_admission_unavailable',
  'checkout_admission_epoch_mismatch',
  'checkout_admission_effect_unknown',
]) {
  requireText(paymentMigration, `'${code}'`, `bounded refusal vocabulary ${code}`);
}
for (const join of ['from checkout_operations', 'join checkout_operations']) {
  forbidText(paymentMigration, join, `payment migration reading the checkout table (${join})`);
}
const paymentMigrationsMod = read('crates/modules/rustok-payment/src/migrations/mod.rs');
requireText(
  paymentMigrationsMod,
  'mod m20261007_000122_add_provider_operation_admission;',
  'payment admission migration module',
);
requireText(
  paymentMigrationsMod,
  'm20261007_000122_add_provider_operation_admission::Migration),',
  'payment admission migration registration',
);

const paymentEntity = read('crates/modules/rustok-payment/src/entities/provider_operation.rs');
requireText(paymentEntity, 'pub admission_epoch: i64,', 'provider operation generation column');
requireText(
  paymentEntity,
  'pub admission_refusal_code: Option<String>,',
  'provider operation refusal diagnosis column',
);
requireText(
  paymentEntity,
  'pub admission_refused_at: Option<DateTimeWithTimeZone>,',
  'provider operation refusal timestamp column',
);

const paymentAdmission = read(
  'crates/modules/rustok-payment/src/services/checkout_admission.rs',
);
requireText(
  paymentAdmission,
  'pub trait CheckoutExecutionAdmissionPort: Send + Sync {',
  'checkout admission owner port',
);
requireText(
  paymentAdmission,
  'async fn read_checkout_execution_admission(',
  'checkout admission read',
);
requireText(paymentAdmission, 'pub enum ProviderExecutionAdmission {', 'payment level vocabulary');
requireText(
  paymentAdmission,
  'Self::Open => CHECKOUT_OPERATION_ADMISSION_OPEN,',
  'payment level labels from the event contract',
);
requireText(
  paymentAdmission,
  'pub fn decide_checkout_admission_claim(',
  'single claim decision implementation',
);
requireText(
  paymentAdmission,
  'Some(ProviderExecutionEffect::Unwinding) => CheckoutAdmissionDecision::Unfenced,',
  'unwinding effects stay admitted',
);
requireText(
  paymentAdmission,
  'pub fn checkout_operation_id_from_metadata(metadata: &serde_json::Value) -> Option<Uuid> {',
  'checkout link read from the payment-owned collection metadata',
);
for (const label of [
  'checkout_admission_settling',
  'checkout_admission_closed',
  'checkout_admission_unavailable',
  'checkout_admission_epoch_mismatch',
  'checkout_admission_effect_unknown',
]) {
  requireText(paymentAdmission, `=> "${label}",`, `bounded refusal code ${label}`);
}

const paymentJournal = read('crates/modules/rustok-payment/src/services/provider_operation.rs');
requireText(
  paymentJournal,
  'pub fn with_checkout_execution_admission_port(',
  'claim gate admission port wiring',
);
requireText(paymentJournal, 'pub async fn claim_execution(', 'claim gate entry point');
requireText(
  paymentJournal,
  'pub async fn stamp_admission_epoch<C>(',
  'park-time generation stamp owner command',
);
requireText(
  paymentJournal,
  'provider_operation::Column::AdmissionEpoch.is_in([0, epoch])',
  'legacy generation adoption is bounded to the open level',
);
requireText(
  paymentJournal,
  'provider_operation::Column::AdmissionEpoch.eq(epoch)',
  'single conditional claim write on the generation',
);
requireText(
  paymentJournal,
  'async fn record_admission_refusal(',
  'refusal recording',
);
requireText(
  paymentJournal,
  'record_provider_execution_admission_refused(',
  'refusal metric',
);
requireText(
  paymentJournal,
  'refusal_metric_operation_label(operation),',
  'bounded refusal metric operation label',
);
requireText(
  paymentAdmission,
  'pub fn refusal_metric_operation_label(',
  'bounded refusal metric label owner',
);
for (const label of ['"extending"', '"unwinding"', '"unknown"']) {
  requireText(
    paymentAdmission,
    label,
    `bounded refusal metric label ${label}`,
  );
}
requireText(
  paymentJournal,
  'pub fn execution_admission_refusal_error(',
  'bounded refusal error for claim callers',
);
forbidText(paymentJournal, 'checkout_operations', 'payment reading the checkout table');
forbidText(
  paymentJournal,
  'CREATE TRIGGER',
  'claim gate implemented as a database trigger',
);

const checkoutAdmissionPort = read(
  'crates/modules/rustok-commerce/src/services/checkout_execution_admission.rs',
);
requireText(
  checkoutAdmissionPort,
  'impl CheckoutExecutionAdmissionPort for CheckoutExecutionAdmissionReader {',
  'commerce admission port implementation',
);
requireText(
  checkoutAdmissionPort,
  'checkout_operation::Column::TenantId.eq(tenant_id)',
  'tenant-scoped admission read',
);
requireText(
  checkoutAdmissionPort,
  'pub fn checkout_execution_admission_port(',
  'commerce admission port factory',
);

const stages = read('crates/modules/rustok-commerce/src/services/checkout_payment_stages.rs');
const stageWiring = countText(stages, 'checkout_execution_admission_port(db.clone())');
if (stageWiring !== 2) {
  failures.push(
    `expected both checkout payment execution constructors to wire the admission port, found ${stageWiring}`,
  );
}
forbidText(
  stages,
  '::rustok_payment::in_process_checkout_payment_execution_port(db)',
  'payment execution port built without the admission reader',
);
for (const commerceJournalOwner of [
  'crates/modules/rustok-commerce/src/services/payment_orchestration.rs',
  'crates/modules/rustok-commerce/src/services/refund_reconciliation.rs',
  'crates/modules/rustok-commerce/src/services/checkout_compensation.rs',
]) {
  requireText(
    read(commerceJournalOwner),
    'with_checkout_execution_admission_port(',
    `${commerceJournalOwner}: payment journal without the admission reader`,
  );
}

const legacyStages = read(
  'crates/modules/rustok-commerce/src/services/checkout_payment_stages_legacy.rs',
);
const legacyWiring = countText(legacyStages, 'checkout_execution_admission_port(');
if (legacyWiring !== 2) {
  failures.push(
    `expected both legacy checkout payment stage constructors to wire the admission port, found ${legacyWiring}`,
  );
}
const serverRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
requireText(
  serverRuntime,
  'rustok_commerce::checkout_execution_admission_port(server.db_clone())',
  'host runtime admin collection command admission wiring',
);
const commerceLib = read('crates/modules/rustok-commerce/src/lib.rs');
requireText(
  commerceLib,
  'pub use services::checkout_execution_admission_port;',
  'commerce admission port factory export',
);
const controllers = read('crates/modules/rustok-commerce/src/controllers/mod.rs');
requireText(
  controllers,
  'checkout_execution_admission_port(db.clone()),',
  'admin collection command runtime admission wiring',
);
requireText(
  controllers,
  'checkout_execution_admission_port(runtime.db_clone()),',
  'host runtime admin collection command admission wiring',
);
const graphqlPaymentCommands = read(
  'crates/modules/rustok-commerce/src/graphql_runtime/payment_commands.rs',
);
requireText(
  graphqlPaymentCommands,
  'checkout_execution_admission_port(db.clone()),',
  'graphql payment command runtime admission wiring',
);

// The park stamps the new generation before the new level becomes visible, so a
// claim decided under the previous generation fails its own conditional write.
requireText(
  journal,
  'async fn invalidate_provider_execution_admitted_by<C>(',
  'park-time claim invalidation',
);
requireText(
  journal,
  '.stamp_admission_epoch(',
  'park-time generation stamp call',
);
requireText(
  journal,
  'self.invalidate_provider_execution_admitted_by(txn, operation)',
  'invalidation inside the admission publication',
);

// --- trigger cutover -------------------------------------------------------
const triggerDrop = read(
  'crates/modules/rustok-commerce/src/migrations/m20261007_000013_drop_provider_execution_checkout_guard.rs',
);
requireText(
  triggerDrop,
  'DROP TRIGGER IF EXISTS payment_provider_operations_checkout_guard',
  'provider execution guard removal',
);
requireText(
  triggerDrop,
  'DROP FUNCTION IF EXISTS block_provider_execution_during_checkout_compensation();',
  'provider execution guard function removal',
);
requireText(
  triggerDrop,
  'CREATE TRIGGER payment_provider_operations_checkout_guard',
  'guard restoration in down()',
);
const commerceMigrationsMod = read('crates/modules/rustok-commerce/src/migrations/mod.rs');
requireText(
  commerceMigrationsMod,
  'm20261007_000013_drop_provider_execution_checkout_guard::Migration,',
  'trigger drop migration registration',
);
requireText(
  commerceMigrationsMod,
  '"m20261007_000013_drop_provider_execution_checkout_guard",',
  'trigger drop migration dependency',
);

// --- refusal metric --------------------------------------------------------
const telemetry = read('crates/libs/rustok-telemetry/src/metrics.rs');
requireText(
  telemetry,
  '"rustok_payment_provider_execution_admission_refused_total"',
  'refusal metric name',
);
requireText(
  telemetry,
  '&["operation", "reason"],',
  'bounded refusal metric labels',
);
requireText(
  telemetry,
  'registry.register(Box::new(PROVIDER_EXECUTION_ADMISSION_REFUSED_TOTAL.clone()))?;',
  'refusal metric registration',
);
requireText(
  telemetry,
  'pub fn record_provider_execution_admission_refused(',
  'refusal metric recorder',
);

if (failures.length > 0) {
  console.error('checkout execution admission contract verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log('checkout execution admission contract verification passed');
