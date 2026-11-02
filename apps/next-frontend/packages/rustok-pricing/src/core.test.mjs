/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  assertPricingGraphqlSuccess,
  assertPricingQuoteResponse,
  createPricingSelection,
  parsePricingQuantity,
  parsePricingRouteQuery,
  pricingContextQuery,
  pricingCurrencies,
  pricingDiscovery,
  pricingReadKey,
  pricingSelectionInScope,
  pricingVariantView,
  updatePricingContext,
  validatePricingQuery,
  visiblePricingValue,
} from "./core.ts";

const data = {
  products: { items: [], total: 0, page: 1, perPage: 8, hasNext: false },
  availableChannels: [],
  activePriceLists: [],
  resolutionContext: null,
};

test("a quote cannot be reused across tenant, auth, locale, SKU product or money context", () => {
  const query = {
    selectedHandle: "boots",
    locale: "en",
    currencyCode: "KWD",
    quantity: 1,
  };
  const key = pricingReadKey(query, "tenant-a", "actor-a");
  const scope = { key: "scope", executor: {} };
  const record = { scope, key, value: data };
  assert.equal(visiblePricingValue(record, scope, key), data);
  for (const other of [
    pricingReadKey(query, "tenant-b", "actor-a"),
    pricingReadKey(query, "tenant-a", "actor-b"),
    pricingReadKey({ ...query, locale: "ru" }, "tenant-a", "actor-a"),
    pricingReadKey(
      { ...query, selectedHandle: "other" },
      "tenant-a",
      "actor-a",
    ),
    pricingReadKey({ ...query, quantity: 2 }, "tenant-a", "actor-a"),
    pricingReadKey({ ...query, currencyCode: "JPY" }, "tenant-a", "actor-a"),
    pricingReadKey({ ...query, channelId: "other" }, "tenant-a", "actor-a"),
  ])
    assert.equal(visiblePricingValue(record, scope, other), null);
});

test("non-finite quantities cannot collide with an omitted quantity or silently become defaults", () => {
  const omitted = pricingReadKey({}, "tenant", null);
  assert.notEqual(pricingReadKey({ quantity: NaN }, "tenant", null), omitted);
  for (const quantity of [NaN, Infinity, 0, -1, 1.5, 2147483648]) {
    assert.throws(() =>
      validatePricingQuery({ currencyCode: "USD", quantity }),
    );
  }
});

test("no currency means inspection, not a fabricated USD quote", () => {
  assert.deepEqual(pricingContextQuery({ currencyCode: "", quantity: 1 }), {
    currencyCode: undefined,
    quantity: undefined,
    regionId: undefined,
    priceListId: undefined,
    channelId: undefined,
    channelSlug: undefined,
  });
  assert.throws(() => validatePricingQuery({ quantity: 1 }));
  assert.throws(() => validatePricingQuery({ currencyCode: "EU1" }));
});

test("changing channel supplies a coherent owner selector and clears the previous list", () => {
  const context = {
    currencyCode: "USD",
    channelId: "web",
    channelSlug: "web-store",
    priceListId: "web-sale",
  };
  const next = updatePricingContext(context, { channelId: "mobile" }, [
    { id: "mobile", slug: "mobile-app" },
  ]);
  assert.equal(next.channelId, "mobile");
  assert.equal(next.channelSlug, "mobile-app");
  assert.equal(next.priceListId, null);
  const cleared = updatePricingContext(next, { channelId: null }, []);
  assert.equal(cleared.channelSlug, null);
});

test("currency coverage comes from Pricing and retains explicit unpriced intent", () => {
  const model = {
    ...data,
    products: {
      ...data.products,
      items: [{ currencies: ["KWD", "JPY", "KWD"] }],
    },
  };
  assert.deepEqual(pricingCurrencies(pricingDiscovery(model), "EUR"), [
    "EUR",
    "JPY",
    "KWD",
  ]);
  assert.deepEqual(pricingCurrencies(null, ""), []);
});

test("a missing contextual price never displays another currency or a raw sale as an effective price", () => {
  const raw = { currencyCode: "USD", amount: "12.00", onSale: true };
  const model = pricingVariantView(
    { id: "sku", prices: [raw], effectivePrice: null },
    { currencyCode: "JPY" },
  );
  assert.equal(model.basePrice, null);
  assert.equal(model.effective, null);
  assert.equal(model.onSale, false);
});

