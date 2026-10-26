#!/usr/bin/env node

// Storefront order customer context.
//
// Every customer-owned storefront route resolves the caller's customer through the Customer owner
// port. A failure there decides whether the caller sees 200s at all, so it is diagnosed with the
// truthful owner, the exact owner operation, the consumer operation that triggered the read, and the
// bounded context and error facts — never the tenant value, the user id, or the owner message.
//
// The verifier used to require the raw form (`tenant_id = %context.tenant_id`, `user_id = %user_id`,
// `internal_message = %error.message`), which the hardened mapper no longer emits. It now forbids the
// raw form across the controller and requires the bounded facts, the shared safe HTTP mapping, and
// the ownership behaviour.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging, requireFactFields } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const controller = read("crates/modules/rustok-commerce/src/controllers/store/orders.rs");
const webErrors = read("crates/libs/rustok-web/src/lib.rs");
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
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

const customerMapper = between(
  controller,
  "fn map_storefront_customer_port_error(",
  "fn storefront_order_read_port_context(",
  "storefront order customer mapper",
);
const routeEntry = between(
  controller,
  "fn log_storefront_order_route_entry(",
  "fn map_storefront_customer_port_error(",
  "storefront order route entry diagnostics",
);
const customerLookup = between(
  controller,
  "async fn current_storefront_customer_id(",
  "async fn read_storefront_order_projection(",
  "storefront order customer lookup",
);
const ownership = between(
  controller,
  "async fn ensure_customer_owns_order(",
  "/// Get current storefront customer",
  "storefront order ownership helper",
);
const getMe = between(
  controller,
  "pub async fn get_me(",
  "/// Get customer-owned storefront order",
  "storefront current-customer route",
);

for (const [value, label] of [
  ["use rustok_api::{PortActor, PortContext, PortError, PortErrorKind, RequestContext, TenantContext};", "retained context imports"],
  ['const STOREFRONT_ORDER_CUSTOMER_OWNER: &str = "rustok_customer";', "truthful customer owner"],
  [
    'const STOREFRONT_ORDER_CUSTOMER_OWNER_OPERATION: &str = "read_customer_projection_by_user";',
    "exact customer owner operation",
  ],
  [
    'const STOREFRONT_ORDER_CUSTOMER_BOUNDARY: &str = "commerce_storefront_order_http";',
    "order customer boundary",
  ],
]) requireText(controller, value, label);

// The mapper: public HTTP value first, then diagnostics, then the unchanged return.
for (const [value, label] of [
  ["error: PortError", "original port error input"],
  ["context: &PortContext", "retained context input"],
  ["user_id: Uuid", "user identity input"],
  ["consumer_operation: &'static str", "consumer operation input"],
  ["let owner_error_kind = match &error.kind {", "typed failure classification"],
  ["let owner_code_length = error.code.chars().count();", "owner code shape"],
  ["let retryable = error.retryable;", "retryability evidence"],
  ["let public = port_error_to_http_error(error);", "unchanged safe HTTP mapping"],
  ["StatusCode::SERVICE_UNAVAILABLE", "unavailable status classification"],
  ["if critical {", "technical severity selection"],
  ["tracing::error!(", "technical error severity"],
  ["tracing::warn!(", "ordinary rejection severity"],
  ["owner = STOREFRONT_ORDER_CUSTOMER_OWNER", "owner field"],
  ["owner_operation = STOREFRONT_ORDER_CUSTOMER_OWNER_OPERATION", "owner operation field"],
  ["consumer_operation,", "consumer operation field"],
  ["user_id_non_nil = !user_id.is_nil()", "user identity shape"],
  ["owner_error_kind,", "typed failure label field"],
  ["owner_code_length,", "owner code shape field"],
  ["public_status = %public.status", "public status field"],
  ["boundary = STOREFRONT_ORDER_CUSTOMER_BOUNDARY", "boundary field"],
  ['"storefront customer read failed with bounded diagnostics"', "technical event"],
  ['"storefront customer read was rejected with bounded diagnostics"', "ordinary rejection event"],
  ["\n    public\n}", "mapped HTTP return"],
]) requireText(customerMapper, value, label);

requireFactFields(
  failures,
  customerMapper,
  [
    "correlation_id = %context.correlation_id",
    "tenant_id_length",
    "actor_kind",
    "actor_id_length",
    "claim_count",
    "role_count",
    "channel_present",
    "channel_length",
    "locale_length",
    "causation_id_present",
    "causation_id_length",
    "traceparent_present",
    "traceparent_length",
    "idempotency_key_present",
    "idempotency_key_length",
    "deadline_ms",
    "public_status",
  ],
  "customer mapper bounded context and public facts",
);
forbidRawContextLogging(failures, customerMapper, "customer mapper");

