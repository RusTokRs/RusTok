#!/usr/bin/env node
/**
 * Behavioural tests for the Next storefront facet panel.
 *
 * The panel logic lives in a framework-free TypeScript module, so the tests execute it for real
 * through Node's type stripping instead of grepping it: the URL contract (`;`-joined
 * `attribute_filters`, stable key positions, form encoding), the drill-down selection rules, the
 * panel view model and the localized copy are all asserted as behaviour.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const facetsModulePath = path.join(
  repoRoot,
  "apps/next-frontend/packages/rustok-product/src/catalog/facets.ts",
);
const verifierPath = path.join(scriptDir, "verify-product-storefront-next-facets.mjs");

/** Runs `body` in a child process that can import the TypeScript module, and returns its report. */
function runInModuleScope(body) {
  const directory = mkdtempSync(path.join(tmpdir(), "rustok-next-facets-"));
  const harness = path.join(directory, "harness.mjs");
  writeFileSync(
    harness,
    [
      `import * as module from ${JSON.stringify(facetsModulePath)};`,
      "const api = module;",
      "const checks = {};",
      body,
      "process.stdout.write(JSON.stringify(checks));",
      "",
    ].join("\n"),
  );
  try {
    const result = spawnSync(
      process.execPath,
      ["--experimental-strip-types", harness],
      { encoding: "utf8" },
    );
    assert.equal(
      result.status,
      0,
      `module harness failed: ${result.stderr || result.stdout}`,
    );
    return JSON.parse(result.stdout);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function ftlValues(localeFile, prefix) {
  const source = readFileSync(
    path.join(
      repoRoot,
      "crates/modules/rustok-product/storefront/locales",
      localeFile,
    ),
    "utf8",
  );
  const values = {};
  for (const line of source.split("\n")) {
    const separator = line.indexOf(" = ");
    if (separator < 0) continue;
    const key = line.slice(0, separator);
    if (!key.startsWith(prefix)) continue;
    values[key] = line.slice(separator + 3);
  }
  return values;
}

const behaviour = runInModuleScope(`
  const filterSelection = api.parseAttributeFilters(" color=red ; size=m ;weight=12.5");
  checks.parsedFilters = filterSelection;
  checks.serialized = api.serializeAttributeFilters(filterSelection);
  checks.parsedBlank = api.parseAttributeFilters("   ");

  const selected = ["color=red", "size=m"];
  checks.unknownEntryIgnored = api.parseAttributeFilter("noise");
  checks.blankValueIgnored = api.parseAttributeFilter("color=");
  checks.codeRead = api.attributeFilterCode("size=m");
  checks.isSelected = [
    api.isAttributeFilterSelected(selected, "color", "red"),
    api.isAttributeFilterSelected(selected, "color", "blue"),
  ];
  checks.hasCode = [
    api.hasAttributeFilterForCode(selected, "size"),
    api.hasAttributeFilterForCode(selected, "weight"),
  ];
  checks.toggledAppend = api.toggleAttributeFilter(selected, "color", "blue");
  checks.toggledRemove = api.toggleAttributeFilter(selected, "color", "red");
  checks.toggledRoundTrip = api.toggleAttributeFilter(
    api.toggleAttributeFilter(selected, "color", "blue"),
    "color",
    "blue",
  );
  checks.toggledBlankCode = api.toggleAttributeFilter(selected, "  ", "red");
  checks.clearCode = api.clearAttributeFilterCode(selected, "color");
  checks.countLabel = api.countLabel("({count})", 42);

  checks.appliedPairs = api.applyQueryPairs(
    "/products?search=bag&attribute_filters=color%3Dred",
    [["attribute_filters", "color=red;size=m"]],
  );
  checks.appliedPairsDropped = api.applyQueryPairs("/products?search=bag", [
    ["search", "   "],
  ]);
  checks.appliedPairsAppended = api.applyQueryPairs("/products?search=bag&sort=desc", [
    ["search", "bag"],
    ["page", "2"],
  ]);
  checks.appliedPairsEmpty = api.applyQueryPairs("/products", []);

  const controls = {
    search: "bag",
    categoryId: "11111111-1111-4111-8111-111111111111",
    sortBy: "created_at",
    sortDirection: "asc",
    attributeFilters: ["color=red"],
    currencyCode: "USD",
  };
  checks.toggleHref = api.buildCatalogFacetToggleQuery(
    "/ru/products",
    controls,
    "color",
    "blue",
  );
  checks.clearHref = api.buildCatalogFacetClearQuery("/ru/products", controls);
  checks.clearCodeHref = api.buildCatalogFacetClearCodeQuery(
    "/ru/products",
    controls,
    "color",
  );

  checks.facetCodes = api.buildCatalogFacetCodes({
    categoryOptions: [],
    attributeOptions: [
      { value: " color ", label: "Color" },
      { value: "color", label: "Color duplicate" },
      { value: "   ", label: "Blank" },
      { value: "size", label: "Size" },
    ],
  });
  checks.facetCodesWithoutOptions = api.buildCatalogFacetCodes({});

  const facetInput = [
    {
      code: "color",
      label: "Color",
      valueType: "select",
      isLocalized: false,
      isEnumerable: true,
      isTruncated: true,
      totalProducts: 7,
      values: [
        { value: "red", label: "Red", count: 3 },
        { value: "blue", label: "Blue", count: 4 },
      ],
    },
    {
      code: "weight",
      label: "Weight",
      valueType: "decimal",
      isLocalized: false,
      isEnumerable: false,
      isTruncated: false,
      totalProducts: 5,
      values: [],
    },
  ];
  checks.panel = api.buildCatalogFacetFiltersView(
    "/ru/products",
    facetInput,
    controls,
    api.buildCatalogFacetLabels("ru"),
  );
  checks.emptyPanel = api.buildCatalogFacetFiltersView(
    "/en/products",
    [],
    { attributeFilters: [] },
    api.buildCatalogFacetLabels("en"),
  );
  checks.labelsRu = api.buildCatalogFacetLabels("ru");
  checks.labelsEn = api.buildCatalogFacetLabels("en-US");
`);

test("attribute-filter vocabulary matches the rust storefront contract", () => {
  assert.deepEqual(behaviour.parsedFilters, [
    "color=red",
    "size=m",
    "weight=12.5",
  ]);
  assert.equal(behaviour.serialized, "color=red;size=m;weight=12.5");
  assert.deepEqual(behaviour.parsedBlank, []);
  assert.equal(behaviour.unknownEntryIgnored, null);
  assert.equal(behaviour.blankValueIgnored, null);
  assert.equal(behaviour.codeRead, "size");
  assert.deepEqual(behaviour.isSelected, [true, false]);
  assert.deepEqual(behaviour.hasCode, [true, false]);
  assert.equal(behaviour.countLabel, "(42)");
});

test("toggling keeps the selection order and round-trips", () => {
  assert.deepEqual(behaviour.toggledAppend, [
    "color=red",
    "size=m",
    "color=blue",
  ]);
  assert.deepEqual(behaviour.toggledRemove, ["size=m"]);
  assert.deepEqual(behaviour.toggledRoundTrip, ["color=red", "size=m"]);
  assert.deepEqual(behaviour.toggledBlankCode, ["color=red", "size=m"]);
  assert.deepEqual(behaviour.clearCode, ["size=m"]);
});

test("route pairs keep positions, drop blanks and encode values", () => {
  assert.equal(
    behaviour.appliedPairs,
    "/products?search=bag&attribute_filters=color%3Dred%3Bsize%3Dm",
  );
  assert.equal(behaviour.appliedPairsDropped, "/products");
  assert.equal(behaviour.appliedPairsAppended, "/products?search=bag&sort=desc&page=2");
  assert.equal(behaviour.appliedPairsEmpty, "/products");
});

test("facet links carry the whole catalog state", () => {
  assert.equal(
    behaviour.toggleHref,
    "/ru/products?search=bag&category_id=11111111-1111-4111-8111-111111111111&sort_by=created_at&sort_direction=asc&attribute_filters=color%3Dred%3Bcolor%3Dblue&currency=USD",
  );
  assert.equal(
    behaviour.clearHref,
    "/ru/products?search=bag&category_id=11111111-1111-4111-8111-111111111111&sort_by=created_at&sort_direction=asc&currency=USD",
  );
  assert.equal(
    behaviour.clearCodeHref,
    "/ru/products?search=bag&category_id=11111111-1111-4111-8111-111111111111&sort_by=created_at&sort_direction=asc&currency=USD",
  );
  assert.deepEqual(behaviour.facetCodes, ["color", "size"]);
  assert.deepEqual(behaviour.facetCodesWithoutOptions, []);
});

test("panel view model marks selection, counts, hints and clear actions", () => {
  const panel = behaviour.panel;
  assert.equal(panel.title, "Фильтры");
  assert.equal(panel.showEmptyState, false);
  assert.equal(panel.facets.length, 2);

  const [color, weight] = panel.facets;
  assert.equal(color.code, "color");
  assert.equal(color.isEnumerable, true);
  assert.equal(color.unboundedHint, undefined);
  assert.equal(color.isTruncated, true);
  assert.equal(color.truncatedHint, "Доступны и другие значения, кроме показанных.");
  assert.equal(color.clearHref !== undefined, true);
  assert.equal(color.clearLabel, "Очистить фильтры");

  const [red, blue] = color.values;
  assert.equal(red.selected, true);
  assert.equal(red.marker, "[x]");
  assert.equal(red.countLabel, "(3)");
  assert.equal(blue.selected, false);
  assert.equal(blue.marker, "[ ]");
  assert.equal(blue.countLabel, "(4)");
  assert.equal(
    blue.href,
    "/ru/products?search=bag&category_id=11111111-1111-4111-8111-111111111111&sort_by=created_at&sort_direction=asc&attribute_filters=color%3Dred%3Bcolor%3Dblue&currency=USD",
  );

  assert.equal(weight.isEnumerable, false);
  assert.equal(weight.unboundedHint, "Укажите значение в поле фильтра выше.");
  assert.equal(weight.isTruncated, false);
  assert.equal(weight.truncatedHint, undefined);
  assert.equal(weight.clearHref, undefined);
  assert.deepEqual(weight.values, []);

  assert.equal(panel.clearHref !== undefined, true);
  assert.equal(behaviour.emptyPanel.showEmptyState, true);
  assert.equal(
    behaviour.emptyPanel.emptyMessage,
    "No filters are available for this catalog yet.",
  );
  assert.equal(behaviour.emptyPanel.clearHref, undefined);
});

test("panel copy stays in sync with the storefront locale files", () => {
  const english = ftlValues("en.ftl", "product-list-facets");
  const russian = ftlValues("ru.ftl", "product-list-facets");

  assert.equal(behaviour.labelsEn.title, english["product-list-facetsLabel"]);
  assert.equal(
    behaviour.labelsEn.unboundedHint,
    english["product-list-facetsUnbounded"],
  );
  assert.equal(
    behaviour.labelsEn.truncatedHint,
    english["product-list-facetsTruncated"],
  );
  assert.equal(behaviour.labelsEn.clearLabel, english["product-list-facetsClear"]);
  assert.equal(behaviour.labelsEn.emptyMessage, english["product-list-facetsEmpty"]);
  assert.equal(
    behaviour.labelsEn.countTemplate,
    english["product-list-facetsCount"],
  );
  assert.equal(
    behaviour.labelsEn.selectedMarker,
    english["product-list-facetsSelected"],
  );
  assert.equal(
    behaviour.labelsEn.unselectedMarker,
    english["product-list-facetsUnselected"],
  );

  assert.equal(behaviour.labelsRu.title, russian["product-list-facetsLabel"]);
  assert.equal(
    behaviour.labelsRu.unboundedHint,
    russian["product-list-facetsUnbounded"],
  );
  assert.equal(
    behaviour.labelsRu.truncatedHint,
    russian["product-list-facetsTruncated"],
  );
  assert.equal(behaviour.labelsRu.clearLabel, russian["product-list-facetsClear"]);
  assert.equal(behaviour.labelsRu.emptyMessage, russian["product-list-facetsEmpty"]);
  assert.equal(
    behaviour.labelsRu.countTemplate,
    russian["product-list-facetsCount"],
  );
  assert.equal(
    behaviour.labelsRu.selectedMarker,
    russian["product-list-facetsSelected"],
  );
  assert.equal(
    behaviour.labelsRu.unselectedMarker,
    russian["product-list-facetsUnselected"],
  );
});

test("source gate passes and rejects a dropped facet panel link", () => {
  const result = spawnSync(process.execPath, [verifierPath], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /verification passed/);
});