test("selections are scoped before IO across tenant, actor, locale and executor changes", () => {
  const executor = {};
  const scope = {
    key: pricingReadKey({ locale: "en" }, "tenant-a", "actor-a"),
    executor,
  };
  const state = createPricingSelection(scope, {
    ...data,
    selectedHandle: "old-tenant-product",
    resolutionContext: {
      currencyCode: "EUR",
      regionId: "old-region",
      priceListId: "old-list",
      channelId: "old-channel",
      channelSlug: "old-store",
      quantity: 7,
    },
  });
  assert.equal(pricingSelectionInScope(state, { ...scope }), state);
  const record = { scope, key: "quote", value: data };
  for (const nextScope of [
    { key: pricingReadKey({ locale: "en" }, "tenant-b", "actor-a"), executor },
    { key: pricingReadKey({ locale: "en" }, "tenant-a", "actor-b"), executor },
    { key: pricingReadKey({ locale: "ru" }, "tenant-a", "actor-a"), executor },
    { ...scope, executor: {} },
  ]) {
    const fresh = pricingSelectionInScope(state, nextScope);
    assert.equal(fresh.selectedHandle, null);
    assert.deepEqual(fresh.context, { currencyCode: "", quantity: 1 });
    const query = pricingContextQuery(fresh.context);
    assert.equal(query.regionId, undefined);
    assert.equal(query.priceListId, undefined);
    assert.equal(query.channelId, undefined);
    assert.equal(query.channelSlug, undefined);
    assert.equal(query.quantity, undefined);
    assert.equal(visiblePricingValue(record, nextScope, "quote"), null);
  }
});

test("failed re-quotes retain only same-scope discovery, never old effective prices or errors", () => {
  const scope = { key: "tenant-and-actor", executor: {} };
  const discovery = pricingDiscovery({
    ...data,
    selectedProduct: {
      variants: [
        {
          prices: [{ currencyCode: "JPY" }],
          effectivePrice: { amount: "123" },
        },
      ],
    },
  });
  assert.deepEqual(Object.keys(discovery).sort(), [
    "activePriceLists",
    "availableChannels",
    "currencies",
  ]);
  assert.equal(JSON.stringify(discovery).includes("effectivePrice"), false);
  const record = { scope, key: "channel-a", value: discovery };
  assert.equal(visiblePricingValue(record, scope, "channel-a"), discovery);
  assert.equal(visiblePricingValue(record, scope, "channel-b"), null);
  assert.equal(visiblePricingValue(record, scope), discovery); // Channel options remain usable.
  assert.equal(visiblePricingValue(record, { ...scope, executor: {} }), null);
  const quote = { scope, key: "quantity-1", value: data };
  assert.equal(visiblePricingValue(quote, scope, "quantity-2"), null);
  const failure = { scope, key: "quantity-1", value: "old failure" };
  assert.equal(visiblePricingValue(failure, scope, "quantity-2"), null);
});

test("changing quote currency clears the old region but not a multi-currency list", () => {
  const current = {
    currencyCode: "EUR",
    regionId: "eur-region",
    priceListId: "sale",
    quantity: 2,
  };
  const next = updatePricingContext(current, { currencyCode: "JPY" }, []);
  assert.equal(next.regionId, null);
  assert.equal(next.priceListId, "sale");
  assert.equal(next.quantity, 2);
  assert.equal(
    updatePricingContext(current, { currencyCode: "EUR" }, []).regionId,
    "eur-region",
  );
  assert.equal(
    updatePricingContext(current, { quantity: 3 }, []).regionId,
    "eur-region",
  );
  const inspection = updatePricingContext(current, { currencyCode: "" }, []);
  assert.equal(inspection.regionId, null);
  assert.equal(inspection.priceListId, null);
});

test("currency coverage includes the explicitly selected product outside the catalog page", () => {
  const discovery = pricingDiscovery({
    ...data,
    products: { ...data.products, items: [{ currencies: ["EUR"] }] },
    selectedProduct: {
      variants: [
        { prices: [{ currencyCode: "JPY" }, { currencyCode: "EUR" }] },
        { prices: [{ currencyCode: "CHF" }] },
      ],
    },
  });
  assert.deepEqual(pricingCurrencies(discovery, "KWD"), [
    "CHF",
    "EUR",
    "JPY",
    "KWD",
  ]);
});

