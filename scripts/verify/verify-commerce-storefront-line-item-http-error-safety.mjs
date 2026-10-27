#!/usr/bin/env node

// Storefront line-item HTTP error safety.
//
// Resolving a line item crosses three owners: the Product catalog read project, the Pricing read
// port, and the Inventory public-channel availability check. Every one of them can fail, and each
// failure must reach the caller as a static, typed HTTP envelope — never as an owner message, an
// owner code, or a raw database error — while the operator log keeps only the shape of the failure.
//
// The verifier used to anchor on the pre-`PortError` implementation: a `DbErr` mapper, SeaORM entity
// lookups inside the resolver, a `#[path]` module declaration, and raw diagnostic fields
// (`tenant_id = %tenant_id`, `variant_id = %variant_id`, `public_code = code`). The resolver now
// reads the Product owner projection through its port and the mappers share one bounded emitter, so
// the guard is written against that contract: typed ports in, static envelopes out, bounded facts
// logged, no raw value anywhere.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const storeModule = read("crates/modules/rustok-commerce/src/controllers/store/mod.rs");
const carts = read("crates/modules/rustok-commerce/src/controllers/store/carts.rs");
const boundary = read(
  "crates/modules/rustok-commerce/src/controllers/store/line_item_resolution.rs",
);
const portContract = read("crates/libs/rustok-api/src/ports.rs");
const commerceErrors = read("crates/modules/rustok-commerce-foundation/src/error.rs");
const inventoryOwner = read("crates/modules/rustok-inventory/src/services/public_channel.rs");
const productTests = read("crates/modules/rustok-commerce/src/controllers/store/tests/products.rs");
const testRoot = read("crates/modules/rustok-commerce/src/controllers/store/tests/mod.rs");
const failures = [];

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
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};
const from = (content, start, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex);
};

const facts = between(
  boundary,
  "struct StorefrontLineItemPortFacts {",
  "fn storefront_line_item_port_error_kind(",
  "line-item bounded fact projection",
);
const factConstructor = between(
  boundary,
  "impl StorefrontLineItemPortFacts {",
  "fn storefront_line_item_port_error_kind(",
  "line-item bounded fact constructor",
);
const emitter = between(
  boundary,
  "fn log_storefront_line_item_port_error(",
  "fn map_storefront_line_item_product_port_error(",
  "line-item bounded emitter",
);
const classifier = between(
  boundary,
  "fn is_technical_storefront_status(",
  "fn log_storefront_line_item_port_error(",
  "line-item severity classifier",
);
const productMapper = between(
  boundary,
  "fn map_storefront_line_item_product_port_error(",
  "fn pick_product_translation_response<'a>(",
  "line-item product mapper",
);
const pricingMapper = between(
  boundary,
  "fn map_storefront_line_item_pricing_error(",
  "fn map_storefront_line_item_inventory_error(",
  "line-item pricing mapper",
);
const inventoryMapper = between(
  boundary,
  "fn map_storefront_line_item_inventory_error(",
  "pub(crate) async fn resolve_store_line_item_input(",
  "line-item inventory mapper",
);
const resolver = between(
  boundary,
  "pub(crate) async fn resolve_store_line_item_input(",
  "pub(crate) async fn validate_store_line_item_quantity(",
  "line-item resolver",
);
const quantityValidator = between(
  boundary,
  "pub(crate) async fn validate_store_line_item_quantity(",
  "async fn validate_store_variant_inventory(",
  "line-item quantity validator",
);
const inventoryValidator = from(
  boundary,
  "async fn validate_store_variant_inventory(",
  "line-item inventory validator",
);

// --- mounting -----------------------------------------------------------------------------------

for (const [value, label] of [
  ["pub(crate) mod line_item_resolution;", "line-item module declaration"],
]) requireText(storeModule, value, label);

for (const [value, label] of [
  ["use super::line_item_resolution;", "line-item module use"],
  ["line_item_resolution::resolve_store_line_item_input(", "safe add-line resolver"],
  ["line_item_resolution::validate_store_line_item_quantity(", "safe quantity validator"],
]) requireText(carts, value, label);
for (const value of [
  'super::resolve_store_line_item_input(',
  'super::validate_store_line_item_quantity(',
  '#[path = "line_item_resolution.rs"]',
]) {
  forbidText(carts, value, "context-dependent line-item helper path");
}

// --- imports and boundary identity ---------------------------------------------------------------

