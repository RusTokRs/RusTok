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
 * Panel semantics for server-computed facets, the TypeScript twin of
 * `crates/ui/rustok-grid::facet_panel`.
 *
 * {@link ./facet} carries *what* an owner counted; this module carries *how a table panel shows it
 * and how a selection changes*: which bucket is active, what the count reads, which hint a bounded
 * or unbounded domain gets, and what the selection looks like after a toggle or a clear. Every
 * adapter — the Next storefront catalog, the Next admin tables, the Leptos grids through the Rust
 * original — renders the same numbers and mutates the same selection language.
 *
 * The copy is *not* owned here: an adapter passes {@link FacetPanelLabels}, because only the adapter
 * knows the locale. The panel also never builds a link: it reports the selection each action
 * produces, and the adapter decides whether that becomes an href, a router push or a filter update.
 */

import {
  MAX_GRID_FACETS,
  facetDomainIsEnumerable,
  hasSelectionForKey,
  isSelectionSelected,
  selectionAfterClear,
  selectionAfterClearKey,
  selectionAfterToggle,
  selectionEntry,
  type FacetSource,
} from "./facet";

/** Copy and markers an adapter supplies for one panel rendering. */
export type FacetPanelLabels = {
  title: string;
  /** Sub-label of one unbounded facet: the free-form input stays the UI. */
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

/** Labels with the English defaults the audit-traced panels use; adapters override per locale. */
export const ENGLISH_FACET_PANEL_LABELS: FacetPanelLabels = {
  title: "Filters",
  unboundedHint: "Enter a value in the filter field above.",
  truncatedHint: "More values are available than shown.",
  clearLabel: "Clear filters",
  emptyMessage: "No filters are available for this catalog yet.",
  countTemplate: "({count})",
  selectedMarker: "[x]",
  unselectedMarker: "[ ]",
};

/** Renders a bucket count through the adapter's template. */
export function countLabel(template: string, count: number): string {
  return template.replace("{count}", String(count));
}

/** One bucket rendered by the adapter. */
export type FacetPanelValueView = {
  /** Selection value of the bucket in the `key=<value>` vocabulary. */
  value: string;
  label: string;
  count: number;
  /** Count rendered through the panel template. */
  countLabel: string;
  selected: boolean;
  /** Checkbox-like marker the adapter prints in front of the label. */
  marker: string;
  /** The selection entry this bucket toggles, e.g. `color=blue`. */
  selectionEntry: string;
};

/** One facet of the panel with its bucket list and hints. */
export type FacetPanelFacetView = {
  code: string;
  label: string;
  isEnumerable: boolean;
  /** Set only for unbounded domains. */
  unboundedHint?: string;
  isTruncated: boolean;
  /** Set only for a truncated bucket list. */
  truncatedHint?: string;
  clearLabel: string;
  /** True while at least one bucket of this facet is selected. */
  hasSelection: boolean;
  /** Products matching every other active facet that carry a value for this facet. */
  total: number;
  values: FacetPanelValueView[];
};

/** A whole panel: the facets to render, the copy, and the selection they act on. */
export type FacetPanelView = {
  title: string;
  facets: FacetPanelFacetView[];
  clearLabel: string;
  emptyMessage: string;
  /** True when the owner answered no countable facet at all. */
  showEmptyState: boolean;
  /** Selection the panel renders and rewrites, in its original order. */
  selected: string[];
  /** True while any selection is active; drives the panel-wide clear action. */
  hasSelection: boolean;
};

/**
 * Builds a panel from the owner's facets, the active selection and the adapter's copy.
 *
 * At most {@link MAX_GRID_FACETS} facets are rendered, mirroring the request limit, so a panel can
 * never ask the UI to draw more than the grid is allowed to request.
 */
export function buildFacetPanel(
  facets: readonly FacetSource[],
  selected: readonly string[],
  labels: FacetPanelLabels,
): FacetPanelView {
  const panelFacets: FacetPanelFacetView[] = (facets ?? [])
    .slice(0, MAX_GRID_FACETS)
    .map((facet) => {
      const isEnumerable = facet.isEnumerable && facetDomainIsEnumerable(facet.domain);
      return {
        code: facet.code,
        label: facet.label,
        isEnumerable,
        unboundedHint: isEnumerable ? undefined : labels.unboundedHint,
        isTruncated: facet.isTruncated,
        truncatedHint: facet.isTruncated ? labels.truncatedHint : undefined,
        clearLabel: labels.clearLabel,
        hasSelection: hasSelectionForKey(selected, facet.code),
        total: facet.total,
        values: (facet.values ?? []).map((bucket) => {
          const bucketSelected = isSelectionSelected(
            selected,
            facet.code,
            bucket.value,
          );
          return {
            value: bucket.value,
            label: bucket.label,
            count: bucket.count,
            countLabel: countLabel(labels.countTemplate, bucket.count),
            selected: bucketSelected,
            marker: bucketSelected
              ? labels.selectedMarker
              : labels.unselectedMarker,
            selectionEntry: selectionEntry(facet.code, bucket.value),
          };
        }),
      };
    });

  return {
    title: labels.title,
    facets: panelFacets,
    clearLabel: labels.clearLabel,
    emptyMessage: labels.emptyMessage,
    showEmptyState: panelFacets.length === 0,
    selected: [...(selected ?? [])],
    hasSelection: (selected ?? []).length > 0,
  };
}

/** Selection of this panel after `code=value` is flipped. */
export function toggleFacetSelection(
  panel: FacetPanelView,
  code: string,
  value: string,
): string[] {
  return selectionAfterToggle(panel.selected, code, value);
}

/** Selection of this panel without any selection of `code`. */
export function clearFacetSelection(
  panel: FacetPanelView,
  code: string,
): string[] {
  return selectionAfterClearKey(panel.selected, code);
}

/** Selection of this panel without any facet selection. */
export function clearFacetPanelSelection(panel: FacetPanelView): string[] {
  return selectionAfterClear();
}

/** True when `code=value` is active in this panel. */
export function isFacetValueSelected(
  panel: FacetPanelView,
  code: string,
  value: string,
): boolean {
  return isSelectionSelected(panel.selected, code, value);
}
