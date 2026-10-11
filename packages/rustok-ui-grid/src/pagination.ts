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
 * Formats standard range label for pagination bars ("Showing X to Y of Z").
 */
export function dataTableRangeLabel(
  page: number,
  pageSize: number,
  total: number
): string {
  if (total === 0) {
    return '0 of 0';
  }
  const safePage = Math.max(1, page);
  const safePageSize = Math.max(1, pageSize || DEFAULT_PAGE_SIZE);
  const from = (safePage - 1) * safePageSize + 1;
  const to = Math.min(total, safePage * safePageSize);
  return `${from}–${to} of ${total}`;
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
