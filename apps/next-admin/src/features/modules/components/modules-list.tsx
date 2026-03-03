'use client';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/components/ui/card';
import { Switch } from '@/components/ui/switch';
import {
  IconPackage,
  IconShieldLock,
  IconPlugConnected,
  IconPlugConnectedX
} from '@tabler/icons-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useRouter } from 'next/navigation';
import type { ModuleInfo } from '../api';
import { toggleModule } from '../api';

interface ModulesListProps {
  modules: ModuleInfo[];
}

export function ModulesList({ modules: initialModules }: ModulesListProps) {
  const [modules, setModules] = useState(initialModules);
  const [loading, setLoading] = useState<string | null>(null);
  const router = useRouter();

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
        `${updated.name} ${updated.enabled ? 'enabled' : 'disabled'}`
      );
      router.refresh();
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to toggle module'
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
          <h3 className='text-lg font-semibold'>Core Modules</h3>
          <Badge variant='secondary' className='text-xs'>
            Always active
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
          <h3 className='text-lg font-semibold'>Optional Modules</h3>
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

function ModuleCard({
  module,
  loading,
  onToggle
}: {
  module: ModuleInfo;
  loading: boolean;
  onToggle?: (slug: string, enabled: boolean) => void;
}) {
  const isCore = module.kind === 'core';

  return (
    <Card
      className={`transition-opacity ${!module.enabled && !isCore ? 'opacity-60' : ''}`}
    >
      <CardHeader className='pb-3'>
        <div className='flex items-start justify-between'>
          <div className='flex items-center gap-2'>
            {module.enabled ? (
              <IconPlugConnected className='text-primary h-5 w-5' />
            ) : (
              <IconPlugConnectedX className='text-muted-foreground h-5 w-5' />
            )}
            <CardTitle className='text-base'>{module.name}</CardTitle>
          </div>
          <div className='flex items-center gap-2'>
            {isCore && (
              <Badge variant='default' className='text-xs'>
                Core
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
      <CardContent>
        <div className='flex items-center justify-between'>
          <div className='text-muted-foreground text-xs'>
            {module.dependencies.length > 0 && (
              <span>Depends on: {module.dependencies.join(', ')}</span>
            )}
          </div>
          {isCore ? (
            <Badge variant='secondary' className='text-xs'>
              Always on
            </Badge>
          ) : (
            <div className='flex items-center gap-2'>
              <span className='text-muted-foreground text-xs'>
                {module.enabled ? 'Enabled' : 'Disabled'}
              </span>
              <Switch
                checked={module.enabled}
                disabled={loading}
                onCheckedChange={(checked) =>
                  onToggle?.(module.moduleSlug, checked)
                }
              />
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
