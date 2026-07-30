#!/usr/bin/env node
// Commerce admin order-change native transport error-safety source guard.

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
  const signature = new RegExp(`(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}\\s*\\(`);
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

const adapter = readRepo(adapterPath);
const evidence = JSON.parse(readRepo(evidencePath));

for (const endpoint of [
  'endpoint = "commerce/admin/order-changes"',
  'endpoint = "commerce/admin/apply-order-change"',
  'endpoint = "commerce/admin/cancel-order-change"',
]) {
  assertContains(adapter, endpoint, `${adapterPath}: missing mounted endpoint ${endpoint}`);
}

for (const marker of [
  'const COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER',
  'const COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY',
  'fn order_change_correlation_id(',
  'uuid::Uuid::new_v4()',
  'fn order_change_context_error',
  'fn order_change_auth_context_error',
  'fn order_change_tenant_context_error',
  'optional_order_change_request_context',
  'struct OrderChangeOwnerErrorContext',
  'fn order_change_owner_error(',
  'error: rustok_order::error::OrderError',
  'Commerce order-change runtime is temporarily unavailable',
]) {
  assertContains(adapter, marker, `${adapterPath}: missing order-change safety marker ${marker}`);
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

const runtimeBody = functionBody(adapter, "order_service_from_context");
assertContains(
  runtimeBody,
  'code = "commerce.admin_order_change_runtime_unavailable"',
  `${adapterPath}: runtime composition failure needs a stable diagnostic code`,
);
assertContains(
  runtimeBody,
  'ServerFnError::new("Commerce order-change runtime is temporarily unavailable")',
  `${adapterPath}: runtime composition detail must stay internal`,
);
assertNotContains(
  runtimeBody,
  "Commerce admin requires TransactionalEventBus in host runtime context",
  `${adapterPath}: host composition detail must not cross the public boundary`,
);

for (const marker of [
  'owner = "rustok_order"',
  "consumer",
  "operation",
  "correlation_id",
  "tenant_id",
  "actor_id",
  "order_id",
  "order_change_id",
  "request_tenant_id",
  "request_user_id",
  "channel_id",
  "channel_slug",
  "locale",
  "error_kind",
  "public_code",
  "boundary",
]) {
  assertContains(adapter, marker, `${adapterPath}: missing structured diagnostic field ${marker}`);
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

if (failures.length > 0) {
  console.error("Commerce admin order-change native error-safety check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Commerce admin order-change native error-safety source invariants passed");
