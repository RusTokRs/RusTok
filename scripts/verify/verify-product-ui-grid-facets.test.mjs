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
import { fileURLToPath, pathToFileURL } from "node:url";
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
      `import * as product from ${JSON.stringify(pathToFileURL(productFacetsPath).href)};`,
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
      ["--experimental-strip-types", "--import", pathToFileURL(register).href, harness],
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
  checks.counts = [
    grid.countLabel("({count})", 7),
    grid.countLabel("{count} total", 3),
    grid.countLabel("{count} / {count}", 4)
  ];
  checks.duplicateToggle = grid.selectionAfterToggle(
    ["color=red", "COLOR=red", "broken", "color="],
    "color",
    "red"
  );
  checks.unicodeKeys = grid.isSelectionSelected(["Ä=red"], "ä", "red");
  checks.codePointIds = grid.rowSelectionIds(["😀", "", "a"]);
  checks.rowSelection = {
    created: grid.rowSelectionIds(grid.rowSelectionFromIds(new Set(["b", "a", "b"]))),
    added: grid.rowSelectionIds(grid.addRowsToSelection(
      new Set(["outside", "a"]),
      new Set(["a", "b"])
    )),
    removed: grid.rowSelectionIds(grid.deselectAllRows(
      new Set(["outside", "a", "b"]),
      new Set(["a"])
    )),
    all: grid.areAllRowsSelected(new Set(["a", "b"]), new Set(["a", "b"])),
    emptyAll: grid.areAllRowsSelected(new Set(), []),
    partial: grid.areSomeRowsSelected(new Set(["a"]), ["a", "b"])
  };

  const filterMap = grid.normalizeFilters({
    title: { kind: "text", value: "  hello  " },
    empty: { kind: "select", value: "  " },
    price: { kind: "number_range", min: 20, max: 1 },
    invalidNumbers: { kind: "number_range", min: NaN, max: Infinity },
    activeFalse: { kind: "boolean", value: false }
  });
  checks.filters = {
    keys: grid.columnFilterKeys(filterMap),
    count: grid.columnFilterCount(filterMap),
    title: grid.getColumnFilter(filterMap, "title"),
    price: grid.getColumnFilter(filterMap, "price"),
    falseActive: grid.isFilterValueActive(grid.getColumnFilter(filterMap, "activeFalse")),
    hasEmpty: grid.hasColumnFilterValue(filterMap, "empty"),
    prototypeKeyMissing: grid.getColumnFilter(filterMap, "toString") === undefined,
    entries: grid.columnFilterEntries(filterMap)
  };
  checks.filterNoop = grid.setColumnFilter(filterMap, "title", {
    kind: "text",
    value: "hello"
  }).changed;
  checks.filterCleared = grid.setColumnFilter(filterMap, "title", { kind: "empty" });

  const column = grid.createGridColumnDef("status", "Status");
  const configuredColumn = grid.pinColumn(
    grid.withColumnFilter(
      grid.withColumnWidthRange(column, 120, 80, 240),
      grid.selectFilter([])
    ),
    "left"
  );
  checks.column = {
    filter: grid.hasFilter(configuredColumn),
    filterRow: grid.hasFilterRow([configuredColumn, grid.hideColumn(grid.createGridColumnDef("hidden", "Hidden"))]),
    width: configuredColumn.width,
    pinned: configuredColumn.pinned,
    checkbox: grid.isCheckboxColumn(grid.createCheckboxColumn()),
    actions: grid.createActionsColumn("actions", "Actions").width.current,
    notFilterable: grid.hasFilter(grid.withColumnNotFilterable(configuredColumn))
  };

  checks.sort = {
    first: grid.cycleSort(null, "title"),
    second: grid.cycleSort({ columnId: "title", direction: "asc" }, "title"),
    third: grid.cycleSort({ columnId: "title", direction: "desc" }, "title"),
    direction: grid.sortDirectionForColumn({ columnId: "title", direction: "asc" }, "title"),
    aria: grid.ariaSortForColumn({ columnId: "title", direction: "desc" }, "title"),
    halfActive: grid.isSortActive({ columnId: "title", direction: null }),
    blankActive: grid.isSortActive({ columnId: "  ", direction: "asc" }),
    repaired: grid.normalizeSortState({ columnId: "title", direction: null }),
    repairedCycle: grid.cycleSort({ columnId: "title", direction: null }, "title")
  };

  const page = grid.createPagination(99, 0, 21);
  const infinite = grid.setPaginationMode(grid.createInfinitePagination(10, 55), "infinite");
  const repairedPagedMode = grid.setPaginationMode({
    page: 0,
    pageSize: 0,
    total: 21,
    hasNext: false,
    mode: "paged"
  }, "paged");
  checks.pagination = {
    page,
    pages: grid.totalPages(page),
    countEmpty: grid.calculatePageCount(0, 0),
    offset: grid.calculateOffset(3, 10),
    previous: grid.hasPreviousPage(page.page),
    next: grid.hasNextPage(page.page, page.total, page.pageSize),
    infinite: grid.goToPage(infinite, 3),
    infiniteBounds: grid.sliceBounds(grid.goToPage(infinite, 3), 55),
    modeRepair: repairedPagedMode,
    range: grid.dataTableRangeLabel(3, 20, 55)
  };
  checks.largeTotalThrows = (() => {
    try {
      grid.calculatePageCount(Number.MAX_SAFE_INTEGER + 1, 1);
      return false;
    } catch (error) {
      return error instanceof RangeError;
    }
  })();

  const stored = grid.setColumnWidth({}, "title", 400);
  const clamped = grid.setClampedColumnWidth(stored.widths, "title", 500, 100, 300);
  checks.resize = {
    stored: grid.getColumnWidth(stored.widths, "title", 150),
    clamped: grid.getClampedColumnWidth(clamped.widths, "title", 150, 100, 300),
    changedAgain: grid.setClampedColumnWidth(clamped.widths, "title", 400, 100, 300).changed,
    resized: grid.calculateResizedWidth(100, 2.5, 50, 200),
    resizedNegative: grid.calculateResizedWidth(100, -2.5, 50, 200),
    invalidDelta: grid.calculateResizedWidth(100, NaN, 200, 50),
    keyboard: grid.stepResizedWidth(60, -16, 50, 400),
    entries: grid.columnWidthEntries(clamped.widths)
  };

  const gridState = grid.withGridPagination(grid.createGridState(), 3, 10, 100);
  const filteredState = grid.setGridFilter(gridState, "title", {
    kind: "text",
    value: " hello "
  });
  const sortedState = grid.toggleGridSort(filteredState.state, "title");
  checks.state = {
    initialPage: gridState.pagination.page,
    filterChanged: filteredState.changed,
    filterPage: filteredState.state.pagination.page,
    filterValue: grid.getColumnFilter(filteredState.state.filters, "title"),
    sortDirection: sortedState.direction,
    sortPage: sortedState.state.pagination.page,
    pruned: [...grid.pruneGridSelection(
      { ...sortedState.state, selection: new Set(["a", "b"]) },
      ["b", "c"]
    ).selection],
    normalized: grid.normalizeGridState({
      pagination: { page: 99, pageSize: 10, total: 22, hasNext: true, mode: "paged" },
      sort: { columnId: null, direction: "desc" },
      filters: { blank: { kind: "text", value: " " } },
      columnWidths: { status: 500 }
    }, [configuredColumn])
  };

  const open = grid.openFacet({ code: "weight", label: "Weight", total: 3 });
  let openBucketReads = 0;
  const forgedOpen = {
    ...open,
    isTruncated: true,
    values: new Proxy(
      Array.from({ length: 100 }, (_, index) => ({
        value: "open_" + index,
        label: "Open",
        count: 1
      })),
      {
        get(target, property, receiver) {
          if (typeof property === "string" && Number.isInteger(Number(property))) {
            openBucketReads += 1;
          }
          return Reflect.get(target, property, receiver);
        }
      }
    )
  };
  const repairedOpen = grid.buildFacetPanel(
    [forgedOpen],
    [],
    grid.ENGLISH_FACET_PANEL_LABELS
  ).facets[0];
  const oversizedFacet = {
    ...grid.facetFromBuckets({
      code: "unsafe",
      label: "Unsafe",
      domain: grid.dictionaryDomain(false),
      total: 25,
      values: []
    }),
    isEnumerable: false,
    values: Array.from({ length: grid.MAX_GRID_FACET_VALUES + 2 }, (_, index) => ({
      value: "value_" + index,
      label: "Value",
      count: 1
    }))
  };
  const repairedOversized = grid.normalizeFacetSource(oversizedFacet);
  checks.repairedFacets = {
    openBucketReads,
    open: {
      enumerable: repairedOpen.isEnumerable,
      truncated: repairedOpen.isTruncated,
      values: repairedOpen.values.length
    },
    oversized: {
      enumerable: repairedOversized.isEnumerable,
      truncated: repairedOversized.isTruncated,
      values: repairedOversized.values.length
    }
  };
  checks.invalidFacetCountThrows = (() => {
    try {
      grid.normalizeFacetSource({ ...open, total: Number.MAX_SAFE_INTEGER + 1 });
      return false;
    } catch (error) {
      return error instanceof RangeError;
    }
  })();
  checks.openFacet = {
    enumerable: open.isEnumerable,
    values: open.values.length,
    truncated: open.isTruncated
  };

  const many = Array.from({ length: grid.MAX_GRID_FACET_VALUES + 100 }, (_, index) => ({
    value: "option_" + index,
    label: "Option",
    count: 1
  }));
  let maxBucketRead = -1;
  const observedBuckets = new Proxy(many, {
    get(target, property, receiver) {
      if (typeof property === "string" && /^\\d+$/.test(property)) {
        maxBucketRead = Math.max(maxBucketRead, Number(property));
      }
      return Reflect.get(target, property, receiver);
    }
  });
  const cut = grid.facetFromBuckets({
    code: "size",
    label: "Size",
    domain: grid.dictionaryDomain(false),
    total: 2,
    values: observedBuckets
  });
  checks.cut = {
    values: cut.values.length,
    truncated: cut.isTruncated,
    enumerable: cut.isEnumerable,
    maxBucketRead
  };
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
  checks.appliedDuplicate = grid.applyQueryPairs(
    "/products?tag=red&search=bag&tag=blue",
    [["tag", "green"]]
  );
  checks.appliedDuplicateRemoved = grid.applyQueryPairs(
    "/products?tag=red&search=bag&tag=blue",
    [["tag", null]]
  );
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
  assert.deepEqual(behaviour.duplicateToggle, []);
  assert.equal(behaviour.unicodeKeys, false);
  assert.deepEqual(behaviour.codePointIds, ["a", "", "😀"]);
});

