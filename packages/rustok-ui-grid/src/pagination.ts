/**
 * Pagination state and arithmetic for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/pagination.rs`.
 *
 * The Rust core supports full `u64` totals. JavaScript numbers are exact only through
 * `Number.MAX_SAFE_INTEGER`; this adapter rejects larger totals rather than silently rounding them.
 */

/** How the grid consumes pages. */
export type PaginationMode = 'paged' | 'infinite';

/** Fallback page size used by a newly created pagination state. */
export const DEFAULT_PAGE_SIZE = 20;

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

/** Calculate the zero-based record offset, saturating at JavaScript's exact-integer boundary. */
export function calculateOffset(page: number, pageSize: number): number {
  const safePage = normalizePositiveIndex(page);
  const safePageSize = normalizePageSize(pageSize);
  return safeMultiply(safePage - 1, safePageSize);
}

/** Check whether a previous page exists. */
export function hasPreviousPage(page: number): boolean {
  return normalizePositiveIndex(page) > 1;
}

/** Check whether a next page exists. */
export function hasNextPage(page: number, total: number, pageSize: number): boolean {
  return normalizePositiveIndex(page) < calculatePageCount(total, pageSize);
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

/** Number of rows rendered for the current page/mode. */
export function visibleLength(
  paginationOrPage: GridPagination | number,
  pageSize?: number,
  total?: number,
  mode: PaginationMode = 'paged',
): number {
  if (typeof paginationOrPage === 'object') {
    const state = normalizePagination(paginationOrPage);
    const end = toIndex(state);
    if (state.mode === 'infinite') return end;
    return Math.max(0, end - (fromIndex(state) - 1));
  }
  const state = createPagination(paginationOrPage, pageSize ?? DEFAULT_PAGE_SIZE, total ?? 0, mode);
  return visibleLength(state);
}

/** Compatibility alias for visibleLength. */
export const visibleLen = visibleLength;

/** Zero-based `[start, end)` bounds for client-side paging over `length` local rows. */
export function sliceBounds(
  paginationOrPage: GridPagination | number,
  lengthOrPageSize: number,
  totalOrLen?: number,
  mode: PaginationMode = 'paged',
): [number, number] {
  if (typeof paginationOrPage === 'object') {
    const state = normalizePagination(paginationOrPage);
    const safeLength = normalizeTotal(lengthOrPageSize);
    if (state.mode === 'infinite') {
      return [0, Math.min(safeMultiply(state.page, state.pageSize), safeLength)];
    }
    const start = Math.min(safeMultiply(state.page - 1, state.pageSize), safeLength);
    const end = Math.min(safeAdd(start, state.pageSize), safeLength);
    return [start, end];
  }
  const state = createPagination(paginationOrPage, lengthOrPageSize, totalOrLen ?? 0, mode);
  return sliceBounds(state, totalOrLen ?? 0);
}

/** Slices an array of items according to pagination state. */
export function paginateArray<T>(
  items: readonly T[],
  paginationOrPage: GridPagination | number,
  pageSize?: number,
  mode: PaginationMode = 'paged',
): T[] {
  if (typeof paginationOrPage === 'object') {
    const [start, end] = sliceBounds(paginationOrPage, items.length);
    return items.slice(start, end);
  }
  const [start, end] = sliceBounds(paginationOrPage, pageSize ?? DEFAULT_PAGE_SIZE, items.length, mode);
  return items.slice(start, end);
}

/** Select a page, clamping it into the valid range and refreshing `hasNext`. */
export function goToPage(pagination: GridPagination, nextPage: number): GridPagination {
  const state = normalizePagination(pagination);
  return normalizePagination({ ...state, page: nextPage });
}

/** Compatibility alias for goToPage. */
export const setPage = goToPage;

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

/** Compatibility alias for setPaginationMode. */
export const setMode = setPaginationMode;

/** Format the standard range label for a paged table. */
export function dataTableRangeLabel(page: number, pageSize: number, total: number): string {
  const state = createPagination(page, pageSize, total);
  if (state.total === 0) return '0 of 0';
  return `${fromIndex(state)}–${toIndex(state)} of ${state.total}`;
}

/** Formats range label directly from a GridPagination object. */
export function rangeLabel(pagination: GridPagination): string {
  return dataTableRangeLabel(pagination.page, pagination.pageSize, pagination.total);
}
