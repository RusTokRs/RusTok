/**
 * Pagination state and arithmetic for RusTok DataGrid — the TypeScript twin of `crates/ui/rustok-grid/src/pagination.rs`.
 *
 * Framework-free: integer-safe, overflow-safe, 1-based page indexing.
 */

/** How the grid consumes pages. */
export type PaginationMode = 'paged' | 'infinite';

/** Fallback page size used when a caller passes 0. */
export const DEFAULT_PAGE_SIZE = 20;

/** Pagination state for a table or grid. */
export interface GridPagination {
  page: number;
  pageSize: number;
  total: number;
  hasNext: boolean;
  mode: PaginationMode;
}

/**
 * Calculates total page count. Always at least 1 (an empty grid still has page 1 of 1).
 */
export function calculatePageCount(total: number, pageSize: number): number {
  const safePageSize = Math.max(1, pageSize || DEFAULT_PAGE_SIZE);
  const safeTotal = Math.max(0, total);
  return Math.max(1, Math.ceil(safeTotal / safePageSize));
}

/**
 * Clamps a 1-based page index within `1..=pageCount`.
 */
export function clampPage(page: number, total: number, pageSize: number): number {
  const pageCount = calculatePageCount(total, pageSize);
  return Math.min(Math.max(1, page), pageCount);
}

/**
 * Calculates the zero-based record offset for SQL/GraphQL queries.
 */
export function calculateOffset(page: number, pageSize: number): number {
  const safePage = Math.max(1, page);
  const safePageSize = Math.max(1, pageSize || DEFAULT_PAGE_SIZE);
  return (safePage - 1) * safePageSize;
}

/**
 * Checks whether a previous page exists.
 */
export function hasPreviousPage(page: number): boolean {
  return page > 1;
}

/**
 * Checks whether a next page exists.
 */
export function hasNextPage(page: number, total: number, pageSize: number): boolean {
  return page < calculatePageCount(total, pageSize);
}

/**
 * Calculates 1-based index of the first rendered row (0 when total is 0).
 */
export function fromIndex(
  page: number,
  pageSize: number,
  total: number,
  mode: PaginationMode = 'paged'
): number {
  if (total <= 0) return 0;
  if (mode === 'infinite') return 1;
  const safePage = Math.max(1, page);
  const safePageSize = Math.max(1, pageSize || DEFAULT_PAGE_SIZE);
  return (safePage - 1) * safePageSize + 1;
}

/**
 * Calculates 1-based index of the last rendered row (0 when total is 0).
 */
export function toIndex(page: number, pageSize: number, total: number): number {
  if (total <= 0) return 0;
  const safePage = Math.max(1, page);
  const safePageSize = Math.max(1, pageSize || DEFAULT_PAGE_SIZE);
  return Math.min(total, safePage * safePageSize);
}

/**
 * Calculates count of visible rows rendered for current page and mode.
 */
export function visibleLen(
  page: number,
  pageSize: number,
  total: number,
  mode: PaginationMode = 'paged'
): number {
  if (total <= 0) return 0;
  if (mode === 'infinite') return toIndex(page, pageSize, total);
  const from = fromIndex(page, pageSize, total, mode);
  const to = toIndex(page, pageSize, total);
  return Math.max(0, to - (from - 1));
}

/**
 * Calculates zero-based `[start, end)` slice bounds for client-side paging over an array.
 */
export function sliceBounds(
  page: number,
  pageSize: number,
  len: number,
  mode: PaginationMode = 'paged'
): [number, number] {
  if (len <= 0) return [0, 0];
  const safePage = Math.max(1, page);
  const safePageSize = Math.max(1, pageSize || DEFAULT_PAGE_SIZE);
  if (mode === 'infinite') {
    return [0, Math.min(len, safePage * safePageSize)];
  }
  const start = Math.min(len, (safePage - 1) * safePageSize);
  const end = Math.min(len, start + safePageSize);
  return [start, end];
}

/**
 * Slices an array of items according to pagination state.
 */
export function paginateArray<T>(
  items: T[],
  page: number,
  pageSize: number,
  mode: PaginationMode = 'paged'
): T[] {
  const [start, end] = sliceBounds(page, pageSize, items.length, mode);
  return items.slice(start, end);
}

/**
 * Formats standard range label for pagination bars ("Showing X to Y of Z" or "0 of 0").
 */
export function dataTableRangeLabel(
  page: number,
  pageSize: number,
  total: number
): string {
  if (total <= 0) {
    return '0 of 0';
  }
  const from = fromIndex(page, pageSize, total);
  const to = toIndex(page, pageSize, total);
  return `${from}–${to} of ${total}`;
}

/**
 * Formats range label directly from a GridPagination object.
 */
export function rangeLabel(pagination: GridPagination): string {
  return dataTableRangeLabel(pagination.page, pagination.pageSize, pagination.total);
}

/**
 * Creates a normalized GridPagination state.
 */
export function createPagination(
  page: number = 1,
  pageSize: number = DEFAULT_PAGE_SIZE,
  total: number = 0,
  mode: PaginationMode = 'paged'
): GridPagination {
  const safePageSize = Math.max(1, pageSize);
  const safeTotal = Math.max(0, total);
  const safePage = clampPage(page, safeTotal, safePageSize);
  return {
    page: safePage,
    pageSize: safePageSize,
    total: safeTotal,
    hasNext: hasNextPage(safePage, safeTotal, safePageSize),
    mode
  };
}

/**
 * Moves to next_page, clamped within valid range (1..=total_pages).
 */
export function setPage(pagination: GridPagination, nextPage: number): GridPagination {
  const safePage = clampPage(nextPage, pagination.total, pagination.pageSize);
  return {
    ...pagination,
    page: safePage,
    hasNext: hasNextPage(safePage, pagination.total, pagination.pageSize)
  };
}

/**
 * Advances one page when possible.
 */
export function nextPage(pagination: GridPagination): GridPagination {
  return setPage(pagination, pagination.page + 1);
}

/**
 * Moves back one page when possible.
 */
export function previousPage(pagination: GridPagination): GridPagination {
  return setPage(pagination, pagination.page - 1);
}

/**
 * Changes page size and resets to page 1.
 */
export function setPageSize(pagination: GridPagination, newSize: number): GridPagination {
  const safeSize = Math.max(1, newSize);
  return createPagination(1, safeSize, pagination.total, pagination.mode);
}

/**
 * Updates total records, keeping current page clamped.
 */
export function setTotal(pagination: GridPagination, newTotal: number): GridPagination {
  const safeTotal = Math.max(0, newTotal);
  const safePage = clampPage(pagination.page, safeTotal, pagination.pageSize);
  return {
    ...pagination,
    total: safeTotal,
    page: safePage,
    hasNext: hasNextPage(safePage, safeTotal, pagination.pageSize)
  };
}

/**
 * Updates pagination mode.
 */
export function setMode(pagination: GridPagination, mode: PaginationMode): GridPagination {
  return {
    ...pagination,
    mode
  };
}
