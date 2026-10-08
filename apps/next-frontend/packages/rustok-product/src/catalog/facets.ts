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
 * Catalog facet panel semantics for the Next storefront.
 *
 * The panel is a pure function of the owner's facet answer and the current route state: every
 * bucket is a link that toggles exactly one `code=value` attribute-filter selection, so selection
 * round-trips through the URL (deep links stay shareable) and the component keeps no client-side
 * state.
 *
 * Everything that is not product-specific lives in the shared table toolkit `@rustok/ui-grid`
 * (the TypeScript twin of `crates/ui/rustok-grid`) — the `code=value` selection vocabulary, the
 * domain rules and the panel view model. What stays here is what only this catalog knows: the route
 * parameter name (`attribute_filters`), the product route pairs, the FTL copy and the facet codes
 * derived from the catalog search options. The rules back the Rust storefront
 * (`rustok-product-storefront`: `catalog_controls`, `core`) and mirror them:
 *
 *   - `attribute_filters` is one route value carrying `code=value` entries joined by `;`;
 *   - a facet's own selections are ignored by the owner while it counts that facet, so the panel
 *     never filters out its own active bucket (drill-down counts);
 *   - unbounded domains (text, numeric, date) stay non-enumerable: the panel shows the hint and
 *     keeps the free-form filter input instead of inventing buckets;
 *   - a truncated bucket list is reported as such rather than silently cut.
 */

import {
  applyQueryPairs,
  buildFacetPanel,
  countLabel,
  facetFromBuckets,
  facetDomainIsEnumerable,
  hasSelectionForKey,
  isSelectionSelected,
  parseSelection,
  selectionAfterClear,
  selectionAfterClearKey,
  selectionAfterToggle,
  serializeSelection,
  splitSelection,
  type FacetDomain,
  type FacetPanelFacetView,
  type FacetPanelLabels,
  type FacetPanelValueView,
  type FacetPanelView,
  type FacetSource
} from "@rustok/ui-grid";

import type {
  ProductCatalogFacet,
  ProductCatalogSearchOptions
} from "../api/types";

/** Route parameter carrying the `;`-joined `code=value` attribute-filter selection. */
export const CATALOG_ATTRIBUTE_FILTERS_PARAM = "attribute_filters";

/** Route state the panel reads and rewrites; mirrors the storefront catalog controls. */
export type CatalogFacetControls = {
  search?: string | null;
  categoryId?: string | null;
  sortBy?: string | null;
  sortDirection?: string | null;
  attributeFilters: readonly string[];
  currencyCode?: string | null;
};

/** Panel copy; the adapter renders it verbatim. */
export type CatalogFacetLabels = FacetPanelLabels;

/** One bucket rendered as a toggle link. */
export type CatalogFacetValueView = FacetPanelValueView & {
  /** Link that toggles exactly this selection. */
  href: string;
};

export type CatalogFacetView = Omit<FacetPanelFacetView, "values"> & {
  /** Drops this facet's selections; absent while nothing of it is selected. */
  clearHref?: string;
  values: CatalogFacetValueView[];
};

export type CatalogFacetFiltersView = Omit<FacetPanelView, "facets"> & {
  facets: CatalogFacetView[];
  /** Drops every attribute-filter selection; absent when nothing is selected. */
  clearHref?: string;
};

/** The key part of one `code=value` entry, or `null` when the entry addresses nothing. */
export function attributeFilterCode(entry: string): string | null {
  return splitSelection(entry)?.key ?? null;
}

/** Splits one `code=value` entry into its parts; both must be non-empty after trimming. */
export function parseAttributeFilter(
  entry: string,
): { code: string; value: string } | null {
  const parsed = splitSelection(entry);
  return parsed === null ? null : { code: parsed.key, value: parsed.value };
}

/** Reads the semicolon-joined route value into attribute-filter entries. */
export function parseAttributeFilters(value?: string | null): string[] {
  return parseSelection(value);
}

/** Serializes attribute-filter entries back into the single route value. */
export function serializeAttributeFilters(filters: readonly string[]): string {
  return serializeSelection(filters);
}

/** True when `code=value` is already part of the active selection. */
export function isAttributeFilterSelected(
  filters: readonly string[],
  code: string,
  value: string,
): boolean {
  return isSelectionSelected(filters, code, value);
}

/** True when `code=<any value>` is part of the active selection. */
export function hasAttributeFilterForCode(
  filters: readonly string[],
  code: string,
): boolean {
  return hasSelectionForKey(filters, code);
}

/**
 * Flips `code=value`: an active entry is removed, an unknown one is appended, and the order of
 * every surviving entry is preserved (stable links on repeated toggles).
 */
export function toggleAttributeFilter(
  filters: readonly string[],
  code: string,
  value: string,
): string[] {
  return selectionAfterToggle(filters, code, value);
}

/** Drops every selection that belongs to `code`, keeping the other facets' selections. */
export function clearAttributeFilterCode(
  filters: readonly string[],
  code: string,
): string[] {
  return selectionAfterClearKey(filters, code);
}

/**
 * Attribute codes the panel asks the owner to count, in catalog search-option order.
 *
 * The owner counts only the facets a client asks for, so the request is derived from the same
 * search options the storefront offers as filter inputs; blank codes and duplicates are dropped.
 */
