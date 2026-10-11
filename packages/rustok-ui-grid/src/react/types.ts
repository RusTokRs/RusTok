import * as React from 'react';

/** Static column definition for table rendering. */
export interface DataTableStaticColumn<T> {
  id: string;
  header: React.ReactNode;
  headerClassName?: string;
  cellClassName?: string;
  cell: (row: T) => React.ReactNode;
  sortable?: boolean;
}

/** Props for the canonical empty state view. */
export interface DataTableEmptyStateProps {
  icon?: React.ReactNode;
  message: React.ReactNode;
  className?: string;
}

/** Props for the canonical static data table. */
export interface DataTableStaticProps<T> {
  rows: T[];
  columns: DataTableStaticColumn<T>[];
  getRowKey?: (row: T) => React.Key;
  emptyState?: React.ReactNode | DataTableEmptyStateProps;
  isLoading?: boolean;
  skeletonRows?: number;
  className?: string;
  ariaLabel?: string;
  sort?: { columnId: string; direction: 'asc' | 'desc' } | null;
  onSort?: (columnId: string) => void;
  density?: 'default' | 'compact';
  /** Optional pagination: when provided, renders the standard pagination bar below. */
  page?: number;
  pageSize?: number;
  total?: number;
  onPageChange?: (nextPage: number) => void;
  rangeLabel?: React.ReactNode;
}

/** Props for the standalone pagination bar. */
export interface DataTablePaginationBarProps {
  page: number;
  pageCount: number;
  onPageChange: (nextPage: number) => void;
  rangeLabel?: React.ReactNode;
  disabled?: boolean;
  className?: string;
}
