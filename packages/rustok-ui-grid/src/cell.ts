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
 * The value vocabulary shared by grid filters and grid sorting — the TypeScript twin of
 * `crates/ui/rustok-grid/src/cell.rs`.
 *
 * A grid column is owned by a module; a *cell* is what the grid knows about a row. The owner maps a
 * row plus a column id onto a `CellValue`, and this module answers the two questions every table
 * has: does this row satisfy the active filter for this column, and in which order do two cells
 * sort. Putting both answers here is what stops every admin table from re-implementing
 * trim/case-folding, range and date rules on its own.
 */

import { normalizeFilters, type ColumnFilters, type FilterValue } from './filter';
import { normalizeSortState, type SortState } from './sort';
import { compareCodePoints } from './order';

/** One cell of a row, in the vocabulary filters and sorting understand. */
export type CellValue =
  | { kind: 'text'; value: string }
  | { kind: 'number'; value: number }
  | { kind: 'boolean'; value: boolean }
  | { kind: 'empty' };

/** How an owner reads one cell of one row: the only row knowledge the grid needs. */
export type CellAccessor<T> = (item: T, columnId: string) => CellValue;

/** A text cell. A missing optional value is expressed with `cellEmpty`. */
export function cellText(value: string): CellValue {
  return { kind: 'text', value };
}

/** A text cell read from an optional field. */
export function cellOptionalText(value: string | null | undefined): CellValue {
  return value === null || value === undefined ? cellEmpty() : { kind: 'text', value };
}

/** A numeric cell. A non-finite number carries no constraint and becomes `empty`. */
export function cellNumber(value: number | null | undefined): CellValue {
  return typeof value === 'number' && Number.isFinite(value)
    ? { kind: 'number', value }
    : cellEmpty();
}

/** A boolean cell. */
export function cellBoolean(value: boolean): CellValue {
  return { kind: 'boolean', value };
}

/** A cell with no value at all. */
export function cellEmpty(): CellValue {
  return { kind: 'empty' };
}

/** True when the cell carries no value (an empty text is treated as no value, as in Rust). */
export function cellIsEmpty(cell: CellValue): boolean {
  return cell.kind === 'empty' || (cell.kind === 'text' && cell.value.trim().length === 0);
}

/**
 * Case-insensitive text containment used by `FilterValue::Text`.
 *
 * Both sides are trimmed and lowercased with Unicode full case conversion, matching Rust's
 * `str::to_lowercase`, so a Cyrillic query finds a Cyrillic value.
 */
export function textContainsIgnoreCase(haystack: string, needle: string): boolean {
  const query = needle.trim().toLowerCase();
  if (query.length === 0) return true;
  return haystack.toLowerCase().includes(query);
}

/**
 * ASCII-case-insensitive equality.
 *
 * Rust compares selection keys and select-option ids with `eq_ignore_ascii_case`; a value is an
 * opaque id, so only the ASCII fold applies on both sides.
 */
export function equalsIgnoreAsciiCase(left: string, right: string): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    const a = left.charCodeAt(index);
    const b = right.charCodeAt(index);
    const foldedA = a >= 65 && a <= 90 ? a + 32 : a;
    const foldedB = b >= 65 && b <= 90 ? b + 32 : b;
    if (foldedA !== foldedB) return false;
  }
  return true;
}

/** `true` when `text` is a plain decimal number (`-12`, `3.5`, `2e-3`) and nothing else. */
const DECIMAL_SPELLING = /^[+-]?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][+-]?\d+)?$/;

/**
 * The number a numeric filter can use: a numeric cell as-is, or a decimal number in a text cell.
 *
 * Hex, `Infinity` and `NaN` spellings are deliberately not numbers — JavaScript's `Number` would
 * accept them while Rust's parser would not, so the shared spelling is the explicit decimal one
 * (`is_decimal_spelling` in `cell.rs`).
 */
export function cellNumberForFilter(cell: CellValue): number | null {
  switch (cell.kind) {
    case 'number':
      return Number.isFinite(cell.value) ? cell.value : null;
    case 'text': {
      const trimmed = cell.value.trim();
      if (!DECIMAL_SPELLING.test(trimmed)) return null;
      const parsed = Number(trimmed);
      return Number.isFinite(parsed) ? parsed : null;
    }
    default:
      return null;
  }
}

/**
 * The date part of a cell: everything before the first `T`, trimmed.
 *
 * Dates are ISO-8601 (`YYYY-MM-DD`), so code-unit ordering is chronological ordering, and a
 * timestamp `2026-05-01T10:00:00Z` still compares against a `2026-05-01` bound.
 */
export function cellDatePart(cell: CellValue): string | null {
  if (cell.kind !== 'text') return null;
  const value = cell.value.trim();
  if (value.length === 0) return null;
  const separator = value.indexOf('T');
  return (separator < 0 ? value : value.slice(0, separator)).trim();
}

