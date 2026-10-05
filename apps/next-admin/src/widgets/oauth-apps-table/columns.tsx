'use client';

import * as React from 'react';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { DataTableColumnHeader } from '@/widgets/data-table/data-table-column-header';
import { OAuthApp, OAuthAppTypeBadge } from '@/entities/oauth-app';

export const APP_TYPE_OPTIONS = [
  { label: 'Public', value: 'PUBLIC' },
  { label: 'Confidential', value: 'CONFIDENTIAL' },
  { label: 'System', value: 'SYSTEM' }
];

function formatDate(value?: string): string {
  if (!value) {
    return 'Never';
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short'
  }).format(date);
}

function capabilityHint(app: OAuthApp): string {
  if (app.managedByManifest) {
    return 'Managed by config/manifest';
  }

  return 'Manual app';
}

export interface GetOAuthAppsColumnsOptions {
  onEditApp: (app: OAuthApp) => void;
  onRotateSecret: (app: OAuthApp) => void;
  onRevokeApp: (app: OAuthApp) => void;
}

export function getOAuthAppsColumns({
  onEditApp,
  onRotateSecret,
  onRevokeApp
}: GetOAuthAppsColumnsOptions): ColumnDef<StockFeatures, OAuthApp, any>[] {
  return [
    {
      id: 'name',
      accessorKey: 'name',
      header: ({ column }) => (
        <DataTableColumnHeader column={column} title='App' />
      ),
      cell: ({ row }) => {
        const app = row.original;
        return (
          <div className='space-y-1 align-top'>
            <div className='font-medium text-slate-900 dark:text-slate-100'>
              {app.name}
            </div>
            <div className='text-muted-foreground font-mono text-xs'>
              {app.slug}
            </div>
            {app.description ? (
              <div className='text-muted-foreground line-clamp-2 max-w-xs text-xs'>
                {app.description}
              </div>
            ) : null}
            <div className='pt-1'>
              <Badge variant={app.managedByManifest ? 'secondary' : 'outline'}>
                {capabilityHint(app)}
              </Badge>
            </div>
          </div>
        );
      },
      enableColumnFilter: true,
      meta: {
        label: 'App Name'
      }
    },
    {
      id: 'appType',
      accessorKey: 'appType',
      header: ({ column }) => (
        <DataTableColumnHeader column={column} title='Type' />
      ),
      cell: ({ row }) => <OAuthAppTypeBadge appType={row.original.appType} />,
      enableColumnFilter: true,
      meta: {
        label: 'Type',
        variant: 'select',
        options: APP_TYPE_OPTIONS
      }
    },
    {
      id: 'scopes',
      header: 'Scopes / Grants',
      cell: ({ row }) => {
        const app = row.original;
        return (
          <div className='max-w-xs text-xs text-slate-600 dark:text-slate-400'>
            <div>
              <span className='font-medium text-slate-900 dark:text-slate-200'>
                Scopes:
              </span>{' '}
              {app.scopes.join(', ') || 'None'}
            </div>
            <div className='mt-1'>
              <span className='font-medium text-slate-900 dark:text-slate-200'>
                Grants:
              </span>{' '}
              {app.grantTypes.join(', ') || 'None'}
            </div>
          </div>
        );
      },
      enableSorting: false
    },
    {
      id: 'clientId',
      accessorKey: 'clientId',
      header: ({ column }) => (
        <DataTableColumnHeader column={column} title='Client ID' />
      ),
      cell: ({ row }) => (
        <span className='font-mono text-xs text-slate-500'>
          {row.original.clientId}
        </span>
      ),
      meta: {
        label: 'Client ID'
      }
    },
    {
      id: 'activeTokenCount',
      accessorKey: 'activeTokenCount',
      header: ({ column }) => (
        <DataTableColumnHeader column={column} title='Tokens' />
      ),
      cell: ({ row }) => (
        <span className='text-sm text-slate-600 dark:text-slate-400'>
          {row.original.activeTokenCount}
        </span>
      ),
      meta: {
        label: 'Tokens'
      }
    },
    {
      id: 'lastUsedAt',
      accessorKey: 'lastUsedAt',
      header: ({ column }) => (
        <DataTableColumnHeader column={column} title='Last Used' />
      ),
      cell: ({ row }) => (
        <span className='text-xs text-slate-500'>
          {formatDate(row.original.lastUsedAt)}
        </span>
      ),
      meta: {
        label: 'Last Used'
      }
    },
    {
      id: 'actions',
      header: () => <span className='sr-only'>Actions</span>,
      cell: ({ row }) => {
        const app = row.original;
        return (
          <div className='flex justify-end gap-2'>
            <Button
              variant='outline'
              size='sm'
              onClick={() => onEditApp(app)}
              disabled={!app.canEdit}
              title={app.canEdit ? 'Edit app' : 'Managed by config/manifest'}
            >
              Edit
            </Button>
            <Button
              variant='outline'
              size='sm'
              onClick={() => onRotateSecret(app)}
              disabled={!app.canRotateSecret}
              title={
                app.canRotateSecret
                  ? 'Rotate client secret'
                  : 'This app does not expose a client secret'
              }
            >
              Rotate Secret
            </Button>
            <Button
              variant='destructive'
              size='sm'
              onClick={() => onRevokeApp(app)}
              disabled={!app.canRevoke}
              title={app.canRevoke ? 'Revoke app' : 'Managed by config/manifest'}
            >
              Revoke
            </Button>
          </div>
        );
      },
      enableSorting: false,
      enableHiding: false
    }
  ];
}
