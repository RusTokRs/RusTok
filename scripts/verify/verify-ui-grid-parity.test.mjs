#!/usr/bin/env node
/**
 * Behaviour parity tests for `@rustok/ui-grid` — the TypeScript twin of `crates/ui/rustok-grid`.
 *
 * Every case here has the same answer in both languages; the assertions are written against the
 * TypeScript surface and the Rust crate carries the mirror-case names in its own `#[cfg(test)]`
 * modules. The symbol-level twin check is `verify-ui-grid-parity.mjs`; this file checks that the
 * twins also *behave* the same, including the boundaries where the core repairs instead of throwing.
 */

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { writeWorkspacePackageResolver } from "./lib/workspace-package-resolver.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const sharedGridRoot = path.join(repoRoot, "packages/rustok-ui-grid");

function runTestInGridModule(code) {
  const directory = mkdtempSync(path.join(tmpdir(), "rustok-ui-grid-parity-"));
  const harness = path.join(directory, "harness.mjs");
  writeFileSync(harness, [`import * as grid from "@rustok/ui-grid";`, code, ""].join("\n"));
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
    assert.equal(result.status, 0, `test failed: ${result.stderr || result.stdout}`);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("column descriptors carry width bounds, visibility and filter capability", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    const width = grid.createResizableColumnWidth(200, 100, 400);
    assert.deepEqual(grid.normalizeColumnWidth(width), { min: 100, max: 400, current: 200 });
    assert.equal(grid.clampColumnWidth(width, 50), 100);
    assert.equal(grid.clampColumnWidth(width, 500), 400);
    assert.equal(grid.isColumnWidthFlexible(grid.createFixedColumnWidth(180)), false);
    assert.equal(grid.isColumnWidthFlexible(width), true);
    assert.throws(() => grid.assertColumnWidthPx(12.5), RangeError);
    assert.throws(() => grid.assertColumnWidthPx(-1), RangeError);

    const checkbox = grid.createCheckboxColumn();
    assert.equal(checkbox.id, grid.CHECKBOX_COLUMN_ID);
    assert.equal(grid.isCheckboxColumn(checkbox), true);

    const name = grid.withColumnFilter(grid.createGridColumnDef("name", "Name"), grid.textFilter());
    const hidden = grid.hideColumn(grid.createGridColumnDef("hidden", "Hidden"));
    const cols = [checkbox, name, hidden];
    assert.equal(grid.visibleColumnCount(cols), 2);
    assert.equal(grid.hasFilter(name), true);
    assert.equal(grid.hasFilterRow(cols), true);
    assert.equal(grid.hasFilterRow([checkbox, hidden]), false);
    assert.equal(grid.pinColumn(name, "left").pinned, "left");
    assert.equal(grid.unpinColumn(grid.pinColumn(name, "right")).pinned, null);
    assert.equal(grid.withColumnNotFilterable(name).filterable, false);
  `);
});

test("pagination repairs state, derives the window and refuses unsound totals", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(grid.calculatePageCount(0, 20), 1);
    assert.equal(grid.calculatePageCount(55, 20), 3);
    assert.equal(grid.calculatePageCount(55, 0), 55);
    assert.equal(grid.clampPage(5, 55, 20), 3);
    assert.equal(grid.clampPage(0, 55, 20), 1);
    assert.throws(() => grid.calculatePageCount(Number.MAX_SAFE_INTEGER + 1, 1), RangeError);

    assert.equal(grid.fromIndex(1, 20, 55), 1);
    assert.equal(grid.toIndex(3, 20, 55), 55);
    assert.equal(grid.visibleLength(1, 20, 55), 20);
    assert.equal(grid.visibleLength(3, 20, 55), 15);
    assert.equal(grid.visibleLength(1, 20, 0), 0);
    assert.equal(grid.calculateOffset(3, 20), 40);

    let paged = grid.createPagination(1, 20, 55);
    assert.equal(paged.hasNext, true);
    assert.equal(grid.hasNextPage(paged), true);
    assert.equal(grid.hasPreviousPage(paged), false);
    paged = grid.nextPage(paged);
    assert.equal(paged.page, 2);
    paged = grid.goToPage(paged, 10);
    assert.equal(paged.page, 3);
    assert.equal(paged.hasNext, false);
    assert.equal(grid.setPageSize(paged, 0).pageSize, 1);
    assert.equal(grid.setTotal(grid.createPagination(5, 10, 100), 12).page, 2);

    assert.equal(grid.dataTableRangeLabel(1, 20, 55), "1–20 of 55");
    assert.equal(grid.dataTableRangeLabel(3, 20, 55), "41–55 of 55");
    assert.equal(grid.dataTableRangeLabel(1, 20, 0), "0 of 0");
    // A paged label of an out-of-range page reports the clamped page, not the requested one.
    assert.equal(grid.dataTableRangeLabel(99, 20, 55), "41–55 of 55");
    assert.equal(grid.rangeLabel(grid.createInfinitePagination(20, 55)), "1–20 of 55");

    // Infinite mode accumulates: the window is the prefix, and the label follows it.
    const infinite = grid.setPaginationMode(grid.goToPage(grid.createInfinitePagination(20, 55), 3), "infinite");
    // The window is clamped to the total, so an over-accumulated infinite page never promises
    // rows that do not exist.
    assert.equal(grid.rangeLabel(infinite), "1–55 of 55");
    assert.equal(grid.visibleLength(infinite), 55);
    assert.equal(grid.sliceBounds(infinite, 55)[1], 55);
    assert.equal(grid.setPaginationMode(infinite, "paged").page, 3);
    assert.equal(grid.createPagination(2, 10, 100).mode, "paged");

    const rows = Array.from({ length: 55 }, (_, index) => index + 1);
    assert.deepEqual(grid.sliceItems(rows, grid.createPagination(3, 20, 55)), [41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55]);
    // The page is clamped before slicing, so an out-of-range page reports the last window.
    assert.equal(grid.sliceItems(rows, grid.createPagination(9, 20, 55)).length, 15);
    // A page past the end of the *locally available* rows renders nothing rather than wrapping.
    assert.deepEqual(grid.sliceItems(rows.slice(0, 5), grid.createPagination(3, 20, 55)), []);
    assert.equal(grid.sliceItems([], grid.createPagination(1, 20, 0)).length, 0);

    assert.deepEqual(grid.normalizePageSizeOptions(undefined, 20), [10, 20, 50, 100]);
    assert.deepEqual(grid.normalizePageSizeOptions([25, 5, 25, 0], 7), [5, 7, 25]);
    assert.deepEqual(grid.normalizePageSizeOptions([], 20), [10, 20, 50, 100]);
  `);
});

