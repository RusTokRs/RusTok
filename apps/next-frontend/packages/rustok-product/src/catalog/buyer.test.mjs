/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { normalizeLocaleTag } from "@rustok/next-fluent/utils";
import { loadTypeScriptCore } from "../../../../../../scripts/verify/ts-core-fixture.mjs";

const pricing = loadTypeScriptCore(
  new URL("../../../rustok-pricing/src/core.ts", import.meta.url),
  [
    "parsePricingUuid",
    "parsePricingQuantity",
    "parsePricingRouteQuery",
    "assertPricingQuoteResponse",
  ],
  { normalizeLocaleTag },
);
const overlay = loadTypeScriptCore(new URL("./pricing.ts", import.meta.url), [
  "mergeProductPricing",
]);
const buyer = loadTypeScriptCore(
  new URL("./buyer.ts", import.meta.url),
  [
    "ProductBuyerError",
    "assertBuyerProduct",
    "selectBuyerTuple",
    "parseBuyerRoute",
    "buyerAxisViews",
    "buyerQuantity",
    "buyerHref",
    "assertBuyerQuote",
    "buildBuyerView",
    "createBuyerInput",
    "buyerInputInScope",
    "updateBuyerInput",
    "buyerCartRecovery",
  ],
  { normalizeLocaleTag, ...pricing, ...overlay },
);

const id = (value) =>
  `00000000-0000-4000-8000-${String(value).padStart(12, "0")}`;
const axis = (attribute, first, second) => ({
  id: id(attribute + 100),
  attributeId: id(attribute),
  name: String(attribute),
  position: attribute,
  allowedValues: [first, second].map((option, position) => ({
    optionId: id(option),
    value: String(option),
    position,
  })),
});
const variant = (number, a, b) => ({
  id: id(number),
  title: "SKU",
  sku: null,
  inventoryQuantity: 8,
  inventoryPolicy: "deny",
  inStock: true,
  prices: [{ currencyCode: "KWD", amount: "999", onSale: true }],
  axisValues: [
    { attributeId: id(10), optionId: id(a) },
    { attributeId: id(20), optionId: id(b) },
  ],
  combinationIdentity: `${id(10)}:${id(a)};${id(20)}:${id(b)}`,
});
const product = () => ({
  id: id(1),
  status: "published",
  sellerId: "platform",
  tags: [],
  images: [],
  attributes: [],
  translations: [],
  variantAxes: [axis(10, 11, 12), axis(20, 21, 22)],
  variants: [variant(3, 11, 21), variant(4, 12, 22)],
});
const request = { currencyCode: "KWD", quantity: 2 };
const context = {
  ...request,
  regionId: null,
  priceListId: null,
  channelId: null,
  channelSlug: null,
};
const price = {
  currencyCode: "KWD",
  amount: "9007199254740993.001",
  compareAtAmount: "9007199254740994.001",
  onSale: true,
  source: {
    priceId: id(50),
    kind: "base",
    ruleKind: null,
    adjustmentPercent: null,
  },
  priceListId: null,
  channelId: null,
  channelSlug: null,
};
const quote = (catalog) => ({
  id: catalog.id,
  resolutionContext: { ...context },
  resolvedAt: "2026-10-30T10:00:00Z",
  variants: catalog.variants.map((variant) => ({
    id: variant.id,
    effectivePrice: { ...price },
  })),
});
const selection = (catalog) =>
  buyer.selectBuyerTuple(catalog, { [id(10)]: id(11), [id(20)]: id(21) });

test("sparse tuples stay partial or unavailable; no first SKU or Cartesian substitution", () => {
  const catalog = product();
  buyer.assertBuyerProduct(catalog);
  assert.equal(buyer.selectBuyerTuple(catalog, {}).status, "incomplete");
  assert.equal(
    buyer.selectBuyerTuple(catalog, { [id(10)]: id(11) }).variant,
    null,
  );
  const missing = buyer.selectBuyerTuple(catalog, {
    [id(10)]: id(11),
    [id(20)]: id(22),
  });
  assert.equal(missing.status, "unavailable");
  assert.equal(missing.variant, null);
  const axes = buyer.buyerAxisViews(catalog, { [id(10)]: id(11) });
  assert.equal(
    axes[1].options.find((option) => option.optionId === id(22)).disabled,
    true,
  );
  assert.equal(selection(catalog).variant.id, id(3));
});

