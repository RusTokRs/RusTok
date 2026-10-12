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
 * Column definitions for RusTok DataGrid — the TypeScript twin of
 * `crates/ui/rustok-grid/src/column.rs`.
 *
 * Framework-free: no React, no DOM, no styling assumptions.
 */

import type { GridFilterType } from './filter';

/** Identifier of the synthetic selection column rendered as a checkbox. */
export const CHECKBOX_COLUMN_ID = '__checkbox';

/** Largest width representable by the Rust core's `u32` width fields. */
export const MAX_COLUMN_WIDTH = 0xffff_ffff;

/** Stable identifier of a grid column. */
export type ColumnId = string;

/** Horizontal alignment of a column's content. */
export type ColumnAlign = 'left' | 'center' | 'right';

/** Pinned side for fixed columns. */
export type PinnedSide = 'left' | 'right';

/*
 * No `text_align` twin of the Rust helper is needed here: `ColumnAlign` is already the CSS keyword
 * union, while the Rust enum has to translate itself. That is a language-shape difference, not a
 * missing capability, and it is listed as such in `scripts/verify/verify-ui-grid-parity.mjs`.
 */

/** Width constraints of a column; `min <= current <= max` always holds. */
export interface ColumnWidth {
  min: number;
  max: number;
  current: number;
}

/** Declarative description of one grid column. */
export interface GridColumnDef {
  id: ColumnId;
  title: string;
  align: ColumnAlign;
  width: ColumnWidth;
  sortable: boolean;
  resizable: boolean;
  filterable: boolean;
  visible: boolean;
  pinned: PinnedSide | null;
  filterType: GridFilterType | null;
}

/** Validate a pixel width that crosses the JavaScript/Rust boundary. */
export function assertColumnWidthPx(value: number, name = 'width'): number {
  if (!Number.isInteger(value) || value < 0 || value > MAX_COLUMN_WIDTH) {
    throw new RangeError(`${name} must be an integer between 0 and ${MAX_COLUMN_WIDTH}`);
  }
  return value;
}

function orderedBounds(min: number, max: number): [number, number] {
  const lower = assertColumnWidthPx(min, 'min width');
  const upper = assertColumnWidthPx(max, 'max width');
  return lower <= upper ? [lower, upper] : [upper, lower];
}

/** The default resizable width used by an ordinary column. */
export function createDefaultColumnWidth(): ColumnWidth {
  return { min: 50, max: 800, current: 150 };
}

/** A column that cannot grow or shrink. */
export function createFixedColumnWidth(px: number): ColumnWidth {
  const width = assertColumnWidthPx(px);
  return { min: width, max: width, current: width };
}

/** Create a resizable width, normalizing reversed bounds and clamping its default. */
export function createResizableColumnWidth(
  defaultPx: number,
  minPx: number,
  maxPx: number,
): ColumnWidth {
  const [min, max] = orderedBounds(minPx, maxPx);
  const preferred = assertColumnWidthPx(defaultPx, 'default width');
  return { min, max, current: Math.min(max, Math.max(min, preferred)) };
}

/** Repair an externally supplied width descriptor. */
export function normalizeColumnWidth(width: ColumnWidth): ColumnWidth {
  return createResizableColumnWidth(width.current, width.min, width.max);
}

/** Clamp an arbitrary width into a column's range. */
export function clampColumnWidth(width: ColumnWidth, px: number): number {
  const normalized = normalizeColumnWidth(width);
  const requested = assertColumnWidthPx(px);
  return Math.min(normalized.max, Math.max(normalized.min, requested));
}

/** Set the preferred width, widening the range instead of discarding existing bounds. */
export function setPreferredColumnWidth(width: ColumnWidth, px: number): ColumnWidth {
  const normalized = normalizeColumnWidth(width);
  const preferred = assertColumnWidthPx(px);
  return {
    min: Math.min(normalized.min, preferred),
    max: Math.max(normalized.max, preferred),
    current: preferred,
  };
}

/** Raise the lower bound, pushing the upper bound and current width up if required. */
export function setColumnMinWidth(width: ColumnWidth, minPx: number): ColumnWidth {
  const normalized = normalizeColumnWidth(width);
  const min = assertColumnWidthPx(minPx, 'min width');
  const max = Math.max(normalized.max, min);
  return { min, max, current: Math.min(max, Math.max(min, normalized.current)) };
}

/** Lower the upper bound, pulling the lower bound and current width down if required. */
export function setColumnMaxWidth(width: ColumnWidth, maxPx: number): ColumnWidth {
  const normalized = normalizeColumnWidth(width);
  const max = assertColumnWidthPx(maxPx, 'max width');
  const min = Math.min(normalized.min, max);
  return { min, max, current: Math.min(max, Math.max(min, normalized.current)) };
}

