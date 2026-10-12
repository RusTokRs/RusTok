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
  type CellAccessor,
  filterRows,
  sortRows,
} from './cell';
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
  sliceItems,
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

/**
 * Toggle sorting on a column and reset pagination.
 *
 * `direction` is the direction the column ends up in, or `null` when the toggle cleared the sort —
 * the same answer `GridState::toggle_sort` gives its adapter.
 */
export function toggleGridSort(
  state: GridState,
  columnId: string,
): { state: GridState; direction: SortDirection | null } {
  const sort = cycleSort(state.sort, columnId);
  return {
    state: {
      ...state,
      sort,
      pagination: goToPage(state.pagination, 1),
    },
    direction: sort.direction,
  };
}

/** Drop selected rows that are no longer part of the visible data set. */
export function pruneGridSelection(state: GridState, visibleIds: Iterable<string>): GridState {
  return { ...state, selection: retainRowIds(state.selection, visibleIds) };
}

/** Clamp persisted widths to the current column definitions, dropping removed columns. */
export function clampGridWidths(state: GridState, columns: readonly GridColumnDef[]): GridState {
  return { ...state, columnWidths: clampColumnWidths(state.columnWidths, columns) };
}

/**
 * Turn a full data set into the rows this state renders: filter, then sort, then page.
 *
 * This is the client-side path a grid takes when the owner hands over all rows instead of a
 * server-projected page. The order is part of the contract — paging before sorting would sort a
 * single page, and sorting before filtering would sort rows that are about to be dropped — and it
 * is the order `GridState::apply_rows` uses.
 */
export function applyGridRows<T>(
  state: GridState,
  items: readonly T[],
  cellOf: CellAccessor<T>,
): T[] {
  const filtered = filterRows(state.filters, items, cellOf);
  const sorted = sortRows(state.sort, filtered, cellOf);
  return sliceItems(sorted, state.pagination);
}