test("projection refuses malformed tuples, duplicate UUIDs, unknown policies and oversized catalogs", () => {
  for (const mutate of [
    (p) => p.variants.push({ ...p.variants[0] }),
    (p) => {
      p.variants[0].combinationIdentity = "other";
    },
    (p) => {
      p.variants[0].axisValues.pop();
    },
    (p) => {
      p.variants[0].inventoryPolicy = "guess";
    },
    (p) => {
      p.variants[0].inStock = "true";
    },
    (p) => {
      p.variants[0].inventoryQuantity = null;
    },
    (p) => {
      p.variants[0].inventoryQuantity = 1.5;
    },
    (p) => {
      p.variantAxes = Array(33).fill(p.variantAxes[0]);
    },
    (p) => {
      p.variants = Array(257).fill(p.variants[0]);
    },
  ]) {
    const catalog = product();
    mutate(catalog);
    assert.throws(() => buyer.assertBuyerProduct(catalog));
  }
  const catalog = product();
  catalog.variantAxes = [];
  catalog.variants = [
    { ...catalog.variants[0], axisValues: [], combinationIdentity: null },
  ];
  assert.equal(buyer.selectBuyerTuple(catalog, {}).variant.id, id(3));
});

test("URL owns a complete SKU or a partial tuple and refuses contradictory/duplicate selectors", () => {
  const catalog = product();
  assert.equal(
    buyer.parseBuyerRoute(catalog, { variant_id: id(4) }, "en").selection
      .variant.id,
    id(4),
  );
  assert.equal(
    buyer.parseBuyerRoute(catalog, { [`axis.${id(10)}`]: id(11) }, "en")
      .selection.status,
    "incomplete",
  );
  for (const params of [
    { variant_id: id(4), [`axis.${id(10)}`]: id(11) },
    { variant_id: [id(3), id(4)] },
    { cart_id: [id(5), id(6)] },
    { [`axis.${id(99)}`]: id(11) },
    { currency: "" },
    { channel_slug: " " },
    { currency: "KWD", region_id: "" },
    { quantity: "1tail" },
    { quantity: "2147483648" },
  ])
    assert.throws(() => buyer.parseBuyerRoute(catalog, params, "en"));
});

test("quantity can be chosen before currency without initiating a fabricated quote", () => {
  const catalog = product();
  catalog.variants.forEach((variant) => {
    variant.prices = [];
  });
  const parsed = buyer.parseBuyerRoute(
    catalog,
    { variant_id: id(3), quantity: "3" },
    "en",
  );
  assert.equal(parsed.pricing.quantity, 3);
  assert.equal(parsed.pricing.currencyCode, undefined);
  assert.equal(
    buyer.buildBuyerView(
      catalog,
      parsed.selection,
      "3",
      parsed.pricing,
      null,
      "idle",
    ).status,
    "chooseCurrency",
  );
  for (const text of ["", "0", "-1", "2.5", "2tail", "2147483648"])
    assert.equal(buyer.buyerQuantity(text), null);
});

test("quote correlation requires every exact SKU, full context and explicit nullable money", () => {
  const catalog = product();
  const observed = quote(catalog);
  assert.doesNotThrow(() => buyer.assertBuyerQuote(catalog, observed, request));
  for (const mutate of [
    (q) => {
      q.id = id(9);
    },
    (q) => q.variants.pop(),
    (q) => {
      q.variants[0].id = id(9);
    },
    (q) => {
      q.variants[1].id = q.variants[0].id;
    },
    (q) => {
      q.resolutionContext.quantity = 3;
    },
    (q) => {
      q.resolutionContext.regionId = id(8);
    },
    (q) => {
      delete q.resolutionContext.priceListId;
    },
    (q) => {
      delete q.variants[0].effectivePrice;
    },
  ]) {
    const invalid = quote(catalog);
    mutate(invalid);
    assert.throws(() => buyer.assertBuyerQuote(catalog, invalid, request));
  }
  const channel = id(77);
  observed.resolutionContext.channelId = channel;
  observed.resolutionContext.channelSlug = "web";
  assert.doesNotThrow(() =>
    buyer.assertBuyerQuote(catalog, observed, {
      ...request,
      channelSlug: "WEB",
    }),
  );
  assert.throws(() =>
    buyer.assertBuyerQuote(catalog, observed, {
      ...request,
      channelId: id(78),
    }),
  );
});

