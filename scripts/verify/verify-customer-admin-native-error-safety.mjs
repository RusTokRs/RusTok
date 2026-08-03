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
  adapter: "crates/rustok-customer/admin/src/transport/native_server_adapter.rs",
  cargo: "crates/rustok-customer/admin/Cargo.toml",
  ownerError: "crates/rustok-customer/src/error.rs",
  evidence:
    "crates/rustok-customer/contracts/evidence/admin-native-error-safety-source.json",
  doc: "crates/rustok-customer/docs/admin-native-error-safety.md",
  review: "crates/rustok-customer/docs/admin-native-error-safety-review.md",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const adapter = read(paths.adapter);
const cargo = read(paths.cargo);
const ownerError = read(paths.ownerError);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const review = read(paths.review);
const masterPlan = read(paths.masterPlan);

for (const endpoint of [
  'endpoint = "customer/bootstrap"',
  'endpoint = "customer/list"',
  'endpoint = "customer/detail"',
  'endpoint = "customer/create"',
  'endpoint = "customer/update"',
]) requireText(adapter, endpoint, `${paths.adapter}: mounted endpoint`);

for (const [value, label] of [
  ["const CUSTOMER_ADMIN_OWNER", "consumer owner constant"],
  ["const CUSTOMER_ADMIN_BOUNDARY", "transport boundary constant"],
  ["fn customer_admin_correlation_id", "per-call correlation helper"],
  ["fn customer_context_error<E>(", "context error mapper without Debug bound"],
  ["_error: E", "unformatted context error input"],
  ["fn auth_context_error<E>(", "auth context mapper"],
  ["fn tenant_context_error<E>(", "tenant context mapper"],
  ["async fn optional_request_context", "diagnostic request context"],
  ["fn customer_owner_error", "typed owner error mapper"],
  ["error_type = std::any::type_name::<E>()", "context error type-only diagnostic"],
  [
    "error_type = std::any::type_name_of_val(&error)",
    "optional request error type-only diagnostic",
  ],
  [
    "let (public_message, public_code, technical, error_kind) = match &error",
    "typed owner classification",
  ],
  ['"validation"', "validation classification"],
  ['"customer_not_found"', "customer not-found classification"],
  ['"customer_by_user_not_found"', "customer-by-user classification"],
  ['"duplicate_email"', "duplicate-email classification"],
  ['"duplicate_user_link"', "duplicate-link classification"],
  ['"profile"', "profile classification"],
  ['"database"', "database classification"],
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
]) requireText(adapter, value, `${paths.adapter}: ${label}`);

const diagnosticBlock = between(
  adapter,
  "fn customer_context_error<E>(",
  "fn ensure_permission(",
  paths.adapter,
);

if (countText(diagnosticBlock, "error_kind,") !== 2) {
  failures.push(`${paths.adapter}: both owner severity branches must record error_kind`);
}
if (countText(diagnosticBlock, "error_type =") !== 2) {
  failures.push(`${paths.adapter}: context diagnostics must expose exactly two type-only fields`);
}
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
for (const marker of [
  "email =",
  "first_name =",
  "last_name =",
  "phone =",
  "search =",
  "profile =",
  "payload =",
]) {
  forbidText(diagnosticBlock, marker, `${paths.adapter}: customer payload field`);
}

for (const operation of [
  'owner_operation = "bootstrap"',
  'owner_operation = "list_customers"',
  'owner_operation = "get_customer_detail"',
  'owner_operation = "create_customer"',
  'owner_operation = "update_customer"',
]) requireText(adapter, operation, `${paths.adapter}: mounted owner operation`);

for (const forbidden of [
  ".map_err(ServerFnError::new)",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
  "ServerFnError::new(format!(\"Database",
  "ServerFnError::new(format!(\"validation failed",
  "ServerFnError::new(format!(\"customer email already exists",
]) forbidText(adapter, forbidden, `${paths.adapter}: raw public error`);

for (const preserved of [
  'Permission denied: {message}',
  'Invalid {field_name}',
  "ProfileAccessAudience::TrustedService",
  "ProfileAccessAudience::Authenticated",
  "get_customer_with_profile",
  "ListCustomersInput",
  "CreateCustomerInput",
  "UpdateCustomerInput",
]) requireText(adapter, preserved, `${paths.adapter}: preserved behavior`);

for (const marker of [
  "Validation(String)",
  "CustomerNotFound(Uuid)",
  "CustomerByUserNotFound(Uuid)",
  "DuplicateEmail(String)",
  "DuplicateUserLink(Uuid)",
  "Profile(#[from] rustok_profiles::ProfileError)",
  "Database(#[from] DbErr)",
]) requireText(ownerError, marker, `${paths.ownerError}: typed owner variant`);

requireText(cargo, "tracing.workspace = true", `${paths.cargo}: diagnostics dependency`);

if (evidence.schema_version !== 1) {
  failures.push(`${paths.evidence}: schema_version must be 1`);
}
if (evidence.status !== "customer_admin_native_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: status mismatch: ${evidence.status}`);
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
  context_error_type_only: true,
  complete_context_error_logged: false,
  typed_customer_error_classification: true,
  complete_customer_error_logged: false,
  customer_error_payload_logged: false,
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
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const marker of [
  "owner",
  "consumer",
  "owner_operation",
  "correlation_id",
  "context_kind",
  "error_type",
  "error_kind",
  "tenant_id",
  "actor_id",
  "customer_id",
  "public_code",
  "boundary",
]) {
  if (!evidence.safe_diagnostics?.includes(marker)) {
    failures.push(`${paths.evidence}: safe_diagnostics must include ${marker}`);
  }
}
for (const marker of [
  "complete framework extraction error",
  "complete CustomerError Debug payload",
  "validation text",
  "duplicate email value",
  "profile error payload",
  "database error payload",
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
  "profile_audience_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

requireText(doc, "Status: `customer_admin_native_error_safety_source_unvalidated`", `${paths.doc}: status`);
requireText(doc, "complete framework or owner error payload is not logged", `${paths.doc}: bounded diagnostic policy`);
requireText(doc, "static Rust error type", `${paths.doc}: context type policy`);
requireText(doc, "typed customer error classification", `${paths.doc}: owner classification policy`);
requireText(review, "complete Debug payloads are absent", `${paths.review}: review finding`);
requireText(review, "No tests, verifiers, Cargo commands", `${paths.review}: validation boundary`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.masterPlan}: broad ecommerce mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Customer admin native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ customer admin native transport keeps static public envelopes and bounded type/variant-only diagnostics; execution evidence remains open",
);