test("requested quotes cannot downgrade to raw inspection or accept a different currency/quantity", () => {
  const inspection = {
    resolutionContext: null,
    resolvedAt: null,
    variants: [
      {
        effectivePrice: null,
        prices: [{ currencyCode: "USD", amount: "12.00" }],
      },
    ],
  };
  assert.doesNotThrow(() => assertPricingQuoteResponse(inspection, {}));
  assert.throws(() =>
    assertPricingQuoteResponse(inspection, { currencyCode: "JPY" }),
  );
  const quoted = {
    ...inspection,
    resolutionContext: {
      currencyCode: "JPY",
      quantity: 2,
      regionId: null,
      priceListId: null,
      channelId: null,
      channelSlug: null,
    },
    resolvedAt: "2026-10-30T10:00:00Z",
    variants: [
      { effectivePrice: { currencyCode: "JPY", amount: "1200" } },
      { effectivePrice: null },
    ],
  };
  assert.doesNotThrow(() =>
    assertPricingQuoteResponse(quoted, { currencyCode: " jpy ", quantity: 2 }),
  );
  assert.throws(() => assertPricingQuoteResponse(quoted, {}));
  assert.throws(() =>
    assertPricingQuoteResponse(quoted, { currencyCode: "EUR", quantity: 2 }),
  );
  assert.throws(() =>
    assertPricingQuoteResponse(quoted, { currencyCode: "JPY", quantity: 1 }),
  );
  assert.throws(() =>
    assertPricingQuoteResponse(
      { ...quoted, resolvedAt: null },
      { currencyCode: "JPY", quantity: 2 },
    ),
  );
  assert.throws(() =>
    assertPricingQuoteResponse(
      { ...quoted, variants: [{ effectivePrice: { currencyCode: "USD" } }] },
      { currencyCode: "JPY", quantity: 2 },
    ),
  );
  assert.throws(() =>
    assertPricingQuoteResponse(
      { ...inspection, resolutionContext: undefined },
      {},
    ),
  );
  // A complete, explicitly unpriced quote is still a successful observation.
  assert.doesNotThrow(() =>
    assertPricingQuoteResponse(
      { ...quoted, variants: [{ effectivePrice: null }] },
      { currencyCode: "JPY", quantity: 2 },
    ),
  );
});

test("partial GraphQL data with a refusal is not a successful not-found quote", () => {
  assert.doesNotThrow(() =>
    assertPricingGraphqlSuccess({ data: { storefrontPricingProduct: null } }),
  );
  assert.doesNotThrow(() =>
    assertPricingGraphqlSuccess({ data: {}, errors: [] }),
  );
  assert.throws(
    () =>
      assertPricingGraphqlSuccess({
        data: { storefrontPricingProduct: null },
        errors: [{ message: "private owner failure" }],
      }),
    { message: "Pricing request could not be completed" },
  );
});

test("quantity URLs reject malformed, fractional and out-of-range values without defaulting", () => {
  assert.equal(parsePricingQuantity(undefined), undefined);
  assert.equal(parsePricingQuantity("12"), 12);
  for (const value of ["", "0", "-1", "1.5", "1tail", "NaN", "2147483648"]) {
    assert.throws(() => parsePricingQuantity(value));
  }
});

test("Pricing route retains region/context and never invents a quantity for inspection", () => {
  const empty = parsePricingRouteQuery({}, "en");
  assert.equal(empty.quantity, undefined);
  assert.equal(empty.currencyCode, undefined);
  const query = parsePricingRouteQuery(
    {
      handle: "boots",
      currency: " jpy ",
      region_id: "00000000-0000-4000-8000-000000000001",
      price_list_id: "00000000-0000-4000-8000-000000000002",
      channel_id: "00000000-0000-4000-8000-000000000003",
      channel_slug: "web-store",
      quantity: "2",
    },
    "ru",
  );
  assert.equal(query.regionId, "00000000-0000-4000-8000-000000000001");
  assert.equal(query.currencyCode, "JPY");
  assert.equal(query.quantity, 2);
  assert.equal(query.locale, "ru");
  for (const params of [
    { quantity: "2" },
    { currency: "JPY", quantity: "2tail" },
    { currency: "JPY", quantity: "0" },
    { currency: "JPY", quantity: "1.5" },
    { currency: ["EUR", "JPY"] },
    { region_id: "00000000-0000-4000-8000-000000000001" },
  ])
    assert.throws(() => parsePricingRouteQuery(params, "en"));
});

