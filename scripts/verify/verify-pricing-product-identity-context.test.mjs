// Regression source for the source guard. Maintainer-run only; do not import from the guard.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  sourcePaths,
  verifyPricingProductIdentityContext,
} from "./verify-pricing-product-identity-context.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const baseline = Object.fromEntries(
  Object.entries(sourcePaths).map(([name, relative]) => [
    name,
    readFileSync(path.join(root, relative), "utf8"),
  ]),
);

function changed(name, before, after) {
  assert.ok(
    baseline[name].includes(before),
    `fixture must mutate actual ${name} source`,
  );
  return { ...baseline, [name]: baseline[name].replace(before, after) };
}

test("current selected Product/Pricing source contract", () => {
  assert.deepEqual(verifyPricingProductIdentityContext(baseline), []);
});

test("missing explicit identity tenant scope is rejected", () => {
  const sources = changed(
    "identities",
    "pub tenant_id: Uuid",
    "pub unscoped_id: Uuid",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("response tenant"),
    ),
  );
});

test("SKU response substitution cannot replace exact request correlation", () => {
  const sources = changed(
    "identities",
    "identity.variant_id != *expected",
    "identity.variant_id.is_nil()",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("exact owner response"),
    ),
  );
});

test("local Product reconstruction inside Pricing is rejected", () => {
  const sources = {
    ...baseline,
    service:
      baseline.service +
      "\nfn wrong_owner() { CatalogService::new(db, bus); }\n",
  };
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("reconstruct Product"),
    ),
  );
});

test("dropping a canonical read budget is rejected", () => {
  const sources = changed(
    "read",
    "crate::ports::read_with_deadline(",
    "incorrect_unbounded_read(",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("eight canonical"),
    ),
  );
});

test("durable checkout cannot silently rebuild embedded Product for repricing", () => {
  const sources = changed(
    "resolver",
    "self.product_catalog_read_runtime.clone()",
    "local_product_runtime()",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("durable checkout repricing"),
    ),
  );
});

test("module-path factory cannot bypass the admitted Pricing facade", () => {
  const sources = changed(
    "ports",
    "crate::read_context::in_process_pricing_read_port(db, event_bus, product_runtime)",
    "Arc::new(PricingService::new(db, event_bus))",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("module-path read facade"),
    ),
  );
});

test("presentation cannot promote an unsupported capability to a retry hint", () => {
  const sources = changed(
    "publicError",
    "retryable: owner_retryable",
    "retryable: true",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("retryability"),
    ),
  );
});

test("native Cart decrement cannot reconstruct embedded Pricing", () => {
  const sources = changed(
    "nativeDecrement",
    "rustok_pricing::in_process_pricing_read_port",
    "PricingService::new",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("native decrement"),
    ),
  );
});

test("native context cannot substitute an optional Cart locale for the admitted string", () => {
  const sources = changed(
    "nativeCart",
    "scope.request.locale.clone(),",
    "cart.locale_code.clone(),",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("native string caller locale"),
    ),
  );
});

test("global or ID-only Cart must clear a page channel", () => {
  const sources = changed(
    "resolver",
    "context.channel = cart.channel_slug.clone();",
    "context.channel = cart.channel_slug.clone().or(context.channel);",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("Cart channel replaces"),
    ),
  );
});

test("checkout presentation must not promote permanent owner refusals", () => {
  const sources = changed(
    "resolver",
    "PortError::new(error.kind, code, public.message, error.retryable)",
    "PortError::new(error.kind, code, public.message, true)",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("checkout owner retryability"),
    ),
  );
});

test("mounted GraphQL pricing alias requires its selected runtime argument", () => {
  const sources = changed(
    "typedReprice",
    "crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(",
    "incorrect_local_product_selection(",
  );
  assert.ok(
    verifyPricingProductIdentityContext(sources).some((failure) =>
      failure.includes("third argument"),
    ),
  );
});
