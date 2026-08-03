#!/usr/bin/env node
// Commerce admin cart-promotion native transport error-safety source guard.

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

const routingPath = "crates/rustok-commerce/admin/src/transport/mod.rs";
const safeAdapterPath =
  "crates/rustok-commerce/admin/src/transport/native_server_adapter_ssr.rs";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/admin-promotion-native-error-safety-source.json";
const reviewPath =
  "crates/rustok-commerce/contracts/evidence/admin-promotion-native-error-safety-source-review.json";
const docPath =
  "crates/rustok-commerce/docs/admin-promotion-native-error-safety.md";

const routing = readRepo(routingPath);
const safeAdapter = readRepo(safeAdapterPath);
const evidence = JSON.parse(readRepo(evidencePath));
const review = JSON.parse(readRepo(reviewPath));
const doc = readRepo(docPath);

assertContains(
  routing,
  '#[cfg(not(feature = "ssr"))]\nmod native_server_adapter;',
  `${routingPath}: client/hydrate server-function contract must remain on the canonical adapter`,
);
assertContains(
  routing,
  '#[cfg(feature = "ssr")]\n#[path = "native_server_adapter_ssr.rs"]\nmod native_server_adapter;',
  `${routingPath}: SSR must route through the safe promotion adapter`,
);

for (const endpoint of [
  'endpoint = "commerce/admin/preview-cart-promotion"',
  'endpoint = "commerce/admin/apply-cart-promotion"',
]) {
  assertContains(safeAdapter, endpoint, `${safeAdapterPath}: missing mounted endpoint ${endpoint}`);
}

for (const marker of [
  "uuid::Uuid::new_v4()",
  "struct PromotionRequestContextFacts",
  "fn promotion_request_context_facts(",
  "fn promotion_context_error<E>(",
  "fn promotion_auth_context_error<E>(",
  "fn promotion_tenant_context_error<E>(",
  "optional_promotion_request_context",
  ".map(|context| context.locale.as_str())",
  "context.with_channel(channel)",
  ".with_idempotency_key(correlation_id.to_string())",
  "fn promotion_port_error(",
  'owner = "rustok_cart.promotion"',
  "ServerFnError::new(error.message)",
]) {
  assertContains(safeAdapter, marker, `${safeAdapterPath}: missing preserved or safe promotion marker ${marker}`);
}

for (const obsolete of [
  "fn promotion_context_error<E: std::fmt::Debug>(",
  "fn promotion_auth_context_error<E: std::fmt::Debug>(",
  "fn promotion_tenant_context_error<E: std::fmt::Debug>(",
]) {
  assertNotContains(
    safeAdapter,
    obsolete,
    `${safeAdapterPath}: type-only promotion context helper must not require Debug: ${obsolete}`,
  );
}

const contextErrorBody = functionBody(safeAdapter, "promotion_context_error");
assertContains(
  contextErrorBody,
  "let error_type = std::any::type_name::<E>();",
  `${safeAdapterPath}: framework extraction diagnostics must retain type only`,
);
assertContains(contextErrorBody, "error_type", `${safeAdapterPath}: framework error type must be logged`);
for (const forbidden of ["error = ?error", "error = %error", "error = ?_error", "error = %_error"]) {
  assertNotContains(
    contextErrorBody,
    forbidden,
    `${safeAdapterPath}: framework extraction diagnostics must not log the complete error`,
  );
}

const optionalContextBody = functionBody(safeAdapter, "optional_promotion_request_context");
assertContains(
  optionalContextBody,
  "let error_type = std::any::type_name_of_val(&error);",
  `${safeAdapterPath}: optional request-context failure must retain type only`,
);
for (const forbidden of ["error = ?error", "error = %error"]) {
  assertNotContains(
    optionalContextBody,
    forbidden,
    `${safeAdapterPath}: optional request-context diagnostics must not log framework text`,
  );
}

const portErrorBody = functionBody(safeAdapter, "promotion_port_error");
for (const marker of [
  "promotion_request_context_facts(request_context)",
  "public_message_present",
  "public_message_length",
  "tenant_id_non_nil = !tenant.id.is_nil()",
  "actor_id_non_nil = !auth.user_id.is_nil()",
  "cart_id_non_nil = !cart_id.is_nil()",
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
  "effective_locale_length",
  "effective_channel_present",
  "public_code = %error.code",
  "error_kind = ?error.kind",
  "retryable = error.retryable",
  "PortErrorKind::Unavailable",
  "PortErrorKind::Timeout",
  "PortErrorKind::InvariantViolation",
  "tracing::error!",
  "tracing::warn!",
  "ServerFnError::new(error.message)",
]) {
  assertContains(portErrorBody, marker, `${safeAdapterPath}: promotion owner mapper missing ${marker}`);
}

