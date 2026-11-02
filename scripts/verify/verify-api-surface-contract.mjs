#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { containsMarker } from "./lib/source-text.mjs";

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const failures = [];

function read(relativePath) {
  const fullPath = path.join(root, relativePath);
  if (!fs.existsSync(fullPath)) {
    failures.push(`missing ${relativePath}`);
    return "";
  }
  return fs.readFileSync(fullPath, "utf8");
}

function requireContains(relativePath, marker, description) {
  if (!read(relativePath).includes(marker)) failures.push(description);
}

function requireNotContains(relativePath, marker, description) {
  if (read(relativePath).includes(marker)) failures.push(description);
}

const axumHostContracts = [
  [
    "apps/server/src/host.rs",
    "pub async fn run",
    "server host exposes the Axum entrypoint",
  ],
  [
    "apps/server/src/services/server_bootstrap.rs",
    "bootstrap_application_router",
    "server owns explicit router bootstrap",
  ],
  [
    "apps/server/src/services/app_router.rs",
    "Router",
    "server composes an Axum router",
  ],
  [
    "apps/server/src/services/server_runtime_context.rs",
    "pub struct ServerRuntimeContext",
    "server owns typed runtime state",
  ],
  [
    "apps/server/src/services/server_runtime_context.rs",
    "pub fn shared_get<T>",
    "server runtime exposes typed shared handles",
  ],
  [
    "crates/libs/rustok-api/src/runtime.rs",
    "pub struct HostRuntimeContext",
    "API owns the host-neutral runtime context",
  ],
  [
    "crates/libs/rustok-api/src/runtime.rs",
    "pub fn with_shared_value<T>",
    "host runtime accepts typed values",
  ],
  [
    "crates/libs/rustok-web/src/lib.rs",
    "json_response",
    "rustok-web owns response formatting",
  ],
  [
    "crates/libs/rustok-runtime/src/lib.rs",
    "RuntimeComposition",
    "runtime composition is owned by rustok-runtime",
  ],
  [
    "crates/utils/rustok-cli/src/main.rs",
    "run_with_environment",
    "CLI owns standalone command execution",
  ],
];

for (const [relativePath, marker, description] of axumHostContracts) {
  requireContains(relativePath, marker, description);
}

for (const relativePath of [
  "crates/modules/alloy/Cargo.toml",
  "crates/modules/rustok-ai/Cargo.toml",
  "crates/modules/rustok-blog/Cargo.toml",
  "crates/modules/rustok-commerce/Cargo.toml",
  "crates/modules/rustok-content-orchestration/Cargo.toml",
  "crates/modules/rustok-forum/Cargo.toml",
  "crates/modules/rustok-outbox/Cargo.toml",
  "crates/modules/rustok-pages/Cargo.toml",
]) {
  requireNotContains(
    relativePath,
    "apps/server",
    `${relativePath} must not depend on the composition host`,
  );
}

for (const [relativePath, marker, description] of [
  [
    "crates/modules/alloy/src/controllers/mod.rs",
    "AlloyHttpRuntime",
    "Alloy HTTP owns narrow runtime state",
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/mod.rs",
    "CommerceHttpRuntime",
    "commerce HTTP owns narrow runtime state",
  ],
  [
    "crates/modules/rustok-blog/src/controllers/mod.rs",
    "axum_router",
    "blog exposes an Axum router",
  ],
  [
    "crates/modules/rustok-pages/src/controllers/mod.rs",
    "axum_router",
    "pages exposes an Axum router",
  ],
  [
    "crates/modules/rustok-ai/src/service/types.rs",
    "AiHostRuntime",
    "AI owns a typed runtime contract",
  ],
  [
    "crates/modules/rustok-content-orchestration/src/lib.rs",
    "build_content_orchestration_service",
    "content orchestration exposes typed construction",
  ],
]) {
  requireContains(relativePath, marker, description);
}

