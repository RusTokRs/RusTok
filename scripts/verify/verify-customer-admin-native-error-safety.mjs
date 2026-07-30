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

const adapterPath = "crates/rustok-customer/admin/src/transport/native_server_adapter.rs";
const cargoPath = "crates/rustok-customer/admin/Cargo.toml";
const evidencePath = "crates/rustok-customer/contracts/evidence/admin-native-error-safety-source.json";
const adapter = read(adapterPath);
const cargo = read(cargoPath);
const evidence = JSON.parse(read(evidencePath));

for (const endpoint of [
  'endpoint = "customer/bootstrap"',
  'endpoint = "customer/list"',
  'endpoint = "customer/detail"',
  'endpoint = "customer/create"',
  'endpoint = "customer/update"',
]) requireText(adapter, endpoint, "mounted customer admin endpoint");

for (const [value, label] of [
  ["const CUSTOMER_ADMIN_OWNER", "consumer owner constant"],
  ["const CUSTOMER_ADMIN_BOUNDARY", "transport boundary constant"],
  ["fn customer_admin_correlation_id", "per-call correlation helper"],
  ["fn customer_context_error", "context error mapper"],
  ["fn auth_context_error", "auth context mapper"],
  ["fn tenant_context_error", "tenant context mapper"],
  ["async fn optional_request_context", "diagnostic request context"],
  ["fn customer_owner_error", "typed owner error mapper"],
  ['owner = "rustok_customer"', "customer owner diagnostics"],
  ["consumer = CUSTOMER_ADMIN_OWNER", "consumer diagnostics"],
  ["owner_operation", "owner operation diagnostics"],
  ["correlation_id", "correlation diagnostics"],
  ["tenant_id = %tenant_id", "tenant diagnostics"],
  ["actor_id = %actor_id", "actor diagnostics"],
  ["customer_id = ?customer_id", "customer diagnostics"],
  ["channel_id = ?request_context.and_then", "channel diagnostics"],
  ["locale = ?request_context.map", "locale diagnostics"],
  ["public_code", "stable public code diagnostics"],
  ["Customer authentication context is temporarily unavailable", "auth public envelope"],
  ["Customer tenant context is temporarily unavailable", "tenant public envelope"],
  ["Customer request is invalid", "validation public envelope"],
  ["Customer was not found", "not-found public envelope"],
  ["Customer email already exists", "duplicate-email public envelope"],
  ["Customer is already linked to a user", "duplicate-link public envelope"],
  ["Customer profile is temporarily unavailable", "profile public envelope"],
  ["Customer data is temporarily unavailable", "storage public envelope"],
]) requireText(adapter, value, label);

for (const operation of [
  'owner_operation = "bootstrap"',
  'owner_operation = "list_customers"',
  'owner_operation = "get_customer_detail"',
  'owner_operation = "create_customer"',
  'owner_operation = "update_customer"',
]) requireText(adapter, operation, "mounted owner operation");

for (const forbidden of [
  ".map_err(ServerFnError::new)",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
  "ServerFnError::new(format!(\"Database",
  "ServerFnError::new(format!(\"validation failed",
  "ServerFnError::new(format!(\"customer email already exists",
]) forbidText(adapter, forbidden, "customer admin native adapter");

for (const preserved of [
  'Permission denied: {message}',
  'Invalid {field_name}',
  "ProfileAccessAudience::TrustedService",
  "ProfileAccessAudience::Authenticated",
  "get_customer_with_profile",
]) requireText(adapter, preserved, "preserved customer admin behavior");

requireText(cargo, "tracing.workspace = true", "diagnostics dependency");

if (evidence.status !== "customer_admin_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  mounted_endpoint_set_preserved: true,
  auth_context_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  request_context_diagnostic_only: true,
  customer_validation_static_public_envelope: true,
  customer_not_found_static_public_envelope: true,
  duplicate_email_static_public_envelope: true,
  duplicate_user_link_static_public_envelope: true,
  profile_failure_static_public_envelope: true,
  storage_failure_static_public_envelope: true,
  owner_context_logging: true,
  raw_customer_error_public: false,
  raw_profile_error_public: false,
  raw_framework_error_public: false,
  permissions_changed: false,
  request_response_dto_changed: false,
  profile_audience_policy_changed: false,
  ffa_status_promoted: false,
  fba_status_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "profile_audience_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error("Customer admin native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ customer admin native transport uses static public envelopes with private owner causes; source evidence remains unvalidated",
);
