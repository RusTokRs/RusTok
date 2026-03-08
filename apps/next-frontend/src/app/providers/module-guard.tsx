'use client';

import type { ReactNode } from 'react';

import { useIsModuleEnabled } from '@/shared/hooks/use-enabled-modules';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export function ModuleGuard({
  slug,
  children,
  fallback
}: {
  slug: string;
  children: ReactNode;
  fallback?: ReactNode;
}) {
  const isEnabled = useIsModuleEnabled(slug);

  if (!isEnabled) {
    return <>{fallback ?? <ModuleUnavailable slug={slug} />}</>;
  }

  return <>{children}</>;
}