for (const [value, label] of [
  ["use rustok_api::{PortContext, PortError};", "typed port imports"],
  ["use rustok_product::{", "typed Product owner import"],
  ["ProductCatalogReadPort", "Product catalog read port"],
  ["StorefrontVariantProductProjectionRequest", "typed Product projection request"],
  ["use rustok_pricing::ResolveProductPriceRequest;", "typed Pricing request"],
  ["use rustok_inventory::{", "typed Inventory owner import"],
  ["PublicChannelInventoryVariantProjectionInput", "typed Inventory projection"],
  ["check_variant_availability_for_public_channel", "Inventory owner availability read"],
  ["use rustok_web::{HttpError, HttpResult, port_error_to_http_error};", "shared safe port mapper"],
  ['boundary = "commerce_storefront_line_item_http"', "line-item boundary name"],
  ["use crate::{", "root-relative owner imports"],
  ["CommerceError,", "typed owner error import"],
]) {
  requireText(boundary, value, label);
}

// The resolver reaches siblings by absolute path so the module keeps working wherever it is
// mounted; `super::super::` silently breaks that.
for (const [value, label] of [
  ["crate::controllers::store::store_line_item_pricing_port_context(", "absolute pricing context"],
  ["crate::controllers::store::storefront_cart_pricing_snapshot(", "absolute pricing snapshot"],
  ["crate::controllers::store::merge_metadata(", "absolute metadata merge"],
  ["crate::controllers::store::seller_snapshot_metadata(", "absolute seller snapshot"],
]) {
  requireText(boundary, value, label);
}
forbidText(boundary, "super::super::", "context-dependent line-item helper path");

// --- bounded diagnostics -------------------------------------------------------------------------

for (const [value, label] of [
  ["correlation_id_length: usize,", "correlation length fact"],
  ["tenant_id_length: usize,", "tenant length fact"],
  ["channel_present: bool,", "channel presence fact"],
  ["variant_id_non_nil: bool,", "variant identity shape"],
  ["product_id_non_nil: Option<bool>,", "product identity shape"],
  ["owner_error_kind: &'static str,", "typed owner failure label"],
  ["owner_code_length: usize,", "owner code shape"],
  ["retryable: bool,", "owner retryability"],
]) requireText(facts, value, label);

for (const [value, label] of [
  ["fn new(", "fact constructor"],
  ["error: &PortError,", "typed owner error input"],
  ["context: &PortContext,", "retained context input"],
  ["storefront_line_item_port_error_kind(&error.kind)", "typed failure label derivation"],
  ["error.code.chars().count()", "owner code length derivation"],
  ["error.retryable", "owner retryability derivation"],
]) requireText(factConstructor, value, label);

for (const [value, label] of [
  ["fn storefront_line_item_port_error_kind(", "failure label mapper"],
  ["rustok_api::PortErrorKind::Validation => \"validation\"", "validation label"],
  ["rustok_api::PortErrorKind::NotFound => \"not_found\"", "not-found label"],
  ["rustok_api::PortErrorKind::Conflict => \"conflict\"", "conflict label"],
  ["rustok_api::PortErrorKind::Forbidden => \"forbidden\"", "forbidden label"],
  ["rustok_api::PortErrorKind::Unavailable => \"unavailable\"", "unavailable label"],
  ["rustok_api::PortErrorKind::Timeout => \"timeout\"", "timeout label"],
  ["rustok_api::PortErrorKind::InvariantViolation => \"invariant_violation\"", "invariant label"],
]) requireText(boundary, value, label);

for (const [value, label] of [
  ["fn is_technical_storefront_status(status: axum::http::StatusCode) -> bool", "severity classifier"],
  ["StatusCode::SERVICE_UNAVAILABLE", "unavailable status"],
  ["StatusCode::GATEWAY_TIMEOUT", "timeout status"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "invariant status"],
]) requireText(classifier, value, label);

for (const [value, label] of [
  ["fn log_storefront_line_item_port_error(", "shared bounded emitter"],
  ['owner: &\'static str,', "owner parameter"],
  ['operation: &\'static str,', "operation parameter"],
  ['technical_event: &\'static str,', "technical event parameter"],
  ['rejected_event: &\'static str,', "rejected event parameter"],
  ["facts: &StorefrontLineItemPortFacts,", "bounded fact input"],
  ["public_status: axum::http::StatusCode,", "public status input"],
  ["if is_technical_storefront_status(public_status) {", "severity selection"],
  ["tracing::error!(", "technical severity"],
  ["tracing::warn!(", "ordinary severity"],
  ["owner,", "truthful owner field"],
  ["operation,", "exact operation field"],
  ["public_status = %public_status,", "public status field"],
]) requireText(emitter, value, label);

