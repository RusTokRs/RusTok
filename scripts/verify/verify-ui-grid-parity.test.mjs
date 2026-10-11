#!/usr/bin/env node
/**
 * Comprehensive parity and correctness tests for @rustok/ui-grid.
 *
 * Verifies all modules of the framework-free TypeScript grid toolkit:
 * column, pagination, sort, filter, selection, resize, state.
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

function runTestInGridModule(code) {
  const directory = mkdtempSync(path.join(tmpdir(), "rustok-ui-grid-test-"));
  const harness = path.join(directory, "harness.mjs");
  writeFileSync(
    harness,
    [
      `import * as grid from "@rustok/ui-grid";`,
      code,
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
    assert.equal(result.status, 0, `test failed: ${result.stderr || result.stdout}`);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("column definitions, constraints and visibility helpers", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    const width = grid.createColumnWidth(200, 100, 400);
    assert.equal(width.default, 200);
    assert.equal(grid.clampColumnWidth(width, 50), 100);
    assert.equal(grid.clampColumnWidth(width, 500), 400);

    const checkbox = grid.createCheckboxColumn();
    assert.equal(checkbox.id, grid.CHECKBOX_COLUMN_ID);
    assert.equal(grid.isCheckboxColumn(checkbox), true);

    const cols = [
      checkbox,
      grid.createColumnDef("name", "Name", { filterable: true, filterType: "text" }),
      grid.createColumnDef("hidden", "Hidden", { visible: false })
    ];
    assert.equal(grid.visibleColumnCount(cols), 2);
    assert.equal(grid.hasFilterRow(cols), true);
  `);
});

test("pagination calculations, bounds, slicing and range labels", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(grid.calculatePageCount(0, 20), 1);
    assert.equal(grid.calculatePageCount(55, 20), 3);
    assert.equal(grid.clampPage(5, 55, 20), 3);
    assert.equal(grid.clampPage(0, 55, 20), 1);

    assert.equal(grid.fromIndex(1, 20, 55), 1);
    assert.equal(grid.toIndex(1, 20, 55), 20);
    assert.equal(grid.fromIndex(3, 20, 55), 41);
    assert.equal(grid.toIndex(3, 20, 55), 55);

    assert.equal(grid.dataTableRangeLabel(1, 20, 55), "1–20 of 55");
    assert.equal(grid.dataTableRangeLabel(3, 20, 55), "41–55 of 55");
    assert.equal(grid.dataTableRangeLabel(1, 20, 0), "0 of 0");

    let p = grid.createPagination(1, 20, 55);
    assert.equal(p.page, 1);
    assert.equal(p.hasNext, true);
    p = grid.nextPage(p);
    assert.equal(p.page, 2);
    p = grid.setPage(p, 10);
    assert.equal(p.page, 3);
    assert.equal(p.hasNext, false);
  `);
});

test("sort state machine, cycling and comparators", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(grid.oppositeSortDirection("asc"), "desc");
    assert.equal(grid.oppositeSortDirection("desc"), "asc");

    let s = grid.cycleSort(null, "col1");
    assert.deepEqual(s, { columnId: "col1", direction: "asc" });
    s = grid.cycleSort(s, "col1");
    assert.deepEqual(s, { columnId: "col1", direction: "desc" });
    s = grid.cycleSort(s, "col1");
    assert.equal(s, null);

    assert.equal(grid.compareValues(10, 20, "asc"), -10);
    assert.equal(grid.compareValues(10, 20, "desc"), 10);
    assert.equal(grid.compareValues("apple", "banana", "asc") < 0, true);

    assert.equal(grid.isSortedBy({ columnId: "title", direction: "asc" }, "title"), "asc");
    assert.equal(grid.isSortedBy({ columnId: "title", direction: "asc" }, "other"), null);
    assert.equal(grid.ariaSortFor({ columnId: "title", direction: "asc" }, "title"), "ascending");
  `);
});

test("filter normalization, immutable changes and activity tracking", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    assert.equal(grid.isFilterActive(""), false);
    assert.equal(grid.isFilterActive("hello"), true);
    assert.equal(grid.isFilterActive([]), false);
    assert.equal(grid.isFilterActive(["a"]), true);

    let filters = {};
    const res1 = grid.setFilter(filters, "title", "test");
    assert.equal(res1.changed, true);
    assert.deepEqual(res1.filters, { title: { kind: "text", value: "test" } });

    const res2 = grid.setFilter(res1.filters, "title", "");
    assert.equal(res2.changed, true);
    assert.deepEqual(res2.filters, {});
    assert.equal(grid.hasActiveFilters(res2.filters), false);
  `);
});

test("selection Set operations, retain and bulk actions", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    let sel = grid.emptyRowSelection();
    assert.equal(grid.isSelectionEmpty(sel), true);

    sel = grid.selectRow(sel, "row-1");
    assert.equal(grid.isRowSelected(sel, "row-1"), true);
    assert.equal(grid.selectedCount(sel), 1);

    sel = grid.toggleRowSelection(sel, "row-2");
    assert.equal(grid.selectedCount(sel), 2);
    assert.equal(grid.areAllRowsSelected(sel, ["row-1", "row-2"]), true);
    assert.equal(grid.areSomeRowsSelected(sel, ["row-1", "row-3"]), true);

    sel = grid.retainSelectedRows(sel, ["row-2", "row-99"]);
    assert.deepEqual(grid.rowSelectionToArray(sel), ["row-2"]);

    sel = grid.clearSelection();
    assert.equal(grid.isSelectionEmpty(sel), true);
  `);
});

test("resize storage, clamping and keyboard stepping", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    const widths = new grid.ColumnWidths({ a: 150 });
    assert.equal(widths.get("a", 100), 150);
    assert.equal(widths.get("b", 100), 100);
    widths.setClamped("a", 900, 100, 300);
    assert.equal(widths.get("a", 100), 300);

    assert.equal(grid.calculateResizedWidth(200, 50, 100, 300), 250);
    assert.equal(grid.calculateResizedWidth(200, 200, 100, 300), 300);
    assert.equal(grid.stepResizedWidth(200, -16, 100, 300), 184);
  `);
});

test("composite GridState invalidation invariants and clamping", () => {
  runTestInGridModule(`
    import assert from "node:assert/strict";
    let state = grid.createGridState({
      pagination: grid.createPagination(3, 10, 100)
    });
    assert.equal(state.pagination.page, 3);

    const fRes = grid.setGridFilter(state, "title", "books");
    assert.equal(fRes.changed, true);
    assert.equal(fRes.state.pagination.page, 1);

    state = fRes.state;
    state.pagination = grid.setPage(state.pagination, 4);
    assert.equal(state.pagination.page, 4);

    const sRes = grid.toggleGridSort(state, "title");
    assert.equal(sRes.state.pagination.page, 1);
    assert.equal(sRes.nextDirection, "asc");

    state.columnWidths = grid.setColumnWidth(state.columnWidths, "title", 9999).widths;
    const cols = [
      grid.createColumnDef("title", "Title", {
        width: grid.createColumnWidth(200, 100, 300)
      })
    ];
    const clampedState = grid.clampWidths(state, cols);
    assert.equal(grid.getColumnWidth(clampedState.columnWidths, "title", 200), 300);
  `);
});
