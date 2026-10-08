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
 * Storefront facet panel semantics for the Next storefront.
 *
 * The panel is a pure function of the owner's facet answer and the current route state: every
 * bucket is a link that toggles exactly one `code=value` attribute-filter selection, so selection
 * round-trips through the URL (deep links stay shareable) and the component keeps no client-side
 * state. The module is framework-free on purpose — the same rules back the Rust storefront
 * (`rustok-product-storefront`: `catalog_controls`, `core`) and this file mirrors them:
 *
 *   - `attribute_filters` is one route value carrying `code=value` entries joined by `;`;
 *   - a facet's own selections are ignored by the owner while it counts that facet, so the panel
 *     never filters out its own active bucket (drill-down counts);
 *   - unbounded domains (text, numeric, date) stay non-enumerable: the panel shows the hint and
 *     keeps the free-form filter input instead of inventing buckets;
 *   - a truncated bucket list is reported as such rather than silently cut.
 */

import type {
  ProductCatalogFacet,
  ProductCatalogSearchOptions,
} from "../api/types";

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
export type CatalogFacetLabels = {
  title: string;
  /** Sub-label of an unbounded facet (text, numeric, date). */
  unboundedHint: string;
  /** Shown under a facet whose bucket list the owner cut. */
  truncatedHint: string;
  clearLabel: string;
  emptyMessage: string;
  /** Bucket count template, e.g. `({count})`. */
  countTemplate: string;
  selectedMarker: string;
  unselectedMarker: string;
};

/** One bucket rendered as a toggle link. */
export type CatalogFacetValueView = {
  value: string;
  label: string;
  countLabel: string;
  selected: boolean;
  marker: string;
  href: string;
};

export type CatalogFacetView = {
  code: string;
  label: string;
  isEnumerable: boolean;
  unboundedHint?: string;
  isTruncated: boolean;
  truncatedHint?: string;
  /** Drops this facet's selections; absent while nothing of it is selected. */
  clearHref?: string;
  clearLabel: string;
  values: CatalogFacetValueView[];
};

export type CatalogFacetFiltersView = {
  title: string;
  facets: CatalogFacetView[];
  showEmptyState: boolean;
  emptyMessage: string;
  /** Drops every attribute-filter selection; absent when nothing is selected. */
  clearHref?: string;
  clearLabel: string;
};

/** The key part of one `code=value` entry, or `null` when the entry addresses nothing. */
export function attributeFilterCode(entry: string): string | null {
  const separator = entry.indexOf("=");
  if (separator < 0) return null;
  const code = entry.slice(0, separator).trim();
  return code.length > 0 ? code : null;
}

/** Splits one `code=value` entry into its parts; both must be non-empty after trimming. */
export function parseAttributeFilter(
  entry: string,
): { code: string; value: string } | null {
  const separator = entry.indexOf("=");
  if (separator < 0) return null;
  const code = entry.slice(0, separator).trim();
  const value = entry.slice(separator + 1).trim();
  if (code.length === 0 || value.length === 0) return null;
  return { code, value };
}

