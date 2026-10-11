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

/** Props for the canonical static data table. */
export interface DataTableStaticProps<T> {
  rows: T[];
  columns: DataTableStaticColumn<T>[];
  getRowKey?: (row: T) => React.Key;
  emptyState?: React.ReactNode;
  isLoading?: boolean;
  skeletonRows?: number;
  className?: string;
  sort?: { columnId: string; direction: 'asc' | 'desc' } | null;
  onSort?: (columnId: string) => void;
  density?: 'default' | 'compact';
}

/** Props for the canonical empty state view. */
export interface DataTableEmptyStateProps {
  icon?: React.ReactNode;
  message: React.ReactNode;
  className?: string;
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
