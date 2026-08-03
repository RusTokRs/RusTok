#!/usr/bin/env node
// Commerce admin order-change native transport diagnostic-safety source guard.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

function functionBody(text, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(text);
  if (!match) {
    fail(`missing function ${functionName}`);
    return "";
  }

  const openBrace = text.indexOf("{", match.index);
  if (openBrace === -1) {
    fail(`missing body for function ${functionName}`);
    return "";
  }

  let depth = 0;
  for (let index = openBrace; index < text.length; index += 1) {
    if (text[index] === "{") depth += 1;
    if (text[index] === "}") {
      depth -= 1;
      if (depth === 0) return text.slice(openBrace, index + 1);
    }
  }

  fail(`unterminated body for function ${functionName}`);
  return "";
}

const adapterPath =
  "crates/rustok-commerce/admin/src/transport/native_server_adapter_ssr.rs";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/admin-order-change-native-error-safety-source.json";
const reviewPath =
  "crates/rustok-commerce/contracts/evidence/admin-order-change-native-error-safety-source-review.json";
const docPath =
  "crates/rustok-commerce/docs/admin-order-change-native-error-safety.md";

const adapter = readRepo(adapterPath);
const evidence = JSON.parse(readRepo(evidencePath));
const review = JSON.parse(readRepo(reviewPath));
const doc = readRepo(docPath);

for (const endpoint of [
  'endpoint = "commerce/admin/order-changes"',
  'endpoint = "commerce/admin/apply-order-change"',
  'endpoint = "commerce/admin/cancel-order-change"',
]) {
  assertContains(adapter, endpoint, `${adapterPath}: missing mounted endpoint ${endpoint}`);
}

for (const marker of [
  "const COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER",
  "const COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY",
  "fn order_change_correlation_id(",
  "uuid::Uuid::new_v4()",
  "struct OrderChangeRequestContextFacts",
  "fn order_change_request_context_facts(",
  "struct OrderChangeOwnerErrorFacts",
  "fn order_change_owner_error_facts(",
  "fn order_change_context_error<E>(",
  "fn order_change_auth_context_error<E>(",
  "fn order_change_tenant_context_error<E>(",
  "optional_order_change_request_context",
  "struct OrderChangeOwnerErrorContext",
  "fn order_change_owner_error(",
  "error: rustok_order::error::OrderError",
  "Commerce order-change runtime is temporarily unavailable",
]) {
  assertContains(adapter, marker, `${adapterPath}: missing order-change safety marker ${marker}`);
}

for (const obsolete of [
  "fn order_change_context_error<E: std::fmt::Debug>(",
  "fn order_change_auth_context_error<E: std::fmt::Debug>(",
  "fn order_change_tenant_context_error<E: std::fmt::Debug>(",
]) {
  assertNotContains(
    adapter,
    obsolete,
    `${adapterPath}: type-only context helper must not require Debug: ${obsolete}`,
  );
}

for (const [variant, publicMessage] of [
  ["OrderError::Validation(_)", "Order change request is invalid"],
  ["OrderError::OrderNotFound(_)", "Order resource was not found"],
  ["OrderError::OrderReturnNotFound(_)", "Order resource was not found"],
  ["OrderError::OrderChangeNotFound(_)", "Order resource was not found"],
  ["OrderError::InvalidTransition { .. }", "Order change conflicts with the current order state"],
  ["OrderError::Database(_)", "Order storage is temporarily unavailable"],
  ["OrderError::Core(_)", "Order change could not be completed safely"],
]) {
  assertContains(adapter, variant, `${adapterPath}: typed owner mapping must cover ${variant}`);
  assertContains(adapter, publicMessage, `${adapterPath}: missing static public message ${publicMessage}`);
}

const contextErrorBody = functionBody(adapter, "order_change_context_error");
assertContains(
  contextErrorBody,
  "let error_type = std::any::type_name::<E>();",
  `${adapterPath}: framework context diagnostics must retain type only`,
);
assertContains(contextErrorBody, "error_type", `${adapterPath}: framework error type must be logged`);
for (const forbidden of ["error = ?error", "error = %error", "error = ?_error", "error = %_error"]) {
  assertNotContains(
    contextErrorBody,
    forbidden,
    `${adapterPath}: framework context diagnostics must not log complete errors`,
  );
}

const optionalContextBody = functionBody(adapter, "optional_order_change_request_context");
assertContains(
  optionalContextBody,
  "let error_type = std::any::type_name_of_val(&error);",
  `${adapterPath}: optional request-context failure must retain type only`,
);
for (const forbidden of ["error = ?error", "error = %error"]) {
  assertNotContains(
    optionalContextBody,
    forbidden,
    `${adapterPath}: optional request-context diagnostics must not log framework text`,
  );
}

