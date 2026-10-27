#!/usr/bin/env node

// Storefront line-item owner context.
//
// The line-item boundary is the only place where a storefront cart request meets the Product,
// Pricing, and Inventory owners. Whatever context it hands to those owners is what the owners'
// policy checks and their correlation see, so the boundary has to retain the caller's identity
// shape — tenant, actor, channel, deadline, correlation — and forward the caller's locale, channel
// slug, variant, and requested quantity unchanged.
//
// The verifier used to describe the previous implementation: a Commerce-side `DbErr` mapper, five
// localized database contexts, and SeaORM model guards (`status != Active`, `published_at.is_none()`,
// `is_metadata_visible_for_public_channel`). None of those exist any more — the Product owner
// projection owns the visibility and lifecycle policy now, and the boundary only passes the request
// through. What survives, and what this guard pins, is context retention: one bounded context per
// owner call, every identity forwarded by value, and no identifier ever written as a value.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const boundary = read(
  "crates/modules/rustok-commerce/src/controllers/store/line_item_resolution.rs",
);
const storeModule = read("crates/modules/rustok-commerce/src/controllers/store/mod.rs");
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
const from = (content, start, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex);
};

const productContext = between(
  boundary,
  "fn storefront_product_port_context(",
  "struct StorefrontLineItemPortFacts {",
  "product port context builder",
);
const factConstructor = between(
  boundary,
  "impl StorefrontLineItemPortFacts {",
  "fn storefront_line_item_port_error_kind(",
  "bounded fact constructor",
);
const pricingMapper = between(
  boundary,
  "fn map_storefront_line_item_pricing_error(",
  "fn map_storefront_line_item_inventory_error(",
  "pricing mapper",
);
const inventoryMapper = between(
  boundary,
  "fn map_storefront_line_item_inventory_error(",
  "pub(crate) async fn resolve_store_line_item_input(",
  "inventory mapper",
);
const resolver = between(
  boundary,
  "pub(crate) async fn resolve_store_line_item_input(",
  "pub(crate) async fn validate_store_line_item_quantity(",
  "line item resolver",
);
const quantityValidator = between(
  boundary,
  "pub(crate) async fn validate_store_line_item_quantity(",
  "async fn validate_store_variant_inventory(",
  "quantity validator",
);
const inventoryValidator = from(
  boundary,
  "async fn validate_store_variant_inventory(",
  "inventory validator",
);
const pricingContextBuilder = between(
  storeModule,
  "fn store_line_item_pricing_port_context(",
  "pub(crate) fn pick_product_translation",
  "pricing port context builder",
);

// --- the context the boundary constructs ---------------------------------------------------------

for (const [value, label] of [
  ["tenant_id: Uuid,", "tenant identity input"],
  ["locale: &str,", "locale input"],
  ["public_channel_slug: Option<&str>,", "channel input"],
  ["variant_id: Uuid,", "variant identity input"],
  ["PortContext::new(", "owner context construction"],
  ['rustok_api::PortActor::service("rustok-commerce.storefront-line-item-product")', "truthful service actor"],
  ['format!("commerce-storefront-line-item:product:{variant_id}")', "resource correlation id"],
  ["with_deadline(std::time::Duration::from_secs(2))", "read deadline"],
  ["Some(channel) => context.with_channel(channel)", "effective channel propagation"],
  ["None => context,", "channel-absent branch"],
]) requireText(productContext, value, label);

for (const [value, label] of [
  ["PortActor::service(\"rustok-commerce.storefront-pricing\")", "truthful pricing actor"],
  ['format!("storefront-add-line-item:{variant_id}")', "pricing resource correlation id"],
  ["with_deadline(std::time::Duration::from_secs(2))", "pricing deadline"],
  ["pricing_context", "pricing caller context"],
  [".channel_slug", "pricing channel source"],
  [".as_deref()", "channel slug borrow"],
  [".map(|channel| context.clone().with_channel(channel))", "pricing channel propagation"],
  [".unwrap_or(context)", "channel-absent fallback"],
]) requireText(pricingContextBuilder, value, label);

// --- identity is forwarded by value, never logged as a value -------------------------------------

for (const [value, label] of [
  ["variant_id_non_nil: !variant_id.is_nil(),", "variant identity shape"],
  ["product_id_non_nil: product_id.map(|value| !value.is_nil()),", "product identity shape"],
  ["channel_present: context.channel.is_some(),", "channel presence fact"],
  ["tenant_id_length: context.tenant_id.chars().count(),", "tenant shape fact"],
  ["correlation_id_length: context.correlation_id.chars().count(),", "correlation shape fact"],
]) requireText(factConstructor, value, label);

for (const [value, label] of [
  ["Some(product_id)", "pricing product identity handoff"],
  ["variant_id, Some(product_id)", "pricing identity pair"],
]) requireText(pricingMapper, value, label);

