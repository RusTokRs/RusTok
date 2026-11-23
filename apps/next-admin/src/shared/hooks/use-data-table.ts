'use client';

import {
  type ColumnDef,
  type ColumnFiltersState,
  type ColumnVisibilityState,
  type PaginationState,
  type RowData,
  type RowSelectionState,
  type SortingState,
  type StockFeatures,
  type TableOptions,
  type TableState,
  type Updater,
  stockFeatures,
  useTable
} from '@tanstack/react-table';
import {
  type Parser,
  type UseQueryStateOptions,
  parseAsArrayOf,
  parseAsInteger,
  parseAsString,
  useQueryState,
  useQueryStates
} from 'nuqs';
import * as React from 'react';

import { useDebouncedCallback } from '@/shared/hooks/use-debounced-callback';
import { getSortingStateParser } from '@/shared/lib/parsers';
import type { ExtendedColumnSort } from '@/types/data-table';

const PAGE_KEY = 'page';
const PER_PAGE_KEY = 'perPage';
const SORT_KEY = 'sort';
const ARRAY_SEPARATOR = ',';
const DEBOUNCE_MS = 300;
const THROTTLE_MS = 50;
const useIsomorphicLayoutEffect =
  typeof window === 'undefined' ? React.useEffect : React.useLayoutEffect;

function columnFiltersSignature(filters: ColumnFiltersState): string {
  return JSON.stringify(
    [...filters].sort((left, right) => left.id.localeCompare(right.id))
  );
}

function routeQuerySignature(query: Record<string, string>): string {
  return JSON.stringify(
    Object.entries(query).sort(([left], [right]) => left.localeCompare(right))
  );
}

function tableStateSignature(
  page: number,
  perPage: number,
  sorting: SortingState,
  filters: string
): string {
  return JSON.stringify([page, perPage, sorting, filters]);
}

interface UseDataTableProps<TData extends RowData = any>
  extends
    Omit<
      TableOptions<StockFeatures, TData>,
      | 'features'
      | 'state'
      | 'pageCount'
      | 'manualFiltering'
      | 'manualPagination'
      | 'manualSorting'
    >,
    Required<Pick<TableOptions<StockFeatures, TData>, 'pageCount'>> {
  columns: ColumnDef<StockFeatures, TData, any>[];
  /** The supplied collection is complete and the canonical grid owns its page slice. */
  clientPagination?: boolean;
  initialState?: Omit<Partial<TableState<StockFeatures>>, 'sorting'> & {
    sorting?: ExtendedColumnSort<TData>[];
  };
  history?: 'push' | 'replace';
  debounceMs?: number;
  throttleMs?: number;
  clearOnDefault?: boolean;
  enableAdvancedFilter?: boolean;
  scroll?: boolean;
  shallow?: boolean;
  /** Full server route query, including non-table filters such as Product facets. */
  routeQuery?: Record<string, string>;
  startTransition?: React.TransitionStartFunction;
}

