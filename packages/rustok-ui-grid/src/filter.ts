/**
 * Column filter specifications and values for RusTok DataGrid — the TypeScript twin of `crates/ui/rustok-grid/src/filter.rs`.
 *
 * Framework-free: normalized non-empty filter representations.
 */

/** One option of a select or multi-select filter. */
export interface FilterOption {
  value: string;
  label: string;
  count?: number;
}

/** Supported filter types. */
export type GridFilterType =
  | 'text'
  | 'select'
  | 'multi_select'
  | 'number_range'
  | 'date_range'
  | 'boolean';

/** Filter value representation across types. */
export type FilterValue =
  | string
  | number
  | boolean
  | string[]
  | [number | null, number | null]
  | [string | null, string | null]
  | null;

/** Map of columnId to its active FilterValue. */
export type ColumnFilters = Record<string, FilterValue>;

/**
 * Checks whether a filter value represents an active condition.
 */
export function isFilterActive(value: FilterValue | undefined): boolean {
  if (value === null || value === undefined) return false;
  if (typeof value === 'string') return value.trim().length > 0;
  if (typeof value === 'number') return !Number.isNaN(value);
  if (typeof value === 'boolean') return true;
  if (Array.isArray(value)) {
    if (value.length === 0) return false;
    return value.some((item) => item !== null && item !== undefined && item !== '');
  }
  return false;
}

/**
 * Normalizes column filters by pruning inactive or empty values.
 */
export function normalizeFilters(filters: ColumnFilters): ColumnFilters {
  const result: ColumnFilters = {};
  for (const [key, value] of Object.entries(filters)) {
    if (isFilterActive(value)) {
      result[key] = value;
    }
  }
  return result;
}

/**
 * Checks whether two filter values are equal.
 */
export function isFilterEqual(a: FilterValue | undefined, b: FilterValue | undefined): boolean {
  if (a === b) return true;
  if (a === undefined || a === null) return b === undefined || b === null;
  if (b === undefined || b === null) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((v, i) => v === b[i]);
  }
  return false;
}

/**
 * Stores a filter value immutably. Empty values remove the entry.
 * Returns `{ filters, changed }`.
 */
export function setFilter(
  filters: ColumnFilters,
  columnId: string,
  value: FilterValue
): { filters: ColumnFilters; changed: boolean } {
  const current = filters[columnId];
  if (isFilterEqual(current, value)) {
    return { filters, changed: false };
  }

  const next = { ...filters };
  if (isFilterActive(value)) {
    next[columnId] = value;
  } else {
    delete next[columnId];
  }

  return { filters: next, changed: true };
}

/**
 * Removes a filter by columnId immutably.
 */
export function removeFilter(
  filters: ColumnFilters,
  columnId: string
): { filters: ColumnFilters; changed: boolean } {
  if (!(columnId in filters)) {
    return { filters, changed: false };
  }
  const next = { ...filters };
  delete next[columnId];
  return { filters: next, changed: true };
}

/**
 * Clears all active filters immutably.
 */
export function clearFilters(filters: ColumnFilters): {
  filters: ColumnFilters;
  changed: boolean;
} {
  const hasAny = Object.keys(filters).length > 0;
  return { filters: {}, changed: hasAny };
}

/**
 * Checks whether any filters are active.
 */
export function hasActiveFilters(filters: ColumnFilters): boolean {
  return Object.keys(normalizeFilters(filters)).length > 0;
}

/**
 * Returns the count of active column filters.
 */
export function activeFilterCount(filters: ColumnFilters): number {
  return Object.keys(normalizeFilters(filters)).length;
}