test("row selection helpers preserve off-page rows and accept iterables", () => {
  assert.deepEqual(behaviour.rowSelection, {
    created: ["a", "b"],
    added: ["a", "b", "outside"],
    removed: ["b", "outside"],
    all: true,
    emptyAll: false,
    partial: true,
  });
});

test("counts replace every template token", () => {
  assert.deepEqual(behaviour.counts, ["(7)", "3 total", "4 / 4"]);
});

test("column builders and filter-row helpers mirror the Rust descriptor", () => {
  assert.deepEqual(behaviour.column, {
    filter: true,
    filterRow: true,
    width: { min: 80, max: 240, current: 120 },
    pinned: "left",
    checkbox: true,
    actions: 80,
    notFilterable: false,
  });
});

test("filters normalize, compare and update immutably", () => {
  assert.deepEqual(behaviour.filters, {
    keys: ["activeFalse", "price", "title"],
    count: 3,
    title: { kind: "text", value: "hello" },
    price: { kind: "number_range", min: 1, max: 20 },
    falseActive: true,
    hasEmpty: false,
    prototypeKeyMissing: true,
    entries: [
      ["activeFalse", { kind: "boolean", value: false }],
      ["price", { kind: "number_range", min: 1, max: 20 }],
      ["title", { kind: "text", value: "hello" }],
    ],
  });
  assert.equal(behaviour.filterNoop, false);
  assert.equal(behaviour.filterCleared.changed, true);
  assert.equal(behaviour.filterCleared.filters.title, undefined);
});

