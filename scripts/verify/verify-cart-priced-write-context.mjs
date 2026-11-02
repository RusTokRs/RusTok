#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { containsMarker, spaced } from "./lib/source-text.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
export const sourcePaths = [
  "crates/modules/rustok-cart/src/pricing_context_guard.rs",
  "crates/modules/rustok-cart/src/guarded_ports.rs",
  "crates/modules/rustok-cart/src/owner_ports.rs",
  "crates/modules/rustok-cart/src/ports.rs",
  "crates/modules/rustok-cart/src/services/cart.rs",
  "crates/modules/rustok-cart/src/services/cart/pricing.rs",
  "crates/modules/rustok-cart/src/services/cart/types.rs",
  "crates/modules/rustok-commerce/src/storefront_cart_runtime.rs",
  "crates/modules/rustok-commerce/src/storefront_checkout_pricing.rs",
  "crates/modules/rustok-commerce/src/controllers/store/carts.rs",
  "crates/modules/rustok-commerce/src/graphql/mutations/cart.rs",
  "crates/modules/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs",
  "crates/modules/rustok-cart/storefront/src/transport/buyer_native.rs",
  "crates/modules/rustok-cart/contracts/cart-fba-registry.json",
  "crates/modules/rustok-cart/contracts/evidence/cart-runtime-contract-smoke.json",
  "crates/modules/rustok-cart/contracts/evidence/cart-contract-test-static-matrix.json",
  "crates/modules/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json",
  "crates/modules/rustok-cart/src/services/marketplace_snapshot.rs",
];

// Lexical source guard only. Scoped Rust parsing/call-shape checks and unrun behavior
// regressions supplement this; matching tokens is not a compilation or runtime claim.
export function rustFunctionBody(source, name) {
  const match = new RegExp(`fn\\s+${name}\\s*(?:<[^>]*>)?\\s*\\(`).exec(source);
  if (!match) return "";
  const start = source.indexOf("{", match.index);
  let depth = 0;
  for (let index = start; index < source.length; index++) {
    if (source[index] === "{") depth++;
    if (source[index] === "}" && --depth === 0)
      return source.slice(start, index + 1);
  }
  return "";
}

