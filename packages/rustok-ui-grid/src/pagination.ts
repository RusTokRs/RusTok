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
 * Pagination state and arithmetic for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/pagination.rs`.
 *
 * The Rust core supports full `u64` totals. JavaScript numbers are exact only through
 * `Number.MAX_SAFE_INTEGER`; this adapter rejects larger totals rather than silently rounding them.
 *
 * Every function takes a `GridPagination` state object; `fromIndex`, `toIndex`, `visibleLength`
 * and the range label additionally accept the raw `(page, pageSize, total)` triple so a renderer
 * can format a label without constructing state.
 */

/** How the grid consumes pages. */
export type PaginationMode = 'paged' | 'infinite';

/** Fallback page size used by a newly created pagination state. */
export const DEFAULT_PAGE_SIZE = 20;

/** Default choices offered by the "rows per page" selector. */
export const DEFAULT_PAGE_SIZE_OPTIONS: readonly number[] = [10, 20, 50, 100];

/** Largest integer that can be represented exactly by JavaScript's `number` type. */
export const MAX_SAFE_GRID_INTEGER = Number.MAX_SAFE_INTEGER;

/** Pagination state for a table or grid. Page indexes are one-based. */
export interface GridPagination {
  page: number;
  pageSize: number;
  total: number;
  hasNext: boolean;
  mode: PaginationMode;
}

function normalizeTotal(total: number): number {
  if (!Number.isFinite(total) || !Number.isInteger(total)) {
    throw new RangeError('total must be a finite integer');
  }
  if (total > MAX_SAFE_GRID_INTEGER) {
    throw new RangeError(`total must not exceed ${MAX_SAFE_GRID_INTEGER} on the JavaScript surface`);
  }
  return Math.max(0, total);
}

function normalizePositiveIndex(value: number, fallback = 1): number {
  if (Number.isNaN(value)) return fallback;
  if (value === Number.POSITIVE_INFINITY) return MAX_SAFE_GRID_INTEGER;
  if (value === Number.NEGATIVE_INFINITY) return 1;
  if (value > MAX_SAFE_GRID_INTEGER) return MAX_SAFE_GRID_INTEGER;
  if (value < 1) return 1;
  return Math.trunc(value);
}

function normalizePageSize(pageSize: number): number {
  // A zero or missing page size is repaired to one, as in `GridPagination::new`, so a caller that
  // forgot to pass one keeps rendering instead of dividing by zero.
  if (!Number.isFinite(pageSize)) return 1;
  if (pageSize > MAX_SAFE_GRID_INTEGER) {
    throw new RangeError(`pageSize must not exceed ${MAX_SAFE_GRID_INTEGER}`);
  }
  return Math.max(1, Math.trunc(pageSize));
}

function safeMultiply(left: number, right: number): number {
  if (left === 0 || right === 0) return 0;
  if (left > Math.floor(MAX_SAFE_GRID_INTEGER / right)) return MAX_SAFE_GRID_INTEGER;
  return left * right;
}

function safeAdd(left: number, right: number): number {
  return left > MAX_SAFE_GRID_INTEGER - right ? MAX_SAFE_GRID_INTEGER : left + right;
}

function normalizeMode(mode: PaginationMode): PaginationMode {
  return mode === 'infinite' ? 'infinite' : 'paged';
}

/**
 * Calculates total page count. Always at least 1 (an empty grid still has page 1 of 1).
 * A zero page size is repaired to 1, matching `GridPagination::new`.
 */
export function calculatePageCount(total: number, pageSize: number): number {
  const safeTotal = normalizeTotal(total);
  const safePageSize = normalizePageSize(pageSize);
  const wholePages = Math.floor(safeTotal / safePageSize);
  const hasRemainder = safeTotal % safePageSize !== 0;
  return Math.max(1, wholePages + (hasRemainder ? 1 : 0));
}

/** Clamp a one-based page index within `1..=pageCount`. */
export function clampPage(page: number, total: number, pageSize: number): number {
  const pageCount = calculatePageCount(total, pageSize);
  return Math.min(normalizePositiveIndex(page), pageCount);
}

/**
 * Zero-based record offset of a page, saturating at JavaScript's exact-integer boundary.
 * Mirrors `GridPagination::offset`.
 */
export function calculateOffset(paginationOrPage: GridPagination | number, pageSize?: number): number {
  if (typeof paginationOrPage === 'object') {
    const state = normalizePagination(paginationOrPage);
    return safeMultiply(state.page - 1, state.pageSize);
  }
  const safePage = normalizePositiveIndex(paginationOrPage);
  const safePageSize = normalizePageSize(pageSize ?? DEFAULT_PAGE_SIZE);
  return safeMultiply(safePage - 1, safePageSize);
}

/** Check whether a previous page exists. */
export function hasPreviousPage(paginationOrPage: GridPagination | number): boolean {
  if (typeof paginationOrPage === 'object') return paginationOrPage.page > 1;
  return normalizePositiveIndex(paginationOrPage) > 1;
}

/**
 * Check whether a next page exists.
 *
 * With a state object the answer is the stored `hasNext` flag, i.e. the same fact a server
 * reported; with a raw triple it is recomputed from the page count.
 */
export function hasNextPage(
  paginationOrPage: GridPagination | number,
  total?: number,
  pageSize?: number,
): boolean {
  if (typeof paginationOrPage === 'object') return paginationOrPage.hasNext;
  return (
    normalizePositiveIndex(paginationOrPage) <
    calculatePageCount(total ?? 0, pageSize ?? DEFAULT_PAGE_SIZE)
  );
}

/**
 * Creates a normalized pagination state. `pageSize = 0` is repaired to one, as in the Rust core;
 * callers that want the normal default should omit the argument.
 */
export function createPagination(
  page = 1,
  pageSize = DEFAULT_PAGE_SIZE,
  total = 0,
  mode: PaginationMode = 'paged',
): GridPagination {
  return normalizePagination({ page, pageSize, total, hasNext: false, mode });
}

/** Create cumulative infinite-scroll pagination, matching `GridPagination::infinite`. */
export function createInfinitePagination(
  pageSize = DEFAULT_PAGE_SIZE,
  total = 0,
): GridPagination {
  return normalizePagination({ page: 1, pageSize, total, hasNext: false, mode: 'infinite' });
}

/** Re-establish pagination invariants after data came from a URL, storage or transport. */
export function normalizePagination(
  pagination: Partial<GridPagination> = {},
): GridPagination {
  const total = normalizeTotal(pagination.total ?? 0);
  const pageSize = normalizePageSize(pagination.pageSize ?? DEFAULT_PAGE_SIZE);
  const page = Math.min(
    normalizePositiveIndex(pagination.page ?? 1),
    calculatePageCount(total, pageSize),
  );
  return {
    page,
    pageSize,
    total,
    hasNext: safeMultiply(page, pageSize) < total,
    mode: normalizeMode(pagination.mode ?? 'paged'),
  };
}

/** Number of pages, never zero. */
export function totalPages(pagination: GridPagination): number {
  return calculatePageCount(pagination.total, pagination.pageSize);
}

/** One-based index of the first rendered row (zero when the data set is empty). */
export function fromIndex(
  paginationOrPage: GridPagination | number,
  pageSize?: number,
  total?: number,
  mode: PaginationMode = 'paged',
): number {
  if (typeof paginationOrPage === 'object') {
    const state = normalizePagination(paginationOrPage);
    if (state.total === 0) return 0;
    if (state.mode === 'infinite') return 1;
    return safeAdd(safeMultiply(state.page - 1, state.pageSize), 1);
  }
  const state = createPagination(paginationOrPage, pageSize ?? DEFAULT_PAGE_SIZE, total ?? 0, mode);
  return fromIndex(state);
}

/** One-based index of the last rendered row (zero when the data set is empty). */
export function toIndex(
  paginationOrPage: GridPagination | number,
  pageSize?: number,
  total?: number,
): number {
  if (typeof paginationOrPage === 'object') {
    const state = normalizePagination(paginationOrPage);
    return Math.min(safeMultiply(state.page, state.pageSize), state.total);
  }
  const state = createPagination(paginationOrPage, pageSize ?? DEFAULT_PAGE_SIZE, total ?? 0);
  return toIndex(state);
}

/**
 * Number of rows rendered for the current page/mode.
 *
 * Infinite mode is cumulative, so page N renders `N * pageSize` rows; an empty data set renders
 * none — never a phantom row.
 */
export function visibleLength(
  paginationOrPage: GridPagination | number,
  pageSize?: number,
  total?: number,
  mode: PaginationMode = 'paged',
): number {
  if (typeof paginationOrPage === 'object') {
    const state = normalizePagination(paginationOrPage);
    if (state.total === 0) return 0;
    const end = toIndex(state);
    if (state.mode === 'infinite') return end;
    return Math.max(0, end - (fromIndex(state) - 1));
  }
  const state = createPagination(paginationOrPage, pageSize ?? DEFAULT_PAGE_SIZE, total ?? 0, mode);
  return visibleLength(state);
}

/**
 * Zero-based `[start, end)` bounds for client-side paging over `length` locally available rows.
 * Mirrors `GridPagination::slice_bounds`.
 */
export function sliceBounds(pagination: GridPagination, length: number): [number, number] {
  const state = normalizePagination(pagination);
  const safeLength = normalizeTotal(length);
  if (state.mode === 'infinite') {
    return [0, Math.min(safeMultiply(state.page, state.pageSize), safeLength)];
  }
  const start = Math.min(safeMultiply(state.page - 1, state.pageSize), safeLength);
  const end = Math.min(safeAdd(start, state.pageSize), safeLength);
  return [start, end];
}

/**
 * The rows of the current page of a locally available data set.
 * Mirrors `GridPagination::slice_items`.
 */
export function sliceItems<T>(items: readonly T[], pagination: GridPagination): T[] {
  const [start, end] = sliceBounds(pagination, items.length);
  return items.slice(start, end);
}

/** Select a page, clamping it into the valid range and refreshing `hasNext`. */
export function goToPage(pagination: GridPagination, nextPage: number): GridPagination {
  const state = normalizePagination(pagination);
  return normalizePagination({ ...state, page: nextPage });
}

/** Advance one page when possible. */
export function nextPage(pagination: GridPagination): GridPagination {
  const state = normalizePagination(pagination);
  return goToPage(state, Math.min(MAX_SAFE_GRID_INTEGER, state.page + 1));
}

/** Go back one page when possible. */
export function previousPage(pagination: GridPagination): GridPagination {
  const state = normalizePagination(pagination);
  return goToPage(state, Math.max(1, state.page - 1));
}

/** Change the page size and reset to the first page. */
export function setPageSize(pagination: GridPagination, pageSize: number): GridPagination {
  const state = normalizePagination(pagination);
  return normalizePagination({ ...state, page: 1, pageSize });
}

/** Update the row count while keeping the current page in range. */
export function setTotal(pagination: GridPagination, total: number): GridPagination {
  const state = normalizePagination(pagination);
  return normalizePagination({ ...state, total });
}

/** Switch page modes; infinite scrolling always renders from the first page downwards. */
export function setPaginationMode(
  pagination: GridPagination,
  mode: PaginationMode,
): GridPagination {
  return normalizePagination({ ...pagination, mode });
}

/**
 * Standard formatted range description of a state: `"1–20 of 100"`, `"1–40 of 100"` on the second
 * cumulative infinite page, `"0 of 0"` when there is no data. Mirrors `GridPagination::range_label`.
 */
export function rangeLabel(pagination: GridPagination): string {
  const state = normalizePagination(pagination);
  if (state.total === 0) return '0 of 0';
  return `${fromIndex(state)}–${toIndex(state)} of ${state.total}`;
}

/** Range description from the raw triple, for renderers that do not hold a state object. */
export function dataTableRangeLabel(
  page: number,
  pageSize: number,
  total: number,
  mode: PaginationMode = 'paged',
): string {
  return rangeLabel(createPagination(page, pageSize, total, mode));
}

/**
 * The page-size choices a grid offers: positive, deduplicated, sorted, and always containing the
 * current size so a `<select>` can never render a blank value.
 * Mirrors `normalize_page_size_options`.
 */
export function normalizePageSizeOptions(
  options: readonly number[] | undefined,
  currentSize: number,
): number[] {
  const choices = (options && options.length > 0 ? options : DEFAULT_PAGE_SIZE_OPTIONS)
    .filter((size) => Number.isFinite(size) && Math.trunc(size) > 0)
    .map((size) => Math.trunc(size));
  const current = normalizePageSize(currentSize);
  if (!choices.includes(current)) choices.push(current);
  return [...new Set(choices)].sort((left, right) => left - right);
}
