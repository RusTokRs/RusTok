#!/usr/bin/env node
// Commerce admin GraphQL consumer error-safety source guard.

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

const modPath = "crates/rustok-commerce/admin/src/transport/mod.rs";
const helperPath = "crates/rustok-commerce/admin/src/transport/graphql_error_safety.rs";
const shippingPath = "crates/rustok-commerce/admin/src/transport/shipping_profile.rs";
const orderPath = "crates/rustok-commerce/admin/src/transport/order_change.rs";
const adapterPath = "crates/rustok-commerce/admin/src/transport/graphql_adapter.rs";
const cargoPath = "crates/rustok-commerce/admin/Cargo.toml";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/admin-graphql-error-safety-source.json";

const routing = readRepo(modPath);
const helper = readRepo(helperPath);
const shipping = readRepo(shippingPath);
const order = readRepo(orderPath);
const adapter = readRepo(adapterPath);
const cargo = readRepo(cargoPath);
const evidence = JSON.parse(readRepo(evidencePath));

assertContains(routing, "mod graphql_adapter;", `${modPath}: GraphQL adapter must remain private`);
assertNotContains(routing, "pub mod graphql_adapter;", `${modPath}: raw GraphQL adapter must not be public`);
assertContains(routing, "mod graphql_error_safety;", `${modPath}: safe consumer mapper must be mounted`);

for (const marker of [
  "GraphqlHttpError::from_str",
  "GraphqlHttpError::Unauthorized",
  "GraphqlHttpError::Network",
  "GraphqlHttpError::Http(_)",
  "GraphqlHttpError::Graphql(_)",
  "Commerce admin authentication is required",
  "Commerce admin service is temporarily unavailable",
  "Commerce admin request could not be completed",
  "uuid::Uuid::new_v4()",
  "correlation_id",
  "tenant_slug_present",
  "tenant_slug_length",
  "error_kind",
  "public_code",
  "boundary = COMMERCE_ADMIN_GRAPHQL_BOUNDARY",
]) {
  assertContains(helper, marker, `${helperPath}: missing ${marker}`);
}

assertNotContains(helper, "token =", `${helperPath}: bearer token must never be logged`);
assertNotContains(helper, "tenant_slug =", `${helperPath}: tenant slug value must not be logged`);
assertContains(
  helper,
  "ApiError::Graphql(public_message.to_string())",
  `${helperPath}: public result must use only a static mapped message`,
);

for (const functionName of [
  "fetch_bootstrap",
  "fetch_shipping_profiles",
  "fetch_shipping_profile",
  "create_shipping_profile",
  "update_shipping_profile",
  "deactivate_shipping_profile",
  "reactivate_shipping_profile",
]) {
  const body = functionBody(shipping, functionName);
  assertContains(body, "graphql_correlation_id(operation)", `${shippingPath}: ${functionName} needs correlation`);
  assertContains(body, "map_graphql_error(", `${shippingPath}: ${functionName} must sanitize GraphQL failure`);
  assertNotContains(body, "ApiError::Graphql(error.to_string())", `${shippingPath}: ${functionName} must not publish raw GraphQL text`);
}

for (const functionName of [
  "fetch_order_changes",
  "apply_order_change",
  "cancel_order_change",
]) {
  const body = functionBody(order, functionName);
  assertContains(body, "if use_graphql_transport()", `${orderPath}: ${functionName} must preserve explicit transport selection`);
  assertContains(body, "graphql_correlation_id(operation)", `${orderPath}: ${functionName} GraphQL path needs correlation`);
  assertContains(body, "map_graphql_error(", `${orderPath}: ${functionName} GraphQL path must sanitize failures`);
  assertContains(body, "native_server_adapter::", `${orderPath}: ${functionName} native path must remain unchanged`);
}

assertContains(
  adapter,
  ".map_err(|error| ApiError::Graphql(error.to_string()))",
  `${adapterPath}: low-level adapter may retain the typed error string only behind private safe wrappers`,
);
assertContains(cargo, "uuid.workspace = true", `${cargoPath}: GraphQL correlation needs uuid in all transports`);
assertNotContains(cargo, '"dep:uuid"', `${cargoPath}: uuid must not remain SSR-only`);

if (evidence.status !== "commerce_admin_graphql_error_safety_source_unvalidated") {
  fail(`${evidencePath}: source evidence must remain explicitly unvalidated`);
}
for (const field of [
  "focused_verifier_executed",
  "aggregate_verifier_executed",
  "cargo_check_executed",
  "tests_executed",
  "browser_runtime_trace_retained",
  "ssr_runtime_trace_retained",
  "ci_executed",
]) {
  if (evidence.validation?.[field] !== false) {
    fail(`${evidencePath}: validation.${field} must remain false until execution evidence exists`);
  }
}

if (failures.length > 0) {
  console.error("Commerce admin GraphQL error-safety check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Commerce admin GraphQL error-safety source invariants passed");
