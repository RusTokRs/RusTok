#!/usr/bin/env node
/**
 * Source gate for the shared table facet toolkit.
 *
 * Facet counting is owner-side, panel semantics are table-side, and an adapter only maps and
 * renders. That split is what makes the same counted buckets usable by the Next storefront, the
 * Next admin data-table host and the Leptos grids. This verifier locks the split on both language
 * sides: the TypeScript toolkit exists as a host package and stays framework-free, its constants
 * match the Rust original, both Next hosts are wired to it, the product surfaces delegate instead of
 * keeping private copies, and the Rust storefront consumes `rustok-grid::facet_panel` rather than its
 * own wave-seven duplicate.
 */

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required shared facet file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireAll(source, markers, description) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
  }
}

function reject(source, markers, description) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${description}: forbidden ${marker}`);
  }
}

const packageJson = read("packages/rustok-ui-grid/package.json");
const columnModule = read("packages/rustok-ui-grid/src/column.ts");
const paginationModule = read("packages/rustok-ui-grid/src/pagination.ts");
const sortModule = read("packages/rustok-ui-grid/src/sort.ts");
const filterModule = read("packages/rustok-ui-grid/src/filter.ts");
const selectionModule = read("packages/rustok-ui-grid/src/selection.ts");
const facetModule = read("packages/rustok-ui-grid/src/facet.ts");
const panelModule = read("packages/rustok-ui-grid/src/panel.ts");
const resizeModule = read("packages/rustok-ui-grid/src/resize.ts");
const stateModule = read("packages/rustok-ui-grid/src/state.ts");
const urlModule = read("packages/rustok-ui-grid/src/url.ts");
const indexModule = read("packages/rustok-ui-grid/src/index.ts");
const readme = read("packages/rustok-ui-grid/README.md");

const rustColumn = read("crates/ui/rustok-grid/src/column.rs");
const rustPagination = read("crates/ui/rustok-grid/src/pagination.rs");
const rustSort = read("crates/ui/rustok-grid/src/sort.rs");
const rustFilter = read("crates/ui/rustok-grid/src/filter.rs");
const rustSelection = read("crates/ui/rustok-grid/src/selection.rs");
const rustFacet = read("crates/ui/rustok-grid/src/facet.rs");
const rustPanel = read("crates/ui/rustok-grid/src/facet_panel.rs");
const rustResize = read("crates/ui/rustok-grid/src/resize.rs");
const rustState = read("crates/ui/rustok-grid/src/state.rs");
const rustGridLib = read("crates/ui/rustok-grid/src/lib.rs");
const leptosPagination = read("crates/ui/rustok-grid/leptos/src/pagination.rs");
const dioxusPagination = read("crates/ui/rustok-grid/dioxus/src/pagination.rs");

const storefrontCargo = read("crates/modules/rustok-product/storefront/Cargo.toml");
const storefrontControls = read(
  "crates/modules/rustok-product/storefront/src/catalog_controls.rs",
);
const storefrontCore = read("crates/modules/rustok-product/storefront/src/core.rs");
const adminFacets = read("crates/modules/rustok-product/admin/src/facets.rs");
const adminControls = read("crates/modules/rustok-product/admin/src/catalog_controls.rs");

const productFacets = read(
  "apps/next-frontend/packages/rustok-product/src/catalog/facets.ts",
);
const adminFacetFilter = read(
  "apps/next-admin/src/widgets/data-table/data-table-faceted-filter.tsx",
);
const frontendPackageJson = read("apps/next-frontend/package.json");
const adminPackageJson = read("apps/next-admin/package.json");
const frontendTsconfig = read("apps/next-frontend/tsconfig.json");
const adminTsconfig = read("apps/next-admin/tsconfig.json");
const frontendNextConfig = read("apps/next-frontend/next.config.mjs");
const adminNextConfig = read("apps/next-admin/next.config.mjs");

// ── The host package itself ────────────────────────────────────────────────
requireAll(
  packageJson,
  [
    '"name": "@rustok/ui-grid"',
    '"private": true',
    '"main": "./src/index.ts"',
    '"./facet": "./src/facet.ts"',
    '"./panel": "./src/panel.ts"',
    '"./url": "./src/url.ts"',
    '"./resize": "./src/resize.ts"',
    '"./state": "./src/state.ts"',
  ],
  "shared ui-grid package",
);
requireAll(
  readme,
  ["crates/ui/rustok-grid", "rustok-grid::facet_panel", "apply_ui_query_pairs"],
  "shared ui-grid readme",
);

// ── Framework-free on purpose ──────────────────────────────────────────────
reject(
  [
    columnModule,
    paginationModule,
    sortModule,
    filterModule,
    selectionModule,
    facetModule,
    panelModule,
    resizeModule,
    stateModule,
    urlModule,
    indexModule,
  ].join("\n"),
  ['from "react"', 'from "next', "useState", "useEffect", "fetch(", "use client"],
  "shared ui-grid must stay framework-free",
);

// ── Core state-machine surfaces exist on both language sides ──────────────
requireAll(
  columnModule,
  [
    "export function hasFilter(",
    "export function hasFilterRow(",
    "export function withColumnWidth(",
    "export function withColumnWidthRange(",
    "export function withColumnFilter(",
    "export function pinColumn(",
  ],
  "TypeScript column contract",
);
requireAll(
  paginationModule,
  [
    "export function createInfinitePagination(",
    "export function calculatePageCount(",
    "export function fromIndex(",
    "export function sliceBounds(",
    "export function setPageSize(",
    "export function setPaginationMode(",
  ],
  "TypeScript pagination contract",
);
requireAll(
  sortModule,
  [
    "export function normalizeSortState(",
    "export function cycleSort(",
  ],
  "TypeScript sort contract",
);
requireAll(
  filterModule,
  [
    "export function normalizeFilters(",
    "export function getColumnFilter(",
    "export function hasColumnFilterValue(",
    "export function columnFilterEntries(",
    "export function setColumnFilter(",
  ],
  "TypeScript filter contract",
);
requireAll(
  selectionModule,
  [
    "export function addRowsToSelection(",
    "export function deselectAllRows(",
    "export function areAllRowsSelected(",
    "export function areSomeRowsSelected(",
  ],
  "TypeScript row-selection contract",
);
requireAll(
  rustSelection,
  [
    "pub fn select_all(",
    "pub fn deselect_all(",
    "pub fn is_all_selected<",
    "pub fn is_partially_selected<",
  ],
  "Rust row-selection contract",
);
requireAll(
  resizeModule,
  [
    "export type ColumnWidths",
    "export function getClampedColumnWidth(",
    "export function setClampedColumnWidth(",
    "export function calculateResizedWidth(",
    "export function stepResizedWidth(",
  ],
  "TypeScript resize contract",
);
requireAll(
  stateModule,
  [
    "export interface GridState",
    "export function normalizeGridState(",
    "export function setGridFilter(",
    "export function toggleGridSort(",
    "export function pruneGridSelection(",
  ],
  "TypeScript composite-state contract",
);
requireAll(
  rustColumn,
  ["pub fn has_filter(&self)", "pub fn has_filter_row(", "impl<'de> Deserialize<'de> for ColumnWidth"],
  "Rust column contract",
);
requireAll(
  rustPagination,
  [
    "pub fn total_pages(&self)",
    "pub fn slice_bounds(&self",
    "pub fn set_mode(&mut self",
    "impl<'de> Deserialize<'de> for GridPagination",
  ],
  "Rust pagination contract",
);
requireAll(
  rustSort,
  ["pub fn normalize(&mut self)", "pub fn toggle(&mut self", "pub fn set(&mut self"],
  "Rust sort normalization contract",
);
requireAll(
  rustFilter,
  ["pub fn normalize(&mut self)", "impl<'de> Deserialize<'de> for ColumnFilters"],
  "Rust filter normalization",
);
requireAll(
  rustResize,
  ["pub struct ColumnWidths", "pub fn calculate_resized_width(", "pub fn step_resized_width("],
  "Rust resize contract",
);
requireAll(
  rustState,
  ["pub struct GridState", "impl<'de> Deserialize<'de> for GridState", "pub fn normalize(&mut self)"],
  "Rust composite-state contract",
);
requireAll(
  leptosPagination,
  [
    'aria-label="Load more rows"',
    '"Load more"',
    "node_ref=sentinel_ref",
    "observer_on_load_more.run(())",
  ],
  "Leptos infinite pagination must support keyboard and observer loading",
);
requireAll(
  dioxusPagination,
  ["\"Load more\"", "onvisible:", "on_load_more.call(())"],
  "Dioxus infinite pagination must support keyboard and observer loading",
);

// ── Contract parity with the Rust original ────────────────────────────────
requireAll(
  facetModule,
  [
    "export const SELECTION_SEPARATOR = ",
    "export const SELECTION_LIST_SEPARATOR = ",
    "export const MAX_GRID_FACETS",
    "export const MAX_GRID_FACET_VALUES",
    "export type FacetDomain",
    "export function facetFromBuckets(",
    "export function facetFilterOptions(",
    "export function openFacet(",
    "export function selectionEntry(",
    "export function splitSelection(",
    "export function parseSelection(",
    "export function serializeSelection(",
    "export function isSelectionSelected(",
    "export function hasSelectionForKey(",
    "export function selectionForKey(",
    "export function selectionExcept(",
    "export function selectionAfterToggle(",
    "export function selectionAfterClearKey(",
    "export function selectionAfterClear(",
  ],
  "shared ui-grid facet contract",
);
requireAll(
  panelModule,
  [
    "export type FacetPanelLabels",
    "export const ENGLISH_FACET_PANEL_LABELS",
    "export function countLabel(",
    "export function buildFacetPanel(",
    "export function toggleFacetSelection(",
    "export function clearFacetSelection(",
    "export function clearFacetPanelSelection(",
  ],
  "shared ui-grid panel contract",
);
requireAll(
  urlModule,
  ["export function applyQueryPairs(", "export function queryParam(", "URLSearchParams"],
  "shared ui-grid route contract",
);
requireAll(
  indexModule,
  [
    "MAX_GRID_FACETS",
    "buildFacetPanel",
    "applyQueryPairs",
    "selectionAfterToggle",
    "calculateResizedWidth",
    "normalizeGridState",
    "hasFilter",
    "countLabel",
  ],
  "shared ui-grid public surface",
);

// The numeric limits must be the same number in both languages, not the same spelling.
const rustLimits = Object.fromEntries(
  [...rustFacet.matchAll(/pub const (MAX_GRID_FACETS|MAX_GRID_FACET_VALUES): usize = (\d+);/g)].map(
    (match) => [match[1], Number(match[2])],
  ),
);
for (const [name, value] of Object.entries(rustLimits)) {
  const tsName = name.replace(/^MAX_GRID_FACET/, "MAX_GRID_FACET").replace(/_VALUES$/, "_VALUES");
  if (!new RegExp(`export const ${tsName} = ${value};`).test(facetModule)) {
    failures.push(`shared ui-grid facet contract: ${tsName} must equal the Rust ${name} (${value})`);
  }
}
if (Object.keys(rustLimits).length === 0) {
  failures.push("rust facet contract: MAX_GRID_FACETS/MAX_GRID_FACET_VALUES not found");
}

// The Rust original must keep exporting the same vocabulary the twin mirrors.
requireAll(
  rustFacet,
  [
    "pub const SELECTION_LIST_SEPARATOR",
    "pub fn parse_selection(",
    "pub fn serialize_selection(",
    "impl<'de> Deserialize<'de> for GridFacet",
    "pub fn normalize(&mut self)",
  ],
  "rust facet normalization and route-selection contract",
);
requireAll(
  rustPanel,
  [
    "pub struct FacetPanelLabels",
    "pub fn build(",
    "pub fn count_label(",
    "pub fn selection_entry(",
    "pub fn split_selection(",
    "pub fn selection_after_toggle(",
    "pub fn selection_after_clear_key(",
    "pub fn selection_after_clear(",
    "pub fn is_selection_selected(",
    "pub fn has_selection_for_key(",
    "pub fn selection_for_key<",
  ],
  "rust facet panel contract",
);
requireAll(
  rustGridLib,
  ["pub mod facet_panel;", "SELECTION_LIST_SEPARATOR", "parse_selection", "serialize_selection"],
  "rust grid facade",
);

// ── Both Next hosts consume the package the same way as any workspace package ─
for (const [app, packageManifest, tsconfig, nextConfig] of [
  ["next-frontend", frontendPackageJson, frontendTsconfig, frontendNextConfig],
  ["next-admin", adminPackageJson, adminTsconfig, adminNextConfig],
]) {
  requireAll(
    packageManifest,
    ['"@rustok/ui-grid": "file:../../packages/rustok-ui-grid"'],
    `${app} dependency`,
  );
  requireAll(tsconfig, ["@rustok/ui-grid"], `${app} tsconfig paths`);
  requireAll(nextConfig, ["@rustok/ui-grid"], `${app} transpilePackages`);
}

// ── Next consumers delegate instead of re-implementing ────────────────────
requireAll(
  productFacets,
  [
    'from "@rustok/ui-grid"',
    "buildFacetPanel(",
    "facetFromBuckets(",
    "catalogFacetToSource(",
    "buildCatalogFacetFiltersView(",
    "CATALOG_ATTRIBUTE_FILTERS_PARAM",
  ],
  "storefront facet core",
);
reject(
  productFacets,
  ["URLSearchParams", "SELECTION_LIST_SEPARATOR", 'replace("{count}"'],
  "storefront facet core must delegate the shared vocabulary",
);
requireAll(
  adminFacetFilter,
  [
    "@rustok/ui-grid'",
    "selectionAfterToggle(",
    "selectionAfterClear(",
    "hasSelectionForKey(",
  ],
  "admin data-table facet filter",
);

// ── Rust consumers delegate to the table library ──────────────────────────
requireAll(storefrontCargo, ["rustok-grid.workspace = true"], "storefront dependency");
requireAll(
  storefrontControls,
  [
    "use rustok_grid::facet::{parse_selection, serialize_selection}",
    "parse_selection(value.as_str())",
    "serialize_selection(filters)",
    "use rustok_grid::facet_panel::",
    "split_selection(",
    "selection_after_toggle(",
    "selection_after_clear_key(",
    "has_selection_for_key(",
    "is_selection_selected(",
    "pub fn to_grid_labels(&self) -> FacetPanelLabels",
  ],
  "storefront facet controls",
);
requireAll(
  storefrontCore,
  [
    "use rustok_grid::facet_panel::{FacetPanel, count_label as grid_count_label}",
    "use rustok_grid::{FacetDomain, FacetValue, GridFacet}",
    "fn catalog_facet_to_grid(",
    "FacetPanel::build(",
    "grid_count_label(template, total)",
    "mapped.is_truncated |= facet.is_truncated",
  ],
  "storefront facet view model",
);
reject(
  storefrontControls,
  ["split_once('=')", 'format!("{code}={value}")', 'filters.join(";")', ".split(';')"],
  "storefront facet controls must not re-implement the selection vocabulary",
);
requireAll(
  adminControls,
  ["serialize_selection(filters)", "parse_selection(value.as_str())"],
  "admin selection-list serialization",
);
reject(
  adminControls,
  ['filters.join(";")', ".split(';')"],
  "admin controls must delegate selection-list encoding to rustok-grid",
);
reject(
  storefrontCore,
  ['template.replace("{count}"', "selection_after_toggle("],
  "storefront facet view model must delegate instead of re-implementing",
);
requireAll(
  adminFacets,
  ["FacetPanel::build(", "mapped.is_truncated |= facet.is_truncated"],
  "admin facet mapping",
);

// The count template is substituted in exactly one place per language.
const countSubstitutions = [
  ["crates/modules/rustok-product/admin/src/facets.rs", adminFacets],
  ["crates/modules/rustok-product/storefront/src/core.rs", storefrontCore],
  ["crates/modules/rustok-product/storefront/src/catalog_controls.rs", storefrontControls],
].filter(([, source]) => source.includes('replace("{count}"'));
if (countSubstitutions.length > 0) {
  failures.push(
    `product surfaces must render counts through the shared panel: ${countSubstitutions
      .map(([file]) => file)
      .join(", ")} substitutes {count} itself`,
  );
}

if (failures.length > 0) {
  console.error("shared ui-grid facet verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("shared ui-grid facet verification passed");