test("sort transitions cycle through ascending, descending and cleared", () => {
  assert.deepEqual(behaviour.sort, {
    first: { columnId: "title", direction: "asc" },
    second: { columnId: "title", direction: "desc" },
    // Clearing returns the empty state, never null: a transition answers with a state.
    third: { columnId: null, direction: null },
    direction: "asc",
    aria: "descending",
    halfActive: true,
    blankActive: false,
    repaired: { columnId: "title", direction: "asc" },
    repairedCycle: { columnId: "title", direction: "desc" },
  });
});

test("pagination repairs zero sizes and keeps safe integer boundaries", () => {
  assert.deepEqual(behaviour.pagination.page, {
    page: 21,
    pageSize: 1,
    total: 21,
    hasNext: false,
    mode: "paged",
  });
  assert.equal(behaviour.pagination.pages, 21);
  assert.equal(behaviour.pagination.countEmpty, 1);
  assert.equal(behaviour.pagination.offset, 20);
  assert.equal(behaviour.pagination.previous, true);
  assert.equal(behaviour.pagination.next, false);
  assert.deepEqual(behaviour.pagination.infinite, {
    page: 3,
    pageSize: 10,
    total: 55,
    hasNext: true,
    mode: "infinite",
  });
  assert.deepEqual(behaviour.pagination.infiniteBounds, [0, 30]);
  assert.deepEqual(behaviour.pagination.modeRepair, {
    page: 1,
    pageSize: 1,
    total: 21,
    hasNext: true,
    mode: "paged",
  });
  assert.equal(behaviour.pagination.range, "41–55 of 55");
  assert.equal(behaviour.largeTotalThrows, true);
});

