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
 * Row selection state for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/selection.rs`.
 *
 * Framework-free: immutable `Set`-based selection operations. A selection is a set of ids and
 * nothing else: which rows exist, and what a bulk action does with them, belongs to the owner.
 */

import { compareCodePoints } from './order';

/** Row selection state representing selected row identifiers. */
export type RowSelection = ReadonlySet<string>;

/** Create an empty row selection, mirroring `RowSelection::new`. */
export function emptyRowSelection(): RowSelection {
  return new Set<string>();
}

/** Build a selection from ids, mirroring Rust's `FromIterator` for `RowSelection`. */
export function rowSelectionFromIds(ids: Iterable<string>): RowSelection {
  return new Set<string>(ids);
}

/** Toggle one row; the new state of that row is the caller's, not ours. Mirrors `toggle`. */
export function toggleRowSelection(selection: RowSelection, id: string): RowSelection {
  const next = new Set<string>(selection);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/** Select one row identifier. */
export function selectRow(selection: RowSelection, id: string): RowSelection {
  if (selection.has(id)) return selection;
  const next = new Set<string>(selection);
  next.add(id);
  return next;
}

/** Deselect one row identifier. */
export function deselectRow(selection: RowSelection, id: string): RowSelection {
  if (!selection.has(id)) return selection;
  const next = new Set<string>(selection);
  next.delete(id);
  return next;
}

/** Set one row's selected state. Mirrors `set_selected`. */
export function setRowSelected(
  selection: RowSelection,
  id: string,
  selected: boolean,
): RowSelection {
  return selected ? selectRow(selection, id) : deselectRow(selection, id);
}

/**
 * Add every provided row identifier to an existing selection, preserving selections outside that
 * page. This is the immutable counterpart of `RowSelection::select_all`, which is what a
 * "select this page" checkbox does.
 */
export function addRowsToSelection(
  selection: RowSelection,
  ids: Iterable<string>,
): RowSelection {
  const next = new Set<string>(selection);
  let changed = false;
  for (const id of ids) {
    if (next.has(id)) continue;
    next.add(id);
    changed = true;
  }
  return changed ? next : selection;
}

/**
 * Deselect the provided row identifiers, preserving selections outside that page.
 * Mirrors `RowSelection::deselect_all`.
 */
export function deselectAllRows(
  selection: RowSelection,
  ids: Iterable<string>,
): RowSelection {
  const next = new Set<string>(selection);
  let changed = false;
  for (const id of ids) {
    if (!next.has(id)) continue;
    next.delete(id);
    changed = true;
  }
  return changed ? next : selection;
}

/** Remove every selected row. */
export function clearRowSelection(): RowSelection {
  return emptyRowSelection();
}

/**
 * Drop identifiers that are no longer present in the data set. Mirrors `retain_ids`.
 *
 * Without this a selection silently accumulates rows the user can no longer see, and a bulk action
 * then hits records outside the current view. The original selection is returned when nothing was
 * dropped, so an adapter does not re-render on a no-op refresh.
 */
export function retainRowIds(
  selection: RowSelection,
  ids: Iterable<string>,
): RowSelection {
  const keep = new Set(ids);
  const next = new Set<string>();
  let changed = false;
  for (const id of selection) {
    if (keep.has(id)) next.add(id);
    else changed = true;
  }
  return changed ? next : selection;
}

/** Check whether a row identifier is selected. */
export function isRowSelected(selection: RowSelection, id: string): boolean {
  return selection.has(id);
}

/**
 * Check whether every id in `ids` is selected.
 *
 * An empty input is `false`, so a "select all" checkbox is never checked for an empty page.
 */
export function areAllRowsSelected(selection: RowSelection, ids: Iterable<string>): boolean {
  let any = false;
  for (const id of ids) {
    any = true;
    if (!selection.has(id)) return false;
  }
  return any;
}

/** Check whether some — but not all — of `ids` are selected (the checkbox `indeterminate` state). */
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

/**
 * The selected ids in deterministic order — the single serializer of a selection.
 *
 * Rust stores the ids in a `BTreeSet`, so its serialization is sorted; a JavaScript `Set` iterates
 * in insertion order, which would make the same selection serialize differently after a different
 * click order. Bulk requests, cache keys and URL state all compare this list, so it is sorted here.
 */

export function rowSelectionIds(selection: RowSelection): string[] {
  return [...selection].sort(compareCodePoints);
}