test("a failed SSR read retains URL intent without pinning an omitted channel to the host", () => {
  const scope = { key: "scope", executor: {} };
  const query = parsePricingRouteQuery(
    {
      handle: "boots",
      currency: "JPY",
      region_id: "00000000-0000-4000-8000-000000000001",
      quantity: "2",
    },
    "en",
  );
  const initial = createPricingSelection(scope, null, query);
  assert.equal(initial.selectedHandle, "boots");
  assert.equal(initial.context.currencyCode, "JPY");
  assert.equal(
    initial.context.regionId,
    "00000000-0000-4000-8000-000000000001",
  );
  assert.equal(initial.context.quantity, 2);
  assert.equal(initial.context.channelId, undefined);
  assert.equal(initial.context.channelSlug, undefined);
});

test("the Pricing RSC route sends only serializable props and keeps the original request", () => {
  const page = readFileSync(
    new URL("../../../src/app/[locale]/pricing/page.tsx", import.meta.url),
    "utf8",
  );
  const bridge = readFileSync(
    new URL(
      "../../../src/features/pricing/components/pricing-route-client.tsx",
      import.meta.url,
    ),
    "utf8",
  );
  assert.ok(page.includes("<PricingRouteClient"));
  assert.ok(!page.includes("graphql={storefrontGraphql}"));
  assert.ok(page.includes("initialQuery={pricingQuery}"));
  assert.ok(bridge.includes('"use client"'));
  assert.ok(bridge.includes("graphql={storefrontGraphql}"));
});

test("explicit blank or nil pricing selectors are refused rather than omitted", () => {
  for (const key of [
    "currency",
    "region_id",
    "price_list_id",
    "channel_id",
    "channel_slug",
  ])
    assert.throws(() =>
      parsePricingRouteQuery({ currency: "JPY", [key]: " " }, "en"),
    );
  assert.throws(() =>
    parsePricingRouteQuery(
      { currency: "JPY", channel_id: "00000000-0000-0000-0000-000000000000" },
      "en",
    ),
  );
  const quoted = {
    resolutionContext: {
      currencyCode: "JPY",
      quantity: 1,
      regionId: null,
      priceListId: null,
      channelId: null,
      channelSlug: null,
    },
    resolvedAt: "2026-10-30T10:00:00Z",
    variants: [{ effectivePrice: null }],
  };
  assert.doesNotThrow(() =>
    assertPricingQuoteResponse(quoted, { currencyCode: "JPY" }),
  );
  assert.throws(() =>
    assertPricingQuoteResponse(
      {
        ...quoted,
        resolutionContext: { ...quoted.resolutionContext, regionId: undefined },
      },
      { currencyCode: "JPY" },
    ),
  );
  assert.throws(() =>
    assertPricingQuoteResponse(
      { ...quoted, variants: [{}] },
      { currencyCode: "JPY" },
    ),
  );
});

test("quote timestamps require valid RFC3339 dates, never prose, rollover or missing zones", () => {
  const product = {
    resolutionContext: {
      currencyCode: "JPY",
      quantity: 1,
      regionId: null,
      priceListId: null,
      channelId: null,
      channelSlug: null,
    },
    variants: [{ effectivePrice: null }],
  };
  for (const resolvedAt of [
    "October 30, 2026",
    "2026-02-30T10:00:00Z",
    "2026-10-30T24:00:00Z",
    "2026-10-30T10:00:00",
    "2026-10-30T10:00:00+24:00",
    " 2026-10-30T10:00:00Z",
  ])
    assert.throws(() =>
      assertPricingQuoteResponse(
        { ...product, resolvedAt },
        { currencyCode: "JPY" },
      ),
    );
  assert.doesNotThrow(() =>
    assertPricingQuoteResponse(
      { ...product, resolvedAt: "2026-10-30T10:00:00.123456789+03:00" },
      { currencyCode: "JPY" },
    ),
  );
});

test("RFC3339 leap-second observations remain observations rather than a client clock policy", () => {
  assert.doesNotThrow(() =>
    assertPricingQuoteResponse(
      {
        resolutionContext: {
          currencyCode: "JPY",
          quantity: 1,
          regionId: null,
          priceListId: null,
          channelId: null,
          channelSlug: null,
        },
        variants: [{ effectivePrice: null }],
        resolvedAt: "2016-12-31T23:59:60Z",
      },
      { currencyCode: "JPY" },
    ),
  );
});