test("sort state cycles, repairs half-filled input and answers ARIA", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(grid.oppositeSortDirection("asc"), "desc");
    assert.equal(grid.oppositeSortDirection("desc"), "asc");
    assert.deepEqual(grid.emptySortState(), { columnId: null, direction: null });

    let sort = grid.cycleSort(null, "col1");
    assert.deepEqual(sort, { columnId: "col1", direction: "asc" });
    sort = grid.cycleSort(sort, "col1");
    assert.deepEqual(sort, { columnId: "col1", direction: "desc" });
    sort = grid.cycleSort(sort, "col1");
    // Clearing answers with the empty state, never null: a transition returns a state.
    assert.deepEqual(sort, { columnId: null, direction: null });
    assert.deepEqual(grid.cycleSort({ columnId: "other", direction: "desc" }, "col1"), {
      columnId: "col1",
      direction: "asc"
    });

    assert.equal(grid.sortDirectionForColumn({ columnId: "title", direction: null }, "title"), "asc");
    assert.equal(grid.sortDirectionForColumn({ columnId: "  ", direction: "asc" }, "  "), null);
    assert.equal(grid.isSortActive({ columnId: "title", direction: null }), true);
    assert.deepEqual(grid.activeSort({ columnId: "title", direction: null }), ["title", "asc"]);
    assert.equal(grid.ariaSortForColumn({ columnId: "title", direction: "asc" }, "title"), "ascending");
    assert.equal(grid.ariaSortForColumn({ columnId: "title", direction: "asc" }, "other"), "none");
    assert.equal(grid.ariaSortValue(null), "none");
    assert.deepEqual(grid.setSortState("  ", "asc"), { columnId: null, direction: null });
    assert.deepEqual(grid.setSortState("title", "desc"), { columnId: "title", direction: "desc" });
    assert.deepEqual(grid.clearSortState(), { columnId: null, direction: null });
    // createSortState mirrors SortState::new: it records what it is told, normalization repairs.
    assert.deepEqual(grid.normalizeSortState(grid.createSortState("  ", "desc")), {
      columnId: null,
      direction: null
    });
  `);
});

test("filters normalize structurally and report whether anything changed", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(grid.isFilterValueActive(""), false);
    assert.equal(grid.isFilterValueActive("hello"), true);
    assert.equal(grid.isFilterValueActive([]), false);
    assert.equal(grid.isFilterValueActive(["a"]), true);
    assert.equal(grid.isFilterValueActive({ kind: "boolean", value: false }), true);
    assert.deepEqual(grid.emptyFilterValue(grid.textFilter()), { kind: "text", value: "" });
    assert.equal(grid.isFilterValueEmpty(grid.emptyFilterValue(grid.numberRangeFilter())), true);
    assert.deepEqual(grid.emptyFilterValue(grid.booleanFilter("Yes", "No")), { kind: "empty" });
    assert.deepEqual(grid.normalizeFilterValue({ kind: "number_range", min: 10, max: 1 }), {
      kind: "number_range",
      min: 1,
      max: 10
    });

    let filters = {};
    const first = grid.setColumnFilter(filters, "title", "  hello  ");
    assert.equal(first.changed, true);
    assert.deepEqual(first.filters, { title: { kind: "text", value: "hello" } });
    assert.equal(grid.setColumnFilter(first.filters, "title", "hello").changed, false);
    assert.equal(grid.hasActiveFilters(first.filters), true);
    assert.equal(grid.columnFilterCount(first.filters), 1);
    assert.equal(grid.getColumnFilter(first.filters, "title").value, "hello");
    assert.equal(grid.filterValueText(grid.getColumnFilter(first.filters, "title")), "hello");
    assert.equal(grid.filterValueNumberRange(grid.getColumnFilter(first.filters, "title")), null);

    // Reversed bounds are the same query, so writing them again is not a change.
    const range = grid.setColumnFilter({}, "price", { kind: "number_range", min: 10, max: 20 });
    assert.equal(grid.setColumnFilter(range.filters, "price", { kind: "number_range", min: 20, max: 10 }).changed, false);
    assert.equal(grid.setColumnFilter(range.filters, "price", { kind: "number_range", min: 11, max: 20 }).changed, true);
    assert.equal(grid.setColumnFilter(range.filters, "price", { kind: "number_range", min: null, max: null }).changed, true);
    assert.equal(grid.hasActiveFilters(grid.setColumnFilter(range.filters, "price", { kind: "number_range", min: null, max: null }).filters), false);

    assert.deepEqual(grid.removeColumnFilter(range.filters, "absent"), { filters: range.filters, changed: false });
    assert.equal(grid.removeColumnFilter(range.filters, "price").changed, true);
    assert.deepEqual(grid.clearColumnFilters({}), { filters: {}, changed: false });
    assert.equal(grid.getColumnFilter({ toString: { kind: "text", value: "x" } }, "absent"), undefined);
    assert.deepEqual(grid.columnFilterKeys(grid.normalizeFilters({ b: "1", a: "2" })), ["a", "b"]);
    assert.deepEqual(grid.columnFilterEntries({}), []);
    assert.equal(grid.isFilterEqual({ kind: "select", value: " A " }, { kind: "select", value: "A" }), true);
    assert.equal(grid.isFilterEqual({ kind: "text", value: "A" }, { kind: "select", value: "A" }), false);
    assert.deepEqual(grid.filterValueSelect({ kind: "select", value: "A" }), "A");
    // Range accessors answer the way Rust's as_number_range/as_date_range do: a positional pair.
    assert.deepEqual(grid.filterValueDateRange({ kind: "date_range", from: "2026-01-01", to: null }), [
      "2026-01-01",
      null
    ]);
    assert.deepEqual(grid.filterValueNumberRange({ kind: "number_range", min: 1, max: null }), [1, null]);
    assert.equal(grid.filterValueBoolean({ kind: "boolean", value: true }), true);
    assert.equal(grid.filterTypeAccepts(grid.textFilter(), { kind: "text", value: "x" }), true);
    assert.equal(grid.filterTypeAccepts(grid.textFilter(), { kind: "boolean", value: true }), false);
    const selectType = grid.selectFilterWithPlaceholder([{ value: "a", label: "A" }], "Any");
    assert.equal(selectType.placeholder, "Any");
    assert.equal(grid.filterTypeAccepts(selectType, { kind: "select", value: "a" }), true);
    assert.equal(grid.filterTypeAccepts(selectType, { kind: "text", value: "a" }), false);
    assert.equal(grid.filterTypeAccepts(selectType, grid.emptyFilterValue(selectType)), true);
  `);
});

