#!/usr/bin/env node
/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * Bidirectional public-surface parity gate for the two grid libraries.
 *
 *   Rust:       crates/ui/rustok-grid/src/*.rs  (canonical owner, AGENTS §5)
 *   TypeScript: packages/rustok-ui-grid/src/*.ts (its mirror)
 *
 * Behaviour is checked by `verify-ui-grid-parity.test.mjs`; this gate checks the *surface*, so a
 * symbol added on one side without an answer on the other fails the build instead of quietly turning
 * into a feature gap. Names are matched across the whole library, not per file, because a twin can
 * legitimately sit next to the data it reads (`filterRows` lives beside the cell vocabulary while
 * its Rust counterpart is a method of `ColumnFilters`).
 *
 * A symbol is covered when one of these holds:
 *   1. the plain name transform matches (Rust `snake_case` ↔ TypeScript `camelCase`);
 *   2. PAIRS / TYPE_PAIRS has a row for it — used where a twin genuinely differs in spelling
 *      (a Rust method against a free TypeScript function, a boundary type name, a polarity flip);
 *   3. RUST_ONLY / TS_ONLY has a row with a reason — used where one side legitimately has no twin
 *      (a capability the other language gets from its type system or a trait).
 *
 * Anything matching none of the three is a finding. The tables are explicit and reviewed on purpose:
 * a heuristic prefix/suffix match would let a coincidence "pass" a real gap, and every exception has
 * to say why it is not one. A row that stops being true is also a finding (see the validation pass),
 * so a closed gap cannot leave a stale row behind.
 */

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  camelCase,
  rustPublicSymbols,
  snakeCase,
  tsPublicSymbols,
} from "./lib/rust-public-surface.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const rustRoot = path.join(repoRoot, "crates/ui/rustok-grid/src");
const tsRoot = path.join(repoRoot, "packages/rustok-ui-grid/src");

/** The files whose public surface is compared, as `[module, rustFile, tsFile]`. */
const MODULES = [
  ["cell", "cell.rs", "cell.ts"],
  ["column", "column.rs", "column.ts"],
  ["facet", "facet.rs", "facet.ts"],
  ["facet_panel", "facet_panel.rs", "panel.ts"],
  ["filter", "filter.rs", "filter.ts"],
  ["pagination", "pagination.rs", "pagination.ts"],
  ["resize", "resize.rs", "resize.ts"],
  ["selection", "selection.rs", "selection.ts"],
  ["sort", "sort.rs", "sort.ts"],
  ["state", "state.rs", "state.ts"],
];

/**
 * Types whose twin is spelled differently.
 *
 * These are boundary names, not synonyms: the Rust core's `GridFacet` is what an owner resolves, and
 * the TypeScript mirror exposes the same data as the `FacetSource` the panel consumes. The `View`
 * suffix exists because the TypeScript side also ships the *input* shape next to it
 * (`FacetSourceInput`), so the rendered record has to be distinguishable. A facet's identity field is
 * `code` in TypeScript and `key` in Rust: the TypeScript value is spread through JSX, where `key` is
 * a reserved attribute name, and the product owner calls it `code` end to end.
 */
const TYPE_PAIRS = new Map([
  ["GridFacet", "FacetSource"],
  ["FacetPanel", "FacetPanelView"],
  ["FacetPanelFacet", "FacetPanelFacetView"],
  ["FacetPanelValue", "FacetPanelValueView"],
]);

/**
 * `"<module>/<RustKey>"` → TypeScript export, for every twin the name transform cannot reach.
 * Rust keys are `name` for a free item or `Type::method` for an inherent impl member.
 */
