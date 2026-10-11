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
 * Column models, resize math, normalized filters, sort and selection state, pagination and facets
 * all live here as pure data/functions. `react/` is a separate render adapter and is not imported by
 * this entry point.
 */

export {
  CHECKBOX_COLUMN_ID,
  MAX_COLUMN_WIDTH,
  clampColumnWidth,
  createActionsColumn,
  createCheckboxColumn,
  createDefaultColumnWidth,
  createFixedColumnWidth,
  createGridColumnDef,
  createResizableColumnWidth,
  hasFilter,
  hasFilterRow,
  hideColumn,
  pinColumn,
  isCheckboxColumn,
  isColumnWidthFlexible,
  normalizeColumnWidth,
  setColumnMaxWidth,
  setColumnMinWidth,
  setPreferredColumnWidth,
  unpinColumn,
  visibleColumnCount,
  visibleColumns,
  withColumnAlign,
  withColumnFilter,
  withColumnMaxWidth,
  withColumnMinWidth,
  withColumnNotFilterable,
  withColumnNotResizable,
  withColumnNotSortable,
  withColumnWidth,
  withColumnWidthRange,
  type ColumnAlign,
  type ColumnId,
  type ColumnWidth,
  type GridColumnDef,
  type PinnedSide,
} from './column';

export {
  DEFAULT_PAGE_SIZE,
  MAX_SAFE_GRID_INTEGER,
  calculateOffset,
  calculatePageCount,
  clampPage,
  createInfinitePagination,
  createPagination,
  dataTableRangeLabel,
  fromIndex,
  goToPage,
  hasNextPage,
  hasPreviousPage,
  nextPage,
  normalizePagination,
  previousPage,
  setPageSize,
  setPaginationMode,
  setTotal,
  sliceBounds,
  toIndex,
  totalPages,
  visibleLength,
  type GridPagination,
  type PaginationMode,
} from './pagination';

export {
  activeSort,
  ariaSortForColumn,
  ariaSortValue,
  clearSortState,
  createSortState,
  cycleSort,
  emptySortState,
  isSortActive,
  normalizeSortState,
  oppositeSortDirection,
  setSortState,
  sortDirectionForColumn,
  type SortDirection,
  type SortState,
} from './sort';

export {
  booleanFilter,
  clearColumnFilters,
  columnFilterCount,
  columnFilterEntries,
  columnFilterKeys,
  dateRangeFilter,
  emptyFilterValue,
  filterTypeAccepts,
  getColumnFilter,
  hasColumnFilterValue,
  isFilterActive,
  normalizeFilterValue,
  normalizeFilters,
  numberRangeFilter,
  removeColumnFilter,
  selectFilter,
  selectFilterWithPlaceholder,
  setColumnFilter,
  textFilter,
  textFilterWithPlaceholder,
  type ColumnFilters,
  type FilterOption,
  type FilterValue,
  type GridFilterType,
} from './filter';

export {
  addRowsToSelection,
  areAllRowsSelected,
  areSomeRowsSelected,
  clearRowSelection,
  deselectAllRows,
  deselectRow,
  emptyRowSelection,
  isRowSelected,
  isRowSelectionEmpty,
  retainRowIds,
  rowSelectionCount,
  rowSelectionIds,
  selectAllRows,
  selectRow,
  setRowSelected,
  toggleRowSelection,
  type RowSelection,
} from './selection';

export {
  MAX_GRID_FACETS,
  MAX_GRID_FACET_VALUES,
  MAX_SAFE_FACET_COUNT,
  SELECTION_LIST_SEPARATOR,
  SELECTION_SEPARATOR,
  booleanDomain,
  dictionaryDomain,
  facetDomainIsEnumerable,
  facetFilterOptions,
  facetFromBuckets,
  hasSelectionForKey,
  isSelectionSelected,
  normalizeFacetSource,
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
  type FacetSource,
} from './facet';

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
  type FacetPanelView,
} from './panel';

export {
  KEYBOARD_RESIZE_STEP,
  calculateResizedWidth,
  clampColumnWidths,
  clearColumnWidths,
  columnWidthEntries,
  columnWidthsAreEmpty,
  getClampedColumnWidth,
  getColumnWidth,
  getStoredColumnWidth,
  normalizeColumnWidths,
  removeColumnWidth,
  setClampedColumnWidth,
  setColumnWidth,
  stepResizedWidth,
  type ColumnWidths,
} from './resize';

export {
  clampGridWidths,
  createGridState,
  normalizeGridState,
  pruneGridSelection,
  resetGridFilters,
  setGridFilter,
  toggleGridSort,
  withGridPagination,
  type GridState,
} from './state';

export { applyQueryPairs, queryParam, type QueryPair } from './url';
