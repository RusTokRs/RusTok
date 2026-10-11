/**
 * Row selection state for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/selection.rs`.
 *
 * Framework-free: immutable Set-based selection operations.
 */

/** Row selection state representing selected row identifiers. */
export type RowSelection = ReadonlySet<string>;

/** Create an empty row selection. */
export function emptyRowSelection(): RowSelection {
  return new Set<string>();
}

/** Toggle selection of a single row identifier. */
export function toggleRowSelection(selection: RowSelection, id: string): RowSelection {
  const next = new Set<string>(selection);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/** Select one row identifier. */
export function selectRow(selection: RowSelection, id: string): RowSelection {
  const next = new Set<string>(selection);
  next.add(id);
  return next;
}

/** Deselect one row identifier. */
export function deselectRow(selection: RowSelection, id: string): RowSelection {
  const next = new Set<string>(selection);
  next.delete(id);
  return next;
}

/** Set one row's selected state. */
export function setRowSelected(
  selection: RowSelection,
  id: string,
  selected: boolean,
): RowSelection {
  return selected ? selectRow(selection, id) : deselectRow(selection, id);
}

/** Create a selection containing every provided row identifier. */
export function selectAllRows(ids: Iterable<string>): RowSelection {
  return new Set<string>(ids);
}

/**
 * Add every provided row identifier to an existing selection, preserving selections outside that
 * page. This is the immutable counterpart of `RowSelection::select_all`.
 */
export function addRowsToSelection(
  selection: RowSelection,
  ids: Iterable<string>,
): RowSelection {
  const next = new Set<string>(selection);
  for (const id of ids) next.add(id);
  return next;
}

/** Deselect the provided row identifiers, preserving selections outside that page. */
export function deselectAllRows(
  selection: RowSelection,
  ids: Iterable<string>,
): RowSelection {
  const next = new Set<string>(selection);
  for (const id of ids) next.delete(id);
  return next;
}

/** Remove every selected row. */
export function clearRowSelection(): RowSelection {
  return emptyRowSelection();
}

/** Drop identifiers that are no longer present in the data set. */
export function retainRowIds(
  selection: RowSelection,
  ids: Iterable<string>,
): RowSelection {
  const keep = new Set(ids);
  return new Set([...selection].filter((id) => keep.has(id)));
}

/** Check whether a row identifier is selected. */
export function isRowSelected(selection: RowSelection, id: string): boolean {
  return selection.has(id);
}

/** Check whether every id in `ids` is selected. An empty input is false. */
export function areAllRowsSelected(selection: RowSelection, ids: Iterable<string>): boolean {
  let any = false;
  for (const id of ids) {
    any = true;
    if (!selection.has(id)) return false;
  }
  return any;
}

/** Check whether some — but not all — of `ids` are selected. */
export function areSomeRowsSelected(selection: RowSelection, ids: Iterable<string>): boolean {
  let selectedCount = 0;
  let total = 0;
  for (const id of ids) {
    total += 1;
    if (selection.has(id)) selectedCount += 1;
  }
  return selectedCount > 0 && selectedCount < total;
}

/** Number of selected row identifiers. */
export function rowSelectionCount(selection: RowSelection): number {
  return selection.size;
}

/** True when no row identifier is selected. */
export function isRowSelectionEmpty(selection: RowSelection): boolean {
  return selection.size === 0;
}

function compareCodePoints(left: string, right: string): number {
  const leftPoints = Array.from(left, (character) => character.codePointAt(0) ?? 0);
  const rightPoints = Array.from(right, (character) => character.codePointAt(0) ?? 0);
  const length = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < length; index += 1) {
    const difference = leftPoints[index] - rightPoints[index];
    if (difference !== 0) return difference;
  }
  return leftPoints.length - rightPoints.length;
}

/**
 * Selected ids in deterministic order, matching Rust's `BTreeSet` serialization rather than
 * JavaScript Set insertion order.
 */
export function rowSelectionIds(selection: RowSelection): string[] {
  return [...selection].sort(compareCodePoints);
}

/** Compatibility alias for setRowSelected. */
export const setSelectedRow = setRowSelected;

/** Compatibility alias for clearRowSelection. */
export const clearSelection = clearRowSelection;

/**
 * Drops ids that are no longer present in the visible dataset (matches Rust `retain_ids`).
 */
export function retainSelectedRows(
  selection: RowSelection,
  ids: Iterable<string>
): RowSelection {
  const allowed = new Set(ids);
  const next = new Set<string>();
  let changed = false;
  for (const id of selection) {
    if (allowed.has(id)) {
      next.add(id);
    } else {
      changed = true;
    }
  }
  return changed ? next : selection;
}

/**
 * Returns the number of selected rows.
 */
export function selectedCount(selection: RowSelection): number {
  return selection.size;
}

/**
 * Checks whether the selection is empty.
 */
export function isSelectionEmpty(selection: RowSelection): boolean {
  return selection.size === 0;
}

/**
 * Converts RowSelection to an array of string IDs.
 */
export function rowSelectionToArray(selection: RowSelection): string[] {
  return Array.from(selection);
}

/** Alias for areAllRowsSelected matching Rust `is_all_selected`. */
export const isAllSelected = areAllRowsSelected;

/** Alias for areSomeRowsSelected matching Rust `is_partially_selected`. */
export const isPartiallySelected = areSomeRowsSelected;