const PAIRS = new Map([
  // ── cell: what a grid knows about one row ──────────────────────────────────────────────────
  ["cell/CellValue::text", "cellText"],
  ["cell/CellValue::optional_text", "cellOptionalText"],
  ["cell/CellValue::number", "cellNumber"],
  ["cell/CellValue::optional_number", "cellNumber"],
  ["cell/CellValue::boolean", "cellBoolean"],
  ["cell/CellValue::empty", "cellEmpty"],
  ["cell/CellValue::is_empty", "cellIsEmpty"],
  ["cell/CellValue::number_for_filter", "cellNumberForFilter"],
  ["cell/CellValue::date_part", "cellDatePart"],
  ["cell/CellValue::matches", "matchesFilterValue"],
  ["cell/CellValue::compare", "compareCells"],
  // ── column: descriptors, width bounds, visibility ───────────────────────────────────────────
  ["column/ColumnWidth::fixed", "createFixedColumnWidth"],
  ["column/ColumnWidth::resizable", "createResizableColumnWidth"],
  ["column/ColumnWidth::clamp", "clampColumnWidth"],
  ["column/ColumnWidth::normalize", "normalizeColumnWidth"],
  ["column/ColumnWidth::set_preferred", "setPreferredColumnWidth"],
  ["column/ColumnWidth::set_min", "setColumnMinWidth"],
  ["column/ColumnWidth::set_max", "setColumnMaxWidth"],
  ["column/ColumnWidth::is_flexible", "isColumnWidthFlexible"],
  ["column/GridColumnDef::new", "createGridColumnDef"],
  ["column/GridColumnDef::checkbox", "createCheckboxColumn"],
  ["column/GridColumnDef::actions", "createActionsColumn"],
  ["column/GridColumnDef::is_checkbox", "isCheckboxColumn"],
  ["column/GridColumnDef::width", "withColumnWidth"],
  ["column/GridColumnDef::width_range", "withColumnWidthRange"],
  ["column/GridColumnDef::min_width", "withColumnMinWidth"],
  ["column/GridColumnDef::max_width", "withColumnMaxWidth"],
  ["column/GridColumnDef::not_resizable", "withColumnNotResizable"],
  ["column/GridColumnDef::not_sortable", "withColumnNotSortable"],
  ["column/GridColumnDef::hidden", "hideColumn"],
  ["column/GridColumnDef::align", "withColumnAlign"],
  ["column/GridColumnDef::filter", "withColumnFilter"],
  ["column/GridColumnDef::not_filterable", "withColumnNotFilterable"],
  ["column/GridColumnDef::pin", "pinColumn"],
  ["column/GridColumnDef::unpin", "unpinColumn"],
  // ── filter: values, controls, the active map ────────────────────────────────────────────────
  ["filter/GridFilterType::text", "textFilter"],
  ["filter/GridFilterType::text_with_placeholder", "textFilterWithPlaceholder"],
  ["filter/GridFilterType::select", "selectFilter"],
  ["filter/GridFilterType::select_with_placeholder", "selectFilterWithPlaceholder"],
  ["filter/GridFilterType::number_range", "numberRangeFilter"],
  ["filter/GridFilterType::date_range", "dateRangeFilter"],
  ["filter/GridFilterType::boolean", "booleanFilter"],
  ["filter/GridFilterType::empty_value", "emptyFilterValue"],
  ["filter/GridFilterType::accepts", "filterTypeAccepts"],
  ["filter/FilterValue::is_empty", "isFilterValueEmpty"],
  ["filter/FilterValue::normalized", "normalizeFilterValue"],
  ["filter/FilterValue::as_text", "filterValueText"],
  ["filter/FilterValue::as_select", "filterValueSelect"],
  ["filter/FilterValue::as_number_range", "filterValueNumberRange"],
  ["filter/FilterValue::as_date_range", "filterValueDateRange"],
  ["filter/FilterValue::as_bool", "filterValueBoolean"],
  ["filter/ColumnFilters::normalize", "normalizeFilters"],
  ["filter/ColumnFilters::get", "getColumnFilter"],
  ["filter/ColumnFilters::contains", "hasColumnFilterValue"],
  ["filter/ColumnFilters::set", "setColumnFilter"],
  ["filter/ColumnFilters::remove", "removeColumnFilter"],
  ["filter/ColumnFilters::clear", "clearColumnFilters"],
  // Polarity pair: Rust asks "is the map empty", the mirror asks "is there anything to apply".
  ["filter/ColumnFilters::is_empty", "hasActiveFilters"],
  ["filter/ColumnFilters::count", "columnFilterCount"],
  ["filter/ColumnFilters::keys", "columnFilterKeys"],
  ["filter/ColumnFilters::iter", "columnFilterEntries"],
  ["filter/ColumnFilters::retain_matching", "filterRows"],
  // ── pagination ──────────────────────────────────────────────────────────────────────────────
  ["pagination/GridPagination::new", "createPagination"],
  ["pagination/GridPagination::infinite", "createInfinitePagination"],
  ["pagination/GridPagination::total_pages", "calculatePageCount"],
  ["pagination/GridPagination::has_previous", "hasPreviousPage"],
  ["pagination/GridPagination::from_index", "fromIndex"],
  ["pagination/GridPagination::to_index", "toIndex"],
  ["pagination/GridPagination::range_label", "rangeLabel"],
  ["pagination/GridPagination::visible_len", "visibleLength"],
  ["pagination/GridPagination::clamp_page", "clampPage"],
  ["pagination/GridPagination::normalize", "normalizePagination"],
  ["pagination/GridPagination::set_page", "goToPage"],
  ["pagination/GridPagination::go_to_page", "goToPage"],
  ["pagination/GridPagination::next_page", "nextPage"],
  ["pagination/GridPagination::previous_page", "previousPage"],
  ["pagination/GridPagination::set_page_size", "setPageSize"],
  ["pagination/GridPagination::set_total", "setTotal"],
  ["pagination/GridPagination::set_mode", "setPaginationMode"],
  ["pagination/GridPagination::offset", "calculateOffset"],
  ["pagination/GridPagination::slice_bounds", "sliceBounds"],
  ["pagination/GridPagination::slice_items", "sliceItems"],
  ["pagination/GridPagination::normalize_page_size_options", "normalizePageSizeOptions"],
  // ── resize: persisted widths and the drag math ──────────────────────────────────────────────
  ["resize/ColumnWidths::get_stored", "getStoredColumnWidth"],
  ["resize/ColumnWidths::get", "getColumnWidth"],
  ["resize/ColumnWidths::get_clamped", "getClampedColumnWidth"],
  ["resize/ColumnWidths::set", "setColumnWidth"],
  ["resize/ColumnWidths::set_clamped", "setClampedColumnWidth"],
  ["resize/ColumnWidths::remove", "removeColumnWidth"],
  ["resize/ColumnWidths::clear", "clearColumnWidths"],
  ["resize/ColumnWidths::is_empty", "columnWidthsAreEmpty"],
  ["resize/ColumnWidths::iter", "columnWidthEntries"],
  ["resize/ColumnWidths::prune_to", "pruneColumnWidths"],
  // ── selection ───────────────────────────────────────────────────────────────────────────────
  ["selection/RowSelection::new", "emptyRowSelection"],
  ["selection/RowSelection::is_selected", "isRowSelected"],
  ["selection/RowSelection::toggle", "toggleRowSelection"],
  ["selection/RowSelection::select", "selectRow"],
  ["selection/RowSelection::deselect", "deselectRow"],
  ["selection/RowSelection::set_selected", "setRowSelected"],
  ["selection/RowSelection::select_all", "addRowsToSelection"],
  ["selection/RowSelection::deselect_all", "deselectAllRows"],
  ["selection/RowSelection::clear", "clearRowSelection"],
  ["selection/RowSelection::retain_ids", "retainRowIds"],
  ["selection/RowSelection::is_all_selected", "areAllRowsSelected"],
  ["selection/RowSelection::is_partially_selected", "areSomeRowsSelected"],
  ["selection/RowSelection::count", "rowSelectionCount"],
  ["selection/RowSelection::is_empty", "isRowSelectionEmpty"],
  ["selection/RowSelection::ids", "rowSelectionIds"],
  ["selection/RowSelection::to_vec", "rowSelectionIds"],
  // ── sort ──────────────────────────────────────────────────────────────────────────────────────
  ["sort/SortDirection::opposite", "oppositeSortDirection"],
  ["sort/SortDirection::aria_sort", "ariaSortValue"],
  ["sort/SortState::new", "createSortState"],
  ["sort/SortState::normalize", "normalizeSortState"],
  ["sort/SortState::is_active", "isSortActive"],
  ["sort/SortState::active", "activeSort"],
  ["sort/SortState::is_sorted_by", "sortDirectionForColumn"],
  ["sort/SortState::aria_sort_for", "ariaSortForColumn"],
  ["sort/SortState::toggle", "cycleSort"],
  ["sort/SortState::set", "setSortState"],
  ["sort/SortState::clear", "clearSortState"],
  ["sort/SortState::sort_items", "sortRows"],
  // ── state: the composite transitions ────────────────────────────────────────────────────────
  ["state/GridState::new", "createGridState"],
  ["state/GridState::with_pagination", "withGridPagination"],
  ["state/GridState::normalize", "normalizeGridState"],
  ["state/GridState::set_filter", "setGridFilter"],
  ["state/GridState::reset_filters", "resetGridFilters"],
  ["state/GridState::toggle_sort", "toggleGridSort"],
  ["state/GridState::prune_selection", "pruneGridSelection"],
  ["state/GridState::clamp_widths", "clampGridWidths"],
  ["state/GridState::apply_rows", "applyGridRows"],
  // ── facet: counting contract and the selection vocabulary ───────────────────────────────────
  ["facet/FacetDomain::is_enumerable", "facetDomainIsEnumerable"],
  ["facet/GridFacet::from_buckets", "facetFromBuckets"],
  ["facet/GridFacet::open", "openFacet"],
  ["facet/GridFacet::normalize", "normalizeFacetSource"],
  ["facet/GridFacet::is_enumerable", "facetDomainIsEnumerable"],
  ["facet/GridFacet::filter_options", "facetFilterOptions"],
  ["facet/GridFacet::other_selection", "selectionExcept"],
  // ── facet panel: the rendered view ──────────────────────────────────────────────────────────
  ["facet_panel/FacetPanel::build", "buildFacetPanel"],
  ["facet_panel/FacetPanel::toggle", "toggleFacetSelection"],
  ["facet_panel/FacetPanel::clear_key", "clearFacetSelection"],
  ["facet_panel/FacetPanel::clear", "clearFacetPanelSelection"],
  ["facet_panel/FacetPanel::is_selected", "isFacetValueSelected"],
  ["facet_panel/FacetPanel::selected_for_key", "facetEntriesForCode"],
  ["facet_panel/FacetPanel::other_selection_for_key", "otherFacetSelectionForCode"],
  ["facet_panel/FacetPanelLabels::english", "ENGLISH_FACET_PANEL_LABELS"],
  ["facet_panel/FacetPanelValue::selection_entry", "selectionEntry"],
]);

