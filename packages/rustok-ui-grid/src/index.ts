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
 * Framework-free table facet toolkit: the TypeScript twin of `crates/ui/rustok-grid`.
 *
 * Table state that an owner counts (filter facets) and table state that lives in the route (query
 * pairs) are both framework-independent, so they live here instead of inside one surface:
 *
 *   - `facet.ts`  — the facet contract, the domain rules and the `code=value` selection vocabulary;
 *   - `panel.ts`  — the panel view model every adapter renders (counts, markers, hints, clear);
 *   - `url.ts`    — route query editing with the same contract as `rustok_ui_core`.
 *
 * Consumers: the Next storefront product catalog re-exports the selection vocabulary and maps its
 * owner answer into a panel; the Next admin data-table host uses the same transitions for its
 * faceted filters. Nothing here imports React, Next or a fetch client.
 */

export {
  MAX_GRID_FACETS,
  MAX_GRID_FACET_VALUES,
  SELECTION_LIST_SEPARATOR,
  SELECTION_SEPARATOR,
  booleanDomain,
  dictionaryDomain,
  facetDomainIsEnumerable,
  facetFilterOptions,
  facetFromBuckets,
  hasSelectionForKey,
  isSelectionSelected,
  openDomain,
  openFacet,
  parseSelection,
  selectionAfterClear,
  selectionAfterClearKey,
  selectionAfterToggle,
  selectionEntry,
  selectionExcept,
  selectionForKey,
  selectionKey,
  serializeSelection,
  splitSelection,
  type FacetBucket,
  type FacetDomain,
  type FacetSource
} from "./facet";

export {
  ENGLISH_FACET_PANEL_LABELS,
  buildFacetPanel,
  clearFacetPanelSelection,
  clearFacetSelection,
  countLabel,
  isFacetValueSelected,
  toggleFacetSelection,
  type FacetPanelFacetView,
  type FacetPanelLabels,
  type FacetPanelValueView,
  type FacetPanelView
} from "./panel";

export { applyQueryPairs, queryParam, type QueryPair } from "./url";
