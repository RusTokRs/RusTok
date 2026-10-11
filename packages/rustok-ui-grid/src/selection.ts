/**
 * Row selection state for RusTok DataGrid — the TypeScript twin of `crates/ui/rustok-grid/src/selection.rs`.
 *
 * Framework-free: immutable Set-based selection operations.
 */

/** Row selection state representing selected row identifiers. */
export type RowSelection = ReadonlySet<string>;

/**
 * Creates an empty row selection.
 */
export function emptyRowSelection(): RowSelection {
  return new Set<string>();
}

/**
 * Toggles selection of a single row identifier.
 */
export function toggleRowSelection(selection: RowSelection, id: string): RowSelection {
  const next = new Set<string>(selection);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  return next;
}

/**
 * Selects all provided row identifiers.
 */
export function selectAllRows(ids: string[]): RowSelection {
  return new Set<string>(ids);
}

/**
 * Checks whether a row identifier is selected.
 */
export function isRowSelected(selection: RowSelection, id: string): boolean {
  return selection.has(id);
}

/**
 * Checks whether all provided row identifiers are selected.
 */
export function areAllRowsSelected(selection: RowSelection, ids: string[]): boolean {
  if (ids.length === 0) return false;
  return ids.every((id) => selection.has(id));
}

/**
 * Checks whether some (but not all) provided row identifiers are selected.
 */
export function areSomeRowsSelected(selection: RowSelection, ids: string[]): boolean {
  if (ids.length === 0) return false;
  const count = ids.filter((id) => selection.has(id)).length;
  return count > 0 && count < ids.length;
}

/**
 * Adds a single row to selection immutably.
 */
export function selectRow(selection: RowSelection, id: string): RowSelection {
  if (selection.has(id)) return selection;
  const next = new Set<string>(selection);
  next.add(id);
  return next;
}

/**
 * Removes a single row from selection immutably.
 */
export function deselectRow(selection: RowSelection, id: string): RowSelection {
  if (!selection.has(id)) return selection;
  const next = new Set<string>(selection);
  next.delete(id);
  return next;
}

/**
 * Sets explicit selection state of a row.
 */
export function setSelectedRow(selection: RowSelection, id: string, selected: boolean): RowSelection {
  return selected ? selectRow(selection, id) : deselectRow(selection, id);
}

/**
 * Deselects all provided row identifiers immutably.
 */
export function deselectAllRows(selection: RowSelection, ids: string[]): RowSelection {
  const next = new Set<string>(selection);
  let changed = false;
  for (const id of ids) {
    if (next.delete(id)) {
      changed = true;
    }
  }
  return changed ? next : selection;
}

/**
 * Clears the selection entirely.
 */
export function clearSelection(): RowSelection {
  return emptyRowSelection();
}

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

