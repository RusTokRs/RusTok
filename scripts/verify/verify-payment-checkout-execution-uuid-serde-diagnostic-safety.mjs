#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

const paths = {
  source: "crates/rustok-payment/src/checkout_execution/validation_errors.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-uuid-serde-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-uuid-serde-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const persisted = functionBody(source, "persisted_provider_result");
for (const marker of [
  "operation.status == PROVIDER_OPERATION_EXECUTING",
  "PROVIDER_OPERATION_COMMITTED",
  "PROVIDER_OPERATION_SUCCEEDED",
  "PROVIDER_OPERATION_RECONCILIATION_REQUIRED",
  "operation.provider_result.clone().ok_or_else(||",
  '"payment provider operation has no normalized durable result"',
  'Value::Null => ("null", None)',
  'Value::Bool(_) => ("bool", None)',
  'Value::Number(_) => ("number", None)',
  'Value::String(_) => ("string", None)',
  'Value::Array(items) => ("array", Some(items.len()))',
  'Value::Object(fields) => ("object", Some(fields.len()))',
  "serde_json::from_value(value).map(Some).map_err(|_|",
  "provider_result_decode_failed = true",
  "provider_result_kind",
  "provider_result_collection_length = ?provider_result_collection_length",
  'code = "payment.provider_invalid_response"',
  '"payment provider operation result is malformed"',
  "manual_reconciliation(",
]) {
  requireText(persisted, marker, `${paths.source}: persisted provider result`);
}
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "error.to_string()",
  "provider_result = ?value",
  "provider_result = %value",
  "value = ?value",
  "value = %value",
]) {
  forbidText(persisted, forbidden, `${paths.source}: serde payload diagnostics`);
}

const tenant = functionBody(source, "parse_tenant_id");
for (const marker of [
  "Uuid::parse_str(&context.tenant_id).map_err(|_|",
  "tenant_id_parse_failed = true",
  "tenant_id_length = context_facts.tenant_id_length",
  "correlation_id = %context.correlation_id",
  "operation = owner_operation",
  'code = "payment.tenant_id_invalid"',
  '"payment checkout execution tenant context is invalid"',
  "PortError::validation(",
  '"payment.tenant_id_invalid"',
  '"payment request context is invalid"',
]) {
  requireText(tenant, marker, `${paths.source}: tenant UUID parsing`);
}
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "error.to_string()",
  "tenant_id = %context.tenant_id",
  "tenant_id = ?context.tenant_id",
]) {
  forbidText(tenant, forbidden, `${paths.source}: UUID payload diagnostics`);
}

const reconciliation = functionBody(source, "manual_reconciliation");
for (const marker of [
  'code = "payment.checkout_execution_manual_reconciliation"',
  "PortError::new(",
  "PortErrorKind::Conflict",
  '"payment checkout execution requires manual reconciliation"',
  "false,",
]) {
  requireText(reconciliation, marker, `${paths.source}: preserved reconciliation envelope`);
}

if (
  evidence.status !==
  "payment_checkout_execution_uuid_serde_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  uuid_diagnostic_site_count: 1,
  serde_diagnostic_site_count: 1,
  complete_uuid_error_logged: false,
  complete_serde_error_logged: false,
  tenant_id_text_logged: false,
  provider_result_payload_logged: false,
  tenant_id_length_logged: true,
  tenant_parse_failure_fact_logged: true,
  provider_result_decode_failure_fact_logged: true,
  provider_result_kind_logged: true,
  provider_result_collection_length_logged: true,
  tenant_validation_mapping_preserved: true,
  persisted_provider_result_mapping_preserved: true,
  manual_reconciliation_routing_preserved: true,
  public_port_error_contract_changed: false,
  payment_lifecycle_changed: false,
  provider_execution_changed: false,
  manual_reconciliation_reason_changed: false,
  local_persistence_diagnostics_changed: false,
  provider_checkpoint_diagnostics_changed: false,
  remaining_payment_execution_diagnostics_open: true,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "compile_proven",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(
  doc,
  "Neither event records parser error text, tenant ID text, or provider-result payload.",
  `${paths.doc}: diagnostic policy`,
);
requireText(
  doc,
  "Remaining payment execution diagnostics",
  `${paths.doc}: remaining work`,
);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout execution UUID/serde diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution UUID and serde diagnostics retain only parser failure and input-shape facts; execution evidence remains open",
);
