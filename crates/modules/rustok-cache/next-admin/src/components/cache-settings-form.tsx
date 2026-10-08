'use client';

import { useState, useTransition } from 'react';
import { useTranslations } from '@rustok/next-fluent';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { toast } from 'sonner';
import {
  updateCacheSettings,
  type CacheSettings,
  type GqlOpts
} from '../api/cache';

interface CacheSettingsFormProps {
  initialSettings: CacheSettings;
  opts: GqlOpts;
}

export function CacheSettingsForm({
  initialSettings,
  opts
}: CacheSettingsFormProps) {
  const t = useTranslations('cache');
  const [settings, setSettings] = useState<CacheSettings>(initialSettings);
  const [isPending, startTransition] = useTransition();

  const isRedisMode = settings.mode === 'redis' || settings.mode === 'hybrid';

  const handleSave = () => {
    startTransition(async () => {
      try {
        const payload: Partial<CacheSettings> = {
          mode: settings.mode,
          redis_url: settings.redis_url || '',
          redis_host: settings.redis_host || '127.0.0.1',
          redis_port: Number(settings.redis_port) || 6379,
          redis_db: Number(settings.redis_db) || 0
        };

        if (settings.redis_password) {
          payload.redis_password = settings.redis_password;
        }

        const updated = await updateCacheSettings(payload, opts);
        setSettings(updated);
        toast.success(t('settings.savedToast'));
      } catch {
        toast.error(t('settings.errorToast'));
      }
    });
  };

  return (
    <div className='space-y-6'>
      <Card>
        <CardHeader>
          <CardTitle>{t('settings.title')}</CardTitle>
          <CardDescription>{t('settings.description')}</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='space-y-2 max-w-md'>
            <Label htmlFor='cache-mode'>{t('settings.mode')}</Label>
            <Select
              value={settings.mode}
              onValueChange={(val: 'in-memory' | 'redis' | 'hybrid') =>
                setSettings((s) => ({ ...s, mode: val }))
              }
            >
              <SelectTrigger id='cache-mode' className='w-full'>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='in-memory'>
                  {t('settings.mode.inmemory')}
                </SelectItem>
                <SelectItem value='redis'>
                  {t('settings.mode.redis')}
                </SelectItem>
                <SelectItem value='hybrid'>
                  {t('settings.mode.hybrid')}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>

      {/* Conditionally rendered Redis configuration card */}
      {isRedisMode && (
        <Card className='animate-in fade-in-50 slide-in-from-top-2 duration-300'>
          <CardHeader>
            <CardTitle>{t('settings.redis.title')}</CardTitle>
            <CardDescription>{t('settings.redis.description')}</CardDescription>
          </CardHeader>
          <CardContent className='space-y-4'>
            <div className='grid gap-4 sm:grid-cols-2'>
              <div className='space-y-2'>
                <Label htmlFor='redis-host'>{t('settings.redis.host')}</Label>
                <Input
                  id='redis-host'
                  value={settings.redis_host ?? '127.0.0.1'}
                  onChange={(e) =>
                    setSettings((s) => ({ ...s, redis_host: e.target.value }))
                  }
                  placeholder='127.0.0.1'
                />
              </div>

              <div className='space-y-2'>
                <Label htmlFor='redis-port'>{t('settings.redis.port')}</Label>
                <Input
                  id='redis-port'
                  type='number'
                  min={1}
                  max={65535}
                  value={settings.redis_port ?? 6379}
                  onChange={(e) =>
                    setSettings((s) => ({
                      ...s,
                      redis_port: Number(e.target.value)
                    }))
                  }
                  placeholder='6379'
                />
              </div>

              <div className='space-y-2'>
                <Label htmlFor='redis-password'>
                  {t('settings.redis.password')}
                </Label>
                <Input
                  id='redis-password'
                  type='password'
                  value={settings.redis_password ?? ''}
                  onChange={(e) =>
                    setSettings((s) => ({
                      ...s,
                      redis_password: e.target.value
                    }))
                  }
                  placeholder={t('settings.redis.password.placeholder')}
                />
              </div>

              <div className='space-y-2'>
                <Label htmlFor='redis-db'>{t('settings.redis.db')}</Label>
                <Input
                  id='redis-db'
                  type='number'
                  min={0}
                  max={255}
                  value={settings.redis_db ?? 0}
                  onChange={(e) =>
                    setSettings((s) => ({
                      ...s,
                      redis_db: Number(e.target.value)
                    }))
                  }
                  placeholder='0'
                />
              </div>
            </div>

            <div className='space-y-2 pt-2 border-t'>
              <Label htmlFor='redis-url'>{t('settings.redis.url')}</Label>
              <Input
                id='redis-url'
                value={settings.redis_url ?? ''}
                onChange={(e) =>
                  setSettings((s) => ({ ...s, redis_url: e.target.value }))
                }
                placeholder={t('settings.redis.url.placeholder')}
              />
            </div>
          </CardContent>
        </Card>
      )}

      <div className='flex justify-end'>
        <Button onClick={handleSave} disabled={isPending}>
          {isPending ? t('settings.saving') : t('settings.save')}
        </Button>
      </div>
    </div>
  );
}