export function buildCatalogFacetCodes(
  options: ProductCatalogSearchOptions,
): string[] {
  const codes: string[] = [];
  for (const option of options.attributeOptions ?? []) {
    const code = (option.value ?? "").trim();
    if (code.length === 0 || codes.includes(code)) continue;
    codes.push(code);
  }
  return codes;
}

/**
 * Maps one owner facet into the shared facet contract.
 *
 * The domain is derived from the owner's `isEnumerable` flag first and from the stored value type
 * second, so a bounded dictionary keeps its `multi` flag and an unbounded domain keeps its open
 * semantics even if a future value type arrives with a different name.
 */
export function catalogFacetToSource(
  facet: ProductCatalogFacet,
): FacetSource {
  const domain: FacetDomain = facet.isEnumerable
    ? facet.valueType?.toLowerCase() === "boolean"
      ? { kind: "boolean" }
      : {
          kind: "dictionary",
          multi: facet.valueType?.toLowerCase() === "multiselect"
        }
    : { kind: "open" };
  const source = facetFromBuckets({
    code: facet.code,
    label: facet.label,
    domain,
    total: facet.totalProducts ?? 0,
    values: facet.values ?? []
  });
  // Truncation is either the owner cutting its value limit or this mapper cutting ours; both mean
  // "there are more values than shown", and neither may be dropped silently.
  return {
    ...source,
    isTruncated: source.isTruncated || (facet.isTruncated ?? false)
  };
}

/** Query of the current catalog page without any attribute-filter selection. */
export function buildCatalogFacetClearQuery(
  routeBase: string,
  controls: CatalogFacetControls,
): string {
  return buildCatalogQueryWithFilters(routeBase, controls, []);
}

/** Query of the current catalog page without the selections of one facet. */
export function buildCatalogFacetClearCodeQuery(
  routeBase: string,
  controls: CatalogFacetControls,
  code: string,
): string {
  return buildCatalogQueryWithFilters(
    routeBase,
    controls,
    clearAttributeFilterCode(controls.attributeFilters, code),
  );
}

/** Query of the current catalog page with one facet selection flipped. */
export function buildCatalogFacetToggleQuery(
  routeBase: string,
  controls: CatalogFacetControls,
  code: string,
  value: string,
): string {
  return buildCatalogQueryWithFilters(
    routeBase,
    controls,
    toggleAttributeFilter(controls.attributeFilters, code, value),
  );
}

function buildCatalogQueryWithFilters(
  routeBase: string,
  controls: CatalogFacetControls,
  filters: readonly string[],
): string {
  const serialized = serializeAttributeFilters(filters);
  return applyQueryPairs(routeBase, [
    ["search", controls.search],
    ["category_id", controls.categoryId],
    ["sort_by", controls.sortBy],
    ["sort_direction", controls.sortDirection],
    [CATALOG_ATTRIBUTE_FILTERS_PARAM, serialized.length > 0 ? serialized : null],
    ["currency", controls.currencyCode]
  ]);
}

/** Copy of the storefront facet panel for the requested locale. */
export function buildCatalogFacetLabels(locale?: string | null): CatalogFacetLabels {
  const isRu = (locale ?? "").trim().toLowerCase().startsWith("ru");
  return isRu
    ? {
        title: "Фильтры",
        unboundedHint: "Укажите значение в поле фильтра выше.",
        truncatedHint: "Доступны и другие значения, кроме показанных.",
        clearLabel: "Очистить фильтры",
        emptyMessage: "Для этого каталога фильтры пока недоступны.",
        countTemplate: "({count})",
        selectedMarker: "[x]",
        unselectedMarker: "[ ]"
      }
    : {
        title: "Filters",
        unboundedHint: "Enter a value in the filter field above.",
        truncatedHint: "More values are available than shown.",
        clearLabel: "Clear filters",
        emptyMessage: "No filters are available for this catalog yet.",
        countTemplate: "({count})",
        selectedMarker: "[x]",
        unselectedMarker: "[ ]"
      };
}

/**
 * Builds the whole panel: every facet with its buckets, selection state and toggle links.
 *
 * Selection always addresses the route, so the shared panel reports the selection each action
 * produces and this builder turns it into the exact link of the current catalog page.
 */
export function buildCatalogFacetFiltersView(
  routeBase: string,
  facets: readonly ProductCatalogFacet[],
  controls: CatalogFacetControls,
  labels: CatalogFacetLabels,
): CatalogFacetFiltersView {
  const selection = controls.attributeFilters;
  const panel = buildFacetPanel(
    (facets ?? []).map(catalogFacetToSource),
    selection,
    labels,
  );

  const facetViews: CatalogFacetView[] = panel.facets.map((facet) => ({
    ...facet,
    clearHref: hasAttributeFilterForCode(selection, facet.code)
      ? buildCatalogFacetClearCodeQuery(routeBase, controls, facet.code)
      : undefined,
    values: facet.values.map((bucket) => ({
      ...bucket,
      href: buildCatalogFacetToggleQuery(
        routeBase,
        controls,
        facet.code,
        bucket.value,
      ),
    })),
  }));

  return {
    ...panel,
    facets: facetViews,
    clearHref:
      selection.length > 0
        ? buildCatalogFacetClearQuery(routeBase, controls)
        : undefined,
  };
}

// Re-exported so one import keeps serving the catalog surface; the implementations live in the
// shared table toolkit and are not duplicated here.
export {
  applyQueryPairs,
  countLabel,
  facetDomainIsEnumerable,
  selectionAfterClear,
  type FacetSource
};
