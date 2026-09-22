'use client';

import { OAuthApp, OAuthAppTypeBadge } from '@/entities/oauth-app';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';

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
  return (
    <div className='overflow-x-auto rounded-md border'>
      <table className='w-full min-w-[960px] text-left text-sm'>
        <thead className='bg-muted/50 text-muted-foreground text-xs uppercase'>
          <tr>
            <th className='px-4 py-3 font-medium'>App</th>
            <th className='px-4 py-3 font-medium'>Type</th>
            <th className='px-4 py-3 font-medium'>Scopes / Grants</th>
            <th className='px-4 py-3 font-medium'>Client ID</th>
            <th className='px-4 py-3 font-medium'>Tokens</th>
            <th className='px-4 py-3 font-medium'>Last Used</th>
            <th className='px-4 py-3 text-right font-medium'>Actions</th>
          </tr>
        </thead>
        <tbody className='divide-y'>
          {isLoading ? (
            <tr>
              <td
                colSpan={7}
                className='text-muted-foreground h-24 px-4 text-center'
              >
                Loading app connections...
              </td>
            </tr>
          ) : null}

          {!isLoading &&
            apps.map((app) => (
              <tr key={app.id} className='hover:bg-muted/40 transition-colors'>
                <td className='px-4 py-3 align-top'>
                  <div className='font-medium text-slate-900'>{app.name}</div>
                  <div className='text-muted-foreground text-xs'>
                    {app.slug}
                  </div>
                  {app.description ? (
                    <div className='text-muted-foreground mt-1 max-w-xs text-xs'>
                      {app.description}
                    </div>
                  ) : null}
                  <div className='mt-2'>
                    <Badge
                      variant={app.managedByManifest ? 'secondary' : 'outline'}
                    >
                      {capabilityHint(app)}
                    </Badge>
                  </div>
                </td>
                <td className='px-4 py-3 align-top'>
                  <OAuthAppTypeBadge appType={app.appType} />
                </td>
                <td className='px-4 py-3 align-top'>
                  <div className='max-w-xs text-xs text-slate-600'>
                    <div>
                      <span className='font-medium text-slate-900'>
                        Scopes:
                      </span>{' '}
                      {app.scopes.join(', ') || 'None'}
                    </div>
                    <div className='mt-1'>
                      <span className='font-medium text-slate-900'>
                        Grants:
                      </span>{' '}
                      {app.grantTypes.join(', ') || 'None'}
                    </div>
                  </div>
                </td>
                <td className='px-4 py-3 align-top font-mono text-xs text-slate-500'>
                  {app.clientId}
                </td>
                <td className='px-4 py-3 align-top text-slate-500'>
                  {app.activeTokenCount}
                </td>
                <td className='px-4 py-3 align-top text-xs text-slate-500'>
                  {formatDate(app.lastUsedAt)}
                </td>
                <td className='px-4 py-3 text-right align-top'>
                  <div className='flex justify-end gap-2'>
                    <Button
                      variant='outline'
                      size='sm'
                      onClick={() => onEditApp(app)}
                      disabled={!app.canEdit}
                      title={
                        app.canEdit ? 'Edit app' : 'Managed by config/manifest'
                      }
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
                      title={
                        app.canRevoke
                          ? 'Revoke app'
                          : 'Managed by config/manifest'
                      }
                    >
                      Revoke
                    </Button>
                  </div>
                </td>
              </tr>
            ))}

          {!isLoading && apps.length === 0 ? (
            <tr>
              <td
                colSpan={7}
                className='text-muted-foreground h-24 px-4 text-center'
              >
                No app connections found.
              </td>
            </tr>
          ) : null}
        </tbody>
      </table>
    </div>
  );
}
