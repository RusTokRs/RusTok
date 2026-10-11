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
