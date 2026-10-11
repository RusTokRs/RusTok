/**
 * Column definitions for RusTok DataGrid — the TypeScript twin of `crates/ui/rustok-grid/src/column.rs`.
 *
 * Framework-free: no React, no DOM, no styling assumptions.
 */

import type { GridFilterType } from './filter';

/** Identifier of the synthetic selection column rendered as a checkbox. */
export const CHECKBOX_COLUMN_ID = '__checkbox';

/** Horizontal alignment of a column's content. */
export type ColumnAlign = 'left' | 'center' | 'right';

/** Pinned side for fixed columns. */
export type PinnedSide = 'left' | 'right';

/** Sizing constraints for a column. */
export interface ColumnWidth {
  min?: number;
  max?: number;
  default?: number;
}

/** Creates a normalized ColumnWidth descriptor. */
export function createColumnWidth(
  defaultPx: number = 150,
  minPx: number = 50,
  maxPx: number = 800
): ColumnWidth {
  const min = Math.min(minPx, maxPx);
  const max = Math.max(minPx, maxPx);
  const def = Math.min(Math.max(min, defaultPx), max);
  return { default: def, min, max };
}

/** Creates a fixed non-resizable ColumnWidth. */
export function fixedColumnWidth(px: number): ColumnWidth {
  return { default: px, min: px, max: px };
}

/** Clamps a pixel width within the bounds of a ColumnWidth. */
export function clampColumnWidth(width: ColumnWidth, px: number): number {
  const min = width.min ?? 50;
  const max = width.max ?? 800;
  const lower = Math.min(min, max);
  const upper = Math.max(min, max);
  return Math.min(Math.max(lower, px), upper);
}

/** Definition of a column in the data grid. */
export interface GridColumnDef<T = unknown> {
  id: string;
  header: string;
  title?: string;
  align?: ColumnAlign;
  sortable?: boolean;
  resizable?: boolean;
  filterable?: boolean;
  filterType?: GridFilterType;
  pinned?: PinnedSide;
  width?: ColumnWidth;
  visible?: boolean;
  cell?: (row: T) => unknown;
}

/** Creates a standardized column definition with sensible defaults. */
export function createColumnDef<T = unknown>(
  id: string,
  header: string,
  options?: Partial<GridColumnDef<T>>
): GridColumnDef<T> {
  return {
    id,
    header,
    title: options?.title ?? header,
    align: options?.align ?? 'left',
    sortable: options?.sortable ?? true,
    resizable: options?.resizable ?? true,
    filterable: options?.filterable ?? true,
    visible: options?.visible ?? true,
    filterType: options?.filterType,
    pinned: options?.pinned,
    width: options?.width ?? createColumnWidth(),
    cell: options?.cell
  };
}

/** Synthetic row-selection checkbox column definition matching Rust CHECKBOX_COLUMN_ID. */
export function createCheckboxColumn<T = unknown>(): GridColumnDef<T> {
  return {
    id: CHECKBOX_COLUMN_ID,
    header: '',
    title: '',
    align: 'center',
    width: fixedColumnWidth(44),
    sortable: false,
    resizable: false,
    filterable: false,
    visible: true,
    pinned: 'left'
  };
}

/** Trailing column definition reserved for row actions. */
export function createActionsColumn<T = unknown>(
  id: string = 'actions',
  header: string = 'Actions'
): GridColumnDef<T> {
  return {
    id,
    header,
    title: header,
    align: 'right',
    width: fixedColumnWidth(80),
    sortable: false,
    resizable: false,
    filterable: false,
    visible: true,
    pinned: 'right'
  };
}

/** Checks whether a column is the synthetic row selection checkbox column. */
export function isCheckboxColumn(col: { id: string }): boolean {
  return col.id === CHECKBOX_COLUMN_ID;
}

/** Checks whether a column should render an interactive filter control. */
export function hasColumnFilter(col: GridColumnDef<unknown>): boolean {
  return col.filterable === true && col.filterType !== undefined;
}

/** Returns the count of visible columns in the definition list. */
export function visibleColumnCount<T>(columns: GridColumnDef<T>[]): number {
  return columns.filter((col) => col.visible !== false).length;
}

/** Filters the column list to only visible columns. */
export function visibleColumns<T>(columns: GridColumnDef<T>[]): GridColumnDef<T>[] {
  return columns.filter((col) => col.visible !== false);
}

/** Checks whether any column in the grid has a filter configured. */
export function hasFilterRow<T>(columns: GridColumnDef<T>[]): boolean {
  return columns.some((col) => col.filterable === true || col.filterType !== undefined);
}
