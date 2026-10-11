/**
 * Composite grid state and cross-field transitions — the TypeScript twin of
 * `crates/ui/rustok-grid/src/state.rs`.
 *
 * State changes are immutable so adapters can feed the result into React, a store or route state
 * without moving validation and pagination policy into a component.
 */

import {
  type GridColumnDef,
} from './column';
import {
  type ColumnFilters,
  clearColumnFilters,
  normalizeFilters,
  setColumnFilter,
} from './filter';
import {
  type GridPagination,
  createPagination,
  goToPage,
  normalizePagination,
} from './pagination';
import {
  type ColumnWidths,
  clampColumnWidths,
  normalizeColumnWidths,
} from './resize';
import {
  type RowSelection,
  emptyRowSelection,
  retainRowIds,
} from './selection';
import {
  type SortDirection,
  type SortState,
  cycleSort,
  emptySortState,
  normalizeSortState,
} from './sort';

/** Whole interaction state for one grid instance. */
export interface GridState {
  columnWidths: ColumnWidths;
  filters: ColumnFilters;
  sort: SortState;
  selection: RowSelection;
  pagination: GridPagination;
}

/** Create the same initial state as `GridState::new`. */
export function createGridState(options?: { pagination?: GridPagination }): GridState {
  return {
    columnWidths: {},
    filters: {},
    sort: emptySortState(),
    selection: emptyRowSelection(),
    pagination: options?.pagination ?? createPagination(),
  };
}

/** Create a grid state with explicit page metadata, matching `GridState::with_pagination`. */
export function withGridPagination(
  state: GridState,
  page: number,
  pageSize: number,
  total: number,
): GridState {
  return { ...state, pagination: createPagination(page, pageSize, total) };
}

/**
 * Repair all state that can be normalized without column definitions. Pass the current columns to
 * also clamp persisted width overrides against their current constraints.
 */
export function normalizeGridState(
  state: Partial<GridState> = {},
  columns?: readonly GridColumnDef[],
): GridState {
  const normalized: GridState = {
    columnWidths: normalizeColumnWidths(state.columnWidths ?? {}),
    filters: normalizeFilters(state.filters ?? {}),
    sort: normalizeSortState(state.sort ?? emptySortState()),
    selection: new Set(state.selection ?? []),
    pagination: normalizePagination(state.pagination ?? {}),
  };
  if (columns !== undefined) {
    normalized.columnWidths = clampColumnWidths(normalized.columnWidths, columns);
  }
  return normalized;
}

/** Apply a filter mutation, resetting pagination only when canonical filter state changes. */
export function setGridFilter(
  state: GridState,
  columnId: string,
  value: unknown,
): { state: GridState; changed: boolean } {
  const update = setColumnFilter(state.filters, columnId, value);
  if (!update.changed) return { state, changed: false };
  return {
    state: {
      ...state,
      filters: update.filters,
      pagination: goToPage(state.pagination, 1),
    },
    changed: true,
  };
}

/** Clear every filter, resetting pagination only when there was an active filter. */
export function resetGridFilters(state: GridState): { state: GridState; changed: boolean } {
  const update = clearColumnFilters(state.filters);
  if (!update.changed) return { state, changed: false };
  return {
    state: {
      ...state,
      filters: update.filters,
      pagination: goToPage(state.pagination, 1),
    },
    changed: true,
  };
}

/** Toggle sorting on a column and reset pagination. */
export function toggleGridSort(
  state: GridState,
  columnId: string,
): { state: GridState; direction: SortDirection | null; nextDirection: SortDirection | null } {
  const next = cycleSort(state.sort, columnId);
  const sort = next ?? emptySortState();
  return {
    state: {
      ...state,
      sort,
      pagination: goToPage(state.pagination, 1),
    },
    direction: sort.direction,
    nextDirection: sort.direction,
  };
}

/** Drop selected rows that are no longer part of the visible data set. */
export function pruneGridSelection(state: GridState, visibleIds: Iterable<string>): GridState {
  return { ...state, selection: retainRowIds(state.selection, visibleIds) };
}

/** Clamp persisted widths to the current column definitions. */
export function clampGridWidths(state: GridState, columns: readonly GridColumnDef[]): GridState {
  return { ...state, columnWidths: clampColumnWidths(state.columnWidths, columns) };
}

/** Compatibility alias for clampGridWidths. */
export const clampWidths = clampGridWidths;
