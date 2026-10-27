#!/usr/bin/env node

// Storefront cart shipping HTTP error safety.
//
// Storefront cart reads are enriched with Fulfillment's shipping options, and a cart context patch
// validates the selected option against the same owner. Both paths are the boundary where a
// fulfilment failure would otherwise reach the browser as an owner message or a database error, so
// every owner failure has to be projected onto a static Commerce envelope and diagnosed with
// bounded facts only.
//
// The verifier used to describe the previous implementation: a `FulfillmentService` constructed
// inside the controller, a `map_storefront_cart_shipping_error` mapper in `controllers/store/mod.rs`,
// and a raw `error = ?error` / `tenant_id = %tenant_id` / `cart_id = %cart_id` log line. The helpers
// now live in `controllers/store/carts/shipping_owner_reads.rs` and read the owner through the
// host-selected `ShippingOptionReadPort`, so the guard is written against that contract: port in,
// static envelope out, bounded facts logged, no raw identity anywhere.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const helpers = read(
  "crates/modules/rustok-commerce/src/controllers/store/carts/shipping_owner_reads.rs",
);
const carts = read("crates/modules/rustok-commerce/src/controllers/store/carts.rs");
const shipping = read("crates/modules/rustok-commerce/src/storefront_shipping.rs");
const fulfillmentErrors = read("crates/modules/rustok-fulfillment/src/error.rs");
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

const constants = between(
  helpers,
  "const STOREFRONT_CART_SHIPPING_OWNER:",
  "fn shipping_read_context(",
  "shipping boundary constants",
);
const contextBuilder = between(
  helpers,
  "fn shipping_read_context(",
  "fn map_shipping_port_error(",
  "shipping read context builder",
);
const mapper = between(
  helpers,
  "fn map_shipping_port_error(",
  "pub(super) async fn enrich_storefront_cart(",
  "shipping port mapper",
);
const enrichment = between(
  helpers,
  "pub(super) async fn enrich_storefront_cart(",
  "async fn validate_selected_shipping_option(",
  "cart shipping enrichment",
);
const selectionValidator = between(
  helpers,
  "async fn validate_selected_shipping_option(",
  "#[allow(clippy::too_many_arguments)]",
  "selected shipping option validation",
);
const contextPatch = from(
  helpers,
  "pub(super) async fn apply_cart_context_patch(",
  "cart context patch helper",
);
const typedEnrichment = between(
  shipping,
  "pub async fn enrich_cart_delivery_groups_typed(",
  "pub async fn enrich_cart_delivery_groups(",
  "typed shipping enrichment",
);
const compatibilityWrapper = between(
  shipping,
  "pub async fn enrich_cart_delivery_groups(",
  "fn extract_allowed_shipping_profile_slugs_from_metadata(",
  "commerce compatibility wrapper",
);
const enrichmentDiagnostics = from(
  compatibilityWrapper,
  "fn log_cart_delivery_group_enrichment_error(",
  "shipping enrichment diagnostics",
);

// --- mounting -----------------------------------------------------------------------------------

requireText(carts, "mod shipping_owner_reads;", "shipping owner read helper module");
for (const [value, label] of [
  ["shipping_owner_reads::enrich_storefront_cart(", "enrichment helper use"],
  ["shipping_owner_reads::apply_cart_context_patch(", "context patch helper use"],
]) requireText(carts, value, label);

for (const value of [
  "FulfillmentService::new(",
  "super::enrich_storefront_cart_for_db(",
  "super::apply_cart_context_patch_for_db(",
]) forbidText(carts, value, "concrete fulfillment construction in HTTP");

// --- boundary identity ---------------------------------------------------------------------------

for (const [value, label] of [
  ['const STOREFRONT_CART_SHIPPING_OWNER: &str = "rustok_fulfillment";', "truthful shipping owner"],
  [
    'const STOREFRONT_CART_SHIPPING_BOUNDARY: &str = "commerce_storefront_cart_shipping_http";',
    "shipping HTTP boundary",
  ],
]) requireText(constants, value, label);

for (const [value, label] of [
  ["fn shipping_read_context(", "shipping context builder"],
  ["tenant_id: Uuid,", "tenant identity input"],
  ["request_context: &RequestContext,", "request context input"],
  ["auth: Option<&AuthContext>,", "optional actor input"],
  ["cart_id: Uuid,", "cart identity input"],
  ["super::super::storefront_cart_port_context(", "shared cart port context"],
  ["operation,", "operation forwarding"],
  ["false,", "read-only admission"],
]) requireText(contextBuilder, value, label);