test("cells answer filters and order, and the engine filters then sorts rows", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(grid.cellIsEmpty(grid.cellEmpty()), true);
    assert.equal(grid.cellIsEmpty(grid.cellText("   ")), true);
    assert.equal(grid.cellIsEmpty(grid.cellNumber(null)), true);
    assert.equal(grid.cellIsEmpty(grid.cellNumber(0)), false);
    assert.equal(grid.cellIsEmpty(grid.cellBoolean(false)), false);
    assert.equal(grid.cellOptionalText(undefined).kind, "empty");
    assert.equal(grid.textContainsIgnoreCase("Красное", "красн"), true);
    assert.equal(grid.textContainsIgnoreCase("abc", ""), true);
    assert.equal(grid.equalsIgnoreAsciiCase("ACTIVE", "active"), true);
    assert.equal(grid.equalsIgnoreAsciiCase("А", "а"), false);
    assert.equal(grid.cellNumberForFilter(grid.cellText("1e")), null);
    assert.equal(grid.cellNumberForFilter(grid.cellText("0x10")), null);
    assert.equal(grid.cellNumberForFilter(grid.cellText(" -12.5 ")), -12.5);
    assert.equal(grid.cellDatePart(grid.cellText("2026-05-01T10:00:00Z")), "2026-05-01");
    assert.equal(grid.cellDatePart(grid.cellNumber(1)), null);

    assert.equal(grid.matchesFilterValue(grid.normalizeFilterValue("  SHIRT "), grid.cellText("Red Shirt")), true);
    assert.equal(grid.matchesFilterValue({ kind: "select", value: "active" }, grid.cellText("ACTIVE")), true);
    assert.equal(grid.matchesFilterValue({ kind: "select", value: "42" }, grid.cellNumber(42)), true);
    assert.equal(grid.matchesFilterValue({ kind: "select", value: "42" }, grid.cellEmpty()), false);
    assert.equal(grid.matchesFilterValue({ kind: "boolean", value: false }, grid.cellBoolean(false)), true);
    assert.equal(grid.matchesFilterValue({ kind: "boolean", value: false }, grid.cellText("false")), false);
    assert.equal(grid.matchesFilterValue(undefined, grid.cellEmpty()), true);

    const order = [grid.cellEmpty(), grid.cellBoolean(false), grid.cellNumber(1.5), grid.cellText("b"), grid.cellText("а")];
    for (let index = 0; index + 1 < order.length; index += 1) {
      assert.equal(grid.compareCells(order[index], order[index + 1]), -1, "rank order at " + index);
    }
    assert.equal(grid.compareCells(order[order.length - 1], order[0]), 1);

    const rows = [
      { name: "hat", price: 30, active: true },
      { name: "shirt", price: 10, active: false },
      { name: "coat", price: 50, active: true },
      { name: "boots", price: 20, active: false }
    ];
    const cellOf = (row, columnId) => {
      if (columnId === "name") return grid.cellText(row.name);
      if (columnId === "price") return grid.cellNumber(row.price);
      if (columnId === "active") return grid.cellBoolean(row.active);
      return grid.cellEmpty();
    };
    assert.deepEqual(grid.filterRows({}, rows, cellOf).map((row) => row.name), ["hat", "shirt", "coat", "boots"]);
    assert.deepEqual(
      grid.filterRows({ name: "o", price: { kind: "number_range", min: 25 } }, rows, cellOf).map((row) => row.name),
      ["coat"]
    );
    assert.deepEqual(grid.sortRows({ columnId: "price", direction: "asc" }, rows, cellOf).map((row) => row.price), [10, 20, 30, 50]);
    assert.deepEqual(grid.sortRows({ columnId: "price", direction: "desc" }, rows, cellOf).map((row) => row.price), [50, 30, 20, 10]);
    // A half-filled state sorts ascending, and equal cells keep their input order.
    const tied = [{ name: "c", price: 2 }, { name: "a", price: 2 }, { name: "b", price: 1 }];
    assert.deepEqual(grid.sortRows({ columnId: "price", direction: null }, tied, cellOf).map((row) => row.name), ["b", "c", "a"]);
    assert.deepEqual(grid.sortRows(null, tied, cellOf).map((row) => row.name), ["c", "a", "b"]);
  `);
});

test("selection is a deterministic set that survives paging", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    const empty = grid.emptyRowSelection();
    assert.equal(grid.isRowSelectionEmpty(empty), true);
    assert.equal(grid.rowSelectionCount(empty), 0);
    assert.deepEqual(grid.rowSelectionIds(empty), []);

    const created = grid.rowSelectionFromIds(["b", "a", "b"]);
    assert.equal(grid.rowSelectionCount(created), 2);
    // Rust serializes a BTreeSet; insertion order would leak the click order into bulk requests.
    assert.deepEqual(grid.rowSelectionIds(created), ["a", "b"]);
    assert.deepEqual(grid.rowSelectionIds(grid.rowSelectionFromIds(["б", "a", "а"])), ["a", "а", "б"]);

    assert.equal(grid.selectRow(created, "a"), created);
    assert.equal(grid.deselectRow(created, "zzz"), created);
    assert.equal(grid.toggleRowSelection(created, "a").has("a"), false);
    assert.equal(grid.setRowSelected(created, "c", true).has("c"), true);
    assert.equal(grid.setRowSelected(created, "b", false).has("b"), false);
    assert.equal(grid.isRowSelected(created, "b"), true);

    const offPage = grid.addRowsToSelection(grid.rowSelectionFromIds(["z"]), ["a", "b"]);
    assert.deepEqual(grid.rowSelectionIds(offPage), ["a", "b", "z"]);
    assert.equal(grid.addRowsToSelection(offPage, ["a"]), offPage);
    assert.deepEqual(grid.rowSelectionIds(grid.deselectAllRows(offPage, ["a"])), ["b", "z"]);
    assert.equal(grid.deselectAllRows(offPage, ["nope"]), offPage);
    assert.equal(grid.isRowSelectionEmpty(grid.clearRowSelection()), true);

    assert.deepEqual(grid.rowSelectionIds(grid.retainRowIds(offPage, ["a", "missing"])), ["a"]);
    // Retaining a selection that is already a subset returns the same set, so an adapter does not
    // re-render on a refresh that dropped nothing.
    const subset = grid.rowSelectionFromIds(["a"]);
    assert.equal(grid.retainRowIds(subset, ["a", "b"]), subset);

    assert.equal(grid.areAllRowsSelected(offPage, []), false);
    assert.equal(grid.areAllRowsSelected(offPage, ["a", "b"]), true);
    assert.equal(grid.areSomeRowsSelected(offPage, ["a", "zzz"]), true);
    assert.equal(grid.areSomeRowsSelected(offPage, ["a", "b"]), false);
  `);
});

