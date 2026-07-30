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

function requireText(source, value, label) {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
}

function forbidText(source, value, label) {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
}

function requireCount(source, value, minimum, label) {
  const count = source.split(value).length - 1;
  if (count < minimum) failures.push(`${label}: expected at least ${minimum}, found ${count}`);
}

const adapterPath =
  "crates/rustok-inventory/admin/src/transport/native_server_adapter.rs";
const evidencePath =
  "crates/rustok-inventory/contracts/evidence/admin-native-error-safety-source.json";
const adapter = read(adapterPath);
const evidence = JSON.parse(read(evidencePath));

for (const [value, label] of [
  ["const INVENTORY_ADMIN_OWNER", "owner constant"],
  ["const INVENTORY_ADMIN_BOUNDARY", "boundary constant"],
  ["fn inventory_admin_correlation_id", "per-call correlation helper"],
  ["fn inventory_context_error", "context mapper"],
  ["fn auth_context_error", "auth mapper"],
  ["fn tenant_context_error", "tenant mapper"],
  ["fn request_context_error", "request mapper"],
  ["fn optional_request_context", "optional write request context"],
  ["fn inventory_owner_error", "owner error mapper"],
  ["error = ?error", "private original cause"],
  ["owner_operation", "owner operation diagnostics"],
  ["correlation_id", "correlation diagnostics"],
  ["tenant_id = %tenant_id", "tenant diagnostics"],
  ["actor_id = ?actor_id", "actor diagnostics"],
  ["subject_id = ?subject_id", "subject diagnostics"],
  ["channel_id = ?request_context", "channel diagnostics"],
  ["Inventory authentication context is temporarily unavailable", "auth public envelope"],
  ["Inventory tenant context is temporarily unavailable", "tenant public envelope"],
  ["Inventory request context is temporarily unavailable", "request public envelope"],
  ["Inventory runtime is temporarily unavailable", "runtime public envelope"],
  ["Inventory products are temporarily unavailable", "list public envelope"],
  ["Inventory product is temporarily unavailable", "detail public envelope"],
  ["Inventory quantity could not be updated", "quantity public envelope"],
  ["Inventory reservation could not be completed", "reservation public envelope"],
  ["Inventory availability is temporarily unavailable", "availability public envelope"],
  ["Inventory reservation could not be released", "release public envelope"],
]) requireText(adapter, value, label);

for (const endpoint of [
  'endpoint = "inventory/bootstrap"',
  'endpoint = "inventory/products"',
  'endpoint = "inventory/product"',
  'endpoint = "inventory/variant/set-quantity"',
  'endpoint = "inventory/variant/adjust-quantity"',
  'endpoint = "inventory/variant/reserve-quantity"',
  'endpoint = "inventory/variant/check-availability"',
  'endpoint = "inventory/variant/release-reservation"',
]) requireText(adapter, endpoint, `mounted endpoint ${endpoint}`);

requireCount(adapter, ".map_err(|error| auth_context_error", 8, "auth extraction mapping");
requireCount(adapter, ".map_err(|error| tenant_context_error", 8, "tenant extraction mapping");
requireCount(adapter, "inventory_owner_error(", 7, "owner operation mapping");
requireCount(adapter, "inventory_admin_correlation_id(owner_operation)", 8, "per-call correlation creation");

for (const value of [
  ".map_err(ServerFnError::new)",
  "native transport requires TransactionalEventBus in host runtime context",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
]) forbidText(adapter, value, "inventory admin native adapter");

if (evidence.status !== "inventory_admin_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  mounted_endpoints_preserved: true,
  auth_context_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  request_context_static_public_envelope: true,
  runtime_dependency_static_public_envelope: true,
  read_owner_raw_error_public: false,
  write_owner_raw_error_public: false,
  original_causes_logged_privately: true,
  per_call_correlation_logging: true,
  request_channel_locale_logging_when_available: true,
  dto_changed: false,
  permission_policy_changed: false,
  tenant_policy_changed: false,
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
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error("Inventory admin native error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Inventory admin native endpoints use static public envelopes with private owner causes; source evidence remains unvalidated",
);