for (const relativePath of [
  "apps/server/src/services/email.rs",
  "apps/server/src/services/event_bus.rs",
  "apps/server/src/services/graphql_schema.rs",
  "apps/server/src/services/runtime_guardrails.rs",
]) {
  requireContains(
    relativePath,
    "ServerRuntimeContext",
    `${relativePath} consumes typed server runtime state`,
  );
}

for (const [relativePath, marker, description] of [
  [
    "crates/libs/rustok-api/src/context/tenant.rs",
    "pub struct TenantContext",
    "rustok-api retains the shared TenantContext contract",
  ],
  [
    "crates/libs/rustok-api/src/context/tenant.rs",
    "pub struct OptionalTenant",
    "rustok-api retains the optional tenant extractor contract",
  ],
]) {
  requireContains(relativePath, marker, description);
}

for (const [relativePath, marker, description] of [
  [
    "crates/libs/rustok-api/src/context/tenant.rs",
    "pub enum TenantError",
    "rustok-api tenant context must not reintroduce the retired persistence-backed TenantError",
  ],
  [
    "crates/libs/rustok-api/src/context/tenant.rs",
    "sea_orm::DbErr",
    "rustok-api tenant context must not depend on SeaORM error types",
  ],
  [
    "crates/libs/rustok-api/src/context/mod.rs",
    "TenantError",
    "rustok-api context module must not re-export the retired TenantError",
  ],
  [
    "crates/libs/rustok-api/src/lib.rs",
    "TenantError",
    "rustok-api root must not re-export the retired TenantError",
  ],
]) {
  requireNotContains(relativePath, marker, description);
}

// HTTP retry advice has one wire owner. This is source evidence, not runtime execution.
function requireCode(relativePath, marker, description) {
  if (!containsMarker(read(relativePath), marker)) failures.push(description);
}

for (const [marker, description] of [
  [
    "pub retryable: Option<bool>",
    "HTTP errors preserve unknown advice explicitly",
  ],
  [
    'skip_serializing_if = "Option::is_none"',
    "HTTP JSON omits unknown, not explicit false",
  ],
  [
    'deserialize_with = "deserialize_retryable"',
    "HTTP JSON rejects malformed present advice",
  ],
  [
    "bool::deserialize(deserializer).map(Some)",
    "HTTP advice accepts only a present boolean",
  ],
  [
    "#[schema(nullable = false)]",
    "HTTP schema declares an optional non-nullable member",
  ],
  [
    "self.retryable = Some(retryable)",
    "HTTP advice setter retains either owner bit",
  ],
  [
    "retryable: self.retryable",
    "HTTP body forwards owner advice without reconstructing policy",
  ],
  [
    "(self.status, Json(self.body())).into_response()",
    "HTTP response uses the canonical body",
  ],
]) {
  requireCode("crates/libs/rustok-web/src/lib.rs", marker, description);
}
requireCode(
  "apps/server/src/error.rs",
  "Self::Http(error) => error.into_response()",
  "server host delegates the typed HTTP envelope unchanged",
);

