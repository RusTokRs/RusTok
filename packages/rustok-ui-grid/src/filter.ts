/**
 * Column filter specifications and values for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/filter.rs`.
 *
 * Framework-free: normalized non-empty filter representations.
 */

/** One option of a select filter. Counts are owned by facets, not filter options. */
export interface FilterOption {
  value: string;
  label: string;
}

/** Filter control configuration, mirroring the Rust `GridFilterType` variants. */
export type GridFilterType =
  | { kind: 'text'; placeholder: string | null }
  | { kind: 'select'; options: FilterOption[]; placeholder: string | null }
  | {
      kind: 'number_range';
      minPlaceholder: string | null;
      maxPlaceholder: string | null;
      step: number | null;
    }
  | {
      kind: 'date_range';
      fromPlaceholder: string | null;
      toPlaceholder: string | null;
    }
  | { kind: 'boolean'; trueLabel: string; falseLabel: string };

/** A value entered in the filter row. */
export type FilterValue =
  | { kind: 'text'; value: string }
  | { kind: 'select'; value: string }
  | { kind: 'number_range'; min: number | null; max: number | null }
  | { kind: 'date_range'; from: string | null; to: string | null }
  | { kind: 'boolean'; value: boolean }
  | { kind: 'empty' };

/** Active filters keyed by column id. Empty values are removed by `normalizeFilters`. */
export type ColumnFilters = Readonly<Record<string, FilterValue>>;

/** A text filter with no placeholder. */
export function textFilter(): GridFilterType {
  return { kind: 'text', placeholder: null };
}

/** A text filter with a placeholder. */
export function textFilterWithPlaceholder(placeholder: string): GridFilterType {
  return { kind: 'text', placeholder };
}

/** A select filter with no placeholder. */
export function selectFilter(options: readonly FilterOption[]): GridFilterType {
  return { kind: 'select', options: options.map((option) => ({ ...option })), placeholder: null };
}

/** A select filter with a placeholder. */
export function selectFilterWithPlaceholder(
  options: readonly FilterOption[],
  placeholder: string,
): GridFilterType {
  return {
    kind: 'select',
    options: options.map((option) => ({ ...option })),
    placeholder,
  };
}

/** A number range filter with no input placeholders or step override. */
export function numberRangeFilter(): GridFilterType {
  return { kind: 'number_range', minPlaceholder: null, maxPlaceholder: null, step: null };
}

/** A date range filter with no input placeholders. */
export function dateRangeFilter(): GridFilterType {
  return { kind: 'date_range', fromPlaceholder: null, toPlaceholder: null };
}

/** A boolean filter with adapter-provided labels. */
export function booleanFilter(trueLabel: string, falseLabel: string): GridFilterType {
  return { kind: 'boolean', trueLabel, falseLabel };
}

/** The neutral value produced by a control when the user clears it. */
export function emptyFilterValue(filterType: GridFilterType): FilterValue {
  switch (filterType.kind) {
    case 'text':
      return { kind: 'text', value: '' };
    case 'select':
      return { kind: 'select', value: '' };
    case 'number_range':
      return { kind: 'number_range', min: null, max: null };
    case 'date_range':
      return { kind: 'date_range', from: null, to: null };
    case 'boolean':
      return { kind: 'empty' };
  }
}

