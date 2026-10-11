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
