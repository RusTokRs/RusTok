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
export function isSortActive(state: SortState): boolean {
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
