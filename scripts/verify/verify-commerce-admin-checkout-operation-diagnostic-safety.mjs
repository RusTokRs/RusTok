#!/usr/bin/env node

// Admin checkout-operation diagnostic safety.
//
// This guard owns the shape of the diagnostic *line* at the admin checkout-operation boundary: the
// internal error context keeps full identity (it is needed for policy selection and for identity
// adoption), while the value handed to `tracing` is a projection that stores only `&'static str`
// shapes. Nothing caller-supplied and nothing owner-supplied may reach the event.
//
// The verifier used to describe an intermediate state: it *required* `error = ?error`,
// `tenant_id = %context.tenant_id`, `actor_id = %context.actor_id`,
// `checkout_operation_id = ?context.checkout_operation_id` and a `redacted` placeholder for the typed
// error. That contract is the leak the platform policy exists to prevent, so the guard is now written
// the other way round: the raw directives are forbidden here and in the broad error-context verifier,
// the typed error is out of scope at the log site because the policy tuple is selected first, and the
// diagnostic context is proven total over every identity field.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");
const failures = [];

const paths = {
  source: "crates/modules/rustok-commerce/src/controllers/admin/checkout_operations.rs",
  sharedPolicies: "crates/modules/rustok-commerce/src/controllers/admin/mod.rs",
  policyLibrary: "scripts/verify/lib/bounded-diagnostics.mjs",
  evidence:
    "crates/modules/rustok-commerce/contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json",
  doc: "crates/modules/rustok-commerce/docs/admin-checkout-operation-diagnostic-safety.md",
  broadVerifier: "scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs",
  plan: "crates/modules/rustok-commerce/docs/implementation-plan.md",
};

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (content, pattern, expected, label) => {
  const actual = content.match(pattern)?.length ?? 0;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
};
const blockBetween = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate block`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};
const requireOrder = (content, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = content.indexOf(marker);
    if (index < 0) {
      failures.push(`${label}: missing ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: ${marker} is out of order`);
      return;
    }
    previous = index;
  }
};

const source = read(paths.source);
const sharedPolicies = read(paths.sharedPolicies);
const policyLibrary = read(paths.policyLibrary);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const broadVerifier = read(paths.broadVerifier);
const plan = read(paths.plan);

// --- The internal context must still carry identity ---------------------------------------------

const internalContext = blockBetween(
  source,
  "struct AdminCheckoutOperationErrorContext {",
  "impl AdminCheckoutOperationErrorContext {",
  "internal error context",
);
for (const [value, label] of [
  ["tenant_id: Uuid", "internal tenant identity"],
  ["actor_id: Uuid", "internal actor identity"],
  ["checkout_operation_id: Option<Uuid>", "internal checkout operation identity"],
  ["reservation_id: Option<Uuid>", "internal reservation identity"],
  ["payment_collection_id: Option<Uuid>", "internal payment collection identity"],
  ["payment_id: Option<Uuid>", "internal payment identity"],
  ["refund_id: Option<Uuid>", "internal refund identity"],
  ["order_id: Option<Uuid>", "internal order identity"],
  ["order_return_id: Option<Uuid>", "internal order return identity"],
  ["order_change_id: Option<Uuid>", "internal order change identity"],
  ["operation: &'static str", "internal operation label"],
]) requireText(internalContext, value, `${paths.source}: ${label}`);

// --- The diagnostic projection must store shapes only -------------------------------------------

const diagnosticContext = blockBetween(
  source,
  "struct AdminCheckoutOperationDiagnosticContext {",
  "impl From<&AdminCheckoutOperationErrorContext> for AdminCheckoutOperationDiagnosticContext {",
  "bounded diagnostic context",
);
for (const field of [
  "tenant_state",
  "actor_state",
  "checkout_operation_state",
  "reservation_state",
  "payment_collection_state",
  "payment_state",
  "refund_state",
  "order_state",
  "order_return_state",
  "order_change_state",
  "operation",
]) {
  requireText(diagnosticContext, `${field}: &'static str,`, `${paths.source}: shape field ${field}`);
}
for (const forbidden of ["Uuid", "Option<", "String", "&'a str"]) {
  forbidText(diagnosticContext, forbidden, `${paths.source}: diagnostic context stores shapes only`);
}

