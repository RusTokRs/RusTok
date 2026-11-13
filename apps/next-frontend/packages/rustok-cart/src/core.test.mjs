/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { normalizeLocaleTag } from "@rustok/next-fluent/utils";
import { loadTypeScriptCore } from "../../../../../scripts/verify/ts-core-fixture.mjs";

const pricing = loadTypeScriptCore(
  new URL("../../rustok-pricing/src/core.ts", import.meta.url),
  ["normalizePricingQuery", "parsePricingUuid"],
  { normalizeLocaleTag },
);
const cartCore = loadTypeScriptCore(
  new URL("./core.ts", import.meta.url),
  [
    "cartStorageKey",
    "cartMutationMarkerKey",
    "cartReferenceFromSearch",
    "adoptBuyerCart",
    "validCartBuyerSelection",
    "validCartPricingExpectation",
    "cartMatchesPricingExpectation",
    "pricingExpectationFromCart",
  ],
  { normalizeLocaleTag, ...pricing },
);
const id = (value) =>
  `00000000-0000-4000-8000-${String(value).padStart(12, "0")}`;
const context = {
  locale: "en",
  currencyCode: "KWD",
  regionId: null,
  priceListId: null,
  channelId: null,
  channelSlug: null,
  quantity: 2,
};
const cart = {
  id: id(1),
  status: "active",
  currencyCode: "KWD",
  regionId: null,
  localeCode: "en",
  channelId: null,
  channelSlug: null,
  lineItems: [],
};

test("only a successful owner missing response may clear a valid reference; inactive/foreign responses refuse adoption", () => {
  assert.equal(cartCore.adoptBuyerCart(null, cart.id), null);
  assert.equal(cartCore.adoptBuyerCart(cart, cart.id), cart);
  assert.throws(() => cartCore.adoptBuyerCart(null, ""));
  assert.throws(() => cartCore.adoptBuyerCart(cart, id(2)));
  for (const status of ["completed", "cancelled", "draft", "unknown"])
    assert.throws(() => cartCore.adoptBuyerCart({ ...cart, status }, cart.id));
});

test("SKU, positive-i32 quantity, locale and ordinary normalized context are admitted before creating a Cart", () => {
  assert.equal(cartCore.validCartBuyerSelection(id(3), context, 2), true);
  for (const sku of ["", "sku", "00000000-0000-0000-0000-000000000000"])
    assert.equal(cartCore.validCartBuyerSelection(sku, context, 2), false);
  for (const quantity of [0, -1, 1.5, 2147483648, NaN])
    assert.equal(
      cartCore.validCartBuyerSelection(
        id(3),
        { ...context, quantity },
        quantity,
      ),
      false,
    );
  for (const patch of [
    { currencyCode: "" },
    { regionId: "" },
    { channelId: "" },
    { channelSlug: " " },
    { locale: "" },
    { priceListId: id(9) },
    { quantity: 3 },
  ])
    assert.equal(
      cartCore.validCartBuyerSelection(id(3), { ...context, ...patch }, 2),
      false,
    );
});

test("Cart context is immutable; null region/global scope and exact locale are not wildcards", () => {
  assert.equal(cartCore.cartMatchesPricingExpectation(cart, context), true);
  for (const patch of [
    { currencyCode: "EUR" },
    { regionId: id(9) },
    { channelId: id(8), channelSlug: "web" },
    { locale: "en-GB" },
    { priceListId: id(7) },
  ])
    assert.equal(
      cartCore.cartMatchesPricingExpectation(cart, { ...context, ...patch }),
      false,
    );
  assert.equal(
    cartCore.cartMatchesPricingExpectation(
      { ...cart, channelId: id(8) },
      { ...context, channelId: id(8), channelSlug: "web" },
    ),
    true,
  );
  assert.equal(
    cartCore.cartMatchesPricingExpectation(
      { ...cart, localeCode: null },
      context,
    ),
    false,
  );
  assert.equal(cartCore.pricingExpectationFromCart(cart, 3).quantity, 3);
});

test("URL references reject blank, nil and duplicates rather than selecting or clearing another cart", () => {
  assert.equal(cartCore.cartReferenceFromSearch("?campaign=boots"), null);
  assert.equal(cartCore.cartReferenceFromSearch(`?cart_id=${id(5)}`), id(5));
  for (const query of [
    "?cart_id=",
    "?cart_id=00000000-0000-0000-0000-000000000000",
    `?cart_id=${id(5)}&cart_id=${id(6)}`,
  ])
    assert.throws(() => cartCore.cartReferenceFromSearch(query));
});

test("persisted references and pending warnings are scoped by tenant and opaque actor hash, never raw bearer", async () => {
  const guest = await cartCore.cartStorageKey("tenant-a", null);
  const actor = await cartCore.cartStorageKey("tenant-a", "private-bearer-a");
  assert.notEqual(actor, guest);
  assert.notEqual(
    actor,
    await cartCore.cartStorageKey("tenant-b", "private-bearer-a"),
  );
  assert.notEqual(
    actor,
    await cartCore.cartStorageKey("tenant-a", "private-bearer-b"),
  );
  assert.equal(actor.includes("private-bearer"), false);
  assert.equal(cartCore.cartMutationMarkerKey(actor), `${actor}:mutation`);
});

test("client adapter fences auth before IO, persists uncertainty first and retains old-scope create IDs before returning", () => {
  const source = readFileSync(
    new URL("./context/cart-context.tsx", import.meta.url),
    "utf8",
  );
  const add = source.slice(
    source.indexOf("const addItem"),
    source.indexOf("const mutate"),
  );
  assert.ok(
    add.indexOf("validCartBuyerSelection") <
      add.indexOf("createStorefrontCart("),
  );
  assert.ok(
    add.indexOf("localStorage.setItem(key, marker.id)") <
      add.indexOf("createStorefrontCart("),
  );
  const received = add.indexOf("adoptBuyerCart(active, active.id)");
  const stored = add.indexOf(
    "localStorage.setItem(current.storageKey, active.id)",
    received,
  );
  const fenced = add.indexOf("if (!scopeCurrent()) {", stored);
  const nextBranch = add.indexOf("if (!cartMatchesPricingExpectation(active, expectation))", fenced);
  const addRequest = add.indexOf("await addStorefrontCartLineItem(", stored);
  assert.ok(received >= 0 && stored > received && fenced > stored && nextBranch > fenced && addRequest > nextBranch);
  assert.ok(add.slice(fenced, nextBranch).includes("return false;"));
  assert.ok(source.includes("liveAuthCheck.current()"));
  const restore = source.slice(source.indexOf("async function restore()"), source.indexOf("void restore();"));
  const markerCheck = restore.indexOf("if (marker !== null)");
  const read = restore.indexOf("fetchStorefrontCart(");
  assert.ok(markerCheck >= 0 && read > markerCheck);
  assert.ok(restore.slice(markerCheck, read).includes("uncertain = true;"));
});
