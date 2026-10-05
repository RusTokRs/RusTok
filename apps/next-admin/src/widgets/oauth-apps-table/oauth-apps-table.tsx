'use client';

import * as React from 'react';
import type { OAuthApp } from '@/entities/oauth-app';
import { DataTable } from '@/widgets/data-table/data-table';
import { DataTableToolbar } from '@/widgets/data-table/data-table-toolbar';
import { DataTableSkeleton } from '@/widgets/data-table/data-table-skeleton';
import { useDataTable } from '@/shared/hooks/use-data-table';
import { parseAsInteger, useQueryState } from 'nuqs';
import { getOAuthAppsColumns } from './columns';

export function OAuthAppsTable({
  apps,
  isLoading,
  onEditApp,
  onRotateSecret,
  onRevokeApp
}: {
  apps: OAuthApp[];
  isLoading?: boolean;
  onEditApp: (app: OAuthApp) => void;
  onRotateSecret: (app: OAuthApp) => void;
  onRevokeApp: (app: OAuthApp) => void;
}) {
  const [pageSize] = useQueryState('perPage', parseAsInteger.withDefault(10));
  const pageCount = Math.max(1, Math.ceil(apps.length / pageSize));

  const columns = React.useMemo(
    () =>
      getOAuthAppsColumns({
        onEditApp,
        onRotateSecret,
        onRevokeApp
      }),
    [onEditApp, onRotateSecret, onRevokeApp]
  );

  const { table } = useDataTable({
    data: apps,
    columns,
    pageCount,
    shallow: false,
    debounceMs: 300
  });

  if (isLoading) {
    return <DataTableSkeleton columnCount={7} rowCount={5} />;
  }

  return (
    <DataTable table={table}>
      <DataTableToolbar table={table} />
    </DataTable>
  );
}