/**
 * Public Rust items with no TypeScript counterpart, and why the mirror does not need one.
 * Every reason must state the language or contract fact that makes the absence correct.
 */
const RUST_ONLY = new Map([
  ["column/ColumnId::new", "a column id is a plain `string` in TypeScript, so no wrapper constructor exists"],
  ["column/ColumnId::as_str", "a column id is a plain `string` in TypeScript, so the unwrap is the identity"],
  ["column/ColumnId::into_string", "a column id is already a `string` in TypeScript, so there is nothing to take ownership of"],
  ["column/ColumnAlign::text_align", "the alignment is the CSS keyword union in TypeScript, so the mapping is the identity"],
  ["cell/CellValue::as_text", "the cell is a tagged union narrowed by `kind`; a per-field accessor would duplicate the pattern match"],
  ["cell/CellValue::as_number", "the cell is a tagged union narrowed by `kind`; a per-field accessor would duplicate the pattern match"],
  ["cell/CellValue::as_bool", "the cell is a tagged union narrowed by `kind`; a per-field accessor would duplicate the pattern match"],
  ["cell/CellValue::display_text", "only the filter matcher needs the rendered form, and it is private state of `matchesFilterValue`"],
  ["sort/SortDirection::as_str", "the direction is the string union `asc`/`desc` in TypeScript, so there is nothing to convert"],
  ["filter/FilterOption::new", "a filter option is an object literal in TypeScript, so a constructor would be noise"],
  ["facet/FacetValue::new", "a facet value is an object literal in TypeScript, so a constructor would be noise"],
  ["filter/ColumnFilters::new", "the empty filter map is the empty record `{}` in TypeScript, so no constructor is needed"],
  ["resize/ColumnWidths::new", "the empty width record is `{}` in TypeScript, so no constructor is needed"],
  ["facet/FacetValue::to_filter_option", "the facet value is consumed by `facetFilterOptions`, which maps the whole list at once"],
]);