/** Reads the semicolon-joined route value into attribute-filter entries. */
export function parseAttributeFilters(value?: string | null): string[] {
  const normalized = (value ?? "").trim();
  if (normalized.length === 0) return [];
  return normalized
    .split(";")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

/** Serializes attribute-filter entries back into the single route value. */
export function serializeAttributeFilters(filters: readonly string[]): string {
  return filters.join(";");
}

/** True when `code=value` is already part of the active selection. */
export function isAttributeFilterSelected(
  filters: readonly string[],
  code: string,
  value: string,
): boolean {
  const wantedCode = code.trim();
  const wantedValue = value.trim();
  return filters.some((entry) => {
    const parsed = parseAttributeFilter(entry);
    return (
      parsed !== null &&
      parsed.code === wantedCode &&
      parsed.value === wantedValue
    );
  });
}

/** True when `code=<any value>` is part of the active selection. */
export function hasAttributeFilterForCode(
  filters: readonly string[],
  code: string,
): boolean {
  const wantedCode = code.trim();
  return filters.some((entry) => attributeFilterCode(entry) === wantedCode);
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
  const wantedCode = code.trim();
  const wantedValue = value.trim();
  if (wantedCode.length === 0 || wantedValue.length === 0) return [...filters];
  const selection = `${wantedCode}=${wantedValue}`;
  const toggled: string[] = [];
  let removed = false;
  for (const entry of filters) {
    const parsed = parseAttributeFilter(entry);
    if (
      !removed &&
      parsed !== null &&
      parsed.code === wantedCode &&
      parsed.value === wantedValue
    ) {
      removed = true;
      continue;
    }
    toggled.push(entry);
  }
  if (!removed) toggled.push(selection);
  return toggled;
}

/** Drops every selection that belongs to `code`, keeping the other facets' selections. */
export function clearAttributeFilterCode(
  filters: readonly string[],
  code: string,
): string[] {
  const wantedCode = code.trim();
  return filters.filter((entry) => attributeFilterCode(entry) !== wantedCode);
}

/** Renders the bucket count from the panel template. */
export function countLabel(template: string, count: number): string {
  return template.replace("{count}", String(count));
}

/**
 * Applies `key=value` pairs to a route path with an optional query string.
 *
 * An existing key keeps its position (stable links), an empty or blank value removes the key, an
 * unknown key is appended, and values are encoded with browser form rules — the same contract as
 * `rustok_ui_core::apply_ui_query_pairs` used by the Leptos storefront.
 */
export function applyQueryPairs(
  base: string,
  pairs: ReadonlyArray<readonly [string, string | null | undefined]>,
): string {
  const separator = base.indexOf("?");
  const path = separator >= 0 ? base.slice(0, separator) : base;
  const query = separator >= 0 ? base.slice(separator + 1) : "";
  const merged: Array<[string, string]> = [];
  for (const [key, value] of new URLSearchParams(query).entries()) {
    merged.push([key, value]);
  }
  for (const [key, rawValue] of pairs) {
    const nextValue =
      rawValue === null || rawValue === undefined ? "" : rawValue.trim();
    const index = merged.findIndex(([existingKey]) => existingKey === key);
    if (index >= 0) {
      if (nextValue.length > 0) {
        merged[index][1] = nextValue;
      } else {
        merged.splice(index, 1);
      }
    } else if (nextValue.length > 0) {
      merged.push([key, nextValue]);
    }
  }
  if (merged.length === 0) return path;
  const params = new URLSearchParams();
  for (const [key, value] of merged) params.append(key, value);
  return `${path}?${params.toString()}`;
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
    ["attribute_filters", serialized.length > 0 ? serialized : null],
    ["currency", controls.currencyCode],
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
        unselectedMarker: "[ ]",
      }
    : {
        title: "Filters",
        unboundedHint: "Enter a value in the filter field above.",
        truncatedHint: "More values are available than shown.",
        clearLabel: "Clear filters",
        emptyMessage: "No filters are available for this catalog yet.",
        countTemplate: "({count})",
        selectedMarker: "[x]",
        unselectedMarker: "[ ]",
      };
}

/**
 * Builds the whole panel: every facet with its buckets, selection state and toggle links.
 *
 * Selection always addresses the route, so the view model only carries data and hrefs — the
 * adapter decides how to render a marker, a count and a clear action.
 */
export function buildCatalogFacetFiltersView(
  routeBase: string,
  facets: readonly ProductCatalogFacet[],
  controls: CatalogFacetControls,
  labels: CatalogFacetLabels,
): CatalogFacetFiltersView {
  const facetViews: CatalogFacetView[] = (facets ?? []).map((facet) => ({
    code: facet.code,
    label: facet.label,
    isEnumerable: facet.isEnumerable,
    unboundedHint: facet.isEnumerable ? undefined : labels.unboundedHint,
    isTruncated: facet.isTruncated,
    truncatedHint: facet.isTruncated ? labels.truncatedHint : undefined,
    clearHref: hasAttributeFilterForCode(controls.attributeFilters, facet.code)
      ? buildCatalogFacetClearCodeQuery(routeBase, controls, facet.code)
      : undefined,
    clearLabel: labels.clearLabel,
    values: (facet.values ?? []).map((bucket) => {
      const selected = isAttributeFilterSelected(
        controls.attributeFilters,
        facet.code,
        bucket.value,
      );
      return {
        value: bucket.value,
        label: bucket.label,
        countLabel: countLabel(labels.countTemplate, bucket.count),
        selected,
        marker: selected ? labels.selectedMarker : labels.unselectedMarker,
        href: buildCatalogFacetToggleQuery(
          routeBase,
          controls,
          facet.code,
          bucket.value,
        ),
      };
    }),
  }));

  return {
    title: labels.title,
    facets: facetViews,
    showEmptyState: facetViews.length === 0,
    emptyMessage: labels.emptyMessage,
    clearHref:
      controls.attributeFilters.length > 0
        ? buildCatalogFacetClearQuery(routeBase, controls)
        : undefined,
    clearLabel: labels.clearLabel,
  };
}