// --- mapper --------------------------------------------------------------------------------------

for (const [value, label] of [
  ["fn map_shipping_port_error(", "shipping port mapper"],
  ["error: PortError,", "typed owner error input"],
  ["context: &PortContext,", "retained owner context"],
  ['operation: &\'static str,', "exact owner operation input"],
  ["tenant_id: Uuid,", "tenant identity input"],
  ["cart_id: Uuid,", "cart identity input"],
  ["PortErrorKind::Validation => (", "validation kind"],
  ["PortErrorKind::NotFound => (", "not-found kind"],
  ["PortErrorKind::Conflict => (", "conflict kind"],
  ["PortErrorKind::Unavailable | PortErrorKind::Timeout => (", "unavailable kinds"],
  ["PortErrorKind::Forbidden | PortErrorKind::InvariantViolation => (", "owner failure kinds"],
  ["StatusCode::BAD_REQUEST", "invalid status"],
  ["StatusCode::NOT_FOUND", "not-found status"],
  ["StatusCode::CONFLICT", "conflict status"],
  ["StatusCode::SERVICE_UNAVAILABLE", "unavailable status"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "fail-closed status"],
  ['"commerce_store_shipping_invalid"', "invalid code"],
  ['"commerce_store_not_found"', "not-found code"],
  ['"commerce_store_shipping_state_conflict"', "conflict code"],
  ['"commerce_store_shipping_unavailable"', "unavailable code"],
  ['"commerce_store_shipping_failed"', "fail-closed code"],
  ['"Shipping request is invalid"', "static invalid message"],
  ['"Commerce resource not found"', "static not-found message"],
  ['"Shipping operation conflicts with the current state"', "static conflict message"],
  ['"Shipping service is temporarily unavailable"', "static unavailable message"],
  [
    '"Shipping operation could not be completed safely"',
    "static fail-closed message",
  ],
  ["HttpError::new(status, code, message)", "static public envelope"],
  ["owner = STOREFRONT_CART_SHIPPING_OWNER", "truthful owner field"],
  ["operation,", "exact operation field"],
  ["correlation_id = %context.correlation_id", "correlation join key"],
  ["tenant_id_non_nil = !tenant_id.is_nil()", "tenant identity shape"],
  ["cart_id_non_nil = !cart_id.is_nil()", "cart identity shape"],
  ["owner_error_kind = ?error.kind", "typed owner failure label"],
  ["owner_code_length = error.code.chars().count()", "owner code shape"],
  ["retryable = error.retryable", "owner retryability"],
  ["public_code = code", "public code field"],
  ["status = status.as_u16()", "public status field"],
  ["boundary = STOREFRONT_CART_SHIPPING_BOUNDARY", "boundary field"],
  [
    '"storefront cart shipping owner read failed with bounded diagnostics"',
    "shipping owner read event",
  ],
]) requireText(mapper, value, label);

forbidRawContextLogging(failures, mapper, "shipping port mapper");
forbidRawContextLogging(failures, helpers, "cart shipping HTTP helpers");

// --- enrichment ----------------------------------------------------------------------------------

for (const [value, label] of [
  ["shipping_option_read_port: &dyn ShippingOptionReadPort,", "host-selected owner port"],
  ["tenant_default_locale: &str,", "tenant default locale"],
  ["cart: CartResponse,", "owned cart input"],
  ["super::super::storefront_public_channel_slug_for_cart(&cart, request_context)", "effective channel"],
  ["let cart_id = cart.id;", "cart identity capture"],
  ['"shipping-options-list"', "list operation label"],
  [".list_shipping_option_projections(", "typed owner list call"],
  ["context.clone(),", "owner context forwarding"],
  ["ListShippingOptionProjectionsRequest {", "typed owner list request"],
  ["requested_locale: Some(request_context.locale.clone())", "requested locale forwarding"],
  ["tenant_default_locale: Some(tenant_default_locale.to_string())", "tenant locale forwarding"],
  ["map_shipping_port_error(", "typed mapper use"],
  ['"list_shipping_option_projections"', "exact owner operation"],
  [
    "enrich_cart_delivery_groups_from_options(cart, options, public_channel_slug.as_deref())",
    "option enrichment",
  ],
  ["if cart.delivery_groups.len() == 1 {", "single-group reconciliation"],
  ["available_shipping_options", "available option projection"],
  ["cart.delivery_groups[0].selected_shipping_option_id = selected_id;", "group selection"],
  ["cart.selected_shipping_option_id = selected_id;", "cart selection"],
]) requireText(enrichment, value, label);