test("persisted widths repair on read and prune on clamp", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    const widths = { title: 150 };
    assert.equal(grid.getStoredColumnWidth(widths, "title"), 150);
    assert.equal(grid.getStoredColumnWidth(widths, "absent"), undefined);
    assert.equal(grid.getColumnWidth(widths, "absent", 120), 120);
    assert.equal(grid.columnWidthsAreEmpty({}), true);

    // A render path must survive corrupted persisted state instead of throwing inside a component.
    assert.equal(grid.getClampedColumnWidth({ title: Number.NaN }, "title", 120, 100, 300), 120);
    assert.equal(grid.getClampedColumnWidth({ title: -40 }, "title", 120, 100, 300), 100);
    assert.equal(grid.getClampedColumnWidth({ title: 9000 }, "title", 120, 100, 300), 300);
    assert.equal(grid.getClampedColumnWidth({ title: 90 }, "title", 120, 300, 100), 100);
    assert.deepEqual(grid.normalizeColumnWidths({ b: 1.9, a: "x", c: 4 }), { b: 1, c: 4 });
    assert.deepEqual(grid.columnWidthEntries({ b: 2, a: 1 }), [["a", 1], ["b", 2]]);

    assert.deepEqual(grid.setColumnWidth(widths, "title", 150), { widths, changed: false });
    const grown = grid.setColumnWidth(widths, "title", 200);
    assert.equal(grown.changed, true);
    assert.throws(() => grid.setColumnWidth(widths, "title", 12.5), RangeError);
    assert.equal(grid.setClampedColumnWidth(widths, "title", 9000, 100, 300).widths.title, 300);
    assert.equal(grid.removeColumnWidth(widths, "absent").changed, false);
    assert.equal(grid.removeColumnWidth(widths, "title").widths.title, undefined);
    assert.deepEqual(grid.clearColumnWidths(widths).widths, {});

    const columns = [{ id: "title", width: grid.createResizableColumnWidth(200, 100, 300) }];
    assert.deepEqual(grid.pruneColumnWidths({ title: 150, retired: 90 }, columns), {
      widths: { title: 150 },
      changed: true
    });
    assert.equal(grid.pruneColumnWidths({ title: 150 }, columns).widths.title, 150);
    assert.equal(grid.pruneColumnWidths({ title: 150 }, columns).changed, false);
    // Clamping enforces the current bounds and drops columns that no longer exist.
    assert.deepEqual(grid.clampColumnWidths({ title: 9000, retired: 90 }, columns), { title: 300 });
    assert.equal(grid.pruneColumnWidths({ title: 150 }, []).changed, true);

    assert.equal(grid.calculateResizedWidth(200, 50, 100, 300), 250);
    assert.equal(grid.calculateResizedWidth(200, 200, 100, 300), 300);
    assert.equal(grid.calculateResizedWidth(200, -2000, 100, 300), 100);
    assert.equal(grid.calculateResizedWidth(200, Number.NaN, 100, 300), 200);
    assert.equal(grid.calculateResizedWidth(200, 10, 300, 100), 210);
    assert.equal(grid.stepResizedWidth(200, -grid.KEYBOARD_RESIZE_STEP, 100, 300), 184);
    assert.throws(() => grid.stepResizedWidth(200, 2 ** 31, 100, 300), RangeError);
  `);
});

test("composite state invalidates paging and applies rows in contract order", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    let state = grid.createGridState({ pagination: grid.createPagination(3, 10, 100) });
    assert.equal(state.pagination.page, 3);
    assert.deepEqual(grid.rowSelectionIds(state.selection), []);
    assert.deepEqual(state.filters, {});

    const filtered = grid.setGridFilter(state, "title", "books");
    assert.equal(filtered.changed, true);
    assert.equal(filtered.state.pagination.page, 1);
    assert.equal(grid.setGridFilter(filtered.state, "title", "books").changed, false);
    state = filtered.state;
    state = { ...state, pagination: grid.goToPage(state.pagination, 4) };

    const sorted = grid.toggleGridSort(state, "title");
    assert.equal(sorted.state.pagination.page, 1);
    assert.equal(sorted.direction, "asc");
    const cleared = grid.toggleGridSort({ ...sorted.state, sort: { columnId: "title", direction: "desc" } }, "title");
    assert.equal(cleared.direction, null);
    assert.deepEqual(cleared.state.sort, { columnId: null, direction: null });
    assert.equal(grid.resetGridFilters(sorted.state).changed, true);
    assert.equal(grid.resetGridFilters(sorted.state).state.pagination.page, 1);

    assert.deepEqual(
      grid.rowSelectionIds(grid.pruneGridSelection({ ...state, selection: grid.rowSelectionFromIds(["a", "b"]) }, ["b", "c"]).selection),
      ["b"]
    );

    const columns = [
      { id: "title", width: grid.createResizableColumnWidth(200, 100, 300) },
      { id: "gone", width: grid.createResizableColumnWidth(200, 100, 300) }
    ];
    let widthState = { ...state, columnWidths: { title: 9000 } };
    widthState = { ...widthState, columnWidths: grid.setColumnWidth(widthState.columnWidths, "gone", 150).widths };
    assert.equal(grid.clampGridWidths(widthState, columns).columnWidths.title, 300);
    assert.equal(grid.clampGridWidths(widthState, [{ id: "title", width: grid.createResizableColumnWidth(200, 100, 300) }]).columnWidths.gone, undefined);

    const normalized = grid.normalizeGridState({
      columnWidths: { title: -5 },
      filters: { blank: "  " },
      sort: { columnId: "title", direction: null },
      selection: ["a", "a"],
      pagination: { page: 0, pageSize: 0, total: 21 }
    });
    // A negative persisted width is repaired to 0, which is drawable; garbage keys are dropped.
    assert.deepEqual(normalized.columnWidths, { title: 0 });
    assert.deepEqual(normalized.filters, {});
    assert.deepEqual(normalized.sort, { columnId: "title", direction: "asc" });
    assert.deepEqual(grid.rowSelectionIds(normalized.selection), ["a"]);
    assert.equal(normalized.pagination.page, 1);
    assert.equal(normalized.pagination.pageSize, 1);

    const rows = [
      { name: "hat", price: 30 },
      { name: "shirt", price: 10 },
      { name: "coat", price: 50 },
      { name: "boots", price: 20 },
      { name: "dress", price: 40 }
    ];
    const cellOf = (row, columnId) => (columnId === "name" ? grid.cellText(row.name) : grid.cellNumber(row.price));
    let rowsState = grid.createGridState({ pagination: grid.createPagination(1, 2, rows.length) });
    assert.deepEqual(grid.applyGridRows(rowsState, rows, cellOf).map((row) => row.name), ["hat", "shirt"]);
    rowsState = { ...rowsState, sort: grid.setSortState("price", "desc") };
    rowsState = { ...rowsState, pagination: grid.goToPage(rowsState.pagination, 2) };
    assert.deepEqual(grid.applyGridRows(rowsState, rows, cellOf).map((row) => row.price), [30, 20]);
    rowsState = grid.setGridFilter(rowsState, "name", "o").state;
    rowsState = { ...rowsState, pagination: grid.goToPage(rowsState.pagination, 1) };
    assert.deepEqual(grid.applyGridRows(rowsState, rows, cellOf).map((row) => row.name), ["coat", "boots"]);
  `);
});

