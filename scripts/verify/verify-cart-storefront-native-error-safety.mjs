#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { containsMarker } from "./lib/source-text.mjs";
import { verifyCartPricedWriteContext } from "./verify-cart-priced-write-context.mjs";

const root =
  process.env.RUSTOK_VERIFY_REPO_ROOT?.trim() ||
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (file) => readFileSync(path.join(root, file), "utf8");
const safe = read(
  "crates/modules/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs",
);
const buyer = read(
  "crates/modules/rustok-cart/storefront/src/transport/buyer_native.rs",
);
const common = read(
  "crates/modules/rustok-cart/storefront/src/transport/native_server_adapter.rs",
);
const routing = read(
  "crates/modules/rustok-cart/storefront/src/transport/mod.rs",
);
const mapping = read(
  "crates/modules/rustok-cart/storefront/src/transport/native_server_mapping.rs",
);
const evidence = JSON.parse(
  read(
    "crates/modules/rustok-cart/contracts/evidence/storefront-native-error-safety-source.json",
  ),
);
const failures = [];
const require = (source, marker, label) => {
  if (!containsMarker(source, marker))
    failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (containsMarker(source, marker))
    failures.push(`${label}: forbidden ${marker}`);
};

for (const marker of [
  "mod native_server_adapter;",
  "mod native_server_adapter_ssr;",
  "mod native_server_mapping;",
])
  require(routing, marker, "one selected native contract");
for (const endpoint of [
  "cart/storefront-data",
  "cart/decrement-line-item",
  "cart/remove-line-item",
])
  require(common, `endpoint = "${endpoint}"`, "preserved endpoint");
for (const marker of [
  "fn context_extraction_error<E>",
  "fn tenant_context_error<E>",
  "fn auth_context_error<E>",
  "fn transactional_event_bus_from_runtime",
  "fn cart_input_error",
  "fn customer_error",
  "std::any::type_name::<E>()",
  "std::any::type_name_of_val(&error)",
  "Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None)",
  "Customer information is temporarily unavailable",
  "Invalid cart selection",
  "Invalid cart line item selection",
])
  require(safe, marker, "existing safe context/input/customer projection");
for (const marker of [
  "CartService::new(",
  "fn cart_error(",
  "fn pricing_error(",
  "fn missing_variant_error(",
  "ServerFnError::new(error.message)",
  "ServerFnError::new(format!(",
  "error = ?error",
  "error = %error",
])
  forbid(safe, marker, "no old raw owner bypass/payload mapper");
for (const marker of [
  "pub(super) fn port_error",
  "kind = ?error.kind",
  "retryable = error.retryable",
  "code_length = error.code.len()",
  "tracing::error!",
  "tracing::warn!",
  "Cart operation could not be confirmed. Inspect the cart before trying again.",
])
  require(buyer, marker, "canonical native owner failure");
const ownerFailure =
  buyer.split("pub(super) fn port_error")[1]?.split("#[cfg")[0] ?? "";
for (const marker of [
  "ServerFnError::new(error.message)",
  "ServerFnError::new(error.to_string())",
  "error = ?error",
  "error = %error",
])
  forbid(ownerFailure, marker, "no owner payload public/log forwarding");
require(buyer, "context.claims.extend(auth.scopes.iter().cloned())", "admitted caller scopes");
require(buyer, "scope.request.correlation_id.clone()", "actual correlation");
require(buyer, "scope.request.locale.clone()", "actual locale");
require(buyer, "request.user_id.is_some_and", "principal agreement");
require(safe, "in_process_cart_storefront_port(", "guarded owner provider");
require(safe, ".update_line_item_quantity(", "shared cross-owner workflow");
require(safe, "CartStorefrontRemoveLineItemRequest", "fenced remove");
require(safe, "CartPricingContextGuard::capture_removal(&cart,", "original observed basis");
require(mapping, "pub(super) fn map_native_cart", "cart projection preserved");
require(mapping, "StorefrontCartShippingOption", "shipping DTO preserved");
forbid(
  mapping,
  "fn storefront_cart_pricing_update",
  "no unused duplicate price mapper",
);
forbid(mapping, "base_unit_price -", "no duplicate discount math");
forbid(
  safe,
  "verify_current_guest_cart_access(&cart.metadata)",
  "do not authorize sanitized guest projection",
);

if (evidence.status !== "storefront_native_error_safety_source_unvalidated")
  failures.push("native evidence must remain source-only/unvalidated");
for (const [key, expected] of Object.entries({
  guarded_owner_reads_and_writes: true,
  original_quantity_observation_preserved: true,
  shared_commerce_quantity_workflow: true,
  guest_projection_not_reauthorized_after_sanitization: true,
  owner_payload_public: false,
  tests_executed: false,
  runtime_execution_proven: false,
}))
  if (evidence.assertions?.[key] !== expected)
    failures.push(`native evidence ${key} must be ${expected}`);
failures.push(...verifyCartPricedWriteContext({ root }).failures);
if (failures.length) {
  console.error(
    `Cart storefront native error-safety verification failed:\n${failures.join("\n")}`,
  );
  process.exitCode = 1;
} else
  console.log(
    "Cart native source safety passed: guarded reads/adoption, one quantity workflow, static owner errors and bounded diagnostics; tests/runtime remain unrun.",
  );
