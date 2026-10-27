#!/usr/bin/env node

// Admin order mutation diagnostic safety.
//
// The four mounted admin order mutations — mark paid, ship, deliver, cancel — live in
// `controllers/admin/orders_owner_ports.rs` and go through `OrderAdminCommandPort` with typed
// `Owner*Request` values. They share `map_order_port_error`, which is also the read mapper, so a
// mutation cannot drift into its own envelope set or its own log shape.
//
// The verifier used to describe the retired `controllers/admin/orders.rs` (an
// `AdminOrderErrorContext` / `AdminOrderMutationDiagnosticContext` / `AdminOrderMutationDiagnosticError`
// trio with a `redacted` Debug marker and a typed `OrderError` policy). That file is not part of the
// module tree — it was never mounted, and it is now deleted, because keeping a second, stale copy of
// the same routes is the exact failure mode that lets a guard "pass" while certifying dead code. The
// guard is written against the live controller: the shared PortError policy, the bounded facts, the
// caller-owned idempotency key, and the typed owner requests.

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
  source: "crates/modules/rustok-commerce/src/controllers/admin/orders_owner_ports.rs",
  retiredSource: "crates/modules/rustok-commerce/src/controllers/admin/orders.rs",
  moduleTree: "crates/modules/rustok-commerce/src/controllers/admin/mod.rs",
  portErrors: "crates/libs/rustok-api/src/ports.rs",
  evidence:
    "crates/modules/rustok-commerce/contracts/evidence/admin-order-mutation-diagnostic-safety-source-review.json",
  doc: "crates/modules/rustok-commerce/docs/admin-order-mutation-diagnostic-safety.md",
  broadVerifier: "scripts/verify/verify-commerce-admin-order-route-error-context.mjs",
  plan: "crates/modules/rustok-commerce/docs/implementation-plan.md",
};

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
const blockBetween = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate block`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};
const normalize = (content) => content.replace(/\s+/g, " ");
const requireTextNormalized = (content, value, label) => {
  if (!normalize(content).includes(normalize(value))) failures.push(`${label}: missing ${value}`);
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

const source = read(paths.source);
const moduleTree = read(paths.moduleTree);
const portErrors = read(paths.portErrors);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const broadVerifier = read(paths.broadVerifier);
const plan = read(paths.plan);

// --- The retired controller is gone -------------------------------------------------------------

requireText(
  moduleTree,
  '#[path = "orders_owner_ports.rs"]',
  `${paths.moduleTree}: admin order module is served by the owner-port file`,
);
let retiredExists = true;
try {
  read(paths.retiredSource);
} catch {
  retiredExists = false;
}
if (retiredExists) {
  failures.push(`${paths.retiredSource}: the unmounted legacy controller must not exist`);
}

// --- Shared mapper ------------------------------------------------------------------------------

const mapper = blockBetween(
  source,
  "fn map_order_port_error(",
  "fn map_payment_detail_port_error(",
  "admin order port mapper",
);
const kindProjection = blockBetween(
  source,
  "fn port_error_kind(",
  "fn map_order_port_error(",
  "port error kind projection",
);

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}
for (const variant of portErrorKinds) {
  requirePattern(
    mapper,
    new RegExp(`PortErrorKind::${variant}\\b`),
    `admin order mapper handles PortErrorKind::${variant}`,
  );
  requirePattern(
    kindProjection,
    new RegExp(`PortErrorKind::${variant}\\b`),
    `port error kind projection handles ${variant}`,
  );
}
forbidText(mapper, "        _ =>", "wildcard in the admin order mapper");

for (const [value, label] of [
  ["let (status, code, message) = match &error.kind {", "typed policy selection first"],
  ["let error_kind = port_error_kind(&error);", "error kind projection"],
  ["tracing::error!(", "diagnostic line"],
  ["HttpError::new(status, code, message)", "single static envelope"],
]) requireText(mapper, value, `admin order mapper: ${label}`);
requireOrder(
  mapper,
  [
    "let (status, code, message) = match &error.kind {",
    "let error_kind = port_error_kind(&error);",
    "tracing::error!(",
    "HttpError::new(status, code, message)",
  ],
  "admin order mapper order",
);

for (const [value, label] of [
  ["owner = ADMIN_ORDER_OWNER,", "owner label"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "actor shape"],
  ["order_id_present = order_id.is_some(),", "order presence"],
  ["order_id_non_nil = order_id.map(|value| !value.is_nil()).unwrap_or(false),", "order shape"],
  ["operation,", "route operation"],
  ["correlation_id = %context.correlation_id,", "correlation join key"],
  ["owner_code_length = error.code.chars().count(),", "bounded owner code"],
  ["retryable = error.retryable,", "retryability"],
  ["error_kind,", "error kind"],
  ["public_code = code,", "public code"],
  ["status = %status,", "status"],
  ["boundary = ADMIN_ORDER_BOUNDARY,", "boundary"],
  ['"commerce admin order owner port failed"', "static message"],
]) requireText(mapper, value, `admin order mapper: ${label}`);

for (const forbidden of ["%error", "error.message", ".to_string()", "format!(", "?error"]) {
  forbidText(mapper, forbidden, "raw admin order mapper payload");
}
forbidRawContextLogging(failures, source, "admin order controller");

// --- The four mutations -------------------------------------------------------------------------

const mutations = [
  {
    handler: "mark_order_paid",
    permission: "[Permission::ORDERS_UPDATE]",
    request: "OwnerMarkOrderPaidRequest {",
    call: ".mark_paid(",
    operation: '"mark_order_paid",',
  },
  {
    handler: "ship_order",
    permission: "[Permission::ORDERS_UPDATE]",
    request: "OwnerShipOrderRequest {",
    call: ".ship(",
    operation: '"ship_order",',
  },
  {
    handler: "deliver_order",
    permission: "[Permission::ORDERS_UPDATE]",
    request: "OwnerDeliverOrderRequest {",
    call: ".deliver(",
    operation: '"deliver_order",',
  },
  {
    handler: "cancel_order",
    permission: "[Permission::ORDERS_UPDATE]",
    request: "OwnerCancelOrderRequest {",
    call: ".cancel(",
    operation: '"cancel_order",',
  },
];

for (const mutation of mutations) {
  const block = blockBetween(
    source,
    `pub async fn ${mutation.handler}(`,
    "\n}\n",
    `${mutation.handler} route`,
  );
  requireText(block, "ensure_permissions(", `${mutation.handler}: permission gate`);
  requireText(block, mutation.permission, `${mutation.handler}: permission set`);
  requireText(block, '"commerce_invalid_tenant"', `${mutation.handler}: tenant guard`);
  requireText(block, '"commerce_invalid_id"', `${mutation.handler}: id guard`);
  requireText(block, "log_admin_order_entry(", `${mutation.handler}: bounded entry line`);
  requireTextNormalized(block, mutation.operation, `${mutation.handler}: operation label`);
  requireText(
    block,
    "let idempotency_key = require_idempotency_key(&headers)?;",
    `${mutation.handler}: caller-owned idempotency key`,
  );
  requireText(block, "admin_order_command_port_context(", `${mutation.handler}: command context`);
  requireText(block, "idempotency_key,", `${mutation.handler}: key forwarded to the context`);
  requireText(block, ".order_admin_command_port()", `${mutation.handler}: host-selected command port`);
  requireText(block, mutation.request, `${mutation.handler}: typed owner request`);
  requireText(block, mutation.call, `${mutation.handler}: owner command`);
  requireText(block, ".map_err(|error| {", `${mutation.handler}: typed mapping closure`);
  requireText(block, "map_order_port_error(", `${mutation.handler}: shared mapper handoff`);
  requireText(block, "Ok(Json(order))", `${mutation.handler}: response`);
  requireOrder(
    block,
    [
      "if tenant.id.is_nil()",
      "if id.is_nil()",
      "log_admin_order_entry(",
      "let idempotency_key = require_idempotency_key(&headers)?;",
      ".map_err(|error| {",
    ],
    `${mutation.handler} route order`,
  );
}

const idempotencyCount =
  source.match(/let idempotency_key = require_idempotency_key\(&headers\)\?;/g) ?? [];
if (idempotencyCount.length !== 4) {
  failures.push(
    `expected four admin order write handlers to require caller-owned Idempotency-Key, found ${idempotencyCount.length}`,
  );
}
requireCount(source, /\[Permission::ORDERS_UPDATE\]/g, 4, "four mutation permissions");
requireCount(
  source,
  /map_order_port_error\(/g,
  7,
  "one shared mapper and six callsites (list, detail, four mutations)",
);

// Retired in-process surface: the four mutations never build an owner service.
for (const forbidden of [
  "OrderService::new(",
  "PaymentService::new(",
  "FulfillmentService::new(",
  ".mark_paid(tenant.id",
  ".ship_order(tenant.id",
  ".deliver_order(tenant.id",
  ".cancel_order(tenant.id",
  "AdminOrderMutationDiagnosticContext",
  "AdminOrderMutationDiagnosticError",
]) forbidText(source, forbidden, "retired in-process admin order mutation surface");

// --- Evidence and documentation -----------------------------------------------------------------

if (
  evidence.status !==
  "commerce_admin_order_mutation_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_order_error_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_order_uuid_logged: false,
  bounded_owner_code_logged: true,
  required_uuid_shapes_logged: true,
  optional_uuid_shapes_logged: true,
  operation_preserved: true,
  shared_port_mapper_used: true,
  host_selected_command_port_used: true,
  caller_owned_idempotency_required: true,
  four_mutation_callsites_preserved: true,
  orders_update_permission_preserved: true,
  retired_legacy_controller_deleted: true,
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
  "`controllers/admin/orders_owner_ports.rs`",
  "`OrderAdminCommandPort`",
  "That file is deleted",
  "The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.",
]) requireTextNormalized(doc, marker, `${paths.doc}: documentation contract`);
forbidText(doc, "Debug output is always `redacted`", `${paths.doc}: retired contract`);

const reviewedScope = evidence.reviewed_scope ?? [];
for (const scope of reviewedScope) {
  if (scope.endsWith("controllers/admin/orders.rs")) {
    failures.push(`${paths.evidence}: reviewed_scope still cites ${scope}`);
  }
}
requireText(
  reviewedScope.join("\n"),
  "controllers/admin/orders_owner_ports.rs",
  `${paths.evidence}: reviewed_scope`,
);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

// --- Broad verifier stays aligned ---------------------------------------------------------------

for (const marker of [
  "forbidRawContextLogging(failures, controller,",
  "map_order_port_error(",
  "log_admin_order_entry(",
]) requireText(broadVerifier, marker, `${paths.broadVerifier}: compatibility marker`);

if (failures.length > 0) {
  console.error("Commerce admin order mutation diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin order mutations use the shared PortError mapper, host-selected command ports, caller-owned idempotency, and bounded diagnostics; the unmounted legacy controller is deleted",
);
