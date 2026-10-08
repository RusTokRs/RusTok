#!/usr/bin/env node
/**
 * Behavioural tests for the shared table facet toolkit.
 *
 * The toolkit is framework-free TypeScript, so these tests execute the real module through Node's
 * type stripping instead of grepping it: the `code=value` vocabulary (case-insensitive keys, exact
 * values, order-preserving toggle), the bucket limits and truncation reporting, the panel view model
 * and the route query editing. The last test runs the product glue and the shared package side by
 * side and asserts they answer identically — that is the point of the extraction.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { writeWorkspacePackageResolver } from "./lib/workspace-package-resolver.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const sharedGridRoot = path.join(repoRoot, "packages/rustok-ui-grid");
const productFacetsPath = path.join(
  repoRoot,
  "apps/next-frontend/packages/rustok-product/src/catalog/facets.ts",
);

/** Runs `body` in a child process that can import the workspace package and the product glue. */
function runInModuleScope(body) {
  const directory = mkdtempSync(path.join(tmpdir(), "rustok-ui-grid-"));
  const harness = path.join(directory, "harness.mjs");
  writeFileSync(
    harness,
    [
      `import * as grid from "@rustok/ui-grid";`,
      `import * as product from ${JSON.stringify(productFacetsPath)};`,
      "const checks = {};",
      body,
      "process.stdout.write(JSON.stringify(checks));",
      "",
    ].join("\n"),
  );
  const register = writeWorkspacePackageResolver(directory, {
    packageName: "@rustok/ui-grid",
    packageRoot: sharedGridRoot,
    id: "ui-grid",
  });
  try {
    const result = spawnSync(
      process.execPath,
      ["--experimental-strip-types", "--import", register, harness],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, `module harness failed: ${result.stderr || result.stdout}`);
    return JSON.parse(result.stdout);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

const behaviour = runInModuleScope(`
  checks.limits = { facets: grid.MAX_GRID_FACETS, values: grid.MAX_GRID_FACET_VALUES };
  checks.entry = grid.selectionEntry("color", "red");
  checks.split = grid.splitSelection(" color = red ");
  checks.splitNoise = grid.splitSelection("noise");
  checks.splitBlank = grid.splitSelection("color=");
  checks.parsed = grid.parseSelection(" color=red ; size=m ;; weight=12.5 ");
  checks.serialized = grid.serializeSelection(["color=red", "size=m"]);
  checks.selected = [
    grid.isSelectionSelected(["SIZE=m"], "size", "m"),
    grid.isSelectionSelected(["SIZE=m"], "size", "M"),
    grid.hasSelectionForKey(["SIZE=m"], "COLOR"),
    grid.hasSelectionForKey(["SIZE=m"], "   ")
  ];
  checks.forKey = grid.selectionForKey(["SIZE=m", "color=red"], "size");
  checks.except = grid.selectionExcept(["SIZE=m", "color=red"], "size");
  checks.toggled = grid.selectionAfterToggle(["color=red", "size=m"], "color", "blue");
  checks.toggledOff = grid.selectionAfterToggle(["color=red", "size=m"], "color", "red");
  checks.toggledBlank = grid.selectionAfterToggle(["color=red"], "  ", "red");
  checks.clearedKey = grid.selectionAfterClearKey(["color=red", "size=m"], "COLOR");
  checks.cleared = grid.selectionAfterClear();
  checks.counts = [grid.countLabel("({count})", 7), grid.countLabel("{count} total", 3)];

  const open = grid.openFacet({ code: "weight", label: "Weight", total: 3 });
  checks.openFacet = {
    enumerable: open.isEnumerable,
    values: open.values.length,
    truncated: open.isTruncated
  };

  const many = Array.from({ length: grid.MAX_GRID_FACET_VALUES + 2 }, (_, index) => ({
    value: "option_" + index,
    label: "Option",
    count: 1
  }));
  const cut = grid.facetFromBuckets({
    code: "size",
    label: "Size",
    domain: grid.dictionaryDomain(false),
    total: 2,
    values: many
  });
  checks.cut = { values: cut.values.length, truncated: cut.isTruncated, enumerable: cut.isEnumerable };
  checks.options = grid.facetFilterOptions(cut).slice(0, 1);

  const color = grid.facetFromBuckets({
    code: "color",
    label: "Color",
    domain: grid.dictionaryDomain(true),
    total: 9,
    values: [
      { value: "red", label: "Red", count: 4 },
      { value: "blue", label: "Blue", count: 5 }
    ]
  });
  const panel = grid.buildFacetPanel(
    [color, open],
    ["color=red"],
    grid.ENGLISH_FACET_PANEL_LABELS
  );
  checks.panel = {
    title: panel.title,
    empty: panel.showEmptyState,
    selection: panel.hasSelection,
    facets: panel.facets.length,
    colorHasSelection: panel.facets[0].hasSelection,
    red: panel.facets[0].values[0],
    blue: panel.facets[0].values[1],
    weightHint: panel.facets[1].unboundedHint,
    weightEnumerable: panel.facets[1].isEnumerable
  };
  checks.panelEmpty = grid.buildFacetPanel([], ["color=red"], grid.ENGLISH_FACET_PANEL_LABELS);
  checks.panelTransitions = [
    grid.toggleFacetSelection(panel, "color", "blue"),
    grid.clearFacetSelection(panel, "color"),
    grid.clearFacetPanelSelection(panel),
    grid.isFacetValueSelected(panel, "color", "red"),
    grid.isFacetValueSelected(panel, "color", "green")
  ];
  checks.panelCap = grid.buildFacetPanel(
    Array.from({ length: grid.MAX_GRID_FACETS + 3 }, (_, index) => ({
      code: "facet_" + index,
      label: "Facet",
      domain: grid.openDomain(),
      isEnumerable: false,
      isTruncated: false,
      total: 1,
      values: []
    })),
    [],
    grid.ENGLISH_FACET_PANEL_LABELS
  ).facets.length;

  checks.applied = grid.applyQueryPairs("/products?search=bag", [
    ["search", "shoe"],
    ["attribute_filters", "color=red;color=blue"],
    ["currency", null]
  ]);
  checks.appliedRemoved = grid.applyQueryPairs("/products?search=bag&currency=USD", [
    ["currency", "  "]
  ]);
  checks.appliedAppended = grid.applyQueryPairs("/products?search=bag", [["sort_by", "created_at"]]);
  checks.appliedEmpty = grid.applyQueryPairs("/products", []);
  checks.param = grid.queryParam("/products?search=bag&currency=USD", "currency");

  // Every generic helper the product glue re-exports must answer exactly like the shared original.
  const selection = ["color=red", "size=m"];
  checks.agree = {
    parse: JSON.stringify(product.parseAttributeFilters(" color=red ; size=m ")) ===
      JSON.stringify(grid.parseSelection(" color=red ; size=m ")),
    serialize: product.serializeAttributeFilters(selection) === grid.serializeSelection(selection),
    toggle:
      JSON.stringify(product.toggleAttributeFilter(selection, "color", "blue")) ===
      JSON.stringify(grid.selectionAfterToggle(selection, "color", "blue")),
    clear:
      JSON.stringify(product.clearAttributeFilterCode(selection, "color")) ===
      JSON.stringify(grid.selectionAfterClearKey(selection, "color")),
    count: product.countLabel("({count})", 12) === grid.countLabel("({count})", 12),
    query:
      product.applyQueryPairs("/products?search=bag", [["search", "shoe"]]) ===
      grid.applyQueryPairs("/products?search=bag", [["search", "shoe"]])
  };
  const selectedByProduct = product.isAttributeFilterSelected(selection, "color", "red");
  const selectedByGrid = grid.isSelectionSelected(selection, "color", "red");
  checks.agree.selected = selectedByProduct === selectedByGrid;
  checks.agree.hasCode =
    product.hasAttributeFilterForCode(selection, "COLOR") ===
    grid.hasSelectionForKey(selection, "COLOR");
`);

test("the shared limits are the Rust limits", () => {
  assert.equal(behaviour.limits.facets, 8);
  assert.equal(behaviour.limits.values, 20);
});

test("selection entries split, serialize and round-trip", () => {
  assert.equal(behaviour.entry, "color=red");
  assert.deepEqual(behaviour.split, { key: "color", value: "red" });
  assert.equal(behaviour.splitNoise, null);
  assert.equal(behaviour.splitBlank, null);
  assert.deepEqual(behaviour.parsed, ["color=red", "size=m", "weight=12.5"]);
  assert.equal(behaviour.serialized, "color=red;size=m");
});

test("keys compare case-insensitively while values compare exactly", () => {
  assert.deepEqual(behaviour.selected, [true, false, false, false]);
  assert.deepEqual(behaviour.forKey, ["SIZE=m"]);
  assert.deepEqual(behaviour.except, ["color=red"]);
});

test("toggling keeps order and clearing keeps other facets", () => {
  assert.deepEqual(behaviour.toggled, ["color=red", "size=m", "color=blue"]);
  assert.deepEqual(behaviour.toggledOff, ["size=m"]);
  assert.deepEqual(behaviour.toggledBlank, ["color=red"]);
  assert.deepEqual(behaviour.clearedKey, ["size=m"]);
  assert.deepEqual(behaviour.cleared, []);
});

test("counts render through the template", () => {
  assert.deepEqual(behaviour.counts, ["(7)", "3 total"]);
});

test("buckets are cut at the value limit and report truncation", () => {
  assert.deepEqual(behaviour.cut, { values: 20, truncated: true, enumerable: true });
  assert.deepEqual(behaviour.options, [{ value: "option_0", label: "Option", count: 1 }]);
  assert.deepEqual(behaviour.openFacet, { enumerable: false, values: 0, truncated: false });
});

test("the panel marks selection, counts, hints and limits", () => {
  assert.equal(behaviour.panel.title, "Filters");
  assert.equal(behaviour.panel.empty, false);
  assert.equal(behaviour.panel.selection, true);
  assert.equal(behaviour.panel.facets, 2);
  assert.equal(behaviour.panel.colorHasSelection, true);
  assert.deepEqual(behaviour.panel.red, {
    value: "red",
    label: "Red",
    count: 4,
    countLabel: "(4)",
    selected: true,
    marker: "[x]",
    selectionEntry: "color=red"
  });
  assert.equal(behaviour.panel.blue.countLabel, "(5)");
  assert.equal(behaviour.panel.blue.marker, "[ ]");
  assert.equal(behaviour.panel.blue.selectionEntry, "color=blue");
  assert.equal(behaviour.panel.weightHint, "Enter a value in the filter field above.");
  assert.equal(behaviour.panel.weightEnumerable, false);

  assert.equal(behaviour.panelEmpty.showEmptyState, true);
  assert.deepEqual(behaviour.panelEmpty.facets, []);
  assert.equal(behaviour.panelEmpty.hasSelection, true);

  assert.deepEqual(behaviour.panelTransitions, [
    ["color=red", "color=blue"],
    [],
    [],
    true,
    false
  ]);
  assert.equal(behaviour.panelCap, 8);
});

test("route pairs replace in place, drop blanks and append unknowns", () => {
  assert.equal(
    behaviour.applied,
    "/products?search=shoe&attribute_filters=color%3Dred%3Bcolor%3Dblue",
  );
  assert.equal(behaviour.appliedRemoved, "/products?search=bag");
  assert.equal(behaviour.appliedAppended, "/products?search=bag&sort_by=created_at");
  assert.equal(behaviour.appliedEmpty, "/products");
  assert.equal(behaviour.param, "USD");
});

test("the product glue answers exactly like the shared toolkit", () => {
  assert.deepEqual(behaviour.agree, {
    parse: true,
    serialize: true,
    toggle: true,
    clear: true,
    count: true,
    query: true,
    selected: true,
    hasCode: true,
  });
});
