'use client';

import * as React from 'react';
import type { OAuthApp } from '@/entities/oauth-app';
import { DataTableShell } from '@/widgets/data-table/data-table-shell';
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
  const columns = React.useMemo(
    () =>
      getOAuthAppsColumns({
        onEditApp,
        onRotateSecret,
        onRevokeApp
      }),
    [onEditApp, onRotateSecret, onRevokeApp]
  );

  return (
    <DataTableShell
      data={apps}
      columns={columns}
      defaultPageSize={10}
      debounceMs={300}
      isLoading={isLoading}
      skeletonColumns={7}
    />
  );
}
