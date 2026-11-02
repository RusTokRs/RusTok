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
  fetchStorefrontProducts,
  fetchStorefrontProduct,
  fetchCatalogSearchOptions,
  fetchStorefrontCatalogFacets,
} from "./api/products.ts";
import {
  buildProductSpecificationLabels,
  buildProductSpecifications,
  formatSpecificationValue,
} from "./catalog/specifications.ts";

test("specifications localizes boolean values and formats attributes correctly", () => {
  const enLabels = buildProductSpecificationLabels("en");
  const ruLabels = buildProductSpecificationLabels("ru");

  assert.equal(formatSpecificationValue({ valueType: "boolean" }, "true", enLabels), "Yes");
  assert.equal(formatSpecificationValue({ valueType: "boolean" }, "false", enLabels), "No");
  assert.equal(formatSpecificationValue({ valueType: "boolean" }, "true", ruLabels), "Да");
  assert.equal(formatSpecificationValue({ valueType: "boolean" }, "false", ruLabels), "Нет");
  assert.equal(formatSpecificationValue({ valueType: "text" }, "cotton", enLabels), "cotton");

  const sampleProduct = {
    attributes: [
      {
        code: "organic",
        label: "Organic Material",
        valueType: "boolean",
        isLocalized: false,
        values: [{ text: "true" }],
      },
      {
        code: "color",
        label: "Color",
        valueType: "text",
        isLocalized: true,
        values: [{ text: "Navy Blue" }],
      },
      {
        code: "empty_attr",
        label: "Empty",
        valueType: "text",
        isLocalized: false,
        values: [],
      },
    ],
  };

  const rows = buildProductSpecifications(sampleProduct, enLabels);
  assert.equal(rows.length, 2); // empty_attr dropped
  assert.equal(rows[0].code, "organic");
  assert.equal(rows[0].value, "Yes");
  assert.equal(rows[1].code, "color");
  assert.equal(rows[1].value, "Navy Blue");
});

test("fetchStorefrontProducts propagates GraphQL errors (anti-swallowing)", async () => {
  const mockFailingGraphql = async () => ({
    errors: [{ message: "Database connection failed" }],
  });

  await assert.rejects(
    async () => {
      await fetchStorefrontProducts(mockFailingGraphql, "en");
    },
    {
      name: "Error",
      message: "Database connection failed",
    }
  );
});

test("fetchStorefrontProduct propagates GraphQL errors (anti-swallowing)", async () => {
  const mockFailingGraphql = async () => ({
    errors: [{ message: "Product query error" }],
  });

  await assert.rejects(
    async () => {
      await fetchStorefrontProduct(mockFailingGraphql, "widget-1", "en");
    },
    {
      name: "Error",
      message: "Product query error",
    }
  );
});
