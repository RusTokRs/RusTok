'use client';

import { Badge } from '@/components/ui/badge';
import {
  IconPackage,
  IconShieldLock
} from '@tabler/icons-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useRouter } from 'next/navigation';
import { useTranslations } from 'next-intl';
import type { ModuleInfo } from '../api';
import { toggleModule } from '../api';
import { ModuleCard } from './module-card';

interface ModulesListProps {
  modules: ModuleInfo[];
}

export function ModulesList({ modules: initialModules }: ModulesListProps) {
  const [modules, setModules] = useState(initialModules);
  const [loading, setLoading] = useState<string | null>(null);
  const router = useRouter();
  const t = useTranslations('modules');

  const coreModules = modules.filter((m) => m.kind === 'core');
  const optionalModules = modules.filter((m) => m.kind === 'optional');

  const handleToggle = async (slug: string, enabled: boolean) => {
    setLoading(slug);
    try {
      const updated = await toggleModule(slug, enabled);
      setModules((prev) =>
        prev.map((m) => (m.moduleSlug === slug ? { ...m, enabled: updated.enabled } : m))
      );
      toast.success(
        updated.enabled ? t('toast.enabled') : t('toast.disabled')
      );
      router.refresh();
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : t('error.load')
      );
    } finally {
      setLoading(null);
    }
  };

  return (
    <div className='space-y-8'>
      {/* Core modules */}
      <div className='space-y-3'>
        <div className='flex items-center gap-2'>
          <IconShieldLock className='text-muted-foreground h-5 w-5' />
          <h3 className='text-lg font-semibold'>{t('section.core')}</h3>
          <Badge variant='secondary' className='text-xs'>
            {t('always_active')}
          </Badge>
        </div>
        <div className='grid gap-4 md:grid-cols-2 lg:grid-cols-3'>
          {coreModules.map((mod) => (
            <ModuleCard
              key={mod.moduleSlug}
              module={mod}
              loading={loading === mod.moduleSlug}
            />
          ))}
        </div>
      </div>

      {/* Optional modules */}
      <div className='space-y-3'>
        <div className='flex items-center gap-2'>
          <IconPackage className='text-muted-foreground h-5 w-5' />
          <h3 className='text-lg font-semibold'>{t('section.optional')}</h3>
        </div>
        <div className='grid gap-4 md:grid-cols-2 lg:grid-cols-3'>
          {optionalModules.map((mod) => (
            <ModuleCard
              key={mod.moduleSlug}
              module={mod}
              loading={loading === mod.moduleSlug}
              onToggle={handleToggle}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