const runtimeBody = functionBody(adapter, "order_service_from_context");
for (const marker of [
  "order_change_request_context_facts(request_context)",
  'code = "commerce.admin_order_change_runtime_unavailable"',
  'ServerFnError::new("Commerce order-change runtime is temporarily unavailable")',
  "tenant_id_non_nil = !tenant.id.is_nil()",
  "actor_id_non_nil = !auth.user_id.is_nil()",
  "request_context_present = request_facts.request_context_present",
  "request_tenant_id_non_nil = ?request_facts.request_tenant_id_non_nil",
  "request_user_id_present = request_facts.request_user_id_present",
  "request_user_id_non_nil = ?request_facts.request_user_id_non_nil",
  "channel_id_present = request_facts.channel_id_present",
  "channel_id_non_nil = ?request_facts.channel_id_non_nil",
  "channel_slug_present = request_facts.channel_slug_present",
  "channel_slug_length = ?request_facts.channel_slug_length",
  "locale_present = request_facts.locale_present",
  "locale_length = ?request_facts.locale_length",
]) {
  assertContains(runtimeBody, marker, `${adapterPath}: runtime mapper missing ${marker}`);
}
for (const forbidden of [
  "tenant_id = %tenant.id",
  "actor_id = %auth.user_id",
  "request_tenant_id = ?request_tenant_id",
  "request_user_id = ?request_user_id",
  "channel_id = ?channel_id",
  "channel_slug = ?channel_slug",
  "locale = ?locale",
  "Commerce admin requires TransactionalEventBus in host runtime context",
]) {
  assertNotContains(runtimeBody, forbidden, `${adapterPath}: runtime diagnostics contain ${forbidden}`);
}

const factsBody = functionBody(adapter, "order_change_owner_error_facts");
for (const marker of [
  "OrderError::Validation(detail)",
  "validation_detail_length: Some(detail.chars().count())",
  "OrderError::OrderNotFound(id)",
  "OrderError::OrderReturnNotFound(id)",
  "OrderError::OrderChangeNotFound(id)",
  "resource_id_non_nil: Some(!id.is_nil())",
  "OrderError::InvalidTransition { from, to }",
  "transition_from_length: Some(from.chars().count())",
  "transition_to_length: Some(to.chars().count())",
  "OrderError::Database(_)",
  "database_cause_present: true",
  "OrderError::Core(_)",
  "core_cause_present: true",
]) {
  assertContains(factsBody, marker, `${adapterPath}: owner error facts missing ${marker}`);
}

const ownerBody = functionBody(adapter, "order_change_owner_error");
for (const marker of [
  "order_change_request_context_facts(context.request_context)",
  "order_change_owner_error_facts(&error)",
  "order_id_present",
  "order_id_non_nil",
  "order_change_id_present",
  "order_change_id_non_nil",
  "tenant_id_non_nil = !context.tenant.id.is_nil()",
  "actor_id_non_nil = !context.auth.user_id.is_nil()",
  "request_context_present = request_facts.request_context_present",
  "request_tenant_id_non_nil = ?request_facts.request_tenant_id_non_nil",
  "request_user_id_present = request_facts.request_user_id_present",
  "request_user_id_non_nil = ?request_facts.request_user_id_non_nil",
  "channel_id_present = request_facts.channel_id_present",
  "channel_id_non_nil = ?request_facts.channel_id_non_nil",
  "channel_slug_present = request_facts.channel_slug_present",
  "channel_slug_length = ?request_facts.channel_slug_length",
  "locale_present = request_facts.locale_present",
  "locale_length = ?request_facts.locale_length",
  "validation_detail_present = error_facts.validation_detail_present",
  "validation_detail_length = ?error_facts.validation_detail_length",
  "resource_id_present = error_facts.resource_id_present",
  "resource_id_non_nil = ?error_facts.resource_id_non_nil",
  "transition_from_length = ?error_facts.transition_from_length",
  "transition_to_length = ?error_facts.transition_to_length",
  "database_cause_present = error_facts.database_cause_present",
  "core_cause_present = error_facts.core_cause_present",
  "tracing::error!",
  "tracing::warn!",
  "ServerFnError::new(public_message)",
]) {
  assertContains(ownerBody, marker, `${adapterPath}: owner mapper missing ${marker}`);
}
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "tenant_id = %context.tenant.id",
  "actor_id = %context.auth.user_id",
  "order_id = ?context.order_id",
  "order_change_id = ?context.order_change_id",
  "request_tenant_id = ?request_tenant_id",
  "request_user_id = ?request_user_id",
  "channel_id = ?channel_id",
  "channel_slug = ?channel_slug",
  "locale = ?locale",
]) {
  assertNotContains(ownerBody, forbidden, `${adapterPath}: owner diagnostics contain ${forbidden}`);
}