/** Public TypeScript exports with no Rust counterpart, and why. */
const TS_ONLY = new Map([
  ["pagination/MAX_SAFE_GRID_INTEGER", "a JavaScript-only exact-integer bound; Rust totals are `u64` by construction"],
  ["column/MAX_COLUMN_WIDTH", "Rust's width is `u32`, so the ceiling is a type property there and a constant here"],
  ["column/assertColumnWidthPx", "Rust's `u32` rejects a non-integer width at the type boundary; TypeScript needs a runtime check"],
  ["column/createDefaultColumnWidth", "Rust reaches the default width through `Default for ColumnWidth`, a trait impl this gate skips"],
  ["sort/emptySortState", "Rust's empty state is `Default for SortState`, a trait impl this gate skips"],
  ["cell/CellAccessor", "Rust takes the same thing as an inline `Fn(&T, &str) -> CellValue` bound on each entry point"],
  ["cell/equalsIgnoreAsciiCase", "Rust compares ASCII-case-insensitively with the built-in `str::eq_ignore_ascii_case`"],
  ["filter/isFilterEqual", "Rust compares two normalized values with the derived `PartialEq`; two object literals are never equal in JavaScript"],
  ["filter/isFilterValueActive", "the positive reading of `FilterValue::is_empty`, exported because every call site asks it the other way round"],
  ["selection/rowSelectionFromIds", "Rust builds a selection through `FromIterator`, a trait impl this gate skips"],
  ["pagination/hasNextPage", "Rust reads the derived `has_next` field; the helper exists so a host never recomputes it inconsistently"],
  ["pagination/dataTableRangeLabel", "a convenience for a host holding page/size/total instead of a pagination state; it delegates to `rangeLabel`"],
  ["resize/normalizeColumnWidths", "Rust repairs persisted widths while deserializing; a TypeScript host restores from JSON, so the repair is explicit"],
  ["resize/clampColumnWidths", "the record form `clampGridWidths` delegates to, for a host that owns widths but not a whole grid state"],
  ["facet/FacetSourceInput", "Rust takes typed builders and never trusts a snapshot, so it has no separate input shape"],
  ["facet/dictionaryDomain", "a Rust domain is an enum variant with a payload; a tagged-union host needs a factory per variant"],
  ["facet/booleanDomain", "a Rust domain is an enum variant; a tagged-union host needs a factory per variant"],
  ["facet/openDomain", "a Rust domain is an enum variant; a tagged-union host needs a factory per variant"],
  // Reading one key out of a route string is a host/router concern. The Rust surfaces receive typed
  // server-function parameters and never parse a URL, so a Rust twin would be a stub nobody calls.
]);

