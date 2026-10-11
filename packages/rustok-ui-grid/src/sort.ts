/**
 * Sort state machine for RusTok DataGrid — the TypeScript twin of `crates/ui/rustok-grid/src/sort.rs`.
 *
 * Framework-free: tri-state cycling, direction flipping, value comparison.
 */

/** Sort direction. */
export type SortDirection = 'asc' | 'desc';

/** Single-column sort state. */
export interface SortState {
  columnId: string;
  direction: SortDirection;
}

/** Inverts the sort direction. */
export function oppositeSortDirection(direction: SortDirection): SortDirection {
  return direction === 'asc' ? 'desc' : 'asc';
}

/** Returns the ARIA sort attribute value. */
export function ariaSortValue(direction?: SortDirection | null): 'ascending' | 'descending' | 'none' {
  if (direction === 'asc') return 'ascending';
  if (direction === 'desc') return 'descending';
  return 'none';
}

/**
 * Cycles sort state: none -> asc -> desc -> none.
 */
export function cycleSort(
  current: SortState | null | undefined,
  columnId: string
): SortState | null {
  if (!current || current.columnId !== columnId) {
    return { columnId, direction: 'asc' };
  }
  if (current.direction === 'asc') {
    return { columnId, direction: 'desc' };
  }
  return null;
}

/**
 * Pure comparator for two values given a direction.
 */
export function compareValues(
  a: unknown,
  b: unknown,
  direction: SortDirection = 'asc'
): number {
  if (a === b) return 0;
  if (a === null || a === undefined) return direction === 'asc' ? 1 : -1;
  if (b === null || b === undefined) return direction === 'asc' ? -1 : 1;

  let cmp = 0;
  if (typeof a === 'number' && typeof b === 'number') {
    cmp = a - b;
  } else if (typeof a === 'string' && typeof b === 'string') {
    cmp = a.localeCompare(b);
  } else if (a instanceof Date && b instanceof Date) {
    cmp = a.getTime() - b.getTime();
  } else {
    cmp = String(a).localeCompare(String(b));
  }

  return direction === 'asc' ? cmp : -cmp;
}

/**
 * Repairs a half-filled sort state (e.g. from query params or storage).
 * Returns null if invalid or blank.
 */
export function normalizeSort(state?: Partial<SortState> | null): SortState | null {
  if (!state || !state.columnId) return null;
  const colId = state.columnId.trim();
  if (colId.length === 0) return null;
  const dir: SortDirection = state.direction === 'desc' ? 'desc' : 'asc';
  return { columnId: colId, direction: dir };
}

/**
 * Checks whether the grid is sorted by the given columnId.
 */
export function isSortedBy(
  state: SortState | null | undefined,
  columnId: string
): SortDirection | null {
  if (!state || state.columnId !== columnId) return null;
  return state.direction;
}

/**
 * Returns aria-sort string for a specific column header.
 */
export function ariaSortFor(
  state: SortState | null | undefined,
  columnId: string
): 'ascending' | 'descending' | 'none' {
  return ariaSortValue(isSortedBy(state, columnId));
}

/**
 * Checks whether sort state is active.
 */
export function isSortActive(state?: SortState | null): boolean {
  return Boolean(state && state.columnId && state.direction);
}

/**
 * Creates or sets a sort state.
 */
export function setSort(columnId: string, direction: SortDirection = 'asc'): SortState {
  return { columnId, direction };
}

/**
 * Clears sort state (returns null).
 */
export function clearSort(): null {
  return null;
}

