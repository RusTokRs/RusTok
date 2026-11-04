/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from "node:test";
import assert from "node:assert/strict";
import { fetchStorefrontPricing, STOREFRONT_PRODUCTS_QUERY, STOREFRONT_PRODUCT_QUERY } from "./api.ts";

test("fetchStorefrontPricing aggregates product list, channels and price lists", async () => {
  const mockGraphql = async ({ query }) => {
    if (query === STOREFRONT_PRODUCTS_QUERY) {
      return {
        data: {
          storefrontPricingCatalog: {
            items: [
              {
                id: "prod-1",
                title: "Premium Widget",
                handle: "premium-widget",
                productType: "Physical",
                createdAt: "2026-01-01T00:00:00Z",
                currencies: ["EUR"]
              }
            ],
            total: 1,
            page: 1,
            perPage: 20,
            hasNext: false
          },
          storefrontPricingChannels: [
            {
              id: "chan-1",
              slug: "default",
              name: "Default Channel",
              isActive: true,
              isDefault: true,
              status: "active"
            }
          ],
          storefrontActivePriceLists: [
            {
              id: "pl-1",
              name: "Retail",
              listType: "standard",
              channelId: "chan-1"
            }
          ]
        }
      };
    }
    if (query === STOREFRONT_PRODUCT_QUERY) {
      return {
        data: {
          storefrontPricingProduct: {
            id: "prod-1",
            status: "active",
            translations: [{ locale: "en", title: "Premium Widget", handle: "premium-widget" }],
            variants: [
              {
                id: "var-1",
                title: "Standard",
                sku: "WID-STD",
                prices: [{ currencyCode: "EUR", amount: "50.00", onSale: false }],
                effectivePrice: {
                  source: { priceId: "p-2", kind: "base", ruleKind: null, adjustmentPercent: null },
                  currencyCode: "EUR",
                  amount: "50.00",
                  compareAtAmount: null,
                  discountPercent: null,
                  onSale: false
                }
              }
            ],
            resolutionContext: {
              currencyCode: "EUR",
              quantity: 2,
              regionId: null,
              priceListId: null,
              channelId: null,
              channelSlug: null
            },
            resolvedAt: "2026-01-01T00:00:00Z"
          }
        }
      };
    }
    return { data: {} };
  };

  const result = await fetchStorefrontPricing(mockGraphql, {
    currencyCode: "EUR",
    quantity: 2
  });

  assert.equal(result.products.total, 1);
  assert.equal(result.products.items[0].title, "Premium Widget");
  assert.deepEqual(result.products.items[0].currencies, ["EUR"]);
  assert.equal(result.availableChannels.length, 1);
  assert.equal(result.availableChannels[0].slug, "default");
  assert.equal(result.activePriceLists.length, 1);
  assert.equal(result.resolutionContext.currencyCode, "EUR");
  assert.equal(result.resolutionContext.quantity, 2);
  assert.ok(result.selectedProduct);
  assert.equal(result.selectedProduct.id, "prod-1");
});

test("fetchStorefrontPricing resolves selected product when selectedHandle is provided", async () => {
  const mockGraphql = async ({ query, variables }) => {
    if (query === STOREFRONT_PRODUCTS_QUERY) {
      return {
        data: {
          storefrontPricingCatalog: { items: [], total: 0, page: 1, perPage: 20, hasNext: false },
          storefrontPricingChannels: [],
          storefrontActivePriceLists: []
        }
      };
    }
    if (query === STOREFRONT_PRODUCT_QUERY) {
      assert.equal(variables.handle, "gadget-x");
      assert.equal(variables.currencyCode, "USD");
      return {
        data: {
          storefrontPricingProduct: {
            id: "prod-gadget",
            status: "active",
            translations: [{ locale: "en", title: "Gadget X", handle: "gadget-x" }],
            variants: [
              {
                id: "var-1",
                title: "Standard",
                sku: "GAD-STD",
                prices: [{ currencyCode: "USD", amount: "99.00", onSale: false }],
                effectivePrice: {
                  source: { priceId: "p-1", kind: "base", ruleKind: null, adjustmentPercent: null },
                  currencyCode: "USD",
                  amount: "79.00",
                  compareAtAmount: "99.00",
                  discountPercent: "20",
                  onSale: true
                }
              }
            ],
            resolutionContext: {
              currencyCode: "USD",
              quantity: 1,
              regionId: null,
              priceListId: null,
              channelId: null,
              channelSlug: null
            },
            resolvedAt: "2026-01-01T00:00:00Z"
          }
        }
      };
    }
    return { data: {} };
  };

  const result = await fetchStorefrontPricing(mockGraphql, {
    selectedHandle: "gadget-x",
    currencyCode: "USD"
  });

  assert.equal(result.selectedHandle, "gadget-x");
  assert.ok(result.selectedProduct);
  assert.equal(result.selectedProduct.id, "prod-gadget");
  assert.equal(result.selectedProduct.variants[0].effectivePrice?.amount, "79.00");
  assert.equal(result.selectedProduct.variants[0].effectivePrice?.onSale, true);
});

test("fetchStorefrontPricing throws when GraphQL query reports execution error (anti-swallowing)", async () => {
  const mockFailingGraphql = async ({ query }) => {
    if (query === STOREFRONT_PRODUCTS_QUERY) {
      return {
        errors: [{ message: "Database connection failed" }]
      };
    }
    return { data: {} };
  };

  await assert.rejects(
    async () => {
      await fetchStorefrontPricing(mockFailingGraphql, {});
    },
    {
      name: "Error",
      message: "Pricing request could not be completed"
    }
  );
});
