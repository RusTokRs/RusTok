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

const paths = {
  source: "crates/rustok-commerce/src/controllers/admin/post_order_reads.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/admin-post-order-read-diagnostic-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-post-order-read-diagnostic-safety.md",
  cutoverVerifier: "scripts/verify/verify-commerce-admin-post-order-read-cutover.mjs",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const actual = source.split(value).length - 1;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
};

function blockBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
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
}

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const cutoverVerifier = read(paths.cutoverVerifier);
const plan = read(paths.plan);

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
  "fn uuid_text_shape(",
  "optional UUID shape",
);
for (const marker of [
  'None => "absent"',
  'Some(value) if value.is_nil() => "present_nil"',
  'Some(_) => "present_non_nil"',
]) requireText(optionalShape, marker, `${paths.source}: optional UUID shape`);

const textShape = blockBetween(
  source,
  "fn uuid_text_shape(",
  "#[allow(clippy::too_many_arguments)]",
  "tenant UUID-text shape",
);
for (const marker of [
  "Uuid::parse_str(value)",
  "Ok(value) => uuid_shape(value)",
  'Err(_) if value.is_empty() => "empty"',
  'Err(_) => "invalid"',
]) requireText(textShape, marker, `${paths.source}: tenant UUID-text shape`);

const mapper = blockBetween(
  source,
  "fn map_admin_post_order_port_error(",
  "#[utoipa::path(",
  "post-order read mapper",
);

for (const marker of [
  "PortErrorKind::Validation",
  "PortErrorKind::NotFound",
  "PortErrorKind::Conflict",
  "PortErrorKind::Forbidden",
  "PortErrorKind::Unavailable | PortErrorKind::Timeout",
  "PortErrorKind::InvariantViolation",
  '"commerce_admin_order_invalid"',
  '"commerce_admin_not_found"',
  '"commerce_admin_order_state_conflict"',
  '"commerce_permission_denied"',
  '"commerce_admin_order_storage_unavailable"',
  '"commerce_admin_order_failed"',
]) requireText(mapper, marker, `${paths.source}: preserved port policy`);

requireOrder(
  mapper,
  [
    "let correlation_id_present = !port_context.correlation_id.is_empty();",
    "let correlation_id_length = port_context.correlation_id.len();",
    "let tenant_id_shape = uuid_text_shape(port_context.tenant_id.as_str());",
    "let actor_id_shape = uuid_shape(actor_id);",
    "let return_id_shape = optional_uuid_shape(return_id);",
    "let change_id_shape = optional_uuid_shape(change_id);",
    "let order_id_shape = optional_uuid_shape(order_id);",
    "let channel_present = port_context.channel.is_some();",
    "let locale_length = port_context.locale.len();",
    "let internal_code = error.code.as_str();",
    "let retryable = error.retryable;",
    'let error = "redacted";',
    "tracing::error!(",
    "HttpError::new(status, code, message)",
  ],
  `${paths.source}: bounded mapper order`,
);

for (const marker of [
  "error = ?error",
  "owner = ADMIN_POST_ORDER_OWNER",
  "owner_operation,",
  "consumer_operation,",
  "correlation_id_present,",
  "correlation_id_length,",
  "tenant_id_shape,",
  "actor_id_shape,",
  "return_id_shape,",
  "change_id_shape,",
  "order_id_shape,",
  "channel_present,",
  "channel_length,",
  "locale_length,",
  "deadline_ms = ?port_context.deadline_ms",
  "internal_code,",
  "retryable,",
  "error_kind,",
  "public_code = code",
  "status = %status",
  "boundary = ADMIN_POST_ORDER_BOUNDARY",
  '"commerce admin post-order owner read failed"',
]) requireText(mapper, marker, `${paths.source}: bounded diagnostic field`);

for (const forbidden of [
  "correlation_id = %port_context.correlation_id",
  "tenant_id = %port_context.tenant_id",
  "actor_id = %actor_id",
  "return_id = ?return_id",
  "change_id = ?change_id",
  "order_id = ?order_id",
  "actor = ?port_context.actor",
  "channel = ?port_context.channel",
  "locale = %port_context.locale",
  "internal_code = %error.code",
  "retryable = error.retryable",
  "error.message",
  "error.to_string()",
]) forbidText(mapper, forbidden, `${paths.source}: raw diagnostic payload`);

requireCount(
  source,
  "map_admin_post_order_port_error(",
  5,
  "one mapper definition plus four route callsites",
);
requireCount(source, "&[Permission::ORDERS_READ]", 4, "four ORDERS_READ routes");
for (const marker of [
  '"list_order_return_projections"',
  '"list_order_returns"',
  '"read_order_return_projection"',
  '"show_order_return"',
  '"list_order_change_projections"',
  '"list_order_changes"',
  '"read_order_change_projection"',
  '"show_order_change"',
  ".list_order_return_projections(",
  ".read_order_return_projection(",
  ".list_order_change_projections(",
  ".read_order_change_projection(",
  "per_page: pagination.limit()",
  "HttpError::new(status, code, message)",
]) requireText(source, marker, `${paths.source}: preserved route behavior`);

for (const marker of [
  "pub mod post_order_reads;",
  "post_order_reads::list_order_returns",
  "post_order_reads::show_order_return",
  "post_order_reads::list_order_changes",
  "post_order_reads::show_order_change",
]) requireText(cutoverVerifier, marker, `${paths.cutoverVerifier}: retained cutover coverage`);

if (
  evidence.status !==
  "commerce_admin_post_order_read_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_port_error_logged: false,
  raw_correlation_id_logged: false,
  raw_tenant_id_logged: false,
  raw_actor_id_logged: false,
  raw_return_id_logged: false,
  raw_change_id_logged: false,
  raw_order_id_logged: false,
  raw_port_actor_logged: false,
  raw_channel_logged: false,
  raw_locale_logged: false,
  redacted_error_marker_logged: true,
  correlation_presence_and_length_logged: true,
  tenant_uuid_text_shape_logged: true,
  actor_uuid_shape_logged: true,
  optional_resource_uuid_shapes_logged: true,
  channel_presence_and_length_logged: true,
  locale_length_logged: true,
  deadline_logged: true,
  internal_code_preserved: true,
  retryability_preserved: true,
  port_error_kind_policy_preserved: true,
  http_envelopes_preserved: true,
  four_admin_read_callsites_preserved: true,
  broad_ecommerce_cleanup_closed: false,
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
  "correlation presence and length",
  "The full `PortError` is replaced in the event by the stable marker `redacted`.",
  "The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.",
]) requireText(doc, marker, `${paths.doc}: documentation contract`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce admin post-order read diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin post-order read diagnostics are bounded while the four mounted owner-port routes and HTTP policies remain unchanged; execution validation remains open",
);