// `url.ts` mirrors the shared UI-core helper rather than the grid crate, so it is asserted directly.
const URL_PAIR = {
  rustPath: "crates/ui/rustok-ui-core/src/ui.rs",
  rustName: "apply_ui_query_pairs",
  tsFile: "url.ts",
  tsName: "applyQueryPairs",
};

function fail(lines) {
  console.error(`ui-grid parity failed:\n\n${lines.map((line) => `  ${line}`).join("\n")}\n`);
  process.exit(1);
}

function read(root, file) {
  return readFileSync(path.join(root, file), "utf8");
}

const lines = [];
const rustSurface = new Map();
const tsSurface = new Map();

for (const [module, rustFile, tsFile] of MODULES) {
  for (const symbol of rustPublicSymbols(read(rustRoot, rustFile))) {
    rustSurface.set(`${module}/${symbol.name}`, { ...symbol, file: rustFile });
  }
  for (const symbol of tsPublicSymbols(read(tsRoot, tsFile))) {
    tsSurface.set(`${module}/${symbol.name}`, { ...symbol, file: tsFile });
  }
}

const rustNames = new Set([...rustSurface.values()].map((symbol) => symbol.name.split("::").pop()));
const tsNames = new Set([...tsSurface.keys()].map((key) => key.split("/")[1]));
const tsNameToKey = new Map([...tsSurface.keys()].map((key) => [key.split("/")[1], key]));

// Rust → TypeScript.
for (const [key, symbol] of rustSurface) {
  const bare = symbol.name.split("::").pop();
  const typeName = symbol.name.split("::")[0];
  const mapped =
    (TYPE_PAIRS.get(symbol.name) ?? null) ||
    (symbol.kind === "type" ? TYPE_PAIRS.get(typeName) ?? null : null);
  const tsName = mapped ?? PAIRS.get(key) ?? camelCase(bare);
  if (tsNames.has(tsName)) continue;
  if (RUST_ONLY.has(key)) continue;
  lines.push(
    `${symbol.file}: public Rust item \`${symbol.name}\` has no TypeScript twin (looked for \`${tsName}\`); add a reviewed PAIRS or RUST_ONLY row`,
  );
}

