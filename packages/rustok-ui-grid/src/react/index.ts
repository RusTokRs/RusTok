/**
 * React table companion for RusTok — the TypeScript twin of `crates/ui/rustok-grid-leptos`.
 *
 * Companion to `@tanstack/react-table` providing standard static table,
 * empty state, and pagination bar components.
 */

export { DataTableStatic } from './data-table-static';
export { DataTableEmptyState } from './data-table-empty-state';
export { DataTablePaginationBar } from './data-table-pagination-bar';
export type {
  DataTableStaticColumn,
  DataTableStaticProps,
  DataTableEmptyStateProps,
  DataTablePaginationBarProps
} from './types';