for (const value of ["HttpError::new(", "err.to_string()", "error.to_string()"]) {
  forbidText(enrichment, value, "early public conversion in enrichment");
}

// --- selection validation -------------------------------------------------------------------------

for (const [value, label] of [
  ["SelectedShippingOptionValidation<'_>", "typed validation input"],
  ["if cart.delivery_groups.len() > 1 {", "multi-group selection guard"],
  ["super::super::current_shipping_selections(cart)", "current selection fallback"],
  ["normalize_shipping_profile_slug(", "profile normalization"],
  [".read_shipping_option_projection(", "typed owner option read"],
  ["ReadShippingOptionProjectionRequest {", "typed owner option request"],
  ["shipping_option_id: selected_shipping_option_id,", "selected option identity"],
  ["validation.requested_locale.map(str::to_string)", "requested locale forwarding"],
  ["validation.tenant_default_locale.map(str::to_string)", "tenant locale forwarding"],
  ['"shipping-option-read"', "option read operation label"],
  ['"read_shipping_option_projection"', "exact owner operation"],
  ["map_shipping_port_error(", "typed option mapper use"],
  ["eq_ignore_ascii_case(validation.currency_code)", "currency compatibility check"],
  [
    "is_metadata_visible_for_public_channel(&option.metadata, validation.public_channel_slug)",
    "channel visibility check",
  ],
  [
    "is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles)",
    "shipping profile compatibility check",
  ],
]) requireText(selectionValidator, value, label);

for (const value of [
  '.map_err(|err| HttpError::bad_request("commerce_store_invalid", err.to_string()))',
  '.map_err(|error| HttpError::bad_request("commerce_store_invalid", error.to_string()))',
]) forbidText(selectionValidator, value, "unsafe option lookup public conversion");

// --- context patch ---------------------------------------------------------------------------------

for (const [value, label] of [
  ["shipping_option_read_port: &dyn ShippingOptionReadPort,", "host-selected owner port"],
  ["super::super::requested_cart_context(cart, request_context, patch)", "requested context"],
  ["super::super::resolve_context_for_db(", "store context resolution"],
  ["validate_selected_shipping_option(", "selection validation call"],
  ["selected_shipping_option_id: requested.selected_shipping_option_id,", "selected option handoff"],
  ["currency_code: &cart.currency_code,", "cart currency handoff"],
  ["public_channel_slug: public_channel_slug.as_deref(),", "effective channel handoff"],
  ["requested_locale: Some(request_context.locale.as_str())", "requested locale handoff"],
  ["tenant_default_locale: Some(tenant_default_locale)", "tenant locale handoff"],
  [".update_storefront_context(", "typed cart owner write"],
]) requireText(contextPatch, value, label);

// --- typed enrichment and compatibility wrapper ----------------------------------------------------

for (const [value, label] of [
  ["use rustok_fulfillment::{", "typed fulfillment owner import"],
  ["FulfillmentResult, FulfillmentService,", "typed fulfillment result import"],
  ["FulfillmentError,", "typed fulfillment error import"],
]) requireText(shipping, value, label);

for (const [value, label] of [
  ["-> FulfillmentResult<CartResponse>", "typed enrichment return"],
  [".list_shipping_options(tenant_id, requested_locale, tenant_default_locale)", "typed option list"],
  [".await?;", "direct typed error propagation"],
  ["enrich_cart_delivery_groups_from_options(", "shared option enrichment"],
  ["public_channel_slug,", "effective channel forwarding"],
]) requireText(typedEnrichment, value, label);
for (const value of ["CommerceError::Validation", "error.to_string()", "err.to_string()"]) {
  forbidText(typedEnrichment, value, "typed enrichment error erasure");
}

