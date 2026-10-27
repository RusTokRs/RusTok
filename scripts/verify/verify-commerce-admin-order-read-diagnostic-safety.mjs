#!/usr/bin/env node

// Admin order read diagnostic safety.
//
// The mounted Commerce admin order reads are `list_orders` and `show_order` in
// `controllers/admin/orders_owner_ports.rs`. They read three host-selected ports — order projections,
// latest payment collection, latest fulfillment — and each failure goes through a dedicated mapper
// that logs shape only: identity as presence/nil-shape, the owner code as a length, retryability, the
// projected error kind, the public code, and the status.
//
// `show_order` is the only place the detail fan-out exists, and the detail mappers are never reused by
// a mutation: a read failure may not be reported through a write envelope, and a write failure may not
// be reported through the detail ones. The verifier used to describe the retired
// `controllers/admin/orders.rs` (`AdminOrderReadDiagnosticContext`, `AdminOrderReadPortDiagnosticContext`,
// `AdminOrderReadPortDiagnosticError`, `text_presence_shape`) — an unmounted duplicate of these routes,
// now deleted. It is rewritten against the live controller so it can no longer certify dead code.

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
    "crates/modules/rustok-commerce/contracts/evidence/admin-order-read-diagnostic-safety-source-review.json",
  doc: "crates/modules/rustok-commerce/docs/admin-order-read-diagnostic-safety.md",
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

// --- Port context: bounded, scoped, and time-limited --------------------------------------------

const portContext = blockBetween(
  source,
  "fn admin_order_port_context(",
  "fn admin_order_command_port_context(",
  "read port context",
);
requireText(portContext, "let resource_id = order_id.unwrap_or(tenant.id);", "resource scoping");
requireText(portContext, "PortContext::new(", "typed port context");
requireText(portContext, "tenant.id.to_string(),", "tenant scoping");
requireText(portContext, "PortActor::user(auth.user_id.to_string()),", "actor scoping");
requireText(portContext, "request_context.locale.as_str(),", "locale forwarding");
requireTextNormalized(
  portContext,
  'format!("commerce-admin-order:{operation}:{resource_id}")',
  "correlation identity",
);
requireText(
  portContext,
  ".with_deadline(std::time::Duration::from_secs(2));",
  "read deadline",
);
requireText(portContext, "match request_context.channel_slug.as_deref() {", "channel presence");
requireText(portContext, "Some(channel) => context.with_channel(channel),", "channel forwarding");
for (const forbidden of ["tracing::", "format!(\"{}\", auth", "println!"]) {
  forbidText(portContext, forbidden, "port context logging side effect");
}

// --- Reads never write and never take a write identity ------------------------------------------

const listRoute = blockBetween(
  source,
  "pub async fn list_orders(",
  "\n}\n",
  "list orders route",
);
const showRoute = blockBetween(
  source,
  "pub async fn show_order(",
  "\n}\n",
  "show order route",
);
for (const [block, label] of [
  [listRoute, "list orders route"],
  [showRoute, "show order route"],
]) {
  requireText(block, "ensure_permissions(", `${label}: permission gate`);
  requireText(block, "log_admin_order_entry(", `${label}: bounded entry line`);
  requireText(block, ".map_err(|error| {", `${label}: typed mapping closure`);
  requireText(block, "map_order_port_error(", `${label}: shared port mapper handoff`);
  for (const forbidden of [
    "HeaderMap",
    "require_idempotency_key",
    "Idempotency-Key",
    ".mark_paid(",
    ".ship(",
    ".deliver(",
    ".cancel(",
  ]) forbidText(block, forbidden, `${label}: read route must not write`);
}
requireText(listRoute, "&[Permission::ORDERS_LIST]", "list permission");
requireText(listRoute, '"Permission denied: orders:list required"', "list permission message");
requireText(listRoute, '"list_orders",', "list operation label");
requireTextNormalized(
  listRoute,
  'admin_order_port_context(&tenant, &auth, &request_context, None, "list_orders")',
  "list port context",
);
requireText(listRoute, ".order_read_port()", "host-selected order read port");
requireText(listRoute, ".list_order_projections(", "list owner read");
requireText(listRoute, "ListOrderProjectionsRequest {", "typed list request");
requireText(listRoute, "page: pagination.page,", "list page forwarding");
requireText(listRoute, "per_page: pagination.limit(),", "list page-size forwarding");
requireText(listRoute, "status: params.status,", "list status filter");
requireText(listRoute, "customer_id,", "list customer filter");
requireText(
  listRoute,
  "tenant_default_locale: Some(tenant.default_locale.clone()),",
  "list locale fallback",
);
requireText(
  listRoute,
  "params.status.is_some(),",
  "list status filter logged as presence only",
);
requireText(
  listRoute,
  "params.customer_id.is_some(),",
  "list customer filter logged as presence only",
);
requireText(
  listRoute,
  "super::super::common::PaginationMeta::new(",
  "list pagination meta",
);

