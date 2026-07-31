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

const paths = {
  shared: "crates/rustok-order/src/checkout_owner_context.rs",
  settlement: "crates/rustok-order/src/checkout_payment_settlement.rs",
  compensation: "crates/rustok-order/src/checkout_compensation.rs",
  compensationLocal: "crates/rustok-order/src/checkout_compensation_local_context.rs",
  lib: "crates/rustok-order/src/lib.rs",
  doc: "crates/rustok-order/docs/checkout-owner-context.md",
  settlementEvidence:
    "crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source.json",
  compensationEvidence:
    "crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json",
};

const shared = read(paths.shared);
const settlement = read(paths.settlement);
const compensation = read(paths.compensation);
const compensationLocal = read(paths.compensationLocal);
const lib = read(paths.lib);
const doc = read(paths.doc);
const settlementEvidence = JSON.parse(read(paths.settlementEvidence));
const compensationEvidence = JSON.parse(read(paths.compensationEvidence));

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function functionBody(content, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(content);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = content.indexOf("{", match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < content.length; index += 1) {
    if (content[index] === "{") depth += 1;
    if (content[index] === "}") {
      depth -= 1;
      if (depth === 0) return content.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return "";
}

for (const marker of [
  "mod checkout_compensation;",
  "mod checkout_payment_settlement;",
  "mod checkout_compensation_local_context;",
  '#[path = "checkout_owner_context.rs"]',
  "mod checkout_owner_context_impl;",
  "pub use checkout_compensation_local_context::{",
  "pub use checkout_owner_context_impl::{",
]) requireText(lib, marker, `${paths.lib}: public checkout facade`);

for (const operation of ["compensate_checkout_order", "settle_checkout_payment"]) {
  const body = functionBody(shared, operation);
  const markers = [
    "require_order_checkout_write_admission(",
    "parse_order_tenant_id(",
    "parse_order_actor_id(",
    "require_order_checkout_causation(",
    operation === "compensate_checkout_order"
      ? "self.inner.compensate_checkout_order(context, request).await"
      : "self.inner.settle_checkout_payment(context, request).await",
  ];
  const positions = markers.map((marker) => body.indexOf(marker));
  if (!positions.every((value, index) => value >= 0 && (index === 0 || positions[index - 1] < value))) {
    failures.push(`${paths.shared}: ${operation} admission/delegation order changed`);
  }
}

const localMapper = functionBody(shared, "map_checkout_order_payment_settlement_local_port_error");
for (const marker of [
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "\n    error\n}",
]) requireText(localMapper, marker, `${paths.shared}: bounded settlement mapper`);
for (const forbidden of ["error = ?error", "internal_message", "error_kind = ?error.kind"]) {
  forbidText(localMapper, forbidden, `${paths.shared}: complete settlement wrapper payload`);
}

const admissionLogger = functionBody(shared, "log_order_checkout_admission_rejection");
for (const marker of [
  "let technical_failure = matches!(",
  "let error_facts = order_checkout_port_error_facts(error);",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "tracing::error!(",
  "tracing::warn!(",
]) requireText(admissionLogger, marker, `${paths.shared}: bounded shared admission diagnostics`);
for (const forbidden of ["error = ?error", "internal_message", "error_kind = ?error.kind"]) {
  forbidText(admissionLogger, forbidden, `${paths.shared}: complete shared admission payload`);
}

const tenantParser = functionBody(shared, "parse_order_tenant_id");
const actorParser = functionBody(shared, "parse_order_actor_id");
const causationValidator = functionBody(shared, "require_order_checkout_causation");
for (const [body, label] of [
  [tenantParser, "tenant parser"],
  [actorParser, "actor parser"],
]) {
  requireText(body, "map_err(|_|", `${paths.shared}: ${label}`);
  requireText(body, "parse_failed: true", `${paths.shared}: ${label}`);
  forbidText(body, "|cause|", `${paths.shared}: ${label} parser payload`);
}
for (const marker of [
  "let (context_operation, parse_failed) =",
  "Err(_) => (None, true)",
  "parse_failed,",
]) requireText(causationValidator, marker, `${paths.shared}: bounded causation validation`);

const contextLogger = functionBody(shared, "log_order_checkout_context_rejection");
for (const marker of [
  "let error_facts = order_checkout_port_error_facts(error);",
  "parse_failed = evidence.parse_failed",
  "expected_checkout_operation_id_present",
  "expected_checkout_operation_id_non_nil",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
]) requireText(contextLogger, marker, `${paths.shared}: bounded shared context diagnostics`);
for (const forbidden of [
  "parse_cause",
  "error = ?error",
  "internal_message",
  "error_kind = ?error.kind",
]) forbidText(contextLogger, forbidden, `${paths.shared}: complete shared context payload`);
for (const forbidden of [
  "parse_cause:",
  "dyn std::fmt::Debug",
]) forbidText(shared, forbidden, `${paths.shared}: parser payload type`);

for (const marker of [
  "struct OrderPaymentSettlementOwnerErrorFacts",
  "fn order_payment_settlement_order_error_facts(",
  "fn order_payment_settlement_status_kind(",
  "parse_failed = true",
  "error_variant = error_facts.error_variant",
  "opaque_payload_present = error_facts.opaque_payload_present",
  "let error_facts = order_payment_settlement_order_error_facts(&error);",
]) requireText(settlement, marker, `${paths.settlement}: closed owner diagnostics`);
for (const forbidden of [
  "fn log_context_parse_rejection<E: std::fmt::Debug>",
  "fn log_order_payment_owner_error<E: std::fmt::Debug>",
  "order_state = ?order_state",
  "error = ?error",
  "from = ?from",
  "to = ?to",
]) forbidText(settlement, forbidden, `${paths.settlement}: complete owner payload`);

for (const marker of [
  "fn validate_identity(",
  "fn manual_reconciliation(",
  '"read_checkout_order_for_compensation"',
  '"cancel_checkout_order"',
]) requireText(compensation, marker, `${paths.compensation}: compensation owner preserved`);
for (const forbidden of ["error = ?error", "internal_message = %error.message"]) {
  forbidText(compensationLocal, forbidden, `${paths.compensationLocal}: compensation wrapper payload`);
}

for (const [key, expected] of Object.entries({
  local_mapper_payload_diagnostic_cleanup_closed: true,
  shared_admission_context_payload_diagnostic_cleanup_closed: true,
  canonical_owner_payload_diagnostic_cleanup_closed: true,
  complete_port_error_logged_by_shared_admission: false,
  port_error_message_text_logged_by_shared_admission: false,
  uuid_parse_error_payload_logged_by_shared_context: false,
  complete_port_error_logged_by_shared_context: false,
  shared_admission_static_port_error_kind_logged: true,
  shared_context_static_parse_failure_logged: true,
  write_admission_order_changed: false,
  public_code_changed: false,
  public_message_changed: false,
})) {
  if (settlementEvidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.settlementEvidence}: source_contract.${key} must be ${expected}`);
  }
}
if (compensationEvidence.source_contract?.checkout_order_compensation_payload_diagnostic_cleanup_closed !== true) {
  failures.push(`${paths.compensationEvidence}: compensation payload cleanup must remain closed`);
}
if (settlementEvidence.validation?.compile_proven !== false) {
  failures.push(`${paths.settlementEvidence}: compile_proven must remain false`);
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "The shared admission/context diagnostic payload is source-closed",
  "The payment-settlement post-delegation mapper and canonical owner payload-diagnostic",
  "No shared checkout wrapper payload-diagnostic gap remains",
]) requireText(doc, marker, `${paths.doc}: truthful source status`);

if (failures.length > 0) {
  console.error("Order checkout owner-context diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order checkout wrappers preserve admission and delegation order; shared admission/context, settlement, and compensation payload diagnostics are source-closed and unvalidated",
);
