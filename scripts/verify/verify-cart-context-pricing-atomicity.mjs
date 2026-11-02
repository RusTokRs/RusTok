#!/usr/bin/env node

// Source contract only: the owner transaction, request/observation and all live transport
// handoffs. This is not SQL, compilation, remote execution, a receipt or concurrency evidence.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { containsMarker, spaced } from "./lib/source-text.mjs";
import { rustFunctionBody } from "./verify-cart-priced-write-context.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
export const sourceFiles = {
  ports: "crates/modules/rustok-cart/src/ports.rs",
  owner: "crates/modules/rustok-cart/src/owner_ports.rs",
  guarded: "crates/modules/rustok-cart/src/guarded_ports.rs",
  context: "crates/modules/rustok-cart/src/services/cart/context.rs",
  types: "crates/modules/rustok-cart/src/services/cart/types.rs",
  pricing: "crates/modules/rustok-cart/src/services/cart/pricing.rs",
  guard: "crates/modules/rustok-cart/src/pricing_context_guard.rs",
  helpers: "crates/modules/rustok-cart/src/services/cart/helpers.rs",
  checkout: "crates/modules/rustok-cart/src/services/cart/checkout.rs",
  snapshot: "crates/modules/rustok-cart/src/checkout_snapshot.rs",
  atomic: "crates/modules/rustok-cart/src/atomic_checkout_port.rs",
  lib: "crates/modules/rustok-cart/src/lib.rs",
  planner: "crates/modules/rustok-commerce/src/storefront_checkout_pricing.rs",
  rest: "crates/modules/rustok-commerce/src/controllers/store/carts/shipping_owner_reads.rs",
  patch: "crates/modules/rustok-commerce/src/controllers/store/mod.rs",
  graphql: "crates/modules/rustok-commerce/src/graphql/mutations/cart.rs",
  gqlError:
    "crates/modules/rustok-commerce/src/graphql/mutations/typed_reprice_helper.rs",
  native: "crates/modules/rustok-commerce/src/storefront_checkout_runtime.rs",
  direct: "crates/modules/rustok-commerce/src/services/checkout.rs",
  region: "crates/modules/rustok-commerce/src/services/context.rs",
  evidence:
    "crates/modules/rustok-cart/contracts/evidence/cart-contract-test-static-matrix.json",
};
export const sourcePaths = Object.values(sourceFiles);

