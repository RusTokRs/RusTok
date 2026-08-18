import type { DataTableConfig } from '@/config/data-table';
import type { FilterItemSchema } from '@/shared/lib/parsers';
import type {
  CellData,
  ColumnDef as TanStackColumnDef,
  Column as TanStackColumn,
  ColumnSort,
  Header as TanStackHeader,
  Row as TanStackRow,
  RowData,
  StockFeatures,
  Table as TanStackTable,
  TableFeatures
} from '@tanstack/react-table';

declare module '@tanstack/table-core' {
  // biome-ignore lint/correctness/noUnusedVariables: Interface type parameters required by @tanstack/table-core
  interface ColumnMeta<
    in out TFeatures extends TableFeatures,
    in out TData extends RowData,
    TValue extends CellData = CellData
  > {
    label?: string;
    placeholder?: string;
    variant?: FilterVariant;
    options?: Option[];
    range?: [number, number];
    unit?: string;
    icon?: React.FC<React.SVGProps<SVGSVGElement>>;
  }
}

declare module '@tanstack/react-table' {
  // biome-ignore lint/correctness/noUnusedVariables: Interface type parameters required by @tanstack/react-table
  interface ColumnMeta<
    in out TFeatures extends TableFeatures,
    in out TData extends RowData,
    TValue extends CellData = CellData
  > {
    label?: string;
    placeholder?: string;
    variant?: FilterVariant;
    options?: Option[];
    range?: [number, number];
    unit?: string;
    icon?: React.FC<React.SVGProps<SVGSVGElement>>;
  }
}

export type AppTable<TData extends RowData = any> = TanStackTable<
  StockFeatures,
  TData
>;
export type AppColumn<
  TData extends RowData = any,
  TValue = unknown
> = TanStackColumn<StockFeatures, TData, TValue>;
export type AppRow<TData extends RowData = any> = TanStackRow<
  StockFeatures,
  TData
>;
export type AppHeader<
  TData extends RowData = any,
  TValue = unknown
> = TanStackHeader<StockFeatures, TData, TValue>;
export type AppColumnDef<
  TData extends RowData = any,
  TValue = unknown
> = TanStackColumnDef<StockFeatures, TData, TValue>;

export interface Option {
  label: string;
  value: string;
  count?: number;
  icon?: React.FC<React.SVGProps<SVGSVGElement>>;
}

export type FilterOperator = DataTableConfig['operators'][number];
export type FilterVariant = DataTableConfig['filterVariants'][number];
export type JoinOperator = DataTableConfig['joinOperators'][number];

export interface ExtendedColumnSort<TData> extends Omit<ColumnSort, 'id'> {
  id: Extract<keyof TData, string>;
}

export interface ExtendedColumnFilter<TData> extends FilterItemSchema {
  id: Extract<keyof TData, string>;
}

export interface DataTableRowAction<TData extends RowData = any> {
  row: TanStackRow<StockFeatures, TData>;
  variant: 'update' | 'delete';
}
