#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function readJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON (${error.message})`);
    return {};
  }
}

function requireMarker(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function forbidMarker(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const errorPath = "crates/rustok-payment/src/error.rs";
const registrySourcePath = "crates/rustok-payment/src/providers.rs";
const journalPath = "crates/rustok-payment/src/services/provider_operation.rs";
const orchestrationPath =
  "crates/rustok-commerce/src/services/journaled_payment_provider.rs";
const refundReconciliationPath =
  "crates/rustok-commerce/src/services/refund_reconciliation.rs";
const stripePath = "crates/rustok-payment/src/stripe_provider.rs";
const migrationPath =
  "crates/rustok-payment/src/migrations/m20260714_000120_allow_uncertain_provider_outcomes.rs";
const migrationRegistryPath = "crates/rustok-payment/src/migrations/mod.rs";
const integrationTestPath =
  "crates/rustok-migrations/tests/payment_provider_operation_uncertain_outcome.rs";
const registryPath = "crates/rustok-payment/contracts/payment-fba-registry.json";
const planPath = "crates/rustok-commerce/docs/implementation-plan.md";
const packagePath = "package.json";

const errorSource = read(errorPath);
const registrySource = read(registrySourcePath);
const journalSource = read(journalPath);
const orchestration = read(orchestrationPath);
const refundReconciliation = read(refundReconciliationPath);
const stripe = read(stripePath);
const migration = read(migrationPath);
const migrationRegistry = read(migrationRegistryPath);
const integrationTest = read(integrationTestPath);
const registry = readJson(registryPath);
const plan = read(planPath);
const packageJson = read(packagePath);

for (const marker of [
  "ProviderUnavailable",
  "ProviderRejected",
  "ProviderInvalidResponse",
  "ProviderOutcomeUnknown",
  "ProviderConfiguration",
  "requires_provider_reconciliation",
  "Self::ProviderOutcomeUnknown { .. } | Self::ProviderInvalidResponse { .. }",
  "is_provider_retryable",
]) {
  requireMarker(errorSource, marker, `${errorPath}: missing ${marker}`);
}
for (const marker of [
  "PaymentError::provider_configuration(provider_id)",
  "PaymentError::provider_unavailable(provider_id, operation)",
  "PaymentError::provider_invalid_response",
]) {
  requireMarker(registrySource, marker, `${registrySourcePath}: missing ${marker}`);
}
for (const marker of [
  "PROVIDER_OPERATION_EXECUTING",
  "PROVIDER_OPERATION_RECONCILIATION_REQUIRED",
  "executing_outcome_requires_reconciliation",
]) {
  requireMarker(journalSource, marker, `${journalPath}: missing ${marker}`);
}
requireMarker(
  migration,
  "executing',\n                                'reconciliation_required'",
  `${migrationPath}: executing must transition to reconciliation_required`,
);
requireMarker(
  migrationRegistry,
  "m20260714_000120_allow_uncertain_provider_outcomes",
  `${migrationRegistryPath}: migration 000120 is not registered`,
);
for (const marker of [
  "uncertain_executing_provider_operation_requires_reconciliation_without_reclaim",
  "mark_reconciliation_required",
  "second_claim.is_none()",
  "mark_committed",
  "provider_completed_at.is_some()",
]) {
  requireMarker(integrationTest, marker, `${integrationTestPath}: missing ${marker}`);
}
for (const marker of [
  "source.requires_provider_reconciliation()",
  "mark_reconciliation_required",
  "ProviderOutcomeUnknown",
  "unresolved_reconciliation_does_not_reexecute_provider",
]) {
  requireMarker(orchestration, marker, `${orchestrationPath}: missing ${marker}`);
}
requireMarker(
  refundReconciliation,
  "operation.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED",
  `${refundReconciliationPath}: reconciliation-required refund must not be replayed`,
);
forbidMarker(
  refundReconciliation,
  "PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED",
  `${refundReconciliationPath}: unknown refund outcome must not be treated as provider success`,
);
for (const marker of [
  "provider_unavailable",
  "provider_rejected",
  "provider_invalid_response",
  "provider_outcome_unknown",
  "provider_configuration",
  "request_timeout_seconds",
]) {
  requireMarker(stripe, marker, `${stripePath}: missing ${marker}`);
}

const taxonomy = registry.provider_spi?.error_taxonomy;
for (const key of [
  "provider_unavailable",
  "provider_rejected",
  "provider_configuration",
  "provider_invalid_response",
  "provider_outcome_unknown",
]) {
  if (!taxonomy?.[key]) failures.push(`${registryPath}: missing error taxonomy ${key}`);
}
if (taxonomy?.provider_unavailable?.retryable !== true) {
  failures.push(`${registryPath}: provider_unavailable must be retryable`);
}
for (const key of ["provider_invalid_response", "provider_outcome_unknown"]) {
  if (taxonomy?.[key]?.reconciliation_required !== true) {
    failures.push(`${registryPath}: ${key} must require reconciliation`);
  }
}
if (
  registry.provider_spi?.outcome_policy?.automatic_reexecution_from_reconciliation !== false ||
  registry.provider_spi?.outcome_policy?.uncertain_transition !==
    "executing_to_reconciliation_required"
) {
  failures.push(`${registryPath}: provider outcome policy drift`);
}

requireMarker(
  plan,
  "verify-payment-provider-outcome-contract.mjs",
  `${planPath}: provider outcome verifier must be listed`,
);
requireMarker(
  packageJson,
  '"verify:payment:provider-outcomes"',
  `${packagePath}: provider outcome verifier script is not registered`,
);
requireMarker(
  packageJson,
  "npm run verify:payment:provider-outcomes",
  `${packagePath}: aggregate verification must run provider outcome guard`,
);

if (failures.length > 0) {
  console.error("payment provider outcome verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("payment provider outcome verification passed");