for (const [value, label] of [
  ["-> CommerceResult<CartResponse>", "legacy Commerce compatibility contract"],
  ["enrich_cart_delivery_groups_typed(", "typed implementation delegation"],
  ["log_cart_delivery_group_enrichment_error(", "bounded compatibility diagnostics"],
  [
    'crate::CommerceError::Validation(\n            "Cart shipping details are temporarily unavailable".to_string(),\n        )',
    "stable GraphQL compatibility mapping",
  ],
]) requireText(compatibilityWrapper, value, label);
for (const value of ["error.to_string()", "err.to_string()", 'format!("{error:?}")']) {
  forbidText(compatibilityWrapper, value, "unsafe legacy GraphQL compatibility mapping");
}

for (const [value, label] of [
  ["FulfillmentError::Validation(_)", "validation branch"],
  ["FulfillmentError::ShippingOptionNotFound(_)", "shipping option branch"],
  ["FulfillmentError::FulfillmentNotFound(_)", "fulfillment branch"],
  ["FulfillmentError::InvalidTransition { .. }", "transition branch"],
  ["FulfillmentError::Database(_)", "database branch"],
  ['"fulfillment.validation"', "validation owner code"],
  ['"fulfillment.shipping_option_not_found"', "option owner code"],
  ['"fulfillment.database_unavailable"', "database owner code"],
  ["let technical = matches!(error, FulfillmentError::Database(_));", "technical classification"],
  ["let tenant_id_shape = uuid_shape(tenant_id);", "tenant shape"],
  ["let cart_id_shape = uuid_shape(cart_id);", "cart shape"],
  ["let public_channel_slug_shape = optional_text_shape(public_channel_slug);", "channel shape"],
  ["let requested_locale_shape = optional_text_shape(requested_locale);", "locale shape"],
  ["let error = StorefrontShippingDiagnosticError;", "redacted owner error marker"],
  ["error = ?error,", "redacted owner error evidence"],
  ['owner = "rustok_fulfillment"', "truthful owner"],
  ['operation = "list_shipping_options"', "exact owner operation"],
  ["owner_code,", "owner code label"],
  ["owner_kind,", "owner kind label"],
  ["owner_retryable,", "owner retryability"],
  ["boundary = STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY", "enrichment boundary"],
  ["if technical {", "technical severity selection"],
  ["tracing::error!(", "technical severity"],
  ["tracing::warn!(", "ordinary severity"],
  [
    '"storefront cart shipping enrichment owner read failed"',
    "technical enrichment event",
  ],
  [
    '"storefront cart shipping enrichment owner read was rejected"',
    "rejected enrichment event",
  ],
]) requireText(enrichmentDiagnostics, value, label);

// --- owner contract and counts ----------------------------------------------------------------------

for (const [value, label] of [
  ["pub enum FulfillmentError {", "owner fulfillment enum"],
  ["Validation(String)", "owner validation variant"],
  ["ShippingOptionNotFound(Uuid)", "owner shipping option variant"],
  ["FulfillmentNotFound(Uuid)", "owner fulfillment variant"],
  ["InvalidTransition { from: String, to: String }", "owner transition variant"],
  ["ProviderResultInvalid(String)", "owner provider result variant"],
  ["ShippingOptionTranslationRevisionConflict(Uuid)", "owner revision conflict variant"],
  ["Database(#[from] DbErr)", "owner database variant"],
]) requireText(fulfillmentErrors, value, label);

requireCount(helpers, /fn map_shipping_port_error\(/g, 1, "one shared shipping mapper");
requireCount(helpers, /map_shipping_port_error\(/g, 3, "mapper definition plus two owner calls");
requireCount(helpers, /fn shipping_read_context\(/g, 1, "one shipping context builder");
requireCount(helpers, /shipping_read_context\(/g, 3, "context builder plus two owner calls");
requireCount(
  shipping,
  /pub async fn enrich_cart_delivery_groups_typed\(/g,
  1,
  "one typed enrichment definition",
);
requireCount(
  shipping,
  /pub async fn enrich_cart_delivery_groups\(/g,
  1,
  "one commerce compatibility wrapper",
);
requireCount(carts, /shipping_owner_reads::enrich_storefront_cart\(/g, 5, "five enriched cart routes");
requireCount(
  carts,
  /shipping_owner_reads::apply_cart_context_patch\(/g,
  1,
  "one cart context patch route",
);

if (failures.length > 0) {
  console.error("Commerce storefront cart shipping HTTP error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Storefront cart shipping reads keep the host-selected owner port, static safe envelopes, and bounded diagnostics",
);