test("route edits keep positions, drop blanks and preserve the fragment", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(
      grid.applyQueryPairs("/products?search=bag", [["search", "shoe"], ["color", "red"], ["empty", ""]]),
      "/products?search=shoe&color=red"
    );
    assert.equal(grid.applyQueryPairs("/products?tag=red&search=bag&tag=blue", [["tag", "green"]]), "/products?tag=green&search=bag");
    assert.equal(grid.applyQueryPairs("/products?tag=red&search=bag&tag=blue", [["tag", "  "]]), "/products?search=bag");
    assert.equal(grid.applyQueryPairs("/products", []), "/products");
    assert.equal(grid.applyQueryPairs("/products?a=1", [["a", null]]), "/products");
    // A fragment is part of the URL, never part of the last value.
    assert.equal(grid.queryParam("/products?search=bag#reviews", "search"), "bag");
    assert.equal(grid.queryParam("/products?search=#reviews", "search"), null);
    assert.equal(grid.queryParam("/products#reviews", "search"), null);
    assert.equal(grid.applyQueryPairs("/products?search=bag#reviews", [["search", "shoe"]]), "/products?search=shoe#reviews");
    assert.equal(grid.applyQueryPairs("/products?search=bag#reviews", [["search", ""]]), "/products#reviews");
    assert.equal(grid.applyQueryPairs("/products#reviews", [["color", "red"]]), "/products?color=red#reviews");
    assert.equal(grid.applyQueryPairs("/products?a=1&b=2#x", [["c", "3"]]), "/products?a=1&b=2&c=3#x");
  `);
});

test("facets cut at the value limit and the panel exposes per-facet counting", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    const buckets = Array.from({ length: grid.MAX_GRID_FACET_VALUES + 3 }, (_, index) => ({
      value: "v" + index,
      label: "V" + index,
      count: index
    }));
    const facet = grid.facetFromBuckets({
      code: "color",
      label: "Color",
      domain: grid.dictionaryDomain(true),
      total: 40,
      values: buckets
    });
    assert.equal(facet.values.length, grid.MAX_GRID_FACET_VALUES);
    assert.equal(facet.isTruncated, true);
    assert.equal(facet.isEnumerable, true);
    assert.equal(grid.openFacet({ code: "brand", label: "Brand", total: 40 }).isEnumerable, false);
    assert.throws(() => grid.normalizeFacetSource({ code: "c", label: "C", domain: grid.dictionaryDomain(true), total: Number.MAX_SAFE_INTEGER + 1, isTruncated: false, values: [] }), RangeError);

    const open = grid.openFacet({ code: "brand", label: "Brand", total: 40 });
    const panel = grid.buildFacetPanel([facet, open], ["color=v0", "brand=nike"], grid.ENGLISH_FACET_PANEL_LABELS);
    assert.equal(panel.facets.length, 2);
    assert.equal(panel.facets[0].hasSelection, true);
    assert.equal(panel.facets[0].values[0].selected, true);
    assert.equal(panel.facets[0].values[0].selectionEntry, "color=v0");
    assert.equal(panel.facets[1].unboundedHint, grid.ENGLISH_FACET_PANEL_LABELS.unboundedHint);
    assert.equal(panel.facets[0].truncatedHint, grid.ENGLISH_FACET_PANEL_LABELS.truncatedHint);
    assert.deepEqual(grid.facetEntriesForCode(panel, "color"), ["color=v0"]);
    assert.deepEqual(grid.otherFacetSelectionForCode(panel, "color"), ["brand=nike"]);
    assert.deepEqual(grid.otherFacetSelectionForCode(panel, "brand"), ["color=v0"]);
    // A new entry is appended: the selection keeps the order the user clicked in, which is what
    // makes repeated toggles produce a stable link.
    assert.deepEqual(grid.toggleFacetSelection(panel, "color", "v1"), ["color=v0", "brand=nike", "color=v1"]);
    assert.deepEqual(grid.toggleFacetSelection(panel, "color", "v0"), ["brand=nike"]);
    assert.deepEqual(grid.clearFacetSelection(panel, "color"), ["brand=nike"]);
    assert.equal(grid.isFacetValueSelected(panel, "color", "v0"), true);
    assert.deepEqual(grid.clearFacetPanelSelection(panel), []);
    assert.equal(panel.hasSelection, true);
    assert.equal(panel.showEmptyState, false);
    assert.equal(grid.countLabel("({count})", 12), "(12)");
    assert.deepEqual(grid.facetFilterOptions(facet)[0], { value: "v0", label: "V0" });
    const capped = grid.buildFacetPanel(
      Array.from({ length: grid.MAX_GRID_FACETS + 4 }, (_, index) =>
        grid.openFacet({ code: "f" + index, label: "F" + index, total: 1 })),
      [],
      grid.ENGLISH_FACET_PANEL_LABELS
    );
    assert.equal(capped.facets.length, grid.MAX_GRID_FACETS);
  `);
});