// The mapper list pins the completed typed-owner slice, including nested and final runtime policy.
const httpOwnerForwarders = [
  [
    "crates/modules/rustok-commerce/src/controllers/products.rs",
    "map_admin_product_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/changes.rs",
    "map_admin_order_change_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs",
    "map_fulfillment_command_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs",
    "map_fulfillment_create_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs",
    "map_fulfillment_read_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/orders_owner_ports.rs",
    "map_order_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/orders_owner_ports.rs",
    "map_payment_detail_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/orders_owner_ports.rs",
    "map_fulfillment_detail_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/payments_owner_reads.rs",
    "map_payment_read_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/payments_owner_reads.rs",
    "map_payment_command_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/payments_owner_reads.rs",
    "map_refund_command_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/post_order_commands.rs",
    "map_admin_post_order_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/post_order_reads.rs",
    "map_admin_post_order_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/product_media.rs",
    "map_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/returns.rs",
    "map_admin_return_decision_order_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/returns.rs",
    "map_admin_return_decision_payment_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/shipping.rs",
    "map_admin_shipping_option_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/checkout.rs",
    "payment_collection_http_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/orders.rs",
    "map_storefront_order_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/orders.rs",
    "map_storefront_order_command_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/orders.rs",
    "map_storefront_payment_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/products.rs",
    "map_storefront_product_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/products.rs",
    "map_storefront_shipping_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/products_owner_list.rs",
    "map_storefront_product_list_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/carts/shipping_owner_reads.rs",
    "map_shipping_port_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-media/src/controllers/mod.rs",
    "public_image_error",
    "error.retryable",
    1,
  ],
  [
    "crates/libs/rustok-web/src/lib.rs",
    "port_error_to_http_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/store/checkout.rs",
    "storefront_checkout_http_error",
    "error.retryable()",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/admin/mod.rs",
    "map_post_order_orchestration_error",
    "error.retryable",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/marketplace_financial.rs",
    "map_inbox_error",
    "error.retryable()",
    1,
  ],
  [
    "crates/modules/rustok-commerce/src/controllers/marketplace_reversal_financial.rs",
    "map_operator_error",
    "error.retryable()",
    2,
  ],
];
for (const [
  relativePath,
  name,
  decision,
  expectedCount,
] of httpOwnerForwarders) {
  const source = read(relativePath);
  const start = source.indexOf(`fn ${name}(`);
  const end = source.indexOf("\n}", start);
  if (start < 0 || end < start) {
    failures.push(`${relativePath}: missing top-level HTTP mapper ${name}`);
    continue;
  }
  const body = source.slice(start, end + 2);
  const marker = `.with_retryable(${decision})`;
  const count =
    body.replace(/\s+/g, "").split(marker.replace(/\s+/g, "")).length - 1;
  if (count !== expectedCount)
    failures.push(
      `${relativePath}: ${name} must forward ${decision} exactly ${expectedCount} time(s)`,
    );
}

for (const relativePath of [
  "apps/server/src/controllers/swagger.rs",
  "crates/modules/rustok-commerce/src/openapi.rs",
]) {
  requireCode(
    relativePath,
    "rustok_web::ErrorBody,",
    `${relativePath} registers the canonical error schema`,
  );
}
for (const relativePath of [
  "crates/modules/rustok-commerce/src/controllers/store/carts.rs",
  "crates/modules/rustok-commerce/src/controllers/store/checkout.rs",
  "crates/modules/rustok-commerce/src/controllers/store/products.rs",
  "crates/modules/rustok-commerce/src/controllers/store/products_owner_list.rs",
]) {
  requireCode(
    relativePath,
    "body = rustok_web::ErrorBody",
    `${relativePath} references the canonical error response schema`,
  );
}

const httpClient = "apps/next-admin/src/shared/api/http.ts";
for (const marker of [
  "'owner' | 'absent' | 'invalid'",
  "Object.hasOwn(body, 'retryable')",
  "typeof body.retryable === 'boolean'",
  "retryable = body.retryable",
  "MAX_ERROR_BODY_BYTES = 16_384",
  "MAX_ERROR_BODY_WAIT_MS = 2_000",
  "void activeReader.cancel().catch(() => undefined)",
  "Promise.race([payload, expired])",
  "clearTimeout(timer)",
  "reader?.releaseLock()",
])
  requireCode(httpClient, marker, `Next HTTP decoder must retain ${marker}`);
for (const marker of [
  "Boolean(",
  "response.text()",
  "fetch(",
  "status >=",
  "status === 503",
  "status === 409",
]) {
  requireNotContains(
    httpClient,
    marker,
    `Next HTTP decoder must not derive advice or replay requests via ${marker}`,
  );
}
requireCode(
  "apps/next-admin/src/shared/api/modules.ts",
  "throw await readHttpError(",
  "registry REST client consumes the shared typed HTTP decoder",
);
requireNotContains(
  "apps/next-admin/src/shared/api/modules.ts",
  "if (parsed.message)",
  "registry REST client must not retain the removed raw-body error parser",
);
requireCode(
  "apps/next-admin/src/shared/api/index.ts",
  "export * from './http'",
  "Next shared API exports the HTTP error projection",
);

if (failures.length > 0) {
  console.error("API surface contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("API surface contract verification passed");