test("pending, rejected or wrong-quantity quotes never display or sell their previous money", () => {
  const catalog = product();
  const selected = selection(catalog);
  const observed = quote(catalog);
  for (const status of ["pending", "failed", "idle"]) {
    const view = buyer.buildBuyerView(
      catalog,
      selected,
      "2",
      request,
      observed,
      status,
    );
    assert.equal(view.price, null);
    assert.equal(view.canPurchase, false);
  }
  const stale = buyer.buildBuyerView(
    catalog,
    selected,
    "3",
    { ...request, quantity: 3 },
    observed,
    "ready",
  );
  assert.equal(stale.price, null);
  assert.equal(stale.canPurchase, false);
  const ready = buyer.buildBuyerView(
    catalog,
    selected,
    "2",
    request,
    observed,
    "ready",
  );
  assert.equal(ready.price.amount, "9007199254740993.001");
  assert.equal(ready.canPurchase, true);
  observed.variants.forEach((variant) => {
    variant.effectivePrice = null;
  });
  assert.equal(
    buyer.buildBuyerView(catalog, selected, "2", request, observed, "ready")
      .status,
    "unpriced",
  );
});

test("list-context observations stay previews and stock remains Product-owned", () => {
  const catalog = product();
  const selected = selection(catalog);
  const observed = quote(catalog);
  observed.resolutionContext.priceListId = id(90);
  const preview = buyer.buildBuyerView(
    catalog,
    selected,
    "2",
    { ...request, priceListId: id(90) },
    observed,
    "ready",
  );
  assert.equal(preview.status, "preview");
  assert.equal(preview.canPurchase, false);
  catalog.variants[0].inventoryQuantity = 1;
  const limited = buyer.buildBuyerView(
    catalog,
    selection(catalog),
    "2",
    request,
    quote(catalog),
    "ready",
  );
  assert.equal(limited.status, "stock");
  assert.equal(limited.canPurchase, false);
});

test("links retain unrelated query/context/raw invalid quantity without inventing a sparse SKU", () => {
  const catalog = product();
  const partial = buyer.selectBuyerTuple(catalog, { [id(10)]: id(11) });
  const href = buyer.buyerHref(
    "/en/products/boots",
    `campaign=a+b&cart_id=${id(70)}`,
    partial,
    request,
    "2tail",
  );
  const params = new URL(href, "https://shop.example").searchParams;
  assert.equal(params.get("campaign"), "a b");
  assert.equal(params.get("cart_id"), id(70));
  assert.equal(params.get("quantity"), "2tail");
  assert.equal(params.get("variant_id"), null);
  assert.equal(params.get(`axis.${id(10)}`), id(11));
});

test("input revisions fence A→B→A; auth/executor quote changes do not erase input", () => {
  const catalog = product();
  const scope = {};
  const initial = buyer.parseBuyerRoute(
    catalog,
    { variant_id: id(3), quantity: "2" },
    "en",
  );
  const input = buyer.createBuyerInput(scope, initial);
  const back = buyer.updateBuyerInput(
    buyer.updateBuyerInput(input, { quantityText: "3" }),
    { quantityText: "2" },
  );
  assert.equal(back.quantityText, input.quantityText);
  assert.equal(back.revision, input.revision + 2);
  assert.equal(buyer.buyerInputInScope(back, scope, initial), back);
  const source = readFileSync(
    new URL("../components/product-buyer-panel.tsx", import.meta.url),
    "utf8",
  ).replace(/\s/g, "");
  assert.ok(source.includes("({product,handle,locale,tenantSlug})"));
  assert.ok(source.includes("input.revision"));
  assert.ok(source.includes("read.scope===quoteScope"));
});

test("locale recovery uses exact supported locale; it never drops a regional subtag", () => {
  assert.equal(
    buyer.buyerCartRecovery("en-GB", "en", { en: "/en/products/boots" }).kind,
    "unavailable",
  );
  assert.equal(buyer.buyerCartRecovery("en", "en", {}).kind, "inline");
  assert.equal(
    buyer.buyerCartRecovery("ru", "en", { ru: "/ru/products/boots" }).kind,
    "navigate",
  );
});
