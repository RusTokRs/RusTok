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
const requireCount = (content, value, expected, label) => {
  const count = content.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
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

for (const marker of [
  "struct OrderCheckoutContextFacts",
  "fn order_checkout_context_facts(",
  "tenant_id_length: context.tenant_id.chars().count()",
  "actor_id_length: context.actor.id.chars().count()",
  "claim_count: context.claims.len()",
  "role_count: context.roles.len()",
  "channel_present: context.channel.is_some()",
  "locale_length: context.locale.chars().count()",
  "causation_id_present: context.causation_id.is_some()",
  "traceparent_present: context.traceparent.is_some()",
  "idempotency_key_present: context.idempotency_key.is_some()",
]) requireText(shared, marker, `${paths.shared}: shared context shape`);

const mapper = functionBody(shared, "map_checkout_order_payment_settlement_local_port_error");
const facts = functionBody(shared, "order_checkout_port_error_facts");
for (const [variant, label] of [
  ["Validation", "validation"],
  ["NotFound", "not_found"],
  ["Conflict", "conflict"],
  ["Forbidden", "forbidden"],
  ["Unavailable", "unavailable"],
  ["Timeout", "timeout"],
  ["InvariantViolation", "invariant_violation"],
]) requireText(facts, `PortErrorKind::${variant} => "${label}"`, `${paths.shared}: ${variant}`);
for (const marker of [
  "message_present: !error.message.trim().is_empty()",
  "message_length: error.message.chars().count()",
  "let error_facts = order_checkout_port_error_facts(&error);",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "\n    error\n}",
]) requireText(`${facts}\n${mapper}`, marker, `${paths.shared}: bounded settlement mapper`);
requireCount(mapper, "error_message_present = error_facts.message_present", 2, "two mapper presence fields");
for (const forbidden of [
  "error = ?error",
  "internal_message",
  "error_kind = ?error.kind",
  "match (error.code.as_str(), error.message.as_str())",
]) forbidText(mapper, forbidden, `${paths.shared}: settlement mapper complete payload`);

for (const openMarker of [
  "fn log_order_checkout_admission_rejection(",
  "fn log_order_checkout_context_rejection(",
  "parse_cause = ?evidence.parse_cause",
  "internal_message = %error.message",
  "error_kind = ?error.kind",
  "error = ?error",
]) requireText(shared, openMarker, `${paths.shared}: retained shared admission/context gap`);

for (const [content, label] of [
  [shared, paths.shared],
  [compensationLocal, paths.compensationLocal],
]) {
  for (const forbidden of [
    "tenant_id = %context.tenant_id",
    "actor = ?context.actor",
    "channel = ?context.channel",
    "locale = %context.locale",
    "causation_id = ?context.causation_id",
    "traceparent = ?context.traceparent",
    "idempotency_key = ?context.idempotency_key",
  ]) forbidText(content, forbidden, `${label}: raw context value`);
}

for (const marker of [
  "fn validate_request(",
  'order_error_to_port_error(&context, "mark_checkout_order_paid", error)',
  '"order.checkout_payment_identity_missing"',
  '"order.checkout_payment_identity_conflict"',
  '"order.checkout_payment_state_conflict"',
  '"order.checkout_payment_reference_conflict"',
]) requireText(settlement, marker, `${paths.settlement}: owner behavior preserved`);
for (const marker of [
  "fn validate_identity(",
  "fn manual_reconciliation(",
  '"read_checkout_order_for_compensation"',
  '"cancel_checkout_order"',
]) requireText(compensation, marker, `${paths.compensation}: compensation preserved`);

for (const [key, expected] of Object.entries({
  local_mapper_payload_diagnostic_cleanup_closed: true,
  shared_admission_context_payload_diagnostic_cleanup_closed: false,
  canonical_owner_payload_diagnostic_cleanup_closed: false,
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
  "Status: **partial source-ready / unvalidated**",
  "The shared admission/context diagnostic payload itself is not yet closed",
  "Both severity branches now retain only static `PortErrorKind`",
  "Canonical payment-settlement owner payload diagnostics remain a separate open slice.",
  "The compensation local wrapper and canonical owner payload-diagnostic sites are",
]) requireText(doc, marker, `${paths.doc}: truthful source status`);

if (failures.length > 0) {
  console.error("Order checkout owner-context diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order checkout wrappers preserve admission and delegation order; settlement local outcomes use bounded PortError shape while shared admission/context and settlement-owner payload cleanup remain open",
);
