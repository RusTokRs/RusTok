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

/** Definition of a column in the data grid. */
export interface GridColumnDef<T = unknown> {
  id: string;
  header: string;
  align?: ColumnAlign;
  sortable?: boolean;
  filterable?: boolean;
  filterType?: GridFilterType;
  pinned?: PinnedSide;
  width?: ColumnWidth;
  visible?: boolean;
  cell?: (row: T) => unknown;
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
