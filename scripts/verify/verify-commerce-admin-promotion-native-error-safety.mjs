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

const routingPath = "crates/rustok-commerce/admin/src/transport/mod.rs";
const safeAdapterPath =
  "crates/rustok-commerce/admin/src/transport/native_server_adapter_ssr.rs";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/admin-promotion-native-error-safety-source.json";

const routing = readRepo(routingPath);
const safeAdapter = readRepo(safeAdapterPath);
const evidence = JSON.parse(readRepo(evidencePath));

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

assertContains(
  safeAdapter,
  'uuid::Uuid::new_v4()',
  `${safeAdapterPath}: promotion calls need a unique transport correlation id`,
);
assertContains(
  safeAdapter,
  'optional_promotion_request_context',
  `${safeAdapterPath}: request context must be captured for attribution without changing admission`,
);
assertContains(
  safeAdapter,
  '.map(|context| context.locale.as_str())',
  `${safeAdapterPath}: effective request locale must cross the promotion port`,
);
assertContains(
  safeAdapter,
  'context.with_channel(channel)',
  `${safeAdapterPath}: resolved request channel must cross the promotion port`,
);
assertContains(
  safeAdapter,
  '.with_idempotency_key(correlation_id.to_string())',
  `${safeAdapterPath}: promotion write must carry non-empty idempotency semantics`,
);
assertContains(
  safeAdapter,
  'fn promotion_port_error(',
  `${safeAdapterPath}: owner PortError must have a consumer-side diagnostic boundary`,
);
assertContains(
  safeAdapter,
  'ServerFnError::new(error.message)',
  `${safeAdapterPath}: only the already-sanitized PortError public message may cross the boundary`,
);

for (const [functionName, contextMapper] of [
  ["commerce_admin_preview_cart_promotion_native", "promotion_auth_context_error"],
  ["commerce_admin_apply_cart_promotion_native", "promotion_auth_context_error"],
]) {
  const body = functionBody(safeAdapter, functionName);
  assertContains(body, contextMapper, `${safeAdapterPath}: ${functionName} must sanitize auth extraction`);
  assertContains(body, "promotion_tenant_context_error", `${safeAdapterPath}: ${functionName} must sanitize tenant extraction`);
  assertContains(body, "optional_promotion_request_context", `${safeAdapterPath}: ${functionName} must capture request attribution`);
  assertNotContains(body, ".map_err(ServerFnError::new)", `${safeAdapterPath}: ${functionName} must not publish raw framework extraction errors`);
}

for (const functionName of [
  "preview_cart_promotion_native_with_context",
  "apply_cart_promotion_native_with_context",
]) {
  const body = functionBody(safeAdapter, functionName);
  assertContains(body, "promotion_port_error", `${safeAdapterPath}: ${functionName} must log typed owner failures at the consumer boundary`);
  assertNotContains(body, "ServerFnError::new(error.to_string())", `${safeAdapterPath}: ${functionName} must not serialize raw owner errors`);
  assertNotContains(body, "ServerFnError::new(err.to_string())", `${safeAdapterPath}: ${functionName} must not serialize raw owner errors`);
}

assertNotContains(
  functionBody(safeAdapter, "cart_promotion_port_context"),
  'format!("commerce-admin-cart-promotion:{operation}:{cart_id}")',
  `${safeAdapterPath}: promotion correlation must not be deterministic only by operation and cart`,
);

assertContains(
  safeAdapter,
  'owner = "rustok_cart.promotion"',
  `${safeAdapterPath}: diagnostics must identify the cart promotion owner`,
);
for (const marker of [
  "consumer_operation",
  "owner_operation",
  "correlation_id",
  "tenant_id",
  "actor_id",
  "cart_id",
  "channel_id",
  "channel_slug",
  "locale",
  "public_code",
  "error_kind",
  "retryable",
  "boundary",
]) {
  assertContains(safeAdapter, marker, `${safeAdapterPath}: missing structured diagnostic field ${marker}`);
}

if (evidence.status !== "commerce_admin_promotion_native_error_safety_source_unvalidated") {
  fail(`${evidencePath}: source evidence must remain explicitly unvalidated`);
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

if (failures.length > 0) {
  console.error("Commerce admin promotion native error-safety check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Commerce admin promotion native error-safety source invariants passed");
