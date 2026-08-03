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
const countText = (source, value) => source.split(value).length - 1;
const requireCount = (source, value, expected, label) => {
  const count = countText(source, value);
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};
const between = (source, start, end, label) => {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: could not isolate ${start} before ${end}`);
    return "";
  }
  return source.slice(from, to);
};

const paths = {
  adapter: "crates/rustok-inventory/admin/src/transport/native_server_adapter.rs",
  evidence:
    "crates/rustok-inventory/contracts/evidence/admin-native-error-safety-source.json",
  doc: "crates/rustok-inventory/docs/admin-native-error-safety.md",
  clientGuard:
    "scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const adapter = read(paths.adapter);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const clientGuard = read(paths.clientGuard);
const masterPlan = read(paths.masterPlan);

for (const [value, label] of [
  ["const INVENTORY_ADMIN_OWNER", "owner constant"],
  ["const INVENTORY_ADMIN_BOUNDARY", "boundary constant"],
  ["fn inventory_admin_correlation_id", "per-call correlation helper"],
  ["fn inventory_context_error<E>(", "context mapper without Debug bound"],
  ["_error: E", "unformatted error input"],
  ["fn auth_context_error<E>(", "auth mapper"],
  ["fn tenant_context_error<E>(", "tenant mapper"],
  ["fn request_context_error<E>(", "request mapper"],
  ["fn optional_request_context", "optional write request context"],
  ["fn inventory_owner_error<E>(", "owner mapper without Debug bound"],
  ["error_type = std::any::type_name::<E>()", "static generic error type"],
  [
    "error_type = std::any::type_name_of_val(&error)",
    "optional context error type",
  ],
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
]) requireText(adapter, value, `${paths.adapter}: ${label}`);

const diagnosticBlock = between(
  adapter,
  "fn inventory_context_error<E>(",
  "fn ensure_permission(",
  paths.adapter,
);
requireCount(
  diagnosticBlock,
  "error_type = std::any::type_name::<E>()",
  2,
  `${paths.adapter}: generic type-only diagnostics`,
);
requireCount(
  diagnosticBlock,
  "error_type = std::any::type_name_of_val(&error)",
  1,
  `${paths.adapter}: optional context type-only diagnostic`,
);
for (const marker of [
  "error = ?error",
  "error = %error",
  "raw_error",
  "internal_error",
  "error_message",
  "format!(\"{error:?}\")",
  "format!(\"{error}\")",
]) {
  forbidText(diagnosticBlock, marker, `${paths.adapter}: complete diagnostic payload`);
}

for (const endpoint of [
  'endpoint = "inventory/bootstrap"',
  'endpoint = "inventory/products"',
  'endpoint = "inventory/product"',
  'endpoint = "inventory/variant/set-quantity"',
  'endpoint = "inventory/variant/adjust-quantity"',
  'endpoint = "inventory/variant/reserve-quantity"',
  'endpoint = "inventory/variant/check-availability"',
  'endpoint = "inventory/variant/release-reservation"',
]) requireText(adapter, endpoint, `${paths.adapter}: mounted endpoint ${endpoint}`);

requireCount(adapter, ".map_err(|error| auth_context_error", 8, "auth extraction mapping");
requireCount(adapter, ".map_err(|error| tenant_context_error", 8, "tenant extraction mapping");
requireCount(adapter, "inventory_owner_error(", 8, "owner mapper definition and calls");
requireCount(
  adapter,
  "inventory_admin_correlation_id(owner_operation)",
  8,
  "per-call correlation creation",
);

for (const value of [
  ".map_err(ServerFnError::new)",
  "native transport requires TransactionalEventBus in host runtime context",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
]) forbidText(adapter, value, `${paths.adapter}: raw public error`);

for (const preserved of [
  'Permission denied: {message}',
  'Invalid {field_name}',
  "Invalid product status",
  "Requested tenant_id does not match request tenant context",
  "TransactionalEventBus",
  "AdminInventoryProductsFilter",
  "set_variant_quantity",
  "adjust_variant_quantity",
  ".reserve(tenant.id, variant_id, quantity)",
  ".check_variant_availability(tenant.id, variant_id, requested_quantity)",
  ".release_reservation_quantity(tenant.id, variant_id, quantity)",
]) requireText(adapter, preserved, `${paths.adapter}: preserved behavior`);

requireText(
  clientGuard,
  "Inventory Admin client transport error-safety verification failed",
  `${paths.clientGuard}: prior client guard remains present`,
);

if (evidence.schema_version !== 1) {
  failures.push(`${paths.evidence}: schema_version must be 1`);
}
if (evidence.status !== "inventory_admin_native_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  mounted_endpoints_preserved: true,
  auth_context_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  request_context_static_public_envelope: true,
  runtime_dependency_static_public_envelope: true,
  read_owner_raw_error_public: false,
  write_owner_raw_error_public: false,
  original_causes_logged_privately: false,
  context_error_type_only: true,
  owner_error_type_only: true,
  complete_context_error_logged: false,
  complete_owner_error_logged: false,
  error_payload_logged: false,
  per_call_correlation_logging: true,
  request_channel_locale_logging_when_available: true,
  dto_changed: false,
  permission_policy_changed: false,
  tenant_policy_changed: false,
  ffa_status_promoted: false,
  fba_status_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const marker of [
  "owner",
  "consumer",
  "owner_operation",
  "context_kind",
  "error_type",
  "correlation_id",
  "tenant_id",
  "actor_id",
  "subject_id",
  "code",
  "boundary",
]) {
  if (!evidence.safe_diagnostics?.includes(marker)) {
    failures.push(`${paths.evidence}: safe_diagnostics must include ${marker}`);
  }
}
for (const marker of [
  "complete framework extraction error",
  "complete inventory read error",
  "complete inventory write error",
  "database or event publication payload",
  "validation or invariant detail",
]) {
  if (!evidence.forbidden_diagnostics?.includes(marker)) {
    failures.push(`${paths.evidence}: forbidden_diagnostics must include ${marker}`);
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
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

requireText(doc, "Status: source-ready, runtime-unvalidated.", `${paths.doc}: status`);
requireText(doc, "complete framework or owner error payload is not logged", `${paths.doc}: bounded payload policy`);
requireText(doc, "static Rust error type", `${paths.doc}: type-only policy`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.masterPlan}: broad ecommerce mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Inventory admin native error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Inventory admin native endpoints use static public envelopes with bounded type-only diagnostics; execution evidence remains open",
);
