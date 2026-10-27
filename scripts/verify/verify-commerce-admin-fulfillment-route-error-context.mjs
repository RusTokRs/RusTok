#!/usr/bin/env node

// Admin fulfillment route error context.
//
// `controllers/admin/fulfillments_owner_commands.rs` (mounted as `admin::fulfillments`) serves eight
// routes: list, show, create, ship, deliver, reopen, reship, cancel. Reads go through the
// Fulfillment read port, commands through the Fulfillment admin command port, and create through the
// manual-fulfillment orchestration, which itself composes the Order read, Fulfillment read, shipping
// option read, and Fulfillment create ports. Every one of those can fail with a `PortError`.
//
// The controller maps them with one policy function and three mappers — a read mapper, a command
// mapper, and a cross-owner create mapper — so a route can never invent its own envelope. The policy
// is code-first: the owner's bounded code (`fulfillment.reconciliation_required`,
// `fulfillment.database_unavailable`, `fulfillment.invalid_transition`) picks a specific envelope,
// and anything else falls back to the seven `PortErrorKind` arms.
//
// The verifier used to describe the retired `admin/fulfillments.rs`: an `AdminFulfillmentErrorContext`
// and `AdminFulfillmentDiagnosticContext` pair, `map_admin_fulfillment_port_error` /
// `map_admin_fulfillment_error` / `map_admin_fulfillment_orchestration_error`, a redacted `Debug`
// marker, and `internal_code = %error.code`. That file no longer exists. This guard is written
// against the live file, and it also checks the owner side — `rustok-fulfillment` must translate its
// typed error into a bounded `PortError`, never into owner text.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const controller = read(
  "crates/modules/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs",
);
const adminMod = read("crates/modules/rustok-commerce/src/controllers/admin/mod.rs");
const portErrors = read("crates/libs/rustok-api/src/ports.rs");
const ownerErrors = read("crates/modules/rustok-fulfillment/src/error.rs");
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const requirePattern = (content, pattern, label) => {
  if (!pattern.test(content)) failures.push(`${label}: missing ${pattern}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (content, pattern, expected, label) => {
  const actual = content.match(pattern)?.length ?? 0;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};
const normalize = (content) => content.replace(/\s+/g, " ");
const requireTextNormalized = (content, value, label) => {
  if (!normalize(content).includes(normalize(value))) failures.push(`${label}: missing ${value}`);
};
const enumVariants = (content, name, label) => {
  const marker = `pub enum ${name} {`;
  const start = content.indexOf(marker);
  if (start < 0) {
    failures.push(`${label}: missing enum ${name}`);
    return [];
  }
  const body = content.slice(start + marker.length);
  const end = body.indexOf("\n}");
  if (end < 0) {
    failures.push(`${label}: unterminated enum ${name}`);
    return [];
  }
  return [...body.slice(0, end).matchAll(/^ {4}([A-Z][A-Za-z0-9]*)\b/gm)].map(
    (match) => match[1],
  );
};

const constants = between(
  controller,
  "const ADMIN_FULFILLMENT_COMMAND_OWNER:",
  "type AdminFulfillmentCommandHttpPolicy",
  "fulfillment constants",
);
const entryFacts = between(
  controller,
  "struct AdminFulfillmentRouteFacts {",
  "/// Record that a fulfillment route was entered",
  "fulfillment entry facts",
);
const entryLogger = between(
  controller,
  "fn log_admin_fulfillment_route_entry(",
  "fn require_idempotency_key(",
  "fulfillment entry logger",
);
const commandContext = between(
  controller,
  "fn admin_fulfillment_command_context(",
  "fn admin_fulfillment_create_read_context(",
  "fulfillment command context",
);
const createCommandContext = between(
  controller,
  "fn admin_fulfillment_create_command_context(",
  "fn fulfillment_command_error_policy(",
  "fulfillment create command context",
);
const readContext = between(
  controller,
  "fn admin_fulfillment_read_context(",
  "fn map_fulfillment_read_error(",
  "fulfillment read context",
);
const policy = between(
  controller,
  "fn fulfillment_command_error_policy(",
  "fn map_fulfillment_command_error(",
  "fulfillment command error policy",
);
const commandMapper = between(
  controller,
  "fn map_fulfillment_command_error(",
  "fn map_fulfillment_create_error(",
  "fulfillment command mapper",
);
const createMapper = between(
  controller,
  "fn map_fulfillment_create_error(",
  "fn admin_fulfillment_read_context(",
  "fulfillment create mapper",
);
const readMapper = between(
  controller,
  "fn map_fulfillment_read_error(",
  "/// List admin fulfillments",
  "fulfillment read mapper",
);

// --- Wiring -------------------------------------------------------------------------------------

for (const [value, label] of [
  ['#[path = "fulfillments_owner_commands.rs"]', "fulfillment module path"],
  ["pub mod fulfillments;", "fulfillment module mount"],
]) requireText(adminMod, value, `admin module wiring: ${label}`);

for (const [value, label] of [
  ['const ADMIN_FULFILLMENT_COMMAND_OWNER: &str = "rustok_fulfillment.admin_command";',
   "command owner"],
  ['const ADMIN_FULFILLMENT_CREATE_OWNER: &str = "rustok_fulfillment.admin_create_command";',
   "create owner"],
  [
    'const ADMIN_FULFILLMENT_COMMAND_BOUNDARY: &str = "commerce_admin_fulfillment_command_http";',
    "command boundary",
  ],
]) requireText(constants, value, label);

// --- Bounded entry line -------------------------------------------------------------------------

for (const [value, label] of [
  ["fulfillment_id: Option<Uuid>,", "fulfillment identity fact"],
  ["order_id: Option<Uuid>,", "order identity fact"],
  ["status_filter_present: bool,", "status filter fact"],
  ["customer_filter_present: bool,", "customer filter fact"],
  ["carrier_length: Option<usize>,", "bounded carrier fact"],
]) requireText(entryFacts, value, `entry facts: ${label}`);

for (const [value, label] of [
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "actor shape"],
  ["fulfillment_id_present = facts.fulfillment_id.is_some(),", "fulfillment presence"],
  [
    "fulfillment_id_non_nil = facts.fulfillment_id.is_some_and(|value| !value.is_nil()),",
    "fulfillment shape",
  ],
  ["order_id_present = facts.order_id.is_some(),", "order presence"],
  ["order_id_non_nil = facts.order_id.is_some_and(|value| !value.is_nil()),", "order shape"],
  ["status_filter_present = facts.status_filter_present,", "status filter"],
  ["customer_filter_present = facts.customer_filter_present,", "customer filter"],
  ["carrier_length = facts.carrier_length.unwrap_or_default(),", "bounded carrier length"],
  ["boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,", "entry boundary"],
  ['"commerce admin fulfillment route entered with bounded diagnostics"', "entry message"],
]) requireText(entryLogger, value, `entry logger: ${label}`);

requireCount(
  controller,
  /log_admin_fulfillment_route_entry\(\n        "/g,
  8,
  "eight bounded fulfillment entry lines",
);
requireCount(controller, /tracing::debug!\(/g, 1, "one debug line");
requireCount(controller, /tracing::error!\(/g, 3, "three mapper error lines");
requireCount(controller, /tracing::warn!\(/g, 0, "no warn line");

forbidRawContextLogging(failures, controller, "admin fulfillment controller");
for (const forbidden of [
  "tenant_id = %tenant.id",
  "actor_id = %auth.user_id",
  "fulfillment_id = %id",
  "order_id = %order_id",
  "carrier = ?input.carrier",
  "status = ?params.status",
]) forbidText(controller, forbidden, "raw fulfillment entry value");

// --- Port contexts ------------------------------------------------------------------------------

for (const [value, label] of [
  ["PortContext::new(", "command context constructor"],
  ["PortActor::user(auth.user_id.to_string())", "actor projection"],
  ['format!("commerce-admin-fulfillment-command:{operation}:{fulfillment_id}")',
   "command correlation key"],
  [".with_idempotency_key(idempotency_key)", "command idempotency key"],
  [".with_deadline(std::time::Duration::from_secs(2))", "command deadline"],
  ["Some(channel) => context.with_channel(channel),", "command channel forwarding"],
]) requireTextNormalized(commandContext, value, `command context: ${label}`);

for (const [value, label] of [
  ['format!("commerce-admin-fulfillment-create-command:{order_id}")', "create correlation key"],
  [".with_idempotency_key(idempotency_key)", "create idempotency key"],
]) requireTextNormalized(createCommandContext, value, `create command context: ${label}`);

for (const [value, label] of [
  ["let resource_id = fulfillment_id.unwrap_or(tenant.id);", "bounded resource identity"],
  ['format!("commerce-admin-fulfillment:{operation}:{resource_id}")', "read correlation key"],
  [".with_deadline(std::time::Duration::from_secs(2))", "read deadline"],
]) requireTextNormalized(readContext, value, `read context: ${label}`);

// --- Error policy -------------------------------------------------------------------------------

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}
for (const variant of portErrorKinds) {
  requirePattern(
    policy,
    new RegExp(`PortErrorKind::${variant}\\b`),
    `fulfillment policy handles PortErrorKind::${variant}`,
  );
}

for (const [value, label] of [
  ['"fulfillment.reconciliation_required"', "reconciliation owner code"],
  ["StatusCode::CONFLICT", "conflict status"],
  ['"commerce_admin_fulfillment_reconciliation_required"', "reconciliation public code"],
  ['"Fulfillment operation requires reconciliation"', "reconciliation message"],
  ['"fulfillment.database_unavailable"', "storage owner code"],
  ["StatusCode::SERVICE_UNAVAILABLE", "storage status"],
  ['"commerce_admin_fulfillment_storage_unavailable"', "storage public code"],
  ['"Fulfillment storage is temporarily unavailable"', "storage message"],
  ['"fulfillment.invalid_transition"', "transition owner code"],
  ['"commerce_admin_fulfillment_state_conflict"', "state conflict public code"],
  ['"Fulfillment operation conflicts with the current state"', "state conflict message"],
  ["StatusCode::BAD_REQUEST", "validation status"],
  ['"commerce_admin_fulfillment_invalid"', "validation public code"],
  ['"Fulfillment request is invalid"', "validation message"],
  ["StatusCode::NOT_FOUND", "not-found status"],
  ['"commerce_admin_not_found"', "not-found public code"],
  ['"Commerce resource not found"', "not-found message"],
  ["StatusCode::FORBIDDEN", "forbidden status"],
  ['"commerce_permission_denied"', "forbidden public code"],
  ['"Permission denied"', "forbidden message"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "fail-closed status"],
  ['"commerce_admin_fulfillment_failed"', "fail-closed public code"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "fail-closed status"],
]) requireText(policy, value, `fulfillment policy: ${label}`);

// The policy returns data; only the mappers build the envelope.
forbidText(policy, "HttpError::", "policy returns a policy tuple, not an HTTP response");

// --- Mappers ------------------------------------------------------------------------------------

for (const [mapper, label, owner] of [
  [commandMapper, "command mapper", "owner = ADMIN_FULFILLMENT_COMMAND_OWNER,"],
  [createMapper, "create mapper", "owner = ADMIN_FULFILLMENT_CREATE_OWNER,"],
  [readMapper, "read mapper", "owner = ADMIN_FULFILLMENT_COMMAND_OWNER,"],
]) {
  requireText(
    mapper,
    "let (status, code, message, error_kind) = fulfillment_command_error_policy(&error);",
    `${label}: shared policy selection`,
  );
  requireText(mapper, "HttpError::new(status, code, message)", `${label}: single static envelope`);
  requireText(mapper, owner, `${label}: owner label`);
  requireText(mapper, "correlation_id = %context.correlation_id,", `${label}: correlation join key`);
  requireText(mapper, "owner_code_length = error.code.chars().count(),",
    `${label}: bounded owner code`);
  requireText(mapper, "retryable = error.retryable,", `${label}: retryability`);
  requireText(mapper, "error_kind,", `${label}: error kind`);
  requireText(mapper, "public_code = code,", `${label}: public code`);
  requireText(mapper, "status = %status,", `${label}: status`);
  requireText(mapper, "boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,", `${label}: boundary`);
  for (const forbidden of ["%error", "error.message", ".to_string()", "format!(", "?error"]) {
    forbidText(mapper, forbidden, `${label}: raw error payload`);
  }
}

for (const [mapper, label, identity] of [
  [commandMapper, "command mapper", "fulfillment_id_non_nil = !fulfillment_id.is_nil(),"],
  [createMapper, "create mapper", "order_id_non_nil = !order_id.is_nil(),"],
  [readMapper, "read mapper", "fulfillment_id_present = fulfillment_id.is_some(),"],
]) requireText(mapper, identity, `${label}: resource shape`);

requireText(commandMapper, "operation,", "command mapper: operation label");
requireText(createMapper, 'operation = "create_fulfillment",', "create mapper: operation label");
requireText(
  createMapper,
  '"commerce admin fulfillment cross-owner create failed"',
  "create mapper: diagnostic message",
);
requireText(
  commandMapper,
  '"commerce admin fulfillment owner command failed"',
  "command mapper: diagnostic message",
);
requireText(
  readMapper,
  '"commerce admin fulfillment owner read failed"',
  "read mapper: diagnostic message",
);
requireTextNormalized(
  readMapper,
  "operation,\n        correlation_id = %context.correlation_id,",
  "read mapper: operation before the correlation key",
);

// --- Routes -------------------------------------------------------------------------------------

const routes = [
  ["list_fulfillments", "[Permission::FULFILLMENTS_READ]", ".list_fulfillment_projections(", "map_fulfillment_read_error("],
  ["show_fulfillment", "[Permission::FULFILLMENTS_READ]", ".read_fulfillment_projection(", "map_fulfillment_read_error("],
  ["create_fulfillment", "[Permission::FULFILLMENTS_CREATE]", ".create_manual_fulfillment(", "map_fulfillment_create_error("],
  ["ship_fulfillment", "[Permission::FULFILLMENTS_UPDATE]", ".ship_fulfillment(", "map_fulfillment_command_error("],
  ["deliver_fulfillment", "[Permission::FULFILLMENTS_UPDATE]", ".deliver_fulfillment(", "map_fulfillment_command_error("],
  ["reopen_fulfillment", "[Permission::FULFILLMENTS_UPDATE]", ".reopen_fulfillment(", "map_fulfillment_command_error("],
  ["reship_fulfillment", "[Permission::FULFILLMENTS_UPDATE]", ".reship_fulfillment(", "map_fulfillment_command_error("],
  ["cancel_fulfillment", "[Permission::FULFILLMENTS_UPDATE]", ".cancel_fulfillment(", "map_fulfillment_command_error("],
];

for (const [handler, permission, call, mapper] of routes) {
  const block = between(
    controller,
    `pub async fn ${handler}(`,
    "\n}\n",
    `${handler} route`,
  );
  requireText(block, "ensure_permissions(", `${handler}: permission gate`);
  requireText(block, permission, `${handler}: permission set`);
  requireText(block, '"commerce_invalid_tenant"', `${handler}: tenant guard`);
  requireText(block, `log_admin_fulfillment_route_entry(`, `${handler}: bounded entry line`);
  requireTextNormalized(block, `"${handler}",`, `${handler}: bounded entry operation label`);
  requireText(block, call, `${handler}: owner port call`);
  requireText(block, mapper, `${handler}: mapper handoff`);
  requireText(block, ".map_err(", `${handler}: typed mapping closure`);
}

requireCount(
  controller,
  /map_fulfillment_command_error\(/g,
  6,
  "one command mapper and five callsites",
);
requireCount(controller, /map_fulfillment_read_error\(/g, 3, "one read mapper and two callsites");
requireCount(controller, /map_fulfillment_create_error\(/g, 2, "one create mapper and one callsite");

for (const [value, label] of [
  ["fn admin_fulfillment_create_read_context(", "create read context"],
  ["runtime.order_read_port(),", "order read port"],
  ["runtime.fulfillment_read_port(),", "fulfillment read port"],
  ["runtime.shipping_option_read_port(),", "shipping option read port"],
  ["runtime.fulfillment_admin_create_command_port(),", "fulfillment create command port"],
  ["AdminManualFulfillmentOrchestrationService::new(", "manual fulfillment orchestration"],
  [".create_manual_fulfillment(read_context, write_context.clone(), input)", "create orchestration call"],
  ["ShipAdminFulfillmentRequest {", "typed ship request"],
  ["ListFulfillmentProjectionsRequest {", "typed list request"],
  ["ReadFulfillmentProjectionRequest { fulfillment_id: id }", "typed read request"],
]) requireTextNormalized(controller, value, `fulfillment orchestration: ${label}`);

// --- Owner side: typed error becomes a bounded PortError -----------------------------------------

for (const [value, label] of [
  ["pub(crate) fn map_fulfillment_error_without_context(error: FulfillmentError) -> PortError {",
   "owner translation entry point"],
  ['PortError::validation("fulfillment.validation"', "owner validation code"],
  ['"fulfillment.reconciliation_required"', "owner reconciliation code"],
  ['PortError::not_found(\n            "fulfillment.not_found"', "owner not-found code"],
  ['"fulfillment.invalid_transition"', "owner transition code"],
  ['"fulfillment.shipping_option_translation_revision_conflict"', "owner revision-conflict code"],
  ['"fulfillment.database_unavailable"', "owner storage code"],
]) requireTextNormalized(ownerErrors, value, `rustok-fulfillment errors: ${label}`);

for (const [content, label] of [
  [ownerErrors, "rustok-fulfillment errors"],
  [controller, "admin fulfillment controller"],
]) {
  for (const forbidden of ["error.to_string()", "err.to_string()", "error.message", "format!(\"{error}"]) {
    forbidText(content, forbidden, `${label}: owner text leak`);
  }
}

if (failures.length > 0) {
  console.error("Commerce admin fulfillment route error-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin fulfillment routes map owner-port failures through one code-first policy, one envelope constructor, and bounded diagnostics",
);
