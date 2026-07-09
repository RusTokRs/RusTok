'use client';

import { useState, useTransition } from 'react';
import { useTranslations } from 'next-intl';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Switch } from '@/shared/ui/shadcn/switch';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { toast } from 'sonner';
import {
  updateEmailSettings,
  type EmailSettings,
  type GqlOpts
} from '../api/email';

interface EmailSettingsFormProps {
  initialSettings: EmailSettings;
  opts: GqlOpts;
}

export function EmailSettingsForm({
  initialSettings,
  opts
}: EmailSettingsFormProps) {
  const t = useTranslations('email');
  const [settings, setSettings] = useState<EmailSettings>(initialSettings);
  const [isPending, startTransition] = useTransition();

  const handleSave = () => {
    startTransition(async () => {
      try {
        const updated = await updateEmailSettings(settings, opts);
        setSettings(updated);
        toast.success(t('savedToast'));
      } catch {
        toast.error(t('errorToast'));
      }
    });
  };

  return (
    <div className='space-y-6'>
      <Card>
        <CardHeader>
          <CardTitle>{t('general.title')}</CardTitle>
          <CardDescription>{t('general.description')}</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='flex items-center gap-3'>
            <Switch
              id='email-enabled'
              checked={settings.enabled}
              onCheckedChange={(checked) =>
                setSettings((s) => ({ ...s, enabled: checked }))
              }
            />
            <Label htmlFor='email-enabled'>{t('general.enabled')}</Label>
          </div>
          <div className='space-y-2'>
            <Label htmlFor='email-from'>{t('general.from')}</Label>
            <Input
              id='email-from'
              type='email'
              value={settings.from}
              onChange={(e) =>
                setSettings((s) => ({ ...s, from: e.target.value }))
              }
              placeholder='noreply@example.com'
            />
          </div>
          <div className='space-y-2'>
            <Label htmlFor='reset-base-url'>{t('general.resetBaseUrl')}</Label>
            <Input
              id='reset-base-url'
              value={settings.resetBaseUrl}
              onChange={(e) =>
                setSettings((s) => ({ ...s, resetBaseUrl: e.target.value }))
              }
              placeholder='https://example.com'
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t('smtp.title')}</CardTitle>
          <CardDescription>{t('smtp.description')}</CardDescription>
        </CardHeader>
        <CardContent className='grid gap-4 sm:grid-cols-2'>
          <div className='space-y-2'>
            <Label htmlFor='smtp-host'>{t('smtp.host')}</Label>
            <Input
              id='smtp-host'
              value={settings.smtpHost}
              onChange={(e) =>
                setSettings((s) => ({ ...s, smtpHost: e.target.value }))
              }
              placeholder='smtp.example.com'
            />
          </div>
          <div className='space-y-2'>
            <Label htmlFor='smtp-port'>{t('smtp.port')}</Label>
            <Input
              id='smtp-port'
              type='number'
              value={settings.smtpPort}
              onChange={(e) =>
                setSettings((s) => ({ ...s, smtpPort: Number(e.target.value) }))
              }
              placeholder='587'
            />
          </div>
          <div className='space-y-2'>
            <Label htmlFor='smtp-username'>{t('smtp.username')}</Label>
            <Input
              id='smtp-username'
              value={settings.smtpUsername}
              onChange={(e) =>
                setSettings((s) => ({ ...s, smtpUsername: e.target.value }))
              }
            />
          </div>
          <div className='space-y-2'>
            <Label htmlFor='smtp-password'>{t('smtp.password')}</Label>
            <Input
              id='smtp-password'
              type='password'
              value={settings.smtpPassword}
              onChange={(e) =>
                setSettings((s) => ({ ...s, smtpPassword: e.target.value }))
              }
            />
          </div>
        </CardContent>
      </Card>

      <div className='flex justify-end'>
        <Button onClick={handleSave} disabled={isPending}>
          {isPending ? t('saving') : t('save')}
        </Button>
      </div>
    </div>
  );
}