export function useDataTable<TData extends RowData = any>(
  props: UseDataTableProps<TData>
) {
  const {
    columns,
    clientPagination = false,
    pageCount = -1,
    initialState,
    history = 'replace',
    debounceMs = DEBOUNCE_MS,
    throttleMs = THROTTLE_MS,
    clearOnDefault = false,
    enableAdvancedFilter = false,
    scroll = false,
    shallow = true,
    routeQuery,
    startTransition,
    ...tableProps
  } = props;

  const queryStateOptions = React.useMemo<
    Omit<UseQueryStateOptions<string>, 'parse'>
  >(
    () => ({
      history,
      scroll,
      shallow,
      throttleMs,
      debounceMs,
      clearOnDefault,
      startTransition
    }),
    [
      history,
      scroll,
      shallow,
      throttleMs,
      debounceMs,
      clearOnDefault,
      startTransition
    ]
  );

  const [rowSelection, setRowSelection] = React.useState<RowSelectionState>(
    initialState?.rowSelection ?? {}
  );
  const [columnVisibility, setColumnVisibility] =
    React.useState<ColumnVisibilityState>(initialState?.columnVisibility ?? {});

  const [page, setPage] = useQueryState(
    PAGE_KEY,
    parseAsInteger.withOptions(queryStateOptions).withDefault(1)
  );
  const [perPage, setPerPage] = useQueryState(
    PER_PAGE_KEY,
    parseAsInteger
      .withOptions(queryStateOptions)
      .withDefault(initialState?.pagination?.pageSize ?? 10)
  );

  const pagination: PaginationState = React.useMemo(() => {
    return {
      pageIndex: page - 1, // zero-based index -> one-based index
      pageSize: perPage
    };
  }, [page, perPage]);

  const onPaginationChange = React.useCallback(
    (updaterOrValue: Updater<PaginationState>) => {
      if (typeof updaterOrValue === 'function') {
        const newPagination = updaterOrValue(pagination);
        void setPage(newPagination.pageIndex + 1);
        void setPerPage(newPagination.pageSize);
      } else {
        void setPage(updaterOrValue.pageIndex + 1);
        void setPerPage(updaterOrValue.pageSize);
      }
    },
    [pagination, setPage, setPerPage]
  );

  const columnIds = React.useMemo(() => {
    return new Set(
      columns
        .map((column: ColumnDef<StockFeatures, TData, any>) => column.id)
        .filter(Boolean) as string[]
    );
  }, [columns]);

  const [sorting, setSorting] = useQueryState(
    SORT_KEY,
    getSortingStateParser<TData>(columnIds)
      .withOptions(queryStateOptions)
      .withDefault(initialState?.sorting ?? [])
  );

  const onSortingChange = React.useCallback(
    (updaterOrValue: Updater<SortingState>) => {
      if (typeof updaterOrValue === 'function') {
        const newSorting = updaterOrValue(sorting);
        setSorting(newSorting as ExtendedColumnSort<TData>[]);
      } else {
        setSorting(updaterOrValue as ExtendedColumnSort<TData>[]);
      }
    },
    [sorting, setSorting]
  );

  const filterableColumns = React.useMemo(() => {
    if (enableAdvancedFilter) return [];

    return columns.filter(
      (column: ColumnDef<StockFeatures, TData, any>) =>
        column.enableColumnFilter
    );
  }, [columns, enableAdvancedFilter]);

  const filterParsers = React.useMemo(() => {
    if (enableAdvancedFilter) return {};

    return filterableColumns.reduce<
      Record<string, Parser<string> | Parser<string[]>>
    >((acc, column: ColumnDef<StockFeatures, TData, any>) => {
      if (column.meta?.options) {
        acc[column.id ?? ''] = parseAsArrayOf(
          parseAsString,
          ARRAY_SEPARATOR
        ).withOptions(queryStateOptions);
      } else {
        acc[column.id ?? ''] = parseAsString.withOptions(queryStateOptions);
      }
      return acc;
    }, {});
  }, [filterableColumns, queryStateOptions, enableAdvancedFilter]);

  const [filterValues, setFilterValues] = useQueryStates(filterParsers);

  const filterNavigationVersion = React.useRef(0);
  // More than one local route write can be in flight while the server renders.
  // Keep their signatures ordered so an older response cannot invalidate a newer edit.
  const pendingLocalRouteSignatures = React.useRef<string[]>([]);

  const initialColumnFilters: ColumnFiltersState = React.useMemo(() => {
    if (enableAdvancedFilter) return [];

    return Object.entries(filterValues).reduce<ColumnFiltersState>(
      (filters, [key, value]) => {
        if (value !== null) {
          // Text filters must remain scalar strings for their controlled inputs.
          // Faceted filters already arrive as arrays from their URL parser.
          filters.push({
            id: key,
            value
          });
        }
        return filters;
      },
      []
    );
  }, [filterValues, enableAdvancedFilter]);

  const [columnFilters, setColumnFilters] =
    React.useState<ColumnFiltersState>(initialColumnFilters);

  const currentFilterSignature = columnFiltersSignature(initialColumnFilters);
  const currentRouteSignature = routeQuery
    ? routeQuerySignature(routeQuery)
    : tableStateSignature(page, perPage, sorting, currentFilterSignature);
  const debouncedSetFilterValues = useDebouncedCallback(
    (values: typeof filterValues, version: number) => {
      // Any route navigation invalidates callbacks scheduled against the old URL.
      if (version !== filterNavigationVersion.current) return;

      const expectedFilters = Object.entries(values)
        .filter(([, value]) => value !== null)
        .map(([id, value]) => ({ id, value }));
      const expectedFilterSignature = columnFiltersSignature(expectedFilters);

      let expectedRouteSignature: string;
      if (routeQuery) {
        const expectedQuery = { ...routeQuery };
        // nuqs does not rewrite the default page when it is already selected.
        if (page !== 1 || Object.prototype.hasOwnProperty.call(expectedQuery, PAGE_KEY)) {
          expectedQuery[PAGE_KEY] = '1';
        }
        for (const [key, value] of Object.entries(values)) {
          if (value === null) {
            delete expectedQuery[key];
          } else {
            expectedQuery[key] = Array.isArray(value)
              ? value.join(ARRAY_SEPARATOR)
              : value;
          }
        }
        expectedRouteSignature = routeQuerySignature(expectedQuery);
      } else {
        expectedRouteSignature = tableStateSignature(
          1,
          perPage,
          sorting,
          expectedFilterSignature
        );
      }

      if (
        expectedRouteSignature !== currentRouteSignature &&
        !pendingLocalRouteSignatures.current.includes(expectedRouteSignature)
      ) {
        pendingLocalRouteSignatures.current.push(expectedRouteSignature);
      }

      if (page !== 1) void setPage(1);
      void setFilterValues(values);
    },
    debounceMs
  );

  // Server navigation can change facets/category/page without changing the
  // table's own filter keys. Compare the complete route query when supplied.
  // Local filter writes are recognized so they do not cancel a newer edit.
  const lastUrlRouteSignature = React.useRef(currentRouteSignature);
  useIsomorphicLayoutEffect(() => {
    if (lastUrlRouteSignature.current !== currentRouteSignature) {
      lastUrlRouteSignature.current = currentRouteSignature;
      const matchedLocalNavigation =
        pendingLocalRouteSignatures.current.indexOf(currentRouteSignature);
      if (matchedLocalNavigation !== -1) {
        // Consume this navigation and all older writes it supersedes, while
        // preserving any newer local route writes that are still in flight.
        pendingLocalRouteSignatures.current.splice(0, matchedLocalNavigation + 1);
        return;
      }

      pendingLocalRouteSignatures.current = [];
      filterNavigationVersion.current += 1;
      setColumnFilters(initialColumnFilters);
    }
  }, [currentRouteSignature, initialColumnFilters]);

  const onColumnFiltersChange = React.useCallback(
    (updaterOrValue: Updater<ColumnFiltersState>) => {
      if (enableAdvancedFilter) return;

      setColumnFilters((prev) => {
        const next =
          typeof updaterOrValue === 'function'
            ? updaterOrValue(prev)
            : updaterOrValue;

        const filterUpdates = next.reduce<
          Record<string, string | string[] | null>
        >((acc, filter) => {
          if (
            filterableColumns.find(
              (column: ColumnDef<StockFeatures, TData, any>) =>
                column.id === filter.id
            )
          ) {
            acc[filter.id] = filter.value as string | string[];
          }
          return acc;
        }, {});

        for (const prevFilter of prev) {
          if (!next.some((filter) => filter.id === prevFilter.id)) {
            filterUpdates[prevFilter.id] = null;
          }
        }

        debouncedSetFilterValues(filterUpdates, filterNavigationVersion.current);
        return next;
      });
    },
    [debouncedSetFilterValues, filterableColumns, enableAdvancedFilter]
  );

  const table = useTable({
    ...tableProps,
    features: stockFeatures,
    columns,
    initialState,
    pageCount,
    state: {
      pagination,
      sorting,
      columnVisibility,
      rowSelection,
      columnFilters
    },
    defaultColumn: {
      ...tableProps.defaultColumn,
      enableColumnFilter: false
    },
    enableRowSelection: true,
    onRowSelectionChange: setRowSelection,
    onPaginationChange,
    onSortingChange,
    onColumnFiltersChange,
    onColumnVisibilityChange: setColumnVisibility,
    manualPagination: !clientPagination,
    manualSorting: true,
    manualFiltering: true
  });

  return { table, shallow, debounceMs, throttleMs };
}
