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

import {
  parseCsv,
  fetchStorefrontSearch,
  fetchStorefrontFilterPresets,
  fetchStorefrontSuggestions,
  STOREFRONT_SEARCH_QUERY,
  STOREFRONT_FILTER_PRESETS_QUERY,
  STOREFRONT_SUGGESTIONS_QUERY,
} from "./api.ts";

test("parseCsv splits, trims, and filters empty segments", () => {
  assert.deepEqual(parseCsv(""), []);
  assert.deepEqual(parseCsv("   "), []);
  assert.deepEqual(parseCsv("cat1, cat2, cat3"), ["cat1", "cat2", "cat3"]);
  assert.deepEqual(parseCsv("shoes, , boots,   ,"), ["shoes", "boots"]);
  assert.deepEqual(parseCsv("single"), ["single"]);
});

test("fetchStorefrontSearch constructs correct GraphQL variables and returns payload", async () => {
  let capturedQuery = "";
  let capturedVariables = null;

  const mockGraphql = async ({ query, variables }) => {
    capturedQuery = query;
    capturedVariables = variables;
    return {
      data: {
        storefrontSearch: {
          queryLogId: "log-1",
          presetKey: "products",
          total: 1,
          tookMs: 12,
          engine: "tantivy",
          rankingProfile: "bm25",
          items: [
            {
              id: "item-1",
              entityType: "product",
              sourceModule: "rustok-product",
              title: "Winter Boots",
              snippet: "Warm <em>winter</em> boots",
              score: 0.95,
              locale: "en",
              url: "/products/winter-boots",
              payload: "{}",
            },
          ],
          facets: [
            {
              name: "category",
              buckets: [{ value: "footwear", label: "Footwear", count: 1 }],
            },
          ],
        },
      },
    };
  };

  const payload = await fetchStorefrontSearch(
    "winter",
    "products",
    {
      channelId: "default",
      categoryIds: "footwear, winter-gear",
      attributeCode: "material",
      attributeValues: "leather, gore-tex",
      attributeMin: "10",
      attributeMax: "100",
      sortAttributeCode: "price",
      sortDesc: true,
    },
    {
      graphql: mockGraphql,
      locale: "en",
      token: "test-token",
      tenantSlug: "test-tenant",
    },
  );

  assert.equal(capturedQuery, STOREFRONT_SEARCH_QUERY);
  assert.equal(capturedVariables.input.query, "winter");
  assert.equal(capturedVariables.input.locale, "en");
  assert.equal(capturedVariables.input.presetKey, "products");
  assert.equal(capturedVariables.input.channelId, "default");
  assert.deepEqual(capturedVariables.input.categoryIds, ["footwear", "winter-gear"]);
  assert.deepEqual(capturedVariables.input.attributeFilters, [
    {
      attributeCode: "material",
      values: ["leather", "gore-tex"],
      min: "10",
      max: "100",
    },
  ]);
  assert.equal(capturedVariables.input.sortAttributeCode, "price");
  assert.equal(capturedVariables.input.sortDesc, true);

  assert.equal(payload.total, 1);
  assert.equal(payload.items[0].title, "Winter Boots");
  assert.equal(payload.facets[0].name, "category");
});

test("fetchStorefrontSearch propagates GraphQL errors (anti-swallowing)", async () => {
  const mockFailingGraphql = async () => ({
    errors: [{ message: "Search index unreachable" }],
  });

  await assert.rejects(
    async () => {
      await fetchStorefrontSearch(
        "query",
        null,
        {
          channelId: "",
          categoryIds: "",
          attributeCode: "",
          attributeValues: "",
          attributeMin: "",
          attributeMax: "",
          sortAttributeCode: "",
          sortDesc: false,
        },
        {
          graphql: mockFailingGraphql,
        },
      );
    },
    {
      name: "Error",
      message: "Search index unreachable",
    },
  );
});

test("fetchStorefrontFilterPresets returns presets and propagates errors", async () => {
  const mockGraphql = async ({ query }) => {
    assert.equal(query, STOREFRONT_FILTER_PRESETS_QUERY);
    return {
      data: {
        storefrontSearchFilterPresets: [
          {
            key: "catalog",
            label: "Catalog Only",
            entityTypes: ["product"],
            sourceModules: ["rustok-product"],
            statuses: ["published"],
            rankingProfile: null,
          },
        ],
      },
    };
  };

  const presets = await fetchStorefrontFilterPresets({ graphql: mockGraphql });
  assert.equal(presets.length, 1);
  assert.equal(presets[0].key, "catalog");

  // Error propagation
  const mockFailingGraphql = async () => ({
    errors: [{ message: "Failed to load presets" }],
  });

  await assert.rejects(
    async () => {
      await fetchStorefrontFilterPresets({ graphql: mockFailingGraphql });
    },
    {
      name: "Error",
      message: "Failed to load presets",
    },
  );
});

test("fetchStorefrontSuggestions returns suggestions and propagates errors", async () => {
  let capturedVariables = null;
  const mockGraphql = async ({ query, variables }) => {
    assert.equal(query, STOREFRONT_SUGGESTIONS_QUERY);
    capturedVariables = variables;
    return {
      data: {
        storefrontSearchSuggestions: [
          {
            text: "running shoes",
            kind: "query",
            locale: "en",
            url: null,
            score: 0.88,
          },
        ],
      },
    };
  };

  const suggestions = await fetchStorefrontSuggestions("run", { graphql: mockGraphql });
  assert.equal(capturedVariables.input.query, "run");
  assert.equal(capturedVariables.input.limit, 6);
  assert.equal(suggestions.length, 1);
  assert.equal(suggestions[0].text, "running shoes");

  // Error propagation
  const mockFailingGraphql = async () => ({
    errors: [{ message: "Autocomplete timeout" }],
  });

  await assert.rejects(
    async () => {
      await fetchStorefrontSuggestions("run", { graphql: mockFailingGraphql });
    },
    {
      name: "Error",
      message: "Autocomplete timeout",
    },
  );
});
