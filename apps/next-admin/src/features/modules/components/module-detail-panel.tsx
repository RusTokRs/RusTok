'use client';

import { IconExternalLink, IconX } from '@tabler/icons-react';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';

import type { MarketplaceModule } from '../api';

function humanizeToken(value: string): string {
  return value
    .split(/[-_]/g)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

function shortChecksum(value?: string | null): string | null {
  if (!value) {
    return null;
  }

  return value.length > 16 ? `${value.slice(0, 12)}...` : value;
}

function formatTimestamp(value?: string | null): string {
  if (!value) {
    return 'Unknown';
  }

  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short'
  }).format(timestamp);
}

interface ModuleDetailPanelProps {
  adminSurface: 'leptos-admin' | 'next-admin';
  slug: string;
  module: MarketplaceModule | null;
  loading: boolean;
  onClose: () => void;
}

export function ModuleDetailPanel({
  adminSurface,
  slug,
  module,
  loading,
  onClose
}: ModuleDetailPanelProps) {
  const versionTrail = module?.versions.slice(0, 5) ?? [];
  const checksum = shortChecksum(module?.checksumSha256);
  const matchesPrimary = Boolean(
    module?.recommendedAdminSurfaces.includes(adminSurface)
  );
  const matchesShowcase = Boolean(
    module?.showcaseAdminSurfaces.includes(adminSurface)
  );

  return (
    <Card className='border-primary/20 bg-primary/5'>
      <CardHeader className='pb-3'>
        <div className='flex items-start justify-between gap-3'>
          <div className='space-y-1'>
            <CardTitle className='text-base'>Module detail</CardTitle>
            <CardDescription>
              {loading && !module
                ? `Loading ${slug} from the internal marketplace catalog.`
                : module
                  ? `${module.name} metadata from the internal marketplace catalog.`
                  : `No catalog entry resolved for ${slug}.`}
            </CardDescription>
          </div>
          <Button variant='ghost' size='sm' onClick={onClose}>
            <IconX className='mr-2 h-4 w-4' />
            Close
          </Button>
        </div>
      </CardHeader>
      <CardContent className='space-y-4'>
        {module ? (
          <>
            <div className='space-y-2'>
              <div className='flex flex-wrap items-center gap-2'>
                <h3 className='text-lg font-semibold'>{module.name}</h3>
                <Badge variant='outline'>v{module.latestVersion}</Badge>
                <Badge variant='secondary'>{humanizeToken(module.source)}</Badge>
                <Badge variant='outline'>{humanizeToken(module.category)}</Badge>
                <Badge variant={module.compatible ? 'outline' : 'destructive'}>
                  {module.compatible ? 'Compatible' : 'Compatibility risk'}
                </Badge>
                {module.signaturePresent && <Badge variant='secondary'>Signed</Badge>}
                {module.installed && (
                  <Badge variant='secondary'>
                    Installed
                    {module.installedVersion ? ` v${module.installedVersion}` : ''}
                  </Badge>
                )}
                {module.updateAvailable && (
                  <Badge variant='outline'>Update available</Badge>
                )}
              </div>
              <p className='text-muted-foreground text-sm'>{module.description}</p>
            </div>

            <div className='flex flex-wrap items-center gap-2 text-xs'>
              <Badge variant='secondary'>{humanizeToken(module.ownership)}</Badge>
              <Badge variant='outline'>{humanizeToken(module.trustLevel)}</Badge>
              <Badge variant='outline'>
                {matchesPrimary
                  ? 'Primary for this admin'
                  : matchesShowcase
                    ? 'Showcase for this admin'
                    : 'No dedicated UI for this admin'}
              </Badge>
            </div>

            <div className='grid gap-4 lg:grid-cols-2'>
              <div className='rounded-lg border bg-background/70 p-4 text-sm'>
                <p className='text-muted-foreground text-xs uppercase tracking-wide'>
                  Package metadata
                </p>
                <dl className='mt-3 space-y-2'>
                  <div className='flex items-start justify-between gap-3'>
                    <dt className='text-muted-foreground'>Slug</dt>
                    <dd className='font-mono text-right'>{module.slug}</dd>
                  </div>
                  <div className='flex items-start justify-between gap-3'>
                    <dt className='text-muted-foreground'>Crate</dt>
                    <dd className='font-mono text-right'>{module.crateName}</dd>
                  </div>
                  <div className='flex items-start justify-between gap-3'>
                    <dt className='text-muted-foreground'>Publisher</dt>
                    <dd className='text-right'>{module.publisher ?? 'Workspace / unknown'}</dd>
                  </div>
                  <div className='flex items-start justify-between gap-3'>
                    <dt className='text-muted-foreground'>RusTok range</dt>
                    <dd className='text-right'>
                      {module.rustokMinVersion ? `>= ${module.rustokMinVersion}` : 'no min'}
                      {module.rustokMaxVersion ? `, <= ${module.rustokMaxVersion}` : ', no max'}
                    </dd>
                  </div>
                  <div className='flex items-start justify-between gap-3'>
                    <dt className='text-muted-foreground'>Checksum</dt>
                    <dd className='font-mono text-right'>{checksum ?? 'Not published'}</dd>
                  </div>
                </dl>
              </div>

              <div className='rounded-lg border bg-background/70 p-4 text-sm'>
                <p className='text-muted-foreground text-xs uppercase tracking-wide'>
                  Surface policy
                </p>
                <div className='mt-3 space-y-3'>
                  <div className='flex flex-wrap gap-2'>
                    {module.recommendedAdminSurfaces.length > 0 ? (
                      module.recommendedAdminSurfaces.map((surface) => (
                        <Badge key={`${module.slug}-${surface}`} variant='outline'>
                          Primary: {humanizeToken(surface)}
                        </Badge>
                      ))
                    ) : (
                      <span className='text-muted-foreground text-xs'>
                        No primary admin surface declared.
                      </span>
                    )}
                  </div>
                  <div className='flex flex-wrap gap-2'>
                    {module.showcaseAdminSurfaces.length > 0 ? (
                      module.showcaseAdminSurfaces.map((surface) => (
                        <Badge
                          key={`${module.slug}-showcase-${surface}`}
                          variant='outline'
                        >
                          Showcase: {humanizeToken(surface)}
                        </Badge>
                      ))
                    ) : (
                      <span className='text-muted-foreground text-xs'>
                        No showcase admin surface declared.
                      </span>
                    )}
                  </div>
                  <div className='text-muted-foreground text-xs'>
                    {module.dependencies.length > 0
                      ? `Depends on: ${module.dependencies.join(', ')}`
                      : 'No module dependencies declared.'}
                  </div>
                </div>
              </div>
            </div>

            <div className='rounded-lg border bg-background/70 p-4'>
              <div className='flex items-center gap-2'>
                <p className='text-muted-foreground text-xs uppercase tracking-wide'>
                  Version history
                </p>
                {loading && <Badge variant='outline'>Refreshing</Badge>}
              </div>
              {versionTrail.length > 0 ? (
                <div className='mt-3 space-y-3'>
                  {versionTrail.map((version) => (
                    <div
                      key={`${module.slug}-${version.version}`}
                      className='flex flex-col gap-2 rounded-lg border px-3 py-3 text-sm'
                    >
                      <div className='flex flex-wrap items-center gap-2'>
                        <Badge variant='outline'>v{version.version}</Badge>
                        {version.yanked && <Badge variant='destructive'>Yanked</Badge>}
                        {version.signaturePresent && (
                          <Badge variant='secondary'>Signed</Badge>
                        )}
                        <span className='text-muted-foreground text-xs'>
                          {formatTimestamp(version.publishedAt)}
                        </span>
                      </div>
                      {version.changelog && (
                        <p className='text-muted-foreground text-sm'>{version.changelog}</p>
                      )}
                      {version.checksumSha256 && (
                        <div className='text-muted-foreground flex items-center gap-2 text-xs'>
                          <IconExternalLink className='h-3.5 w-3.5' />
                          <span className='font-mono'>
                            sha256 {shortChecksum(version.checksumSha256)}
                          </span>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <p className='text-muted-foreground mt-3 text-sm'>
                  No version history has been published for this module yet.
                </p>
              )}
            </div>
          </>
        ) : (
          <p className='text-muted-foreground text-sm'>
            The selected module is not available in the current catalog snapshot.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
