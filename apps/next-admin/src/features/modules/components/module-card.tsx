'use client';

import { IconPackage, IconPlugConnected, IconPlugConnectedX } from '@tabler/icons-react';
import { useTranslations } from 'next-intl';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Switch } from '@/shared/ui/shadcn/switch';

import type { MarketplaceModule, ModuleInfo } from '../api';

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

interface ModuleCardProps {
  module: ModuleInfo;
  catalogModule?: MarketplaceModule | null;
  loading: boolean;
  platformLoading: boolean;
  platformInstalled: boolean;
  platformBusy: boolean;
  platformVersion?: string | null;
  recommendedVersion?: string | null;
  onToggle?: (slug: string, enabled: boolean) => void;
  onInstall?: (slug: string, version: string) => void;
  onInspect?: (slug: string) => void;
  onUninstall?: (slug: string) => void;
}

export function ModuleCard({
  module,
  catalogModule,
  loading,
  platformLoading,
  platformInstalled,
  platformBusy,
  platformVersion,
  recommendedVersion,
  onToggle,
  onInstall,
  onInspect,
  onUninstall
}: ModuleCardProps) {
  const t = useTranslations('modules');
  const isCore = module.kind === 'core';
  const tenantEnabled = isCore || (platformInstalled && module.enabled);
  const hasUpdate = Boolean(
    platformInstalled &&
      platformVersion &&
      recommendedVersion &&
      platformVersion !== recommendedVersion
  );
  const versionTrail = catalogModule?.versions.slice(0, 3) ?? [];
  const opacityClass = !tenantEnabled && !isCore ? 'opacity-60' : '';

  return (
    <Card className={`transition-opacity ${opacityClass}`}>
      <CardHeader className='pb-3'>
        <div className='flex items-start justify-between gap-3'>
          <div className='flex items-center gap-2'>
            {tenantEnabled ? (
              <IconPlugConnected className='text-primary h-5 w-5' />
            ) : (
              <IconPlugConnectedX className='text-muted-foreground h-5 w-5' />
            )}
            <CardTitle className='text-base'>{module.name}</CardTitle>
          </div>
          <div className='flex flex-wrap items-center justify-end gap-2'>
            {isCore && (
              <Badge variant='default' className='text-xs'>
                {t('badge.core')}
              </Badge>
            )}
            {!isCore && !platformInstalled && (
              <Badge variant='secondary' className='text-xs'>
                Not installed
              </Badge>
            )}
            {!isCore && platformInstalled && platformVersion && (
              <Badge variant='secondary' className='text-xs'>
                Manifest v{platformVersion}
              </Badge>
            )}
            {hasUpdate && recommendedVersion && (
              <Badge variant='outline' className='text-xs'>
                Update v{recommendedVersion}
              </Badge>
            )}
            <Badge variant='outline' className='text-xs'>
              v{module.version}
            </Badge>
          </div>
        </div>
        <CardDescription className='text-sm'>
          {module.description}
        </CardDescription>
      </CardHeader>
      <CardContent className='space-y-4'>
        <div className='flex flex-wrap items-center gap-2 text-xs'>
          <Badge variant='secondary'>{humanizeToken(module.ownership)}</Badge>
          <Badge variant='outline'>{humanizeToken(module.trustLevel)}</Badge>
          {catalogModule && (
            <Badge variant={catalogModule.compatible ? 'outline' : 'destructive'}>
              {catalogModule.compatible ? 'Compatible' : 'Compatibility risk'}
            </Badge>
          )}
          {catalogModule?.signaturePresent && (
            <Badge variant='secondary'>Signed</Badge>
          )}
          {module.recommendedAdminSurfaces.map((surface) => (
            <Badge key={`${module.moduleSlug}-${surface}`} variant='outline'>
              Primary: {humanizeToken(surface)}
            </Badge>
          ))}
          {module.showcaseAdminSurfaces.map((surface) => (
            <Badge key={`${module.moduleSlug}-showcase-${surface}`} variant='outline'>
              Showcase: {humanizeToken(surface)}
            </Badge>
          ))}
        </div>

        {catalogModule && (
          <div className='grid gap-2 rounded-lg border border-border/60 bg-muted/30 p-3 text-xs'>
            <div className='flex flex-wrap items-center gap-2'>
              <span className='text-muted-foreground'>Publisher:</span>
              <span>{catalogModule.publisher ?? 'Workspace / unknown'}</span>
              {catalogModule.checksumSha256 && (
                <Badge variant='outline' className='font-mono'>
                  sha256 {shortChecksum(catalogModule.checksumSha256)}
                </Badge>
              )}
            </div>
            <div className='flex flex-wrap items-center gap-2'>
              <span className='text-muted-foreground'>RusTok:</span>
              <span>
                {catalogModule.rustokMinVersion
                  ? `>= ${catalogModule.rustokMinVersion}`
                  : 'no min'}
                {catalogModule.rustokMaxVersion
                  ? `, <= ${catalogModule.rustokMaxVersion}`
                  : ', no max'}
              </span>
            </div>
            {versionTrail.length > 0 && (
              <div className='flex flex-wrap items-center gap-2'>
                <span className='text-muted-foreground'>Versions:</span>
                {versionTrail.map((version) => (
                  <Badge key={`${module.moduleSlug}-${version.version}`} variant='outline'>
                    v{version.version}
                    {version.yanked ? ' yanked' : ''}
                  </Badge>
                ))}
              </div>
            )}
          </div>
        )}

        <div className='flex items-center justify-between gap-3'>
          <div className='text-muted-foreground text-xs'>
            {module.dependencies.length > 0 && (
              <span>
                {t('depends_on')}: {module.dependencies.join(', ')}
              </span>
            )}
          </div>
          {isCore ? (
            <span className='text-muted-foreground text-xs'>
              Built into the platform manifest
            </span>
          ) : (
            <div className='flex items-center gap-2'>
              <span className='text-muted-foreground text-xs'>
                {platformInstalled
                  ? tenantEnabled
                    ? t('enabled')
                    : t('disabled')
                  : 'Unavailable'}
              </span>
              <Switch
                checked={tenantEnabled}
                disabled={loading || platformLoading || !platformInstalled}
                onCheckedChange={(checked) =>
                  onToggle?.(module.moduleSlug, checked)
                }
              />
            </div>
          )}
        </div>

        <div className='flex items-center justify-between gap-3 border-t pt-3'>
          <Button
            variant='ghost'
            size='sm'
            disabled={platformLoading}
            onClick={() => onInspect?.(module.moduleSlug)}
          >
            Details
          </Button>
          {isCore ? (
            <Badge variant='secondary' className='text-xs'>
              {t('always_on')}
            </Badge>
          ) : (
            <div className='flex items-center gap-3'>
              <div className='text-muted-foreground flex items-center gap-2 text-xs'>
                <IconPackage className='h-4 w-4' />
                <span>
                  {platformInstalled
                    ? platformVersion
                      ? `Installed in platform manifest as v${platformVersion}`
                      : 'Installed in platform manifest'
                    : 'Missing from platform manifest'}
                </span>
              </div>
              {platformInstalled ? (
                <Button
                  variant='outline'
                  size='sm'
                  disabled={platformBusy || platformLoading}
                  onClick={() => onUninstall?.(module.moduleSlug)}
                >
                  {platformLoading ? 'Queueing...' : 'Uninstall'}
                </Button>
              ) : (
                <Button
                  size='sm'
                  disabled={platformBusy || platformLoading}
                  onClick={() => onInstall?.(module.moduleSlug, module.version)}
                >
                  {platformLoading ? 'Queueing...' : 'Install'}
                </Button>
              )}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