export function verifyCartContextPricingAtomicity({ root = repository } = {}) {
  const failures = [];
  const sources = Object.fromEntries(
    Object.entries(sourceFiles).map(([key, file]) => [
      key,
      fs.readFileSync(path.join(root, file), "utf8"),
    ]),
  );
  const require = (source, marker, label) => {
    if (!containsMarker(source, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (source, marker, label) => {
    if (containsMarker(source, marker))
      failures.push(`${label}: forbidden ${marker}`);
  };
  const body = (source, name) => {
    const result = rustFunctionBody(source, name);
    if (!result) failures.push(`missing function ${name}`);
    return result;
  };
  const before = (source, first, second, label) => {
    const text = spaced(source),
      a = text.indexOf(spaced(first)),
      b = text.indexOf(spaced(second));
    if (a < 0 || b < 0 || a >= b)
      failures.push(`${label}: ${first} must precede ${second}`);
  };

  for (const marker of [
    "pub struct CartContextUpdateRequest",
    "pub input: UpdateCartContextInput",
    "pub pricing_context: crate::CartPricingContextGuard",
    "pub pricing: crate::CartContextPricing",
  ])
    require(sources.ports, marker, "one owner context command");
  if (
    (sources.ports.match(/pub struct CartContextUpdateRequest\b/g) ?? [])
      .length !== 1
  )
    failures.push("one context command definition is required");
  for (const retired of [
    "CartStorefrontContextUpdateRequest",
    "CartCheckoutContextUpdateRequest",
  ])
    for (const key of ["ports", "owner", "guarded", "lib"])
      forbid(sources[key], retired, "no parallel context request aliases");
  for (const type of [
    "CartContextPricing",
    "CartContextPricingPlan",
    "CartPricingScope",
  ])
    require(sources.lib, type, "public owner command dependencies");
  for (const marker of [
    "pub enum CartContextPricing",
    "Preserve",
    "Reprice(CartContextPricingPlan)",
    "pub scope: crate::CartPricingScope",
    "pub updates: Vec<CartLineItemPricingUpdate>",
  ])
    require(sources.types, marker, "explicit complete price plan");

  const update = body(sources.context, "update_context");
  for (const [first, second] of [
    ["normalize_context_input(input)?", "self.db.begin()"],
    ["load_cart_for_update_in_tx(", "ensure_active("],
    ["ensure_active(", "pricing_context.verify_locked("],
    ["pricing_context.verify_locked(", "match pricing"],
    ["plan.scope != target", "apply_pricing_updates("],
    ["apply_pricing_updates(", "let updated = apply_context_input("],
    ["apply_context_input(", "recalculate_totals("],
    ["recalculate_totals(", "txn.commit()"],
  ])
    before(update, first, second, "one locked context/price adoption");
  require(update, "PricingCoverage::Complete", "context repricing is complete, not a supplied subset");
  require(update, "ensure_preserved_price_basis(", "explicit preserve validation");
  for (const foreign of [
    "resolve_product_price(",
    "resolve_product_prices(",
    "check_availability(",
    "ProductCatalogReadPort",
  ])
    forbid(
      update,
      foreign,
      "new Product/Pricing/Inventory IO precedes owner lock",
    );
  const batch = body(sources.pricing, "apply_pricing_updates");
  for (const marker of [
    "PricingCoverage::Complete && updates_map.len() != lines.len()",
    "updates_map.insert(update.line_item_id, update).is_some()",
    "*quantity != update.quantity",
    "CartError::CartLineItemNotFound(update.line_item_id)",
    "update_line_item_marketplace_snapshot_in_tx(",
    "replace_pricing_adjustments(",
  ])
    require(batch, marker, "exact shared price adoption");
  before(
    batch,
    "for update in updates_map.values()",
    "for line in lines",
    "validate all coverage/quantities before any line write",
  );
  const basis = body(sources.guard, "same_price_basis");
  for (const field of [
    "currency_code",
    "region_id",
    "channel_id",
    "channel_slug",
    "locale_code",
  ])
    require(basis, `self.${field} == other.${field}`, "preserve unit-price basis");
  forbid(
    basis,
    "self.country_code ==",
    "country belongs to current tax/shipping, not an invented Pricing selector",
  );
  require(body(
    sources.context,
    "normalize_context_input",
  ), "selection.validate()", "bounded nested input validation");
  const projection = body(sources.context, "project_context");
  require(projection, "resolve_shipping_selection_patch(", "owner-normalized hypothetical shipping context");
  forbid(
    projection,
    ".update(",
    "projection is not an acknowledgement or write",
  );
  const shipping = body(sources.helpers, "resolve_shipping_selection_patch");
  require(shipping, "selected_keys.insert(key.clone())", "reject overlapping resolved groups");
  for (const old of [
    ".or(cart.selected_shipping_option_id)",
    ".or(current_selected_shipping_option_id)",
  ])
    forbid(shipping, old, "explicit scalar None is not old-option fallback");
  require(shipping, "let option_id = input.selected_shipping_option_id;", "scalar replacement remains explicit");

  for (const name of [
    "update_storefront_context",
    "update_cart_checkout_context",
  ]) {
    const admitted = body(sources.guarded, name);
    before(
      admitted,
      "authorize_cart(",
      "bind_authorized_access(",
      `${name} current authorization`,
    );
    require(admitted, "request.pricing_context.bind_authorized_access(&authorized)", `${name} access binding without content rebasing`);
    require(body(
      sources.owner,
      name,
    ), ".update_context(tenant_id, request)", `${name} forwards the whole original command`);
  }
  require(body(
    sources.owner,
    "update_storefront_context",
  ), "CartContextPricing::Reprice", "buyer updates cannot choose Preserve");

  const prepare = body(sources.planner, "prepare_storefront_cart_context");
  before(
    prepare,
    "CartPricingContextGuard::capture(cart)",
    "cart.project_context(",
    "observe original context before target projection",
  );
  for (const marker of [
    "CartStatus::Active",
    "cart_pricing_context(context.clone(), &target)",
    "(line.product_id, line.quantity)",
    "entry(variant_id)",
    "chunks(MAX_PRICE_RESOLUTION_VARIANTS)",
    ".resolve_product_prices(read_context.clone(), request)",
    "batch.items.len() != chunk.len()",
    "chunk.iter().zip(batch.items)",
    "item.variant_id != *expected",
    "parent.is_some_and(|id| item.product_id != id)",
    "item.price.ok_or_else(",
    "cart_quote_matches(&expected_request, &quote)",
    "CartContextPricing::Reprice",
    "CartPricingScope::capture(&target)",
  ])
    require(prepare, marker, "complete selected batch observation");
  require(prepare, "tokio::time::timeout(budget, async", "all batches share one inherited wait budget");
  require(prepare, "context.deadline_ms.unwrap_or(2000)", "caller deadline is retained");
  for (const marker of [
    ".resolve_product_price(",
    "PortContext::new(",
    ".update_storefront_context(",
    ".reprice_storefront_line_items(",
    "CartContextPricing::Preserve",
  ])
    forbid(
      prepare,
      marker,
      "no fallback/provider/Cart write during price preparation",
    );

  for (const [key, name] of [
    ["rest", "apply_cart_context_patch"],
    ["graphql", "update_storefront_cart_context"],
    ["native", "select_storefront_shipping_option"],
    ["native", "create_storefront_payment_collection"],
  ]) {
    const transport = body(sources[key], name);
    before(
      transport,
      "crate::prepare_storefront_cart_context(",
      ".update_storefront_context(",
      `${name} observes before its single owner command`,
    );
    if ((transport.match(/\.update_storefront_context\(/g) ?? []).length !== 1)
      failures.push(`${name} must issue one context command`);
    forbid(
      transport,
      "reprice_storefront_cart_line_items(",
      `${name} cannot commit context then reprice`,
    );
    forbid(
      transport,
      "reprice_storefront_cart_line_items_for_db(",
      `${name} cannot commit context then reprice`,
    );
  }
  for (const key of ["rest", "graphql"])
    require(sources[
      key
    ], "resolve_cart_channel(", "shipping scope resolves the saved Cart selector, not page scope");
  forbid(
    body(sources.rest, "apply_cart_context_patch"),
    "region.countries.first()",
    "requested country/null must not become first regional country",
  );
  require(body(
    sources.patch,
    "requested_cart_context",
  ), "shipping_selections: patch.shipping_selections", "missing group patch is not an invented vector overriding scalar null");
  require(sources.graphql, "cart_context_preparation_error", "established GraphQL refresh envelope");
  require(body(
    sources.gqlError,
    "cart_context_preparation_error",
  ), "public_graphql_error(error.retryable)", "exact retry metadata on preparation failures");

  const direct = body(sources.direct, "complete_checkout");
  before(
    direct,
    'cart.status == "completed"',
    "context_is_current(",
    "completed recovery before mutable context",
  );
  before(
    direct,
    'cart.status == "checking_out"',
    "context_is_current(",
    "locked recovery before mutable context",
  );
  require(direct, "CartContextPricing::Preserve", "availability-only checkout cannot invent a price plan");
  require(direct, "CartError::Validation(message) => CheckoutError::Validation(message)", "pure context refusal keeps validation classification");
  require(body(
    sources.checkout,
    "prepare_checkout_with_pricing",
  ), "ensure_preserved_price_basis(", "planless atomic prepare cannot change unit-price basis");
  const snapshot = body(
    sources.snapshot.slice(
      sources.snapshot.indexOf(
        "impl CartCheckoutPort for InProcessCartCheckoutPort",
      ),
    ),
    "prepare_checkout",
  );
  require(snapshot, ".prepare_checkout(tenant_id, request)", "snapshot adapter delegates atomic owner prepare");
  for (const marker of [".begin_checkout(", ".update_context("])
    forbid(snapshot, marker, "no split snapshot preparation");
  require(body(
    sources.atomic,
    "update_cart_checkout_context",
  ), '"cart.checkout_context_locked"', "frozen materialization is not a context writer");
  require(sources.region, "Country is not part of the requested region", "region/country eligibility");
  const matrix = JSON.parse(sources.evidence);
  for (const name of [
    "update_storefront_context",
    "update_cart_checkout_context",
  ]) {
    const row = matrix.cases.find((item) => item.operation === name);
    if (!row?.assertions?.includes("active_only_atomic_context_price_adoption"))
      failures.push(
        `${name}: static evidence must record atomic context semantics`,
      );
    if (row?.execution_status !== "static_locked_runtime_pending")
      failures.push(`${name}: do not promote unexecuted runtime evidence`);
  }
  return { failures };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyCartContextPricingAtomicity();
  if (result.failures.length) {
    console.error(result.failures.join("\n"));
    process.exitCode = 1;
  } else
    console.log(
      "Cart atomic context/pricing source contract passed: one owner command, original observation, exact bounded batches and frozen recovery; no compilation/runtime/test claim.",
    );
}
