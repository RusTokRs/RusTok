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
 * Single-column sort state machine for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/sort.rs`.
 *
 * The core owns state transitions and ARIA values; actual ordering stays with the data owner (a
 * server-driven grid forwards this state) or with `sortRows` in `cell.ts` (a client-side grid).
 * Comparators themselves live with the cell vocabulary, not here.
 */

/** Sort direction. */
export type SortDirection = 'asc' | 'desc';

/** Which column the grid is sorted by, if any. Both fields are set or neither is. */
export interface SortState {
  columnId: string | null;
  direction: SortDirection | null;
}

/** Unsorted state, mirroring `SortState::default`. */
export function emptySortState(): SortState {
  return { columnId: null, direction: null };
}

/** Create a sort state with both fields active, mirroring `SortState::new`. */
export function createSortState(columnId: string, direction: SortDirection): SortState {
  return { columnId, direction };
}

/**
 * Repair a half-filled state coming from a URL or storage, mirroring `SortState::normalize`.
 *
 * A nonblank column with a missing direction sorts ascending; a blank column id clears the state.
 */
export function normalizeSortState(state: Partial<SortState> | null | undefined = {}): SortState {
  const columnId = state?.columnId ?? null;
  if (columnId === null || columnId.trim().length === 0) return emptySortState();
  const direction = state?.direction === 'desc' ? 'desc' : 'asc';
  return { columnId, direction };
}

/** True when a column is sorted (a missing direction still counts as ascending). */
export function isSortActive(state?: SortState | null): boolean {
  return state !== null && state !== undefined && activeSort(state) !== null;
}

/** Active sort as a normalized pair, or null when the grid is unsorted. */
export function activeSort(state: SortState): [string, SortDirection] | null {
  const normalized = normalizeSortState(state);
  if (normalized.columnId === null || normalized.direction === null) return null;
  return [normalized.columnId, normalized.direction];
}

/** Direction when sorted by `columnId`, otherwise null. Mirrors `SortState::is_sorted_by`. */
export function sortDirectionForColumn(
  state: SortState | null | undefined,
  columnId: string,
): SortDirection | null {
  if (!state) return null;
  const active = activeSort(state);
  return active !== null && active[0] === columnId ? active[1] : null;
}

/** Invert the sort direction. */
export function oppositeSortDirection(direction: SortDirection): SortDirection {
  return direction === 'asc' ? 'desc' : 'asc';
}

/** Value for the WAI-ARIA `aria-sort` attribute. */
export function ariaSortValue(
  direction?: SortDirection | null,
): 'ascending' | 'descending' | 'none' {
  if (direction === 'asc') return 'ascending';
  if (direction === 'desc') return 'descending';
  return 'none';
}

/** `aria-sort` value for a header cell of `columnId`. Mirrors `SortState::aria_sort_for`. */
export function ariaSortForColumn(
  state: SortState | null | undefined,
  columnId: string,
): 'ascending' | 'descending' | 'none' {
  return ariaSortValue(sortDirectionForColumn(state, columnId));
}

/** Set the active sort, normalizing a blank column away. Mirrors `SortState::set`. */
export function setSortState(columnId: string, direction: SortDirection): SortState {
  return normalizeSortState({ columnId, direction });
}

/** Clear sorting, mirroring `SortState::clear`. */
export function clearSortState(): SortState {
  return emptySortState();
}

/**
 * Cycle through `none -> asc -> desc -> none` for one column, mirroring `SortState::toggle`.
 *
 * Selecting a different column restarts at ascending. Clearing returns the empty state, never
 * `null`, so every caller handles exactly one shape.
 */
export function cycleSort(
  current: SortState | null | undefined,
  columnId: string,
): SortState {
  const state = normalizeSortState(current ?? undefined);
  const currentDirection = sortDirectionForColumn(state, columnId);
  if (currentDirection === 'asc') return { columnId, direction: 'desc' };
  if (currentDirection === 'desc') return emptySortState();
  return { columnId, direction: 'asc' };
}
