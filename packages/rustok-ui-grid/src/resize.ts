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
 * Column resize math and persisted width state — the TypeScript twin of
 * `crates/ui/rustok-grid/src/resize.rs`.
 *
 * No DOM or pointer APIs live here: adapters provide the pointer delta and render the result.
 *
 * `ColumnWidths` is persisted state (local storage, a URL, a user profile), so **reads repair**:
 * a stale, hand-edited or out-of-range value is clamped instead of throwing in a render path.
 * **Writes stay strict**: a caller that hands us a non-integer or a negative width is a bug, and a
 * thrown `RangeError` is the honest answer.
 */

import { compareCodePoints } from './order';

import {
  MAX_COLUMN_WIDTH,
  assertColumnWidthPx,
  type ColumnWidth,
} from './column';

/** Keyboard resizing step (px) used by the grid adapters. */
export const KEYBOARD_RESIZE_STEP = 16;

/** User-overridden column widths, stored as a JSON-friendly record keyed by column id. */
export type ColumnWidths = Readonly<Record<string, number>>;

/** Repair one persisted width: non-finite input means "no usable override". */
function repairStoredWidth(value: unknown): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) return undefined;
  const truncated = Math.trunc(value);
  if (truncated <= 0) return 0;
  return truncated > MAX_COLUMN_WIDTH ? MAX_COLUMN_WIDTH : truncated;
}

/** Stored override, if the user ever resized this column. Repairs a corrupted value. */
export function getStoredColumnWidth(widths: ColumnWidths, columnId: string): number | undefined {
  if (!Object.prototype.hasOwnProperty.call(widths, columnId)) return undefined;
  return repairStoredWidth(widths[columnId]);
}

/** Read a stored width or return the column's default width. */
export function getColumnWidth(
  widths: ColumnWidths,
  columnId: string,
  defaultWidth: number,
): number {
  const stored = getStoredColumnWidth(widths, columnId);
  if (stored !== undefined) return stored;
  return repairStoredWidth(defaultWidth) ?? 0;
}

/**
 * Read a persisted width while enforcing the current column constraints.
 *
 * This is what protects the renderer from stale or hand-edited serialized state: reversed bounds
 * are ordered and any number is pulled into the range, so the answer is always a drawable width.
 */
export function getClampedColumnWidth(
  widths: ColumnWidths,
  columnId: string,
  defaultWidth: number,
  minWidth: number,
  maxWidth: number,
): number {
  const [low, high] = orderedBounds(minWidth, maxWidth);
  const value = getColumnWidth(widths, columnId, defaultWidth);
  return Math.min(high, Math.max(low, value));
}

function orderedBounds(minWidth: number, maxWidth: number): [number, number] {
  const min = repairStoredWidth(minWidth) ?? 0;
  const max = repairStoredWidth(maxWidth) ?? MAX_COLUMN_WIDTH;
  return min <= max ? [min, max] : [max, min];
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

/** Store a width while enforcing the column's constraints; reports whether it actually changed. */
export function setClampedColumnWidth(
  widths: ColumnWidths,
  columnId: string,
  width: number,
  minWidth: number,
  maxWidth: number,
): { widths: ColumnWidths; changed: boolean } {
  const [low, high] = orderedBounds(minWidth, maxWidth);
  const value = assertColumnWidthPx(width);
  return setColumnWidth(widths, columnId, Math.min(high, Math.max(low, value)));
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

/** Stored width overrides in Rust-compatible deterministic key order. */
export function columnWidthEntries(widths: ColumnWidths): Array<[string, number]> {
  const entries: Array<[string, number]> = [];
  for (const columnId of Object.keys(widths).sort(compareCodePoints)) {
    const width = repairStoredWidth(widths[columnId]);
    // An override that carries no usable width is dropped rather than rendered: repairing it here
    // is what keeps `normalizeColumnWidths` a repair and not a validation error.
    if (width === undefined) continue;
    entries.push([columnId, width]);
  }
  return entries;
}

/** Repair a serialized width record to valid Rust-compatible `u32` values, in key order. */
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

/**
 * Keep only the overrides of columns that still exist.
 *
 * A width stored for a column that has since been removed would otherwise live in the persisted
 * state forever, and silently win again if the column id ever comes back with other bounds.
 */
export function pruneColumnWidths(
  widths: ColumnWidths,
  columns: readonly { id: string }[],
): { widths: ColumnWidths; changed: boolean } {
  const known = new Set(columns.map((column) => column.id));
  const next: Record<string, number> = {};
  let changed = false;
  for (const [columnId, width] of columnWidthEntries(widths)) {
    if (!known.has(columnId)) {
      changed = true;
      continue;
    }
    next[columnId] = width;
  }
  return changed ? { widths: next, changed } : { widths, changed };
}

/**
 * Clamp every stored override against the current column definitions and drop the overrides of
 * removed columns. Mirrors `GridState::clamp_widths`.
 */
export function clampColumnWidths(
  widths: ColumnWidths,
  columns: readonly { id: string; width: ColumnWidth }[],
): ColumnWidths {
  let next = normalizeColumnWidths(widths);
  for (const column of columns) {
    const stored = getStoredColumnWidth(next, column.id);
    if (stored === undefined) continue;
    next = setClampedColumnWidth(next, column.id, stored, column.width.min, column.width.max)
      .widths;
  }
  const known = new Set(columns.map((column) => column.id));
  const kept: Record<string, number> = {};
  for (const [columnId, width] of columnWidthEntries(next)) {
    if (known.has(columnId)) kept[columnId] = width;
  }
  return kept;
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
  const [low, high] = orderedBounds(minWidth, maxWidth);
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