export function verifyCartPricedWriteContext({ root = repository } = {}) {
  const failures = [];
  const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
  const sources = sourcePaths.map(read);
  const [
    guard,
    admitted,
    owner,
    ports,
    cart,
    pricing,
    types,
    workflow,
    quote,
    rest,
    graphql,
    native,
    buyer,
  ] = sources;
  const require = (source, marker, label) => {
    if (!containsMarker(source, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (source, marker, label) => {
    if (containsMarker(source, marker))
      failures.push(`${label}: forbidden ${marker}`);
  };
  const before = (source, first, second, label) => {
    const normalized = spaced(source);
    const a = normalized.indexOf(spaced(first));
    const b = normalized.indexOf(spaced(second));
    if (a < 0 || b < 0 || a >= b)
      failures.push(`${label}: ${first} must precede ${second}`);
  };
  const body = (source, name) => {
    const value = rustFunctionBody(source, name);
    if (!value) failures.push(`missing function ${name}`);
    return value;
  };

  require(ports, "pub use crate::guarded_ports::guarded_cart_storefront_port as in_process_cart_storefront_port", "canonical module factory");
  forbid(
    ports,
    "impl CartStorefrontPort for crate::CartService",
    "raw storefront provider bypass",
  );
  for (const source of [ports, owner, admitted, rest, graphql]) {
    forbid(
      source,
      "CartStorefrontLineItemQuantityRequest",
      "retired unquoted quantity DTO",
    );
    forbid(
      source,
      "update_storefront_line_item_quantity",
      "retired unquoted quantity operation",
    );
  }
  const registry = JSON.parse(read(sourcePaths[13]));
  const storefront = registry.ports.find(
    (port) => port.name === "CartStorefrontPort",
  );
  const declared = storefront.operations;
  const trait =
    ports
      .split("pub trait CartStorefrontPort")[1]
      ?.split("pub trait CartPromotionPort")[0] ?? "";
  const actual = [...trait.matchAll(/async\s+fn\s+(\w+)/g)].map(
    (match) => match[1],
  );
  if (
    JSON.stringify([...declared].sort()) !== JSON.stringify(actual.sort()) ||
    actual.length !== 7
  )
    failures.push(
      "Cart storefront registry must describe exactly the seven surviving operations",
    );
  if (
    storefront.implementation_source !== sourcePaths[2] ||
    storefront.implementation_service !== "OwnerCartPort" ||
    storefront.context_policy_helper !== "require_cart_policy"
  )
    failures.push(
      "Cart registry must identify the actual admitted storefront implementation",
    );
  for (const index of [14, 15]) {
    const artifact = JSON.parse(read(sourcePaths[index]));
    for (const operation of declared)
      if (!artifact.cases.some((row) => row.operation === operation))
        failures.push(`Cart source artifact missing ${operation}`);
    if (
      artifact.cases.some(
        (row) => row.operation === "update_storefront_line_item_quantity",
      )
    )
      failures.push(
        "Cart source artifact still advertises the retired unsafe write",
      );
  }
  const trace = JSON.parse(read(sourcePaths[16])).modules.find(
    (row) => row.provider_module === "cart",
  );
  for (const operation of declared)
    if (!trace.operations.includes(operation))
      failures.push(`Cart invocation descriptor missing ${operation}`);
  require(body(
    workflow,
    "add_line_item",
  ), "CartPricingContextGuard::capture_add(&cart)?", "priced insertion capacity before foreign IO");
  require(body(
    cart,
    "add_line_item_in_context",
  ), "context.verify_add_capacity()?", "locked insertion capacity");
  require(guard, "pub fn capture_removal", "bounded explicit removal escape");
  const marketplace = read(sourcePaths[17]);
  if (
    (
      marketplace.match(
        /load_cart_for_update_in_tx\(&transaction, tenant_id, cart_id\)/g,
      ) ?? []
    ).length !== 2
  )
    failures.push(
      "both marketplace Cart snapshot mutations must serialize through the same Cart lock",
    );
  for (const name of ["add_marketplace_line_item", "bind_line_snapshot"])
    require(body(
      marketplace,
      name,
    ), "load_cart_for_update_in_tx(", "marketplace mutation lock");
  forbid(
    body(marketplace, "list_cart_snapshots"),
    "load_cart_for_update_in_tx(",
    "marketplace inspection remains read-only",
  );
  const insert = body(owner, "add_storefront_line_item");
  require(insert, ".add_line_item_in_context(", "owner add provider");
  require(insert, "Some(request.pricing_context)", "owner add observation");
  for (const operation of [
    "update_storefront_line_item_pricing",
    "remove_storefront_line_item",
    "reprice_storefront_line_items",
  ]) {
    require(body(
      owner,
      operation,
    ), "request.pricing_context", `${operation} owner forwarding`);
    const admission = body(admitted, operation);
    before(
      admission,
      "authorize_cart(",
      "bind_authorized_access(",
      `${operation} access admission`,
    );
    require(admission, "request.pricing_context.bind_authorized_access(&authorized)", `${operation} access binding`);
  }
  const access = body(guard, "bind_authorized_access");
  forbid(access, "Self::capture", "no precondition rebasing");
  forbid(access, "self.line_items =", "no content rebasing");
  forbid(access, "self.cart_updated_at =", "no update-witness rebasing");
  for (const marker of [
    "MAX_PRICED_CART_LINES: usize = 1024",
    "cart_id: Uuid",
    "tenant_id: Uuid",
    "quantity: i32",
    "unit_price: Decimal",
    "fulfillment_requirement: String",
    "shipping_profile_slug: Option<String>",
    "ids.insert(line.id)",
    "self.cart_id != cart.id",
    "self.tenant_id != cart.tenant_id",
    "current != self.line_items",
    ".limit((MAX_PRICED_CART_LINES + 1) as u64)",
  ]) {
    require(guard, marker, "material Cart observation");
  }
  const locked = body(guard, "verify_locked");
  require(locked, "self.purpose != GuardPurpose::Pricing", "no removal-to-price guard substitution");
  for (const foreign of [
    "resolve_product_price",
    "check_availability",
    "ProductCatalogReadPort",
  ])
    forbid(locked, foreign, "observation verifier reads only Cart-owned rows");
  before(
    body(cart, "add_line_item_in_context"),
    "load_cart_for_update_in_tx(",
    "context.verify_locked(&txn, &cart)",
    "add row lock",
  );
  before(
    body(cart, "add_line_item_in_context"),
    "context.verify_locked(&txn, &cart)",
    "entities::cart_line_item::ActiveModel",
    "add adoption before insert",
  );
  for (const operation of ["update_line_item_pricing", "reprice_line_items"]) {
    const operationBody = body(pricing, operation);
    before(
      operationBody,
      "load_cart_for_update_in_tx(",
      "pricing_context.verify_locked(",
      `${operation} row lock`,
    );
    before(
      operationBody,
      "pricing_context.verify_locked(",
      operation === "reprice_line_items"
        ? "apply_pricing_updates("
        : "active.update(&txn)",
      `${operation} adoption`,
    );
  }
  const batch = body(pricing, "apply_pricing_updates");
  require(body(
    pricing,
    "reprice_line_items",
  ), "PricingCoverage::Supplied", "bulk repricing uses the shared validated adopter");
  require(batch, "PricingCoverage::Complete", "shared adoption also validates context-wide coverage");
  for (const marker of [
    "updates_map.insert(update.line_item_id, update).is_some()",
    "CartError::CartLineItemNotFound(update.line_item_id)",
    "*quantity != update.quantity",
  ])
    require(batch, marker, "exact batch adoption");
  before(
    batch,
    "for update in updates_map.values()",
    "for line in lines",
    "whole-plan validation before writes",
  );
  require(types, "pub quantity: i32", "quoted quantity contract");
  require(pricing, "unit_price.checked_mul(Decimal::from(quantity))", "checked line money");
  before(
    body(cart, "remove_line_item"),
    "pricing_context.verify_removal_locked(",
    "delete_many()",
    "removal fence",
  );

  const quantity = body(workflow, "update_line_item_quantity");
  for (const marker of [
    "CartPricingContextGuard::capture(&cart)?",
    "cart_pricing_context(context.clone(), &cart)",
    "read_storefront_cart_line_quote(",
    "read_storefront_variant_product_projection(",
    "line.fulfillment_requirement != requirement",
    "Some(line_item_id)",
    "CartStorefrontLineItemPricingRequest",
    "pricing_context",
  ])
    require(quantity, marker, "shared quantity workflow");
  before(
    quantity,
    "read_storefront_cart_line_quote(",
    "require_cart_inventory(",
    "quote before stock correlation",
  );
  before(
    quantity,
    "require_cart_inventory(",
    "update_storefront_line_item_pricing(",
    "stock observation before adoption",
  );
  const inventory = body(workflow, "require_cart_inventory");
  require(inventory, "Some(line.id) != replaced_line", "replace, do not double-add demand");
  require(inventory, "quantity.checked_add(line.quantity)", "checked aggregate demand");
  require(inventory, "inventory.check_availability(context, request)", "Inventory owner observation");
  forbid(inventory, "reserve_inventory(", "no stock hold claim");
  require(quote, "pub(crate) fn cart_pricing_context", "one saved-scope/locale policy");
  for (const [source, operation] of [
    [rest, "update_cart_line_item"],
    [graphql, "update_storefront_cart_line_item"],
    [native, "storefront_cart_decrement_line_item"],
  ]) {
    const operationBody = body(source, operation);
    require(operationBody, ".update_line_item_quantity(", `${operation} shared workflow`);
    forbid(
      operationBody,
      "validate_storefront_line_item_quantity(",
      `${operation} old stock path`,
    );
    forbid(operationBody, "CartService::new(", `${operation} raw owner bypass`);
  }
  const decrement = body(native, "storefront_cart_decrement_line_item");
  require(decrement, "quantity - 1", "original decrement intent");
  require(decrement, "CartPricingContextGuard::capture_removal(&cart, line_id)", "target-only decrement removal observation");
  require(decrement, "CartStorefrontRemoveLineItemRequest", "guarded decrement-to-remove");
  require(body(
    native,
    "storefront_cart_remove_line_item",
  ), "pricing_context", "native removal observation");
  require(body(
    buyer,
    "port_context",
  ), "context.claims.extend(auth.scopes.iter().cloned())", "native caller scopes");
  require(body(
    buyer,
    "buyer_scope",
  ), "request.user_id.is_some_and", "native principal agreement");
  return { failures, sourceOnly: true, testsExecuted: false };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const result = verifyCartPricedWriteContext();
  if (result.failures.length) {
    console.error(result.failures.join("\n"));
    process.exitCode = 1;
  } else
    console.log(
      "Cart priced-write source contract passed: original observations, exact tier quantity, aggregate replacement stock, and all three adapters; no runtime/test proof.",
    );
}
