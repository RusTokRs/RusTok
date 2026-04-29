'use client';

import { IconRefresh } from '@tabler/icons-react';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';

import type { InstalledModule, MarketplaceModule } from '../api';

function humanizeToken(value: string): string {
  return value
    .split(/[-_]/g)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

interface ModuleUpdateCardProps {
  module: MarketplaceModule;
  installedModule: InstalledModule;
  loading: boolean;
  platformBusy: boolean;
  onInspect?: (slug: string) => void;
  onUpgrade: (slug: string, version: string) => void;
}

function shortChecksum(value?: string | null): string | null {
  if (!value) {
    return null;
  }

  return value.length > 16 ? `${value.slice(0, 12)}...` : value;
}

export function ModuleUpdateCard({
  module,
  installedModule,
  loading,
  platformBusy,
  onInspect,
  onUpgrade
}: ModuleUpdateCardProps) {
  const currentVersion = installedModule.version ?? 'unpinned';
  const versionTrail = module.versions.slice(0, 3);

  return (
    <Card>
      <CardHeader className='pb-3'>
        <div className='flex items-start justify-between gap-3'>
          <div>
            <CardTitle className='text-base'>{module.name}</CardTitle>
            <CardDescription className='mt-1 text-sm'>
              {module.description}
            </CardDescription>
          </div>
          <Badge variant='outline' className='text-xs'>
            {installedModule.source}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className='space-y-4'>
        <div className='flex flex-wrap items-center gap-2 text-xs'>
          <Badge variant='secondary'>{humanizeToken(module.ownership)}</Badge>
          <Badge variant='outline'>{humanizeToken(module.trustLevel)}</Badge>
          <Badge variant={module.compatible ? 'outline' : 'destructive'}>
            {module.compatible ? 'Compatible' : 'Compatibility risk'}
          </Badge>
          {module.signaturePresent && <Badge variant='secondary'>Signed</Badge>}
          <Badge variant='secondary'>Current v{currentVersion}</Badge>
          <Badge variant='outline'>Latest v{module.latestVersion}</Badge>
          {module.recommendedAdminSurfaces.map((surface) => (
            <Badge key={`${module.slug}-${surface}`} variant='outline'>
              Primary: {humanizeToken(surface)}
            </Badge>
          ))}
          {module.showcaseAdminSurfaces.map((surface) => (
            <Badge key={`${module.slug}-showcase-${surface}`} variant='outline'>
              Showcase: {humanizeToken(surface)}
            </Badge>
          ))}
        </div>

        <div className='border-border/60 bg-muted/30 grid gap-2 rounded-lg border p-3 text-xs'>
          <div className='flex flex-wrap items-center gap-2'>
            <span className='text-muted-foreground'>Publisher:</span>
            <span>{module.publisher ?? 'Workspace / unknown'}</span>
            {module.checksumSha256 && (
              <Badge variant='outline' className='font-mono'>
                sha256 {shortChecksum(module.checksumSha256)}
              </Badge>
            )}
          </div>
          <div className='flex flex-wrap items-center gap-2'>
            <span className='text-muted-foreground'>RusTok:</span>
            <span>
              {module.rustokMinVersion
                ? `>= ${module.rustokMinVersion}`
                : 'no min'}
              {module.rustokMaxVersion
                ? `, <= ${module.rustokMaxVersion}`
                : ', no max'}
            </span>
          </div>
          {versionTrail.length > 0 && (
            <div className='flex flex-wrap items-center gap-2'>
              <span className='text-muted-foreground'>Versions:</span>
              {versionTrail.map((version) => (
                <Badge
                  key={`${module.slug}-${version.version}`}
                  variant='outline'
                >
                  v{version.version}
                  {version.yanked ? ' yanked' : ''}
                </Badge>
              ))}
            </div>
          )}
        </div>

        {module.dependencies.length > 0 && (
          <p className='text-muted-foreground text-xs'>
            Depends on: {module.dependencies.join(', ')}
          </p>
        )}

        <p className='text-muted-foreground text-xs'>
          Upgrade writes the target version into modules.toml and queues a
          platform rebuild for both admin stacks.
        </p>

        <div className='flex items-center justify-between gap-3'>
          <Button
            variant='ghost'
            size='sm'
            disabled={loading}
            onClick={() => onInspect?.(module.slug)}
          >
            Details
          </Button>
          <Button
            size='sm'
            disabled={platformBusy || loading}
            onClick={() => onUpgrade(module.slug, module.latestVersion)}
          >
            <IconRefresh className='mr-2 h-4 w-4' />
            {loading ? 'Queueing...' : `Upgrade to v${module.latestVersion}`}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
