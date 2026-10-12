/**
 * Column resize math and persisted width state — the TypeScript twin of
 * `crates/ui/rustok-grid/src/resize.rs`.
 *
 * No DOM or pointer APIs live here: adapters provide the pointer delta and render the result.
 */

import {
  assertColumnWidthPx,
  type ColumnWidth,
} from './column';

/** Keyboard resizing step (px) used by the grid adapters. */
export const KEYBOARD_RESIZE_STEP = 16;

/** User-overridden column widths, stored as a JSON-friendly record keyed by column id. */
export type ColumnWidths = Readonly<Record<string, number>>;

/**
 * Compatibility class for object-oriented ColumnWidths usage.
 */
export class ColumnWidthsStore {
  private widths: Record<string, number>;
  constructor(initial?: Record<string, number>) {
    this.widths = { ...(initial ?? {}) };
  }
  getStored(columnId: string): number | undefined {
    return getStoredColumnWidth(this.widths, columnId);
  }
  get(columnId: string, defaultWidth: number): number {
    return getColumnWidth(this.widths, columnId, defaultWidth);
  }
  getClamped(columnId: string, defaultWidth: number, min: number, max: number): number {
    return getClampedColumnWidth(this.widths, columnId, defaultWidth, min, max);
  }
  set(columnId: string, width: number): void {
    const res = setColumnWidth(this.widths, columnId, width);
    this.widths = { ...res.widths };
  }
  setClamped(columnId: string, width: number, min: number, max: number): boolean {
    const res = setClampedColumnWidth(this.widths, columnId, width, min, max);
    this.widths = { ...res.widths };
    return res.changed;
  }
  toRecord(): Record<string, number> {
    return { ...this.widths };
  }
}

/** Stored override, if the user ever resized this column. */
export function getStoredColumnWidth(widths: ColumnWidths, columnId: string): number | undefined {
  if (!Object.prototype.hasOwnProperty.call(widths, columnId)) return undefined;
  return assertColumnWidthPx(widths[columnId], 'stored column width');
}

/** Read a stored width or return the column's default width. */
export function getColumnWidth(
  widths: ColumnWidths,
  columnId: string,
  defaultWidth: number,
): number {
  return getStoredColumnWidth(widths, columnId) ?? assertColumnWidthPx(defaultWidth, 'default width');
}

/** Read a persisted width while enforcing the current column constraints. */
export function getClampedColumnWidth(
  widths: ColumnWidths,
  columnId: string,
  defaultWidth: number,
  minWidth: number,
  maxWidth: number,
): number {
  const min = assertColumnWidthPx(minWidth, 'min width');
  const max = assertColumnWidthPx(maxWidth, 'max width');
  const lower = Math.min(min, max);
  const upper = Math.max(min, max);
  return Math.min(upper, Math.max(lower, getColumnWidth(widths, columnId, defaultWidth)));
}

/** Store a width without applying column bounds. */
export function setColumnWidth(
  widths: ColumnWidths,
  columnId: string,
  width: number,
): { widths: ColumnWidths; changed: boolean } {
  const value = assertColumnWidthPx(width);
  if (getStoredColumnWidth(widths, columnId) === value) return { widths, changed: false };
  return { widths: { ...widths, [columnId]: value }, changed: true };
}

/** Store a width while enforcing the column's current constraints. */
export function setClampedColumnWidth(
  widths: ColumnWidths,
  columnId: string,
  width: number,
  minWidth: number,
  maxWidth: number,
): { widths: ColumnWidths; changed: boolean } {
  const min = assertColumnWidthPx(minWidth, 'min width');
  const max = assertColumnWidthPx(maxWidth, 'max width');
  const low = Math.min(min, max);
  const high = Math.max(min, max);
  const clamped = Math.min(high, Math.max(low, assertColumnWidthPx(width)));
  return setColumnWidth(widths, columnId, clamped);
}

/** Remove one stored override. */
export function removeColumnWidth(
  widths: ColumnWidths,
  columnId: string,
): { widths: ColumnWidths; changed: boolean } {
  if (!Object.prototype.hasOwnProperty.call(widths, columnId)) return { widths, changed: false };
  const next = { ...widths };
  delete next[columnId];
  return { widths: next, changed: true };
}

/** Remove every stored override. */
export function clearColumnWidths(widths: ColumnWidths): { widths: ColumnWidths; changed: boolean } {
  const changed = Object.keys(widths).length > 0;
  return { widths: {}, changed };
}

/** True when there are no user-overridden widths. */
export function columnWidthsAreEmpty(widths: ColumnWidths): boolean {
  return Object.keys(widths).length === 0;
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

/** Stored width overrides in Rust-compatible deterministic key order. */
export function columnWidthEntries(widths: ColumnWidths): Array<[string, number]> {
  return Object.keys(widths)
    .sort(compareCodePoints)
    .map((columnId) => [columnId, assertColumnWidthPx(widths[columnId], 'stored column width')]);
}

/** Repair a serialized width record to valid Rust-compatible `u32` values. */
export function normalizeColumnWidths(widths: ColumnWidths): ColumnWidths {
  const normalized: Record<string, number> = {};
  for (const [columnId, width] of columnWidthEntries(widths)) {
    Object.defineProperty(normalized, columnId, {
      configurable: true,
      enumerable: true,
      value: width,
      writable: true,
    });
  }
  return normalized;
}

/** Clamp every stored override against the current column definitions. */
export function clampColumnWidths(
  widths: ColumnWidths,
  columns: readonly { id: string; width: ColumnWidth }[],
): ColumnWidths {
  let next = normalizeColumnWidths(widths);
  for (const column of columns) {
    const stored = getStoredColumnWidth(next, column.id);
    if (stored === undefined) continue;
    next = setClampedColumnWidth(
      next,
      column.id,
      stored,
      column.width.min,
      column.width.max,
    ).widths;
  }
  return next;
}

function roundTiesAwayFromZero(value: number): number {
  return Math.sign(value) * Math.floor(Math.abs(value) + 0.5);
}

/**
 * Calculate a resized width with min/max bounds. Non-finite pointer deltas leave the current width
 * in place, and reversed bounds are normalized before use.
 */
export function calculateResizedWidth(
  initialWidth: number,
  deltaX: number,
  minWidth: number,
  maxWidth: number,
): number {
  const initial = assertColumnWidthPx(initialWidth, 'initial width');
  const min = assertColumnWidthPx(minWidth, 'min width');
  const max = assertColumnWidthPx(maxWidth, 'max width');
  const low = Math.min(min, max);
  const high = Math.max(min, max);
  if (!Number.isFinite(deltaX)) return Math.min(high, Math.max(low, initial));

  const proposed = initial + deltaX;
  if (proposed < low) return low;
  if (proposed > high) return high;
  const rounded = roundTiesAwayFromZero(proposed);
  return Math.min(high, Math.max(low, rounded));
}

/** Apply a keyboard-driven resize step, saturating at the bounds. */
export function stepResizedWidth(
  currentWidth: number,
  stepPx: number,
  minWidth: number,
  maxWidth: number,
): number {
  if (!Number.isInteger(stepPx) || stepPx < -0x8000_0000 || stepPx > 0x7fff_ffff) {
    throw new RangeError('stepPx must be a signed 32-bit integer');
  }
  return calculateResizedWidth(currentWidth, stepPx, minWidth, maxWidth);
}