for (const [value, label] of [
  ["tenant_id_non_nil = !tenant_id.is_nil()", "inventory tenant shape"],
  ["variant_id_non_nil = !variant_id.is_nil()", "inventory variant shape"],
  ["product_id_non_nil = !product_id.is_nil()", "inventory product shape"],
  ["channel_present = public_channel_slug.is_some()", "inventory channel presence"],
  ["channel_length = public_channel_slug.map(str::len)", "inventory channel shape"],
  ["locale_present = locale.is_some()", "inventory locale presence"],
  ["locale_length = locale.map(str::len)", "inventory locale shape"],
]) requireText(inventoryMapper, value, label);

forbidRawContextLogging(failures, boundary, "storefront line-item boundary");

// --- propagation into the owner calls ------------------------------------------------------------

for (const [value, label] of [
  ["storefront_product_port_context(tenant_id, locale, public_channel_slug, input.variant_id)", "product context construction"],
  ["port_context.clone(),", "product context forwarding"],
  ["locale: Some(locale.to_string())", "requested locale forwarding"],
  ["fallback_locale: Some(default_locale.to_string())", "fallback locale forwarding"],
  ["public_channel_slug: public_channel_slug.map(str::to_owned)", "channel slug forwarding"],
  ["store_line_item_pricing_port_context(", "pricing context construction"],
  ["pricing_port_context.clone(),", "pricing context forwarding"],
  ["channel_slug: pricing_context.channel_slug.clone()", "pricing channel forwarding"],
  ["currency_code: pricing_context.currency_code.clone()", "pricing currency forwarding"],
  ["quantity: pricing_context.quantity", "pricing quantity forwarding"],
  ["region_id: pricing_context.region_id", "pricing region forwarding"],
  ["channel_id: pricing_context.channel_id", "pricing channel identity forwarding"],
  ["map_storefront_line_item_pricing_error(", "pricing mapper call"],
  ["validate_store_variant_inventory(", "inventory validation call"],
  ["public_channel_slug,\n        Some(locale),", "localized inventory context"],
  ["effective_shipping_profile_slug(", "shipping profile resolution"],
  ["seller_snapshot_metadata(", "seller metadata"],
  ["merge_metadata(", "metadata merge"],
]) requireText(resolver, value, label);

for (const [value, label] of [
  ["storefront_product_port_context(tenant_id, \"en\", public_channel_slug, variant_id)", "quantity context construction"],
  ["port_context.clone(),", "quantity context forwarding"],
  ["public_channel_slug: public_channel_slug.map(str::to_owned)", "quantity channel forwarding"],
  ["validate_store_variant_inventory(", "quantity inventory validation"],
  ["public_channel_slug,\n        None,", "quantity inventory context"],
  ['"commerce_store_not_found"', "quantity not found code"],
]) requireText(quantityValidator, value, label);

for (const [value, label] of [
  ["locale: Option<&str>,", "inventory validator locale"],
  ["check_variant_availability_for_public_channel(", "inventory owner call"],
  ["variant_id,", "inventory variant forwarding"],
  ["inventory_policy,", "inventory policy forwarding"],
  ["requested_quantity,", "requested quantity forwarding"],
  ["public_channel_slug,", "public channel forwarding"],
  ['"check_variant_availability"', "inventory operation"],
  ["product_id,", "inventory product identity forwarding"],
  ["locale,", "inventory locale forwarding"],
  ['"Requested quantity is not available"', "insufficient response message"],
]) requireText(inventoryValidator, value, label);

// --- retired implementation shapes ----------------------------------------------------------------

for (const value of [
  "map_storefront_line_item_database_error",
  "rustok_product.persistence",
  "product_model.status",
  "published_at.is_none()",
  "is_metadata_visible_for_public_channel",
  "load_variant_for_quantity_validation",
  'err.to_string()',
  'error.to_string()',
  'error.message',
  'HttpError::bad_request("commerce_store_invalid", error',
  'HttpError::bad_request("commerce_store_invalid", err',
  'eprintln!(',
  'dbg!(',
]) {
  forbidText(boundary, value, "unsafe or superseded line-item mapping");
}

const localizedInventoryContexts = resolver.match(/public_channel_slug,\s+Some\(locale\),/g) ?? [];
if (localizedInventoryContexts.length !== 1) {
  failures.push(
    `expected one localized inventory context in the resolver, found ${localizedInventoryContexts.length}`,
  );
}
const productContexts = boundary.match(/storefront_product_port_context\(/g) ?? [];
if (productContexts.length !== 3) {
  failures.push(
    `expected product context definition plus two owner calls, found ${productContexts.length}`,
  );
}
const pricingContexts = boundary.match(/store_line_item_pricing_port_context\(/g) ?? [];
if (pricingContexts.length !== 1) {
  failures.push(`expected one pricing context construction, found ${pricingContexts.length}`);
}

if (failures.length > 0) {
  console.error("Commerce storefront line-item owner-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Storefront line-item Product, Pricing, and Inventory calls retain the caller context, forward locale, channel, quantity, and identity by value, and keep identity out of the log",
);