test("resize math and persisted widths obey Rust bounds", () => {
  assert.deepEqual(behaviour.resize, {
    stored: 400,
    clamped: 300,
    changedAgain: false,
    resized: 103,
    resizedNegative: 98,
    invalidDelta: 100,
    keyboard: 50,
    entries: [["title", 300]],
  });
});

test("composite state transitions reset pages and normalize stored state", () => {
  assert.equal(behaviour.state.initialPage, 3);
  assert.equal(behaviour.state.filterChanged, true);
  assert.equal(behaviour.state.filterPage, 1);
  assert.deepEqual(behaviour.state.filterValue, { kind: "text", value: "hello" });
  assert.equal(behaviour.state.sortDirection, "asc");
  assert.equal(behaviour.state.sortPage, 1);
  assert.deepEqual(behaviour.state.pruned, ["b"]);
  assert.deepEqual(behaviour.state.normalized.pagination, {
    page: 3,
    pageSize: 10,
    total: 22,
    hasNext: false,
    mode: "paged",
  });
  assert.deepEqual(behaviour.state.normalized.sort, { columnId: null, direction: null });
  assert.deepEqual(behaviour.state.normalized.filters, {});
  assert.equal(behaviour.state.normalized.columnWidths.status, 240);
});

test("buckets are cut at the value limit and report truncation", () => {
  assert.deepEqual(behaviour.cut, {
    values: 20,
    truncated: true,
    enumerable: true,
    maxBucketRead: 20,
  });
  assert.deepEqual(behaviour.options, [{ value: "option_0", label: "Option" }]);
  assert.deepEqual(behaviour.openFacet, { enumerable: false, values: 0, truncated: false });
  assert.deepEqual(behaviour.repairedFacets, {
    openBucketReads: 0,
    open: { enumerable: false, truncated: false, values: 0 },
    oversized: { enumerable: true, truncated: true, values: 20 },
  });
  assert.equal(behaviour.invalidFacetCountThrows, true);
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
  assert.equal(behaviour.appliedDuplicate, "/products?tag=green&search=bag");
  assert.equal(behaviour.appliedDuplicateRemoved, "/products?search=bag");
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