for (const functionName of [
  "commerce_admin_order_changes_native",
  "commerce_admin_apply_order_change_native",
  "commerce_admin_cancel_order_change_native",
]) {
  const body = functionBody(adapter, functionName);
  assertContains(body, "order_change_correlation_id", `${adapterPath}: ${functionName} needs a per-call correlation id`);
  assertContains(body, "order_change_auth_context_error", `${adapterPath}: ${functionName} must sanitize auth extraction`);
  assertContains(body, "order_change_tenant_context_error", `${adapterPath}: ${functionName} must sanitize tenant extraction`);
  assertContains(body, "optional_order_change_request_context", `${adapterPath}: ${functionName} must capture optional request attribution`);
  assertNotContains(body, ".map_err(ServerFnError::new)", `${adapterPath}: ${functionName} must not publish raw framework extraction errors`);
}

for (const functionName of [
  "fetch_order_changes_native_with_context",
  "apply_order_change_native_with_context",
  "cancel_order_change_native_with_context",
]) {
  const body = functionBody(adapter, functionName);
  assertContains(body, "order_service_from_context(", `${adapterPath}: ${functionName} must use the safe runtime mapper`);
  assertContains(body, "order_change_owner_error(", `${adapterPath}: ${functionName} must map typed owner errors`);
  assertNotContains(body, ".map_err(ServerFnError::new)", `${adapterPath}: ${functionName} must not publish raw owner errors`);
}

const ownerMapperUses =
  adapter.match(/order_change_owner_error\(\s+OrderChangeOwnerErrorContext\s*\{/g) ?? [];
if (ownerMapperUses.length !== 3) {
  fail(`${adapterPath}: expected three typed order-change owner mapper callsites, found ${ownerMapperUses.length}`);
}

for (const marker of [
  "struct PromotionRequestContextFacts",
  "fn promotion_request_context_facts(",
  "fn promotion_context_error",
  "fn promotion_port_error(",
  "ServerFnError::new(error.message)",
]) {
  assertContains(adapter, marker, `${adapterPath}: prior promotion safety marker must remain ${marker}`);
}

for (const forbidden of [
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
  "ServerFnError::new(other.to_string())",
  ".map_err(ServerFnError::new)",
]) {
  const orderSlice = adapter.slice(
    adapter.indexOf("// Order-change native boundary"),
    adapter.indexOf("// Shared response mapping"),
  );
  assertNotContains(orderSlice, forbidden, `${adapterPath}: unsafe order-change public conversion ${forbidden}`);
}

if (evidence.status !== "commerce_admin_order_change_native_error_safety_source_unvalidated") {
  fail(`${evidencePath}: source evidence must remain explicitly unvalidated`);
}
for (const [field, expected] of Object.entries({
  framework_error_type_only: true,
  framework_debug_bounds_removed: true,
  complete_framework_error_logged: false,
  runtime_identity_shape_only: true,
  owner_error_shape_only: true,
  complete_order_error_logged: false,
  raw_identity_values_logged: false,
  typed_order_errors_use_static_public_envelopes: true,
  promotion_safety_contract_is_preserved: true,
})) {
  if (evidence.source_claims?.[field] !== expected) {
    fail(`${evidencePath}: source_claims.${field} must be ${expected}`);
  }
}
for (const field of [
  "focused_verifier_executed",
  "aggregate_verifier_executed",
  "cargo_check_executed",
  "tests_executed",
  "runtime_failure_injection_executed",
  "mounted_http_parity_executed",
  "ci_executed",
]) {
  if (evidence.validation?.[field] !== false) {
    fail(`${evidencePath}: validation.${field} must remain false until execution evidence exists`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  fail(`${evidencePath}: execution must remain empty`);
}

if (review.status !== "commerce_admin_order_change_native_error_safety_source_reviewed_unvalidated") {
  fail(`${reviewPath}: source review must remain explicitly unvalidated`);
}
for (const field of [
  "framework_error_text_removed",
  "framework_debug_bounds_removed",
  "runtime_identity_values_removed",
  "owner_error_text_removed",
  "owner_identity_values_removed",
  "owner_error_shape_reviewed",
  "public_envelopes_preserved",
  "promotion_native_safety_markers_preserved",
]) {
  if (review.review?.[field] !== true) {
    fail(`${reviewPath}: review.${field} must be true`);
  }
}

assertContains(doc, "Status: `source-complete / unvalidated`", `${docPath}: status must remain unvalidated`);
assertContains(doc, "complete framework extraction errors are not logged", `${docPath}: framework diagnostic policy`);
assertContains(doc, "no longer require a `Debug`", `${docPath}: type-only helper contract`);
assertContains(doc, "complete `OrderError` and identity values are not logged", `${docPath}: owner diagnostic policy`);

if (failures.length > 0) {
  console.error("Commerce admin order-change native diagnostic-safety check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Commerce admin order-change native diagnostics use correlation-safe type/shape only; execution evidence remains open");
