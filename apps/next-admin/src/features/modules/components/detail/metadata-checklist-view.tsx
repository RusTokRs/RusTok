'use client';

import { useMemo } from 'react';
import { Badge } from '@/shared/ui/shadcn/badge';
import type { MarketplaceModule } from '@/shared/api/modules';

interface MetadataItem {
  label: string;
  state: 'ready' | 'warn' | 'info';
  priority: 'required' | 'recommended' | 'optional';
  summary: string;
  detail: string;
}

interface MetadataChecklistViewProps {
  module: MarketplaceModule;
}

export function MetadataChecklistView({ module }: MetadataChecklistViewProps) {
  const checklist = useMemo<MetadataItem[]>(() => {
    const items: MetadataItem[] = [];

    // Description
    const descLen = module.description?.trim().length ?? 0;
    if (descLen >= 20) {
      items.push({
        label: 'Description',
        state: 'ready',
        priority: 'required',
        summary: 'Ready',
        detail: `${descLen} characters available for catalog detail.`
      });
    } else {
      items.push({
        label: 'Description',
        state: 'warn',
        priority: 'required',
        summary: 'Required Gap',
        detail: 'Needs at least 20 characters to satisfy manifest validation.'
      });
    }

    // Publisher
    if (module.publisher?.trim()) {
      items.push({
        label: 'Publisher Identity',
        state: 'ready',
        priority: 'required',
        summary: 'Ready',
        detail: `Published by ${module.publisher.trim()}`
      });
    } else {
      items.push({
        label: 'Publisher Identity',
        state: 'warn',
        priority: 'required',
        summary: 'Required Gap',
        detail: 'Missing publisher identity in module descriptor.'
      });
    }

    // Cryptographic Signature & Checksum
    if (module.signaturePresent && module.checksumSha256) {
      items.push({
        label: 'Integrity & Signature',
        state: 'ready',
        priority: 'required',
        summary: 'Signed',
        detail: `Valid SHA-256 checksum and cryptographic signature present.`
      });
    } else if (module.checksumSha256) {
      items.push({
        label: 'Integrity & Signature',
        state: 'warn',
        priority: 'recommended',
        summary: 'Unsigned',
        detail: 'Checksum is present, but package is not cryptographically signed.'
      });
    } else {
      items.push({
        label: 'Integrity & Signature',
        state: 'warn',
        priority: 'required',
        summary: 'Missing Checksum',
        detail: 'No content-addressed checksum available.'
      });
    }

    // Platform Compatibility Range
    const hasMin = Boolean(module.rustokMinVersion);
    const hasMax = Boolean(module.rustokMaxVersion);
    if (hasMin || hasMax) {
      items.push({
        label: 'Platform Compatibility',
        state: 'ready',
        priority: 'recommended',
        summary: 'Specified',
        detail: `Compatible range: ${module.rustokMinVersion ?? '0.1.0'} - ${module.rustokMaxVersion ?? 'unbounded'}`
      });
    } else {
      items.push({
        label: 'Platform Compatibility',
        state: 'warn',
        priority: 'recommended',
        summary: 'Unbounded',
        detail: 'No minimum or maximum RusTok platform versions defined.'
      });
    }

    // Release Trail
    const versionsCount = module.versions?.length ?? 0;
    const yankedCount = module.versions?.filter((v) => v.yanked).length ?? 0;
    if (versionsCount > 0) {
      items.push({
        label: 'Release History',
        state: 'ready',
        priority: 'optional',
        summary: `${versionsCount} version(s)`,
        detail:
          yankedCount > 0
            ? `${yankedCount} yanked version(s) recorded in immutable history.`
            : 'All recorded releases are active.'
      });
    } else {
      items.push({
        label: 'Release History',
        state: 'info',
        priority: 'optional',
        summary: 'Draft',
        detail: 'No published releases recorded in the registry.'
      });
    }

    return items;
  }, [module]);

  const requiredGaps = checklist.filter(
    (item) => item.priority === 'required' && item.state === 'warn'
  ).length;
  const recommendedGaps = checklist.filter(
    (item) => item.priority === 'recommended' && item.state === 'warn'
  ).length;
  const readySignals = checklist.filter((item) => item.state === 'ready').length;

  return (
    <div className='rounded-lg border bg-background/80 p-4 space-y-3'>
      <div className='flex flex-wrap items-center gap-2'>
        <p className='text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
          Registry Readiness Checklist
        </p>
        <Badge variant={requiredGaps > 0 ? 'destructive' : 'default'} className='text-xs'>
          {requiredGaps > 0
            ? `${requiredGaps} required issue(s)`
            : 'No required metadata gaps'}
        </Badge>
        {recommendedGaps > 0 && (
          <Badge variant='outline' className='text-xs text-amber-600 border-amber-500/40'>
            {recommendedGaps} recommended gap(s)
          </Badge>
        )}
        <Badge variant='secondary' className='text-xs'>
          {readySignals} ready signal(s)
        </Badge>
      </div>

      <div className='grid gap-3 sm:grid-cols-2 lg:grid-cols-3'>
        {checklist.map((item) => {
          const badgeVariant =
            item.state === 'ready'
              ? 'default'
              : item.state === 'warn'
                ? 'destructive'
                : 'secondary';
          const panelBorder =
            item.state === 'ready'
              ? 'border-emerald-500/30 bg-emerald-500/5'
              : item.state === 'warn'
                ? 'border-amber-500/30 bg-amber-500/5'
                : 'border-border bg-background';

          return (
            <div key={item.label} className={`rounded-lg border p-3 text-xs ${panelBorder}`}>
              <div className='flex items-center justify-between gap-2 mb-1'>
                <p className='font-semibold text-foreground'>{item.label}</p>
                <Badge variant={badgeVariant} className='text-[10px]'>
                  {item.summary}
                </Badge>
              </div>
              <p className='text-muted-foreground'>{item.detail}</p>
            </div>
          );
        })}
      </div>
    </div>
  );
}