/** True when a column has an actual range to resize within. */
export function isColumnWidthFlexible(width: ColumnWidth): boolean {
  const normalized = normalizeColumnWidth(width);
  return normalized.min !== normalized.max;
}

/** Create an ordinary column with the same defaults as `GridColumnDef::new`. */
export function createGridColumnDef(id: ColumnId, title: string): GridColumnDef {
  return {
    id,
    title,
    align: 'left',
    width: createDefaultColumnWidth(),
    sortable: true,
    resizable: true,
    filterable: true,
    visible: true,
    pinned: null,
    filterType: null,
  };
}

/** Preferred width, widening min/max when necessary (order-independent). */
export function withColumnWidth(column: GridColumnDef, px: number): GridColumnDef {
  return { ...column, width: setPreferredColumnWidth(column.width, px) };
}

/** Full width specification in one call. */
export function withColumnWidthRange(
  column: GridColumnDef,
  defaultPx: number,
  minPx: number,
  maxPx: number,
): GridColumnDef {
  return { ...column, width: createResizableColumnWidth(defaultPx, minPx, maxPx) };
}

/** Raise the lower bound, pushing the upper bound and current width up if required. */
export function withColumnMinWidth(column: GridColumnDef, minPx: number): GridColumnDef {
  return { ...column, width: setColumnMinWidth(column.width, minPx) };
}

/** Lower the upper bound, pulling the lower bound and current width down if required. */
export function withColumnMaxWidth(column: GridColumnDef, maxPx: number): GridColumnDef {
  return { ...column, width: setColumnMaxWidth(column.width, maxPx) };
}

/** Disable resizing while preserving the configured width descriptor. */
export function withColumnNotResizable(column: GridColumnDef): GridColumnDef {
  return { ...column, resizable: false };
}

/** Disable sorting. */
export function withColumnNotSortable(column: GridColumnDef): GridColumnDef {
  return { ...column, sortable: false };
}

/** Hide a column from the rendered column set. */
export function hideColumn(column: GridColumnDef): GridColumnDef {
  return { ...column, visible: false };
}

/** Set the content alignment. */
export function withColumnAlign(column: GridColumnDef, align: ColumnAlign): GridColumnDef {
  return { ...column, align };
}

/** Attach a filter control to the column. */
export function withColumnFilter(
  column: GridColumnDef,
  filterType: GridFilterType,
): GridColumnDef {
  return { ...column, filterable: true, filterType };
}

/** Disable filtering and discard its control descriptor. */
export function withColumnNotFilterable(column: GridColumnDef): GridColumnDef {
  return { ...column, filterable: false, filterType: null };
}

/** Pin a column to one side of the grid. */
export function pinColumn(column: GridColumnDef, side: PinnedSide): GridColumnDef {
  return { ...column, pinned: side };
}

/** Remove a column's pin. */
export function unpinColumn(column: GridColumnDef): GridColumnDef {
  return { ...column, pinned: null };
}

/** Synthetic row-selection column. */
export function createCheckboxColumn(): GridColumnDef {
  return {
    id: CHECKBOX_COLUMN_ID,
    title: '',
    align: 'center',
    width: createFixedColumnWidth(44),
    sortable: false,
    resizable: false,
    filterable: false,
    visible: true,
    pinned: 'left',
    filterType: null,
  };
}

/** Trailing column reserved for row actions. */
export function createActionsColumn(id: ColumnId, title: string): GridColumnDef {
  return {
    id,
    title,
    align: 'right',
    width: createFixedColumnWidth(80),
    sortable: false,
    resizable: false,
    filterable: false,
    visible: true,
    pinned: 'right',
    filterType: null,
  };
}

/** True for the synthetic row-selection column. */
export function isCheckboxColumn(column: Pick<GridColumnDef, 'id'>): boolean {
  return column.id === CHECKBOX_COLUMN_ID;
}

/** True when this column should render an interactive filter control. */
export function hasFilter(
  column: Pick<GridColumnDef, 'filterable' | 'filterType'>,
): boolean {
  return column.filterable && column.filterType !== null && column.filterType !== undefined;
}

/** Filters the column list to only visible columns, preserving declaration order. */
export function visibleColumns<T extends Pick<GridColumnDef, 'visible'>>(
  columns: readonly T[],
): T[] {
  return columns.filter((column) => column.visible);
}

/** Number of columns actually rendered — the only correct value for `colspan`. */
export function visibleColumnCount<T extends Pick<GridColumnDef, 'visible'>>(
  columns: readonly T[],
): number {
  return visibleColumns(columns).length;
}

/** True when at least one visible column renders a filter control. */
export function hasFilterRow(
  columns: readonly GridColumnDef[],
): boolean {
  return visibleColumns(columns).some(hasFilter);
}
