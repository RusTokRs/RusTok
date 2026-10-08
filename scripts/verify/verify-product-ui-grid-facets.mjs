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
const facetModule = read("packages/rustok-ui-grid/src/facet.ts");
const panelModule = read("packages/rustok-ui-grid/src/panel.ts");
const urlModule = read("packages/rustok-ui-grid/src/url.ts");
const indexModule = read("packages/rustok-ui-grid/src/index.ts");
const readme = read("packages/rustok-ui-grid/README.md");

const rustFacet = read("crates/ui/rustok-grid/src/facet.rs");
const rustPanel = read("crates/ui/rustok-grid/src/facet_panel.rs");
const rustGridLib = read("crates/ui/rustok-grid/src/lib.rs");

const storefrontCargo = read("crates/modules/rustok-product/storefront/Cargo.toml");
const storefrontControls = read(
  "crates/modules/rustok-product/storefront/src/catalog_controls.rs",
);
const storefrontCore = read("crates/modules/rustok-product/storefront/src/core.rs");
const adminFacets = read("crates/modules/rustok-product/admin/src/facets.rs");

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
  [facetModule, panelModule, urlModule, indexModule].join("\n"),
  ['from "react"', 'from "next', "useState", "useEffect", "fetch(", "use client"],
  "shared ui-grid must stay framework-free",
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
requireAll(rustGridLib, ["pub mod facet_panel;"], "rust grid facade");

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
  ["split_once('=')", 'format!("{code}={value}")'],
  "storefront facet controls must not re-implement the selection vocabulary",
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
