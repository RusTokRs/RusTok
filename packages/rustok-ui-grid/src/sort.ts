/**
 * Single-column sort state machine for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/sort.rs`.
 *
 * The core owns state transitions and ARIA values; actual ordering remains with the data owner or
 * renderer, matching the Rust library's explicit boundary.
 */

/** Sort direction. */
export type SortDirection = 'asc' | 'desc';

/** Which column the grid is sorted by, if any. */
export interface SortState {
  columnId: string | null;
  direction: SortDirection | null;
}

/** Unsorted state. */
export function emptySortState(): SortState {
  return { columnId: null, direction: null };
}

/** Create a sort state with both fields active. */
export function createSortState(columnId: string, direction: SortDirection): SortState {
  return { columnId, direction };
}

/** Repair a half-filled state coming from a URL or storage. */
export function normalizeSortState(state: Partial<SortState> = {}): SortState {
  const columnId = state.columnId ?? null;
  const direction = state.direction === 'asc' || state.direction === 'desc'
    ? state.direction
    : null;
  if (columnId === null || columnId.trim().length === 0) return emptySortState();
  return { columnId, direction: direction ?? 'asc' };
}

/** True when both the column and direction are set. */
export function isSortActive(state?: SortState | null): boolean {
  if (!state) return false;
  return activeSort(state) !== null;
}

/** Active sort as a pair, or null when the grid is unsorted. */
export function activeSort(state: SortState): [string, SortDirection] | null {
  const normalized = normalizeSortState(state);
  if (normalized.columnId === null || normalized.direction === null) return null;
  return [normalized.columnId, normalized.direction];
}

/** Direction when sorted by `columnId`, otherwise null. */
export function sortDirectionForColumn(
  state: SortState,
  columnId: string,
): SortDirection | null {
  const active = activeSort(state);
  return active !== null && active[0] === columnId ? active[1] : null;
}

/** Invert the sort direction. */
export function oppositeSortDirection(direction: SortDirection): SortDirection {
  return direction === 'asc' ? 'desc' : 'asc';
}

/** Compares two values according to sort direction. */
export function compareValues<T>(a: T, b: T, direction: SortDirection = 'asc'): number {
  const factor = direction === 'desc' ? -1 : 1;
  if (typeof a === 'number' && typeof b === 'number') {
    return (a - b) * factor;
  }
  return String(a).localeCompare(String(b)) * factor;
}

/** Value for the WAI-ARIA `aria-sort` attribute. */
export function ariaSortValue(direction?: SortDirection | null): 'ascending' | 'descending' | 'none' {
  if (direction === 'asc') return 'ascending';
  if (direction === 'desc') return 'descending';
  return 'none';
}

/** `aria-sort` value for a header cell of `columnId`. */
export function ariaSortForColumn(
  state: SortState,
  columnId: string,
): 'ascending' | 'descending' | 'none' {
  return ariaSortValue(sortDirectionForColumn(state, columnId));
}

/** Set the active sort. */
export function setSortState(columnId: string, direction: SortDirection): SortState {
  return normalizeSortState({ columnId, direction });
}

/** Clear sorting. */
export function clearSortState(): SortState {
  return emptySortState();
}

/**
 * Cycle through `none -> asc -> desc -> none` for one column. Selecting a different column starts
 * at ascending; a cleared state is returned as `null` for convenient adapter binding.
 */
export function cycleSort(
  current: SortState | null | undefined,
  columnId: string,
): SortState | null {
  const state = current === null || current === undefined
    ? emptySortState()
    : normalizeSortState(current);
  const currentDirection = sortDirectionForColumn(state, columnId);
  if (currentDirection === 'asc') return { columnId, direction: 'desc' };
  if (currentDirection === 'desc') return null;
  return { columnId, direction: 'asc' };
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