const mappingIndex = customerMapper.indexOf("let public = port_error_to_http_error(error);");
const errorLogIndex = customerMapper.indexOf("tracing::error!(");
const warnLogIndex = customerMapper.indexOf("tracing::warn!(");
const returnIndex = customerMapper.lastIndexOf("\n    public");
if (
  !(
    mappingIndex >= 0 &&
    mappingIndex < errorLogIndex &&
    errorLogIndex < warnLogIndex &&
    warnLogIndex < returnIndex
  )
) {
  failures.push("customer error must be mapped, diagnosed, and then returned in order");
}
forbidText(customerMapper, "port_error_to_http_error(error.clone())", "error clone before HTTP mapping");

// Route entry diagnostics are the same bounded event on every storefront order route: identity is
// reported as non-nil flags, never as values.
for (const [value, label] of [
  ["tenant_id_non_nil = !tenant_id.is_nil()", "tenant identity shape"],
  ["actor_id_non_nil = !actor_id.is_nil()", "actor identity shape"],
  ["order_id_present = order_id.is_some()", "order identity presence"],
  ["order_id_non_nil = order_id.is_some_and(|value| !value.is_nil())", "order identity shape"],
  ['"storefront order route entered with bounded diagnostics"', "route entry event"],
]) requireText(routeEntry, value, label);
forbidRawContextLogging(failures, routeEntry, "route entry diagnostics");
forbidText(controller, "tenant_id = %tenant.id", "raw tenant value in a route log line");
forbidText(controller, "actor_id = %auth.user_id", "raw actor value in a route log line");

for (const [content, values, label] of [
  [
    customerLookup,
    [
      "let customer_context = super::storefront_customer_port_context(tenant_id, auth.user_id);",
      "read_customer_projection_by_user(",
      "customer_context.clone(),",
      "CustomerUserProjectionRequest {",
      "user_id: auth.user_id,",
      'Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)',
      "&customer_context,",
      "auth.user_id,",
      "operation,",
    ],
    "order-access customer lookup",
  ],
  [
    getMe,
    [
      "super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;",
      "let customer_context = super::storefront_customer_port_context(tenant.id, auth.user_id);",
      "read_customer_projection_by_user(",
      "customer_context.clone(),",
      "CustomerUserProjectionRequest {",
      "user_id: auth.user_id,",
      'map_storefront_customer_port_error(error, &customer_context, auth.user_id, "get_me")',
      "Ok(Json(customer))",
    ],
    "current-customer route",
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

for (const [value, label] of [
  ["current_storefront_customer_id(runtime, tenant_id, auth, operation)", "ownership lookup reuse"],
  ["read_storefront_order_projection(", "owner projection read"],
  ["order.customer_id != Some(customer_id)", "ownership comparison"],
  ['"commerce_store_customer_required"', "customer-required envelope"],
  ['"commerce_store_order_access_denied"', "access-denied envelope"],
  ["Ok(customer_id)", "verified customer identity return"],
]) requireText(ownership, value, label);

const contextBindings =
  controller.match(/let customer_context = super::storefront_customer_port_context\(/g) ?? [];
if (contextBindings.length !== 2) {
  failures.push(`expected two retained customer contexts, found ${contextBindings.length}`);
}
const contextClones = controller.match(/customer_context\.clone\(\),/g) ?? [];
if (contextClones.length !== 2) {
  failures.push(`expected two customer context delegation clones, found ${contextClones.length}`);
}
const mapperUses = controller.match(/map_storefront_customer_port_error\(/g) ?? [];
if (mapperUses.length !== 3) {
  failures.push(`expected customer mapper definition plus two uses, found ${mapperUses.length}`);
}
const routeEntries = controller.match(/log_storefront_order_route_entry\(/g) ?? [];
if (routeEntries.length !== 7) {
  failures.push(`expected route entry helper definition plus six uses, found ${routeEntries.length}`);
}

for (const value of [
  "read_customer_projection_by_user(\n            super::storefront_customer_port_context(",
  "map_storefront_customer_port_error(\n            error, operation, tenant_id",
  'map_storefront_customer_port_error(error, "get_me", tenant.id)',
]) forbidText(controller, value, "context-dropping customer mapping");

for (const [value, label] of [
  ["PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE", "shared safe HTTP contract"],
  ["PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT", "shared timeout contract"],
  ["PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR", "shared invariant contract"],
  ['"The requested service is temporarily unavailable"', "shared unavailable envelope"],
  ['"The requested operation could not be completed"', "shared invariant envelope"],
]) requireText(webErrors, value, label);

if (failures.length > 0) {
  console.error("Commerce storefront order customer-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Storefront order customer reads keep the delegated PortContext, the exact owner and consumer operations, bounded diagnostics, and the safe HTTP envelopes",
);