/**
 * True when `cell` satisfies `filter`.
 *
 * The rules are the ones the whole platform shares: text is a trimmed, case-insensitive substring;
 * a select value is an exact id compared ASCII-case-insensitively; a number range is inclusive and
 * also parses a numeric text cell; a date range compares the ISO date part; a boolean is exact. An
 * empty cell never satisfies a range or a boolean filter, because a missing value is not a value.
 * A normalized, inactive filter accepts everything.
 */
export function matchesFilterValue(filter: FilterValue | undefined, cell: CellValue): boolean {
  if (!filter || filter.kind === 'empty') return true;
  switch (filter.kind) {
    case 'text':
      return textContainsIgnoreCase(cellTextOf(cell), filter.value);
    case 'select': {
      const wanted = filter.value.trim();
      if (wanted.length === 0) return true;
      const current = cellTextOf(cell);
      return current.length > 0 && equalsIgnoreAsciiCase(current, wanted);
    }
    case 'number_range': {
      const value = cellNumberForFilter(cell);
      if (value === null) return false;
      if (filter.min !== null && value < filter.min) return false;
      if (filter.max !== null && value > filter.max) return false;
      return true;
    }
    case 'date_range': {
      const date = cellDatePart(cell);
      if (date === null) return false;
      if (filter.from !== null && date < filter.from) return false;
      if (filter.to !== null && date > filter.to) return false;
      return true;
    }
    case 'boolean':
      return cell.kind === 'boolean' && cell.value === filter.value;
  }
}

function cellTextOf(cell: CellValue): string {
  switch (cell.kind) {
    case 'text':
      return cell.value;
    case 'number':
      return String(cell.value);
    case 'boolean':
      return cell.value ? 'true' : 'false';
    case 'empty':
      return '';
  }
}

/** Rank used when two cells of different kinds are compared: empty < boolean < number < text. */
function cellRank(cell: CellValue): number {
  switch (cell.kind) {
    case 'empty':
      return 0;
    // A non-finite number can only arrive by building the variant directly, bypassing
    // `cellNumber`; rank it with the other valueless cells instead of inventing an order for it.
    case 'number':
      return Number.isFinite(cell.value) ? 2 : 0;
    case 'boolean':
      return 1;
    case 'text':
      return 3;
  }
}

/**
 * Total order of two cells, independent of locale: numbers numerically (a non-finite value never
 * reaches this type), booleans `false` before `true`, texts by code point, and `empty` before every
 * value. Cross-kind comparisons fall back to the kind rank so the order stays deterministic.
 */
export function compareCells(left: CellValue, right: CellValue): number {
  if (left.kind !== right.kind) {
    const rank = cellRank(left) - cellRank(right);
    if (rank !== 0) return Math.sign(rank);
  }
  if (left.kind === 'number' && right.kind === 'number') {
    if (!Number.isFinite(left.value) || !Number.isFinite(right.value)) {
      const byRank = Math.sign(cellRank(left) - cellRank(right));
      if (byRank !== 0) return byRank;
      return 0;
    }
    if (left.value === right.value) return 0;
    return left.value < right.value ? -1 : 1;
  }
  if (left.kind === 'boolean' && right.kind === 'boolean') {
    return Number(left.value) - Number(right.value);
  }
  if (left.kind === 'text' && right.kind === 'text') {
    return Math.sign(compareCodePoints(left.value, right.value));
  }
  if (left.kind === 'empty' && right.kind === 'empty') return 0;
  return 0;
}

/**
 * Rows satisfying every active filter of `filters`; the row-to-cell mapping stays owner-side.
 *
 * The filter map is normalized once for the whole sweep, so a caller may hand it raw, untrusted
 * values and still get "key present means filter active" semantics.
 */
export function filterRows<T>(
  filters: ColumnFilters,
  items: readonly T[],
  cellOf: CellAccessor<T>,
): T[] {
  const active = Object.entries(normalizeFilters(filters)) as Array<[string, FilterValue]>;
  if (active.length === 0) return [...items];
  return items.filter((item) =>
    active.every(([columnId, filter]) => matchesFilterValue(filter, cellOf(item, columnId))),
  );
}

/**
 * Stable, deterministic order of `items` by the active sort column.
 *
 * The sort state is normalized first, so a half-filled state sorts ascending exactly as the Rust
 * core does. Equal cells keep their input order — the same guarantee Rust's
 * decorate-sort-undecorate gives — so a page refresh never reshuffles a column with duplicate
 * values.
 */
export function sortRows<T>(
  sort: SortState | null | undefined,
  items: readonly T[],
  cellOf: CellAccessor<T>,
): T[] {
  const { columnId, direction } = normalizeSortState(sort ?? undefined);
  if (columnId === null || direction === null) return [...items];
  const decorated = items.map((item, index) => ({
    cell: cellOf(item, columnId),
    index,
    item,
  }));
  const factor = direction === 'desc' ? -1 : 1;
  decorated.sort((left, right) => {
    const byCell = compareCells(left.cell, right.cell) * factor;
    return byCell !== 0 ? byCell : left.index - right.index;
  });
  return decorated.map((entry) => entry.item);
}