const conversion = blockBetween(
  source,
  "impl From<&AdminCheckoutOperationErrorContext> for AdminCheckoutOperationDiagnosticContext {",
  "fn uuid_shape(",
  "diagnostic conversion",
);
for (const [value, label] of [
  ["tenant_state: uuid_shape(context.tenant_id),", "tenant shape projection"],
  ["actor_state: uuid_shape(context.actor_id),", "actor shape projection"],
  ["checkout_operation_state: optional_uuid_shape(context.checkout_operation_id),",
   "checkout operation shape projection"],
  ["reservation_state: optional_uuid_shape(context.reservation_id),",
   "reservation shape projection"],
  ["payment_collection_state: optional_uuid_shape(context.payment_collection_id),",
   "payment collection shape projection"],
  ["payment_state: optional_uuid_shape(context.payment_id),", "payment shape projection"],
  ["refund_state: optional_uuid_shape(context.refund_id),", "refund shape projection"],
  ["order_state: optional_uuid_shape(context.order_id),", "order shape projection"],
  ["order_return_state: optional_uuid_shape(context.order_return_id),",
   "order return shape projection"],
  ["order_change_state: optional_uuid_shape(context.order_change_id),",
   "order change shape projection"],
  ["operation: context.operation,", "operation label projection"],
]) requireText(conversion, value, `${paths.source}: ${label}`);
requireCount(
  conversion,
  /optional_uuid_shape\(context\./g,
  8,
  `${paths.source}: eight optional identity shapes`,
);
requireCount(
  conversion,
  /[^_]uuid_shape\(context\./g,
  2,
  `${paths.source}: two required identity shapes`,
);

const requiredShape = blockBetween(
  source,
  "fn uuid_shape(",
  "fn optional_uuid_shape(",
  "required UUID shape",
);
for (const marker of ["value.is_nil()", '"nil"', '"non_nil"']) {
  requireText(requiredShape, marker, `${paths.source}: required UUID shape`);
}

const optionalShape = blockBetween(
  source,
  "fn optional_uuid_shape(",
  "struct AdminCheckoutOperationRouteFacts",
  "optional UUID shape",
);
for (const marker of [
  'None => "absent",',
  'Some(value) if value.is_nil() => "present_nil",',
  'Some(_) => "present_non_nil",',
]) requireText(optionalShape, marker, `${paths.source}: optional UUID shape`);

// --- The log site must not have the error in scope at all ----------------------------------------

const logger = blockBetween(
  source,
  "fn admin_checkout_operation_http_error(",
  "fn map_operation_error(",
  "admin checkout logger",
);
forbidText(logger, "<E>(", `${paths.source}: generic logger over the owner error`);
forbidText(logger, "where\n    E:", `${paths.source}: generic logger bound`);
forbidText(logger, 'let error = "redacted";', `${paths.source}: redacted placeholder`);
requireOrder(
  logger,
  [
    "let context = AdminCheckoutOperationDiagnosticContext::from(context);",
    "let (status, code, message, error_kind) = policy;",
    "tracing::error!(",
    "HttpError::new(status, code, message)",
  ],
  `${paths.source}: bounded logger order`,
);
for (const [value, label] of [
  ["owner = ADMIN_CHECKOUT_OPERATION_OWNER,", "owner field"],
  ["source_owner,", "source owner field"],
  ["error_kind,", "error kind field"],
  ["public_code = code,", "public code field"],
  ["status = %status,", "status field"],
  ["boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY,", "boundary field"],
]) requireText(logger, value, `${paths.source}: ${label}`);
for (const forbidden of ["format!(", ".to_string()", "error.message", "?error", "%error"]) {
  forbidText(logger, forbidden, `${paths.source}: raw logger payload`);
}
requireCount(
  logger,
  /_state = context\.\w+_state,/g,
  10,
  `${paths.source}: ten projected identity shapes in the event`,
);
requireText(logger, "operation = context.operation,", `${paths.source}: route label in the event`);

// --- Severity discipline: one line per outcome, no stray raw event -------------------------------

requireCount(source, /tracing::debug!\(/g, 1, `${paths.source}: one debug entry line`);
requireCount(source, /tracing::info!\(/g, 1, `${paths.source}: one info outcome line`);
requireCount(source, /tracing::error!\(/g, 1, `${paths.source}: one error failure line`);
requireCount(source, /tracing::warn!\(/g, 0, `${paths.source}: no warn line`);
requireCount(
  source,
  /log_admin_checkout_operation_route_entry\(\n        "/g,
  6,
  `${paths.source}: six bounded entry lines`,
);

forbidRawContextLogging(failures, source, "admin checkout operation controller");
forbidRawContextLogging(failures, sharedPolicies, "admin commerce shared mappers");
for (const forbidden of ["%context.", "?context.", "= %tenant_id", "= %actor_id"]) {
  forbidText(source, forbidden, `${paths.source}: raw context directive`);
}

// --- The policy library is the single definition of "raw" ---------------------------------------

for (const [value, label] of [
  ["export function rawContextLogging(", "raw directive census"],
  ["export function forbidRawContextLogging(", "raw directive failure reporter"],
  ['id: "tenant-value"', "tenant identity rule"],
  ['id: "actor-id-value"', "actor identity rule"],
  ['id: "owner-message"', "owner message rule"],
]) requireText(policyLibrary, value, `${paths.policyLibrary}: ${label}`);

// --- The broad verifier must forbid what this slice removed --------------------------------------

for (const [value, label] of [
  ["forbidRawContextLogging(failures, controller,", "broad verifier raw scan"],
  ['"retired raw diagnostic field"', "broad verifier retired field check"],
  ['"tenant_id = %context.tenant_id"', "broad verifier forbids the raw tenant field"],
  ['"actor_id = %context.actor_id"', "broad verifier forbids the raw actor field"],
  ['"error = ?error"', "broad verifier forbids the raw error payload"],
  [
    "shared admin policy type import",
    "broad verifier pins the shared policy type",
  ],
]) requireText(broadVerifier, value, `${paths.broadVerifier}: ${label}`);

// --- Evidence and documentation stay honest -----------------------------------------------------

if (
  evidence.status !==
  "commerce_admin_checkout_operation_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  raw_context_values_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_optional_domain_uuid_logged: false,
  typed_error_logged: false,
  required_uuid_shape_logged: true,
  optional_uuid_shape_logged: true,
  route_operation_logged: true,
  source_owner_logged: true,
  error_kind_logged: true,
  public_code_logged: true,
  http_status_logged: true,
  boundary_logged: true,
  bounded_diagnostics_policy_applied: true,
  shared_admin_policy_reused: true,
  compensation_variants_complete: true,
  identity_adoption_preserved: true,
  http_envelopes_preserved: true,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "compile_proven",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "Required tenant and actor UUIDs are represented only as `nil` or `non_nil`.",
  "The typed error never reaches the event:",
  "The broader ecommerce correlation-safe mapper task remains open.",
]) requireText(doc, marker, `${paths.doc}: documentation contract`);
forbidText(doc, "stable marker `redacted`", `${paths.doc}: retired redacted marker`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce admin checkout-operation diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin checkout-operation diagnostics project shape only, keep the owner error out of the event, and stay bounded under the shared policy",
);