forbidRawContextLogging(failures, boundary, "storefront line-item boundary");

// --- product and pricing mappers ------------------------------------------------------------------

for (const [value, label] of [
  ["fn map_storefront_line_item_product_port_error(", "product mapper"],
  ["error: PortError,", "typed Product owner error"],
  ["context: &PortContext,", "retained Product context"],
  ["variant_id: Uuid,", "variant identity input"],
  ["let facts = StorefrontLineItemPortFacts::new(&error, context, variant_id, None);", "bounded product facts"],
  ["let public = port_error_to_http_error(error);", "shared safe product envelope"],
  ['"rustok_product",', "truthful product owner"],
  ['"read_variant_product_projection",', "exact product owner operation"],
  ['"storefront line item Product owner read failed with bounded diagnostics"', "product technical event"],
  [
    '"storefront line item Product owner read was rejected with bounded diagnostics"',
    "product rejected event",
  ],
  ["&facts,", "bounded fact handoff"],
  ["public.status,", "public status handoff"],
  ["\n    public\n}", "unchanged product envelope return"],
]) requireText(productMapper, value, label);

for (const [value, label] of [
  ["fn map_storefront_line_item_pricing_error(", "pricing mapper"],
  ["error: PortError,", "typed Pricing owner error"],
  ["context: &PortContext,", "retained Pricing context"],
  ["variant_id: Uuid,", "variant identity input"],
  ["product_id: Uuid,", "product identity input"],
  [
    "let facts = StorefrontLineItemPortFacts::new(&error, context, variant_id, Some(product_id));",
    "bounded pricing facts",
  ],
  ["let public = port_error_to_http_error(error);", "shared safe pricing envelope"],
  ['"rustok_pricing",', "truthful pricing owner"],
  ['"resolve_product_price",', "exact pricing owner operation"],
  [
    '"storefront line item pricing resolution failed with bounded diagnostics"',
    "pricing technical event",
  ],
  [
    '"storefront line item pricing resolution was rejected with bounded diagnostics"',
    "pricing rejected event",
  ],
  ["\n    public\n}", "unchanged pricing envelope return"],
]) requireText(pricingMapper, value, label);

// --- inventory mapper ----------------------------------------------------------------------------

for (const [value, label] of [
  ["error: CommerceError,", "typed inventory owner error"],
  ['operation: &\'static str,', "inventory operation input"],
  ["tenant_id: Uuid,", "tenant identity input"],
  ["variant_id: Uuid,", "variant identity input"],
  ["product_id: Uuid,", "product identity input"],
  ["public_channel_slug: Option<&str>,", "channel input"],
  ["locale: Option<&str>,", "locale input"],
  ["CommerceError::Validation(_)", "validation variant"],
  ["CommerceError::ProductNotFound(_)", "product-not-found variant"],
  ["CommerceError::VariantNotFound(_)", "variant-not-found variant"],
  ["CommerceError::ShippingProfileNotFound(_)", "profile-not-found variant"],
  ["CommerceError::InsufficientInventory { .. }", "insufficient inventory variant"],
  ["CommerceError::Database(_)", "database variant"],
  ["CommerceError::DuplicateHandle { .. }", "unexpected duplicate handle variant"],
  ["CommerceError::DuplicateSku(_)", "unexpected duplicate sku variant"],
  ["CommerceError::InvalidPrice(_)", "unexpected invalid price variant"],
  ["CommerceError::InvalidOptionCombination", "unexpected option variant"],
  ["CommerceError::DuplicateShippingProfileSlug(_)", "unexpected profile conflict variant"],
  ["CommerceError::NoVariants", "unexpected no-variants variant"],
  ["CommerceError::CannotDeletePublished", "unexpected delete variant"],
  ["CommerceError::Rich(_)", "unexpected rich variant"],
  ["CommerceError::Core(_)", "unexpected core variant"],
  ["StatusCode::BAD_REQUEST", "invalid status"],
  ["StatusCode::NOT_FOUND", "not-found status"],
  ["StatusCode::SERVICE_UNAVAILABLE", "unavailable status"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "fail-closed status"],
  ['"commerce_store_inventory_invalid"', "invalid code"],
  ['"commerce_store_not_found"', "not-found code"],
  ['"commerce_store_inventory_insufficient"', "insufficient code"],
  ['"commerce_store_inventory_unavailable"', "unavailable code"],
  ['"commerce_store_inventory_failed"', "fail-closed code"],
  ['owner = "rustok_inventory.public_channel"', "truthful inventory owner"],
  ["tenant_id_non_nil = !tenant_id.is_nil()", "tenant identity shape"],
  ["variant_id_non_nil = !variant_id.is_nil()", "variant identity shape"],
  ["product_id_non_nil = !product_id.is_nil()", "product identity shape"],
  ["channel_length = public_channel_slug.map(str::len)", "channel length fact"],
  ["locale_length = locale.map(str::len)", "locale length fact"],
  ["error_kind,", "inventory failure label"],
  ["owner_code_length = code.chars().count(),", "inventory code shape"],
  ["public_status = %status,", "inventory public status"],
  ["if is_technical_storefront_status(status) {", "inventory severity selection"],
  ["tracing::error!(", "inventory technical severity"],
  ["tracing::warn!(", "inventory ordinary severity"],
  [
    '"storefront line item inventory operation failed with bounded diagnostics"',
    "inventory technical event",
  ],
  [
    '"storefront line item inventory operation was rejected with bounded diagnostics"',
    "inventory rejected event",
  ],
  ["HttpError::new(status, code, message)", "static inventory envelope"],
]) requireText(inventoryMapper, value, label);

