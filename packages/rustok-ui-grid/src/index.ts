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
 * Framework-free table toolkit: the TypeScript twin of `crates/ui/rustok-grid`.
 *
 * Table state that an owner counts (filter facets), column models, pagination maths,
 * sorting state machines, column resizing and route query editing are all framework-independent:
 *
 *   - `column.ts`     — column definitions, alignment, widths, pinning;
 *   - `pagination.ts` — pagination state, page count arithmetic, range labels;
 *   - `sort.ts`       — sort directions, tri-state cycling, value comparators;
 *   - `filter.ts`     — filter types, normalized active conditions;
 *   - `selection.ts`  — immutable Set-based row selection;
 *   - `resize.ts`     — column width storage and pointer/keyboard resizing math;
 *   - `state.ts`      — composite grid state with invalidation invariants;
 *   - `facet.ts`      — facet contract, domain rules, selection vocabulary;
 *   - `panel.ts`      — panel view model (counts, markers, hints, clear);
 *   - `url.ts`        — route query editing matching `rustok_ui_core`.
 *
 * Nothing in this root entry imports React, Next or a fetch client.
 */

export {
  CHECKBOX_COLUMN_ID,
  clampColumnWidth,
  createActionsColumn,
  createCheckboxColumn,
  createColumnDef,
  createColumnWidth,
  fixedColumnWidth,
  hasColumnFilter,
  hasFilterRow,
  isCheckboxColumn,
  visibleColumnCount,
  visibleColumns,
  type ColumnAlign,
  type ColumnWidth,
  type GridColumnDef,
  type PinnedSide
} from "./column";

export {
  DEFAULT_PAGE_SIZE,
  calculateOffset,
  calculatePageCount,
  clampPage,
  createPagination,
  dataTableRangeLabel,
  fromIndex,
  hasNextPage,
  hasPreviousPage,
  nextPage,
  normalizePagination,
  paginateArray,
  previousPage,
  rangeLabel,
  setMode,
  setPage,
  setPageSize,
  setTotal,
  sliceBounds,
  toIndex,
  visibleLen,
  type GridPagination,
  type PaginationMode
} from "./pagination";

export {
  ariaSortFor,
  ariaSortValue,
  clearSort,
  compareValues,
  cycleSort,
  isSortActive,
  isSortedBy,
  normalizeSort,
  oppositeSortDirection,
  setSort,
  type SortDirection,
  type SortState
} from "./sort";

export {
  activeFilterCount,
  clearFilters,
  hasActiveFilters,
  isFilterActive,
  isFilterEqual,
  normalizeFilters,
  removeFilter,
  setFilter,
  type ColumnFilters,
  type FilterOption,
  type FilterValue,
  type GridFilterType
} from "./filter";

export {
  areAllRowsSelected,
  areSomeRowsSelected,
  clearSelection,
  deselectAllRows,
  deselectRow,
  emptyRowSelection,
  isAllSelected,
  isPartiallySelected,
  isRowSelected,
  isSelectionEmpty,
  retainSelectedRows,
  rowSelectionToArray,
  selectAllRows,
  selectRow,
  selectedCount,
  setSelectedRow,
  toggleRowSelection,
  type RowSelection
} from "./selection";

export {
  ColumnWidths,
  KEYBOARD_RESIZE_STEP,
  calculateResizedWidth,
  stepResizedWidth
} from "./resize";

export {
  clampWidths,
  createGridState,
  normalizeGridState,
  pruneGridSelection,
  resetGridFilters,
  setGridFilter,
  toggleGridSort,
  type GridState
} from "./state";

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