/** True when a value was produced by this kind of control. Empty is accepted by every control. */
export function filterTypeAccepts(filterType: GridFilterType, value: FilterValue): boolean {
  return value.kind === 'empty' || filterType.kind === value.kind;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function optionalFiniteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function optionalTrimmedString(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

/**
 * Canonical form of one filter value: strings are trimmed, non-finite numbers are dropped, and
 * reversed number/date ranges are swapped into a meaningful order. Invalid runtime input returns
 * `null` instead of leaking malformed state into a query.
 */
export function normalizeFilterValue(value: unknown): FilterValue | null {
  if (!isRecord(value) || typeof value.kind !== 'string') return null;

  switch (value.kind) {
    case 'text':
    case 'select':
      if (typeof value.value !== 'string') return null;
      return { kind: value.kind, value: value.value.trim() };
    case 'number_range': {
      let min = optionalFiniteNumber(value.min);
      let max = optionalFiniteNumber(value.max);
      if (min !== null && max !== null && min > max) [min, max] = [max, min];
      return { kind: 'number_range', min, max };
    }
    case 'date_range': {
      let from = optionalTrimmedString(value.from);
      let to = optionalTrimmedString(value.to);
      // As in Rust, date values are expected to be ISO-8601 so lexical and chronological order agree.
      if (from !== null && to !== null && from > to) [from, to] = [to, from];
      return { kind: 'date_range', from, to };
    }
    case 'boolean':
      return typeof value.value === 'boolean' ? { kind: 'boolean', value: value.value } : null;
    case 'empty':
      return { kind: 'empty' };
    default:
      return null;
  }
}

/** True when the value carries an active constraint. `Boolean(false)` remains active. */
export function isFilterActive(value: unknown): boolean {
  const normalized = normalizeFilterValue(value);
  if (normalized === null || normalized.kind === 'empty') return false;

  switch (normalized.kind) {
    case 'text':
    case 'select':
      return normalized.value.length > 0;
    case 'number_range':
      return normalized.min !== null || normalized.max !== null;
    case 'date_range':
      return normalized.from !== null || normalized.to !== null;
    case 'boolean':
      return true;
  }
}

function compareCodePoints(left: string, right: string): number {
  const leftPoints = Array.from(left, (character) => character.codePointAt(0) ?? 0);
  const rightPoints = Array.from(right, (character) => character.codePointAt(0) ?? 0);
  const length = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < length; index += 1) {
    const difference = leftPoints[index] - rightPoints[index];
    if (difference !== 0) return difference;
  }
  return leftPoints.length - rightPoints.length;
}

function defineFilter(target: Record<string, FilterValue>, key: string, value: FilterValue): void {
  Object.defineProperty(target, key, {
    configurable: true,
    enumerable: true,
    value,
    writable: true,
  });
}

/** Normalize a filter map and drop empty or invalid values in deterministic key order. */
export function normalizeFilters(filters: Readonly<Record<string, unknown>>): ColumnFilters {
  const normalized: Record<string, FilterValue> = {};
  for (const key of Object.keys(filters ?? {}).sort(compareCodePoints)) {
    const value = normalizeFilterValue(filters[key]);
    if (value !== null && isFilterActive(value)) defineFilter(normalized, key, value);
  }
  return normalized;
}

function sameFilterValue(left: FilterValue, right: FilterValue): boolean {
  if (left.kind !== right.kind) return false;
  switch (left.kind) {
    case 'text':
    case 'select':
      return right.kind === left.kind && left.value === right.value;
    case 'number_range':
      return right.kind === left.kind && left.min === right.min && left.max === right.max;
    case 'date_range':
      return right.kind === left.kind && left.from === right.from && left.to === right.to;
    case 'boolean':
      return right.kind === left.kind && left.value === right.value;
    case 'empty':
      return true;
  }
}

/** Immutable counterpart of `ColumnFilters::set`; reports whether canonical state changed. */
export function setColumnFilter(
  filters: ColumnFilters,
  columnId: string,
  rawValue: unknown,
): { filters: ColumnFilters; changed: boolean } {
  const current = normalizeFilters(filters);
  const value = normalizeFilterValue(rawValue);
  const hasCurrent = Object.prototype.hasOwnProperty.call(current, columnId);

  if (value === null || !isFilterActive(value)) {
    if (!hasCurrent) return { filters: current, changed: false };
    const next = { ...current };
    delete next[columnId];
    return { filters: next, changed: true };
  }

  if (hasCurrent && sameFilterValue(current[columnId], value)) {
    return { filters: current, changed: false };
  }

  const next: Record<string, FilterValue> = { ...current };
  defineFilter(next, columnId, value);
  return { filters: next, changed: true };
}

/** Immutable counterpart of `ColumnFilters::remove`. */
export function removeColumnFilter(
  filters: ColumnFilters,
  columnId: string,
): { filters: ColumnFilters; changed: boolean } {
  const current = normalizeFilters(filters);
  if (!Object.prototype.hasOwnProperty.call(current, columnId)) {
    return { filters: current, changed: false };
  }
  const next = { ...current };
  delete next[columnId];
  return { filters: next, changed: true };
}

/** Immutable counterpart of `ColumnFilters::clear`. */
export function clearColumnFilters(
  filters: ColumnFilters,
): { filters: ColumnFilters; changed: boolean } {
  const current = normalizeFilters(filters);
  const changed = Object.keys(current).length > 0;
  return { filters: {}, changed };
}

/** Read one normalized filter by column id. */
export function getColumnFilter(filters: ColumnFilters, columnId: string): FilterValue | undefined {
  const normalized = normalizeFilters(filters);
  return Object.prototype.hasOwnProperty.call(normalized, columnId)
    ? normalized[columnId]
    : undefined;
}

/** True when a normalized filter exists for the column. */
export function hasColumnFilterValue(filters: ColumnFilters, columnId: string): boolean {
  return Object.prototype.hasOwnProperty.call(normalizeFilters(filters), columnId);
}

/** Active filter entries in deterministic key order. */
export function columnFilterEntries(filters: ColumnFilters): Array<[string, FilterValue]> {
  const normalized = normalizeFilters(filters);
  return Object.keys(normalized)
    .sort(compareCodePoints)
    .map((columnId) => [columnId, normalized[columnId]]);
}

/** Active filter keys in deterministic order. */
export function columnFilterKeys(filters: ColumnFilters): string[] {
  return Object.keys(normalizeFilters(filters)).sort(compareCodePoints);
}

/** Number of active filters. */
export function columnFilterCount(filters: ColumnFilters): number {
  return columnFilterKeys(filters).length;
}