// --- resolver ------------------------------------------------------------------------------------

for (const [value, label] of [
  ["let port_context =", "product context retention"],
  ["storefront_product_port_context(tenant_id, locale, public_channel_slug, input.variant_id)", "product context construction"],
  [".read_storefront_variant_product_projection(", "typed Product owner read"],
  ["port_context.clone(),", "product context propagation"],
  ["StorefrontVariantProductProjectionRequest {", "typed Product projection request"],
  ["locale: Some(locale.to_string())", "requested locale"],
  ["fallback_locale: Some(default_locale.to_string())", "fallback locale"],
  ["public_channel_slug: public_channel_slug.map(str::to_owned)", "channel propagation"],
  [
    "map_storefront_line_item_product_port_error(error, &port_context, input.variant_id)",
    "product mapper handoff",
  ],
  ["let pricing_port_context =", "pricing context retention"],
  ["store_line_item_pricing_port_context(", "pricing context construction"],
  [".resolve_product_price(", "typed Pricing owner read"],
  ["pricing_port_context.clone(),", "pricing context propagation"],
  ["ResolveProductPriceRequest {", "typed Pricing request"],
  [
    "map_storefront_line_item_pricing_error(",
    "pricing mapper handoff",
  ],
  ["storefront_cart_pricing_snapshot(", "pricing snapshot"],
  ["validate_store_variant_inventory(", "inventory validation"],
  ["pick_product_translation_response(", "product title fallback"],
  ["pick_variant_translation_response(", "variant title fallback"],
  ["effective_shipping_profile_slug(", "shipping profile resolution"],
  ["seller_snapshot_metadata(", "seller snapshot"],
  ["merge_metadata(", "metadata merge"],
  ["ProductFulfillmentRequirement::Digital => None", "digital delivery branch"],
  ["ProductFulfillmentRequirement::Physical => Some(", "physical delivery branch"],
]) {
  requireText(resolver, value, label);
}
forbidText(
  resolver,
  ".map_err(rustok_web::port_error_to_http_error)?",
  "unlogged pricing mapper call",
);

for (const [value, label] of [
  ["product_catalog_read_port: &dyn ProductCatalogReadPort,", "host-selected Product port"],
  ["let port_context =", "quantity context retention"],
  [".read_storefront_variant_product_projection(", "typed quantity Product read"],
  ["map_storefront_line_item_product_port_error(error, &port_context, variant_id)", "quantity mapper handoff"],
  ['"commerce_store_not_found"', "quantity not-found code"],
  ["validate_store_variant_inventory(", "quantity inventory validation"],
]) requireText(quantityValidator, value, label);

