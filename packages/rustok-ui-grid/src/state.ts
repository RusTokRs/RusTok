/**
 * Composite grid state: the whole interaction model in one value —
 * the TypeScript twin of `crates/ui/rustok-grid/src/state.rs`.
 *
 * Framework-free: enforces invalidation invariants (filter or sort changes reset page to 1).
 */

import { type GridColumnDef } from './column';
import { type ColumnFilters, type FilterValue, isFilterActive, normalizeFilters } from './filter';
import { type GridPagination, createPagination } from './pagination';
import { ColumnWidths } from './resize';
import { type RowSelection, emptyRowSelection } from './selection';
import { type SortDirection, type SortState, cycleSort } from './sort';

/**
 * Composite state of a data grid.
 */
export interface GridState<T = unknown> {
  columnWidths: ColumnWidths;
  filters: ColumnFilters;
  sort: SortState | null;
  selection: RowSelection;
  pagination: GridPagination;
  columns?: GridColumnDef<T>[];
}

/**
 * Creates a fresh, default GridState.
 */
export function createGridState<T = unknown>(
  options?: Partial<Omit<GridState<T>, 'columnWidths'>> & {
    columnWidths?: Record<string, number> | Map<string, number> | ColumnWidths;
  }
): GridState<T> {
  const widths =
    options?.columnWidths instanceof ColumnWidths
      ? options.columnWidths
      : new ColumnWidths(options?.columnWidths);

  return {
    columnWidths: widths,
    filters: options?.filters ? normalizeFilters(options.filters) : {},
    sort: options?.sort ?? null,
    selection: options?.selection ?? emptyRowSelection(),
    pagination: options?.pagination ?? createPagination(),
    columns: options?.columns
  };
}

/**
 * Applies a filter change. Resetting page to 1 on any effective change
 * because the matching row set has changed.
 *
 * Returns `{ state, changed }`.
 */
export function setGridFilter<T>(
  state: GridState<T>,
  columnId: string,
  value: FilterValue
): { state: GridState<T>; changed: boolean } {
  const current = state.filters[columnId];
  if (current === value) {
    return { state, changed: false };
  }

  const nextFilters: ColumnFilters = { ...state.filters };
  if (isFilterActive(value)) {
    nextFilters[columnId] = value;
  } else {
    delete nextFilters[columnId];
  }

  return {
    state: {
      ...state,
      filters: nextFilters,
      pagination: {
        ...state.pagination,
        page: 1
      }
    },
    changed: true
  };
}

/**
 * Clears all active filters and resets pagination to page 1.
 */
export function resetGridFilters<T>(state: GridState<T>): {
  state: GridState<T>;
  changed: boolean;
} {
  if (Object.keys(state.filters).length === 0) {
    return { state, changed: false };
  }

  return {
    state: {
      ...state,
      filters: {},
      pagination: {
        ...state.pagination,
        page: 1
      }
    },
    changed: true
  };
}

/**
 * Cycles sort on a column and resets pagination to page 1.
 */
export function toggleGridSort<T>(
  state: GridState<T>,
  columnId: string
): { state: GridState<T>; nextDirection: SortDirection | null } {
  const nextSort = cycleSort(state.sort, columnId);
  return {
    state: {
      ...state,
      sort: nextSort,
      pagination: {
        ...state.pagination,
        page: 1
      }
    },
    nextDirection: nextSort ? nextSort.direction : null
  };
}

/**
 * Prunes row selection to retain only rows present in visibleIds.
 */
export function pruneGridSelection<T>(
  state: GridState<T>,
  visibleIds: Iterable<string>
): GridState<T> {
  const allowed = new Set(visibleIds);
  const nextSelection = new Set<string>();
  for (const id of state.selection) {
    if (allowed.has(id)) {
      nextSelection.add(id);
    }
  }
  return {
    ...state,
    selection: nextSelection
  };
}