requireText(showRoute, "&[Permission::ORDERS_READ]", "read permission");
requireText(showRoute, '"Permission denied: orders:read required"', "read permission message");
requireText(showRoute, '"commerce_invalid_tenant"', "tenant guard");
requireText(showRoute, '"commerce_invalid_id"', "id guard");
requireText(showRoute, '"show_order",', "show operation label");
requireTextNormalized(
  showRoute,
  'admin_order_port_context(&tenant, &auth, &request_context, Some(id), "get_order")',
  "order read port context",
);
requireText(showRoute, ".order_read_port()", "host-selected order read port");
requireText(showRoute, ".read_order_projection(", "detail owner read");
requireText(showRoute, "ReadOrderProjectionRequest {", "typed detail request");
requireText(showRoute, "order_id: id,", "detail order identity");
requireText(
  showRoute,
  "tenant_default_locale: Some(tenant.default_locale.clone()),",
  "detail locale fallback",
);
requireText(showRoute, '"get_order",', "order read operation label");
requireTextNormalized(
  showRoute,
  'admin_order_port_context(&tenant, &auth, &request_context, Some(id), "payment_detail")',
  "payment read context",
);
requireText(showRoute, ".payment_order_read_port()", "host-selected payment read port");
requireText(
  showRoute,
  ".find_latest_collection_by_order(",
  "latest payment collection read",
);
requireText(showRoute, "LatestPaymentCollectionByOrderRequest { order_id: id },", "typed payment read");
requireText(
  showRoute,
  "map_payment_detail_port_error(tenant.id, id, &payment_context, error)",
  "payment detail mapper handoff",
);
requireText(showRoute, '"fulfillment_detail",', "fulfillment read operation label");
requireCount(
  showRoute,
  /admin_order_port_context\(/g,
  3,
  "order, payment, and fulfillment read contexts",
);
requireText(showRoute, ".fulfillment_read_port()", "host-selected fulfillment read port");
requireText(
  showRoute,
  ".find_latest_fulfillment_by_order_projection(",
  "latest fulfillment read",
);
requireText(
  showRoute,
  "FindLatestFulfillmentByOrderProjectionRequest { order_id: id },",
  "typed fulfillment read",
);
requireText(
  showRoute,
  "map_fulfillment_detail_port_error(tenant.id, id, &fulfillment_context, error)",
  "fulfillment detail mapper handoff",
);
requireText(showRoute, "Ok(Json(AdminOrderDetailResponse {", "detail response");
requireText(showRoute, "payment_collection,", "payment collection in the response");
requireText(showRoute, "fulfillment,", "fulfillment in the response");
requireOrder(
  showRoute,
  [
    "if tenant.id.is_nil()",
    "if id.is_nil()",
    "log_admin_order_entry(",
    ".read_order_projection(",
    ".find_latest_collection_by_order(",
    ".find_latest_fulfillment_by_order_projection(",
  ],
  "show order read fan-out order",
);

// Read/write separation across the whole file.
requireCount(
  source,
  /let idempotency_key = require_idempotency_key\(&headers\)\?;/g,
  4,
  "exactly the four mutations take a caller-owned write identity",
);
requireCount(
  source,
  /headers: HeaderMap,/g,
  4,
  "exactly the four mutations accept the header map",
);
requireCount(source, /fn admin_order_command_port_context\(/g, 1, "single command context builder");
requireCount(source, /fn admin_order_port_context\(/g, 1, "single read context builder");

// --- Detail mappers: static envelopes over the shared 7-variant kind set ------------------------

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}

const detailMappers = [
  {
    name: "map_payment_detail_port_error",
    end: "fn map_fulfillment_detail_port_error(",
    owner: "ADMIN_ORDER_DETAIL_PAYMENT_OWNER",
    operation: '"find_latest_payment_collection_by_order"',
    message: '"commerce admin order detail payment owner read failed"',
    codes: [
      ["StatusCode::BAD_REQUEST", "commerce_admin_payment_invalid"],
      ["StatusCode::NOT_FOUND", "commerce_admin_not_found"],
      ["StatusCode::CONFLICT", "commerce_admin_payment_state_conflict"],
      ["StatusCode::FORBIDDEN", "commerce_permission_denied"],
      ["StatusCode::SERVICE_UNAVAILABLE", "commerce_admin_payment_storage_unavailable"],
      ["StatusCode::CONFLICT", "commerce_admin_payment_reconciliation_required"],
    ],
    callsite: "map_payment_detail_port_error(tenant.id, id, &payment_context, error)",
  },
  {
    name: "map_fulfillment_detail_port_error",
    end: "\n#[utoipa::path(",
    owner: "ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER",
    operation: '"find_latest_fulfillment_by_order_projection"',
    message: '"commerce admin order detail fulfillment owner read failed"',
    codes: [
      ["StatusCode::BAD_REQUEST", "commerce_admin_fulfillment_invalid"],
      ["StatusCode::NOT_FOUND", "commerce_admin_not_found"],
      ["StatusCode::CONFLICT", "commerce_admin_fulfillment_state_conflict"],
      ["StatusCode::FORBIDDEN", "commerce_permission_denied"],
      ["StatusCode::SERVICE_UNAVAILABLE", "commerce_admin_fulfillment_storage_unavailable"],
      ["StatusCode::INTERNAL_SERVER_ERROR", "commerce_admin_fulfillment_failed"],
    ],
    callsite:
      "map_fulfillment_detail_port_error(tenant.id, id, &fulfillment_context, error)",
  },
];

for (const mapper of detailMappers) {
  const block = blockBetween(source, `fn ${mapper.name}(`, mapper.end, `${mapper.name} block`);
  for (const variant of portErrorKinds) {
    requirePattern(
      block,
      new RegExp(`PortErrorKind::${variant}\\b`),
      `${mapper.name} handles PortErrorKind::${variant}`,
    );
  }
  forbidText(block, "        _ =>", `${mapper.name}: wildcard arm`);
  for (const [status, code] of mapper.codes) {
    requireTextNormalized(block, status, `${mapper.name}: status`);
    requireTextNormalized(block, `"${code}"`, `${mapper.name}: public code`);
  }
  for (const [value, label] of [
    ["let (status, code, message) = match &error.kind {", "typed policy selection first"],
    ["let error_kind = port_error_kind(&error);", "error kind projection"],
    ["tracing::error!(", "diagnostic line"],
    ["HttpError::new(status, code, message)", "single static envelope"],
    [`owner = ${mapper.owner},`, "detail owner label"],
    ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape"],
    ["order_id_non_nil = !order_id.is_nil(),", "order shape"],
    [`operation = ${mapper.operation},`, "owner operation"],
    ["correlation_id = %context.correlation_id,", "correlation join key"],
    ["owner_code_length = error.code.chars().count(),", "bounded owner code"],
    ["retryable = error.retryable,", "retryability"],
    ["error_kind,", "error kind"],
    ["public_code = code,", "public code"],
    ["status = %status,", "status"],
    ["boundary = ADMIN_ORDER_DETAIL_BOUNDARY,", "detail boundary"],
    [mapper.message, "static message"],
  ]) requireText(block, value, `${mapper.name}: ${label}`);
  requireOrder(
    block,
    [
      "let (status, code, message) = match &error.kind {",
      "let error_kind = port_error_kind(&error);",
      "tracing::error!(",
      "HttpError::new(status, code, message)",
    ],
    `${mapper.name} order`,
  );
  for (const forbidden of [
    "%error",
    "error.message",
    "error.to_string()",
    "format!(",
    "tenant_id = ",
    "order_id = ",
    "?context",
  ]) forbidText(block, forbidden, `${mapper.name}: raw diagnostic payload`);
  requireCount(
    source,
    new RegExp(`${mapper.name}\\(`, "g"),
    2,
    `${mapper.name}: one definition and one show-order callsite`,
  );
  requireCount(source, new RegExp(mapper.callsite.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "g"), 1,
    `${mapper.name}: single callsite`);
}

forbidRawContextLogging(failures, source, "admin order controller");

// --- Evidence and documentation -----------------------------------------------------------------

if (
  evidence.status !==
  "commerce_admin_order_read_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_port_error_logged: false,
  raw_correlation_id_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_order_uuid_logged: false,
  raw_customer_uuid_logged: false,
  bounded_owner_code_logged: true,
  retryability_logged: true,
  required_uuid_shapes_logged: true,
  filter_presence_logged: true,
  detail_payment_mapper_bounded: true,
  detail_fulfillment_mapper_bounded: true,
  three_read_ports_used: true,
  read_routes_never_write: true,
  read_deadline_bounded: true,
  host_selected_read_port_used: true,
  http_envelopes_preserved: true,
  retired_legacy_controller_deleted: true,
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

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "`controllers/admin/orders_owner_ports.rs`",
  "`OrderReadPort`",
  "`PaymentOrderReadPort`",
  "`FulfillmentReadPort`",
  "`owner_code_length`",
  "That file is deleted",
  "The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.",
]) requireTextNormalized(doc, marker, `${paths.doc}: documentation contract`);
forbidText(doc, "shadowed before `tracing::error!`", `${paths.doc}: retired shadowing contract`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

// --- Broad verifier stays aligned ---------------------------------------------------------------

for (const marker of [
  "[Permission::ORDERS_LIST]",
  "[Permission::ORDERS_READ]",
  ".order_read_port()",
  ".read_order_projection(",
  "map_order_port_error(",
]) requireText(broadVerifier, marker, `${paths.broadVerifier}: compatibility marker`);

if (failures.length > 0) {
  console.error("Commerce admin order read diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin order reads use host-selected order, payment, and fulfillment ports with bounded diagnostics, static envelopes, and no write identity",
);
