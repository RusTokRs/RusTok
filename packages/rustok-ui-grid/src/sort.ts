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