for (const [value, label] of [
  ["check_variant_availability_for_public_channel(", "public-channel inventory read"],
  ["PublicChannelInventoryVariantProjectionInput {", "typed inventory projection"],
  ["inventory_policy,", "inventory policy propagation"],
  ["requested_quantity,", "requested quantity propagation"],
  ["public_channel_slug,", "public channel propagation"],
  ['"check_variant_availability"', "inventory operation label"],
  ["map_storefront_line_item_inventory_error(", "typed inventory mapper use"],
  ["if !available {", "insufficient availability branch"],
  ['"commerce_store_inventory_insufficient"', "stable insufficient code"],
  ['"Requested quantity is not available"', "static insufficient message"],
]) requireText(inventoryValidator, value, label);

// --- forbidden conversions and direct storage access ---------------------------------------------

for (const value of [
  "DbErr",
  "product_variant::Entity",
  "product::Entity",
  "product_translation::Entity",
  "variant_translation::Entity",
  "is_metadata_visible_for_public_channel",
  "err.to_string()",
  "error.to_string()",
  'HttpError::bad_request("commerce_store_invalid", error',
  'HttpError::bad_request("commerce_store_invalid", err',
]) {
  forbidText(boundary, value, "unsafe or context-dropping line-item read");
}

// --- call-site and owner-contract counts ----------------------------------------------------------

requireCount(
  boundary,
  /map_storefront_line_item_product_port_error\(/g,
  3,
  "product mapper definition plus two uses",
);
requireCount(
  boundary,
  /map_storefront_line_item_pricing_error\(/g,
  2,
  "pricing mapper definition plus one use",
);
requireCount(
  boundary,
  /map_storefront_line_item_inventory_error\(/g,
  2,
  "inventory mapper definition plus one use",
);

for (const [content, value, label] of [
  [portContract, "pub struct PortContext {", "shared port context"],
  [portContract, "pub correlation_id: String", "shared correlation field"],
  [portContract, "pub channel: Option<String>", "shared channel field"],
  [portContract, "pub struct PortError {", "shared port error"],
  [portContract, "pub kind: PortErrorKind", "shared port error kind"],
  [portContract, "pub retryable: bool", "shared port retryability"],
  [commerceErrors, "pub enum CommerceError {", "commerce owner enum"],
  [commerceErrors, "Database(#[from] sea_orm::DbErr)", "commerce database variant"],
  [commerceErrors, "ProductNotFound(Uuid)", "commerce product variant"],
  [commerceErrors, "VariantNotFound(Uuid)", "commerce variant variant"],
  [
    commerceErrors,
    "InsufficientInventory { requested: i32, available: i32 }",
    "commerce inventory variant",
  ],
  [commerceErrors, "Validation(String)", "commerce validation variant"],
  [commerceErrors, "ShippingProfileNotFound(Uuid)", "commerce profile variant"],
  [commerceErrors, "Rich(#[source] Box<RichError>)", "commerce rich variant"],
  [commerceErrors, "Core(#[from] CoreError)", "commerce core variant"],
  [inventoryOwner, "-> CommerceResult<bool>", "inventory typed result"],
  [inventoryOwner, "check_public_channel_inventory_request(", "inventory request validation"],
  [
    inventoryOwner,
    "load_available_inventory_for_variant_in_public_channel(",
    "inventory storage read",
  ],
  [inventoryOwner, "Ok(available_inventory >= requested_quantity)", "availability semantics"],
]) requireText(content, value, label);

// --- tests ---------------------------------------------------------------------------------------

for (const [value, label] of [
  [
    "use crate::controllers::store::line_item_resolution::resolve_store_line_item_input;",
    "typed test resolver import",
  ],
  ['"commerce_store_inventory_insufficient"', "typed inventory test code"],
  ['"Requested quantity is not available"', "typed inventory test message"],
]) requireText(productTests, value, label);
for (const value of [
  '"commerce_store_invalid"',
  "does not have enough available inventory for the current channel",
  '#[path = "../line_item_resolution.rs"]',
]) {
  forbidText(productTests, value, "legacy inventory test contract");
}

forbidText(testRoot, "resolve_store_line_item_input,", "legacy line-item resolver test-root import");
requireText(testRoot, "StoreLineItemResolution,", "shared line-item resolution test import");

if (failures.length > 0) {
  console.error("Commerce storefront line-item HTTP error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Storefront line-item Product, Pricing, and Inventory failures keep typed owner reads, static safe envelopes, status-derived severity, and bounded diagnostics",
);
