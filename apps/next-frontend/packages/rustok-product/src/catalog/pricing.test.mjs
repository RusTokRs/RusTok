/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mergeProductPricing } from "./pricing.ts";

const price = {
  currencyCode: "KWD",
  amount: "9007199254740993.001",
  compareAtAmount: "9007199254740994.001",
  onSale: true,
};
const variants = [
  { id: "a", sku: null, inventoryQuantity: 7, inStock: true, prices: [price] },
  { id: "b", sku: null, inventoryQuantity: 2, inStock: true, prices: [] },
];

test("pricing overlay joins only by ID and leaves Product inventory intact", () => {
  const result = mergeProductPricing(variants, [
    { id: "b", effectivePrice: price, inventoryQuantity: 999 },
  ]);
  assert.equal(result[0].inventoryQuantity, 7);
  assert.deepEqual(result[0].prices, []);
  assert.deepEqual(result[1].prices, []);
  assert.equal(result[1].inventoryQuantity, 2);
  assert.equal(result[1].effectivePrice.amount, "9007199254740993.001");
});

test("explicitly absent Pricing price cannot fall back to a stale catalog price", () => {
  const [result] = mergeProductPricing(variants, [
    { id: "a", prices: [], effectivePrice: null },
  ]);
  assert.deepEqual(result.prices, []);
  assert.equal(result.effectivePrice, null);
});

test("buyer view renders owner decimal strings and sale flags without money arithmetic", () => {
  const source = readFileSync(
    new URL("../components/product-buyer-panel.tsx", import.meta.url),
    "utf8",
  );
  assert.ok(source.includes("view.price.amount"));
  assert.ok(source.includes("view.price.onSale"));
  assert.ok(!source.includes(".toFixed("));
  assert.ok(!source.includes("parseFloat("));
});

test("unavailable Pricing fails closed without using catalog price fallback", () => {
  const results = mergeProductPricing(variants, []);
  for (const result of results) {
    assert.deepEqual(result.prices, []);
    assert.equal(result.effectivePrice, null);
  }
});

test("a missing SKU projection is unavailable, not an authoritative unpriced result", () => {
  const [missing] = mergeProductPricing(variants, []);
  assert.equal(missing.pricingStatus, "unavailable");
  const [unpriced] = mergeProductPricing(variants, [
    { id: "a", prices: [], effectivePrice: null },
  ]);
  assert.equal(unpriced.pricingStatus, "unpriced");
});

test("the Product quote SDK normalizes before IO and reads no raw inspection rows", () => {
  const source = readFileSync(
    new URL("../api/products.ts", import.meta.url),
    "utf8",
  );
  const operation = source.slice(
    source.indexOf("export async function fetchStorefrontProductPricing"),
  );
  assert.ok(
    operation.indexOf("normalizePricingQuery") <
      operation.indexOf("await graphql"),
  );
  assert.ok(/assertPricingQuoteResponse\(pricing,\s*request\)/.test(operation));
  const start = source.indexOf("const STOREFRONT_PRICING_PRODUCT_QUERY");
  const end = source.indexOf("const STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY");
  assert.ok(
    start >= 0 && end > start,
    "the real quote document must be inspected",
  );
  const document = source.slice(start, end);
  assert.ok(!/\bprices\s*\{/.test(document));
});

test("a missing nullable money field is unavailable, never an explicit unpriced result", () => {
  assert.equal(
    mergeProductPricing(variants, [{ id: "a" }])[0].pricingStatus,
    "unavailable",
  );
});