for (const forbidden of [
  "error = ?error",
  "error = %error",
  "tenant_id = %tenant.id",
  "actor_id = %auth.user_id",
  "cart_id = %cart_id",
  "request_tenant_id = ?request_tenant_id",
  "request_user_id = ?request_user_id",
  "channel_id = ?channel_id",
  "channel_slug = ?channel_slug",
  "locale = ?locale",
  "public_message = %error.message",
]) {
  assertNotContains(
    portErrorBody,
    forbidden,
    `${safeAdapterPath}: promotion owner diagnostics contain raw error or identity field ${forbidden}`,
  );
}

for (const functionName of [
  "commerce_admin_preview_cart_promotion_native",
  "commerce_admin_apply_cart_promotion_native",
]) {
  const body = functionBody(safeAdapter, functionName);
  assertContains(body, "promotion_auth_context_error", `${safeAdapterPath}: ${functionName} must sanitize auth extraction`);
  assertContains(body, "promotion_tenant_context_error", `${safeAdapterPath}: ${functionName} must sanitize tenant extraction`);
  assertContains(body, "optional_promotion_request_context", `${safeAdapterPath}: ${functionName} must capture optional request context`);
  assertNotContains(body, ".map_err(ServerFnError::new)", `${safeAdapterPath}: ${functionName} must not publish raw extraction errors`);
}

for (const functionName of [
  "preview_cart_promotion_native_with_context",
  "apply_cart_promotion_native_with_context",
]) {
  const body = functionBody(safeAdapter, functionName);
  assertContains(body, "promotion_port_error", `${safeAdapterPath}: ${functionName} must use the safe owner mapper`);
  assertNotContains(body, "ServerFnError::new(error.to_string())", `${safeAdapterPath}: ${functionName} must not serialize raw owner errors`);
  assertNotContains(body, "ServerFnError::new(err.to_string())", `${safeAdapterPath}: ${functionName} must not serialize raw owner errors`);
}

assertContains(
  safeAdapter,
  "fn order_change_context_error<E>(",
  `${safeAdapterPath}: independently guarded order-change type-only context mapper must remain present`,
);
assertNotContains(
  safeAdapter,
  "fn order_change_context_error<E: std::fmt::Debug>(",
  `${safeAdapterPath}: promotion guard must not require an obsolete order-change Debug bound`,
);

if (evidence.status !== "commerce_admin_promotion_native_error_safety_source_unvalidated") {
  fail(`${evidencePath}: source evidence must remain explicitly unvalidated`);
}
for (const [field, expected] of Object.entries({
  framework_error_type_only: true,
  framework_debug_bounds_removed: true,
  complete_framework_error_logged: false,
  owner_port_error_shape_only: true,
  complete_owner_port_error_logged: false,
  raw_tenant_actor_cart_logged: false,
  raw_request_context_values_logged: false,
  public_port_error_message_preserved: true,
  order_change_type_only_context_contract_preserved: true,
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
  "runtime_trace_retained",
  "ci_executed",
]) {
  if (evidence.validation?.[field] !== false) {
    fail(`${evidencePath}: validation.${field} must remain false until execution evidence exists`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  fail(`${evidencePath}: execution must remain empty`);
}

if (review.status !== "commerce_admin_promotion_native_error_safety_source_reviewed_unvalidated") {
  fail(`${reviewPath}: source review must remain explicitly unvalidated`);
}
for (const field of [
  "framework_error_text_removed",
  "framework_debug_bounds_removed",
  "owner_port_error_text_removed",
  "identity_values_removed",
  "safe_shape_diagnostics_reviewed",
  "public_envelopes_preserved",
  "order_change_type_only_contract_preserved",
]) {
  if (review.review?.[field] !== true) {
    fail(`${reviewPath}: review.${field} must be true`);
  }
}

assertContains(doc, "Status: `source-complete / unvalidated`", `${docPath}: status must remain unvalidated`);
assertContains(doc, "complete framework extraction errors are not logged", `${docPath}: framework diagnostic policy`);
assertContains(doc, "no longer require a `Debug`", `${docPath}: type-only helper contract`);
assertContains(doc, "complete `PortError` and identity values are not logged", `${docPath}: owner diagnostic policy`);

if (failures.length > 0) {
  console.error("Commerce admin promotion native error-safety check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Commerce admin promotion native diagnostics use correlation-safe type/shape only; execution evidence remains open");