// TypeScript → Rust.
for (const [key, symbol] of tsSurface) {
  const name = symbol.name;
  // A type or constant is spelled identically on both sides; only functions and methods are
  // transformed, so the reverse direction tries both forms.
  if (rustNames.has(name) || rustNames.has(snakeCase(name))) continue;
  if ([...TYPE_PAIRS.values()].includes(name)) continue;
  if ([...PAIRS.entries()].some(([, tsName]) => tsName === name)) continue;
  if (TS_ONLY.has(key)) continue;
  lines.push(
    `${symbol.file}: exported TypeScript symbol \`${name}\` has no Rust twin; add a reviewed PAIRS or TS_ONLY row`,
  );
}

// The URL pair is asserted by name because its Rust side lives in another crate.
{
  const uiSource = read(repoRoot, URL_PAIR.rustPath);
  const urlSource = read(tsRoot, URL_PAIR.tsFile);
  if (!uiSource.includes(`pub fn ${URL_PAIR.rustName}`)) {
    lines.push(`${URL_PAIR.rustPath}: \`${URL_PAIR.rustName}\` disappeared, breaking the twin of url.ts`);
  }
  if (!urlSource.includes(`export function ${URL_PAIR.tsName}(`)) {
    lines.push(`${URL_PAIR.tsFile}: \`${URL_PAIR.tsName}\` disappeared, breaking the twin of ${URL_PAIR.rustName}`);
  }
  // `queryParam` is deliberately not answered in Rust: reading one key out of a route string is a
  // TypeScript host concern, because a Rust surface receives typed server-function parameters and
  // never parses a URL. It is locked here so removing it cannot pass unnoticed, and documented as
  // boundary-only in packages/rustok-ui-grid/README.md.
  if (!urlSource.includes("export function queryParam(")) {
    lines.push(`${URL_PAIR.tsFile}: \`queryParam\` disappeared while the Next hosts still read route keys with it`);
  }
  // Both sides must keep the fragment out of the last query value: the rule is stated on both
  // because a one-sided fix would silently make the two URL writers disagree again.
  for (const [label, source] of [
    [URL_PAIR.rustPath, uiSource],
    [URL_PAIR.tsFile, urlSource],
  ]) {
    if (!/fragment/i.test(source)) {
      lines.push(
        `${label}: no longer documents fragment handling, which ${URL_PAIR.tsName} promises to both sides`,
      );
    }
  }
}

// A stale row is a finding: exceptions and pairs must describe the surface as it is now.
for (const [key, tsName] of PAIRS) {
  if (!rustSurface.has(key)) {
    lines.push(`PAIRS row \`${key}\` names a Rust item that no longer exists`);
  }
  if (!tsNameToKey.has(tsName)) {
    lines.push(`PAIRS row ${key} → \`${tsName}\`: no TypeScript module exports that symbol`);
  }
}
for (const [key, reason] of RUST_ONLY) {
  if (!rustSurface.has(key)) {
    lines.push(`RUST_ONLY row \`${key}\` names a Rust item that no longer exists`);
  }
  if (typeof reason !== "string" || reason.trim().length < 30) {
    lines.push(`RUST_ONLY row \`${key}\` needs a reason that says why the absence is correct`);
  }
}
for (const [key, reason] of TS_ONLY) {
  if (!tsSurface.has(key)) {
    lines.push(`TS_ONLY row \`${key}\` names a TypeScript export that no longer exists`);
  }
  if (typeof reason !== "string" || reason.trim().length < 30) {
    lines.push(`TS_ONLY row \`${key}\` needs a reason that says why the absence is correct`);
  }
}
// An exception that is *also* paired, or a pair that points at an exception, would let a gap hide
// behind two rows.
for (const key of RUST_ONLY.keys()) {
  if (PAIRS.has(key)) lines.push(`${key} is both paired and Rust-only; pick one`);
}
for (const key of TS_ONLY.keys()) {
  if ([...PAIRS.values()].includes(key.split("/")[1])) {
    lines.push(`${key} is both paired and TypeScript-only; pick one`);
  }
}

if (lines.length) fail(lines);

console.log(
  `ui-grid parity passed: ${MODULES.length} module twins, ${rustSurface.size} Rust items, ` +
    `${tsSurface.size} TypeScript exports, ${PAIRS.size} reviewed pairs, ` +
    `${RUST_ONLY.size + TS_ONLY.size} justified one-sided symbols`,
);
