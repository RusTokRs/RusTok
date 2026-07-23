'use client';

import { useTranslations } from 'next-intl';
import { useState, useTransition } from 'react';
import { Button } from '@/shared/ui/shadcn/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/shadcn/card';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle
} from '@/shared/ui/shadcn/alert-dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import {
  saveEventDeliveryProfile,
  type EventDeliveryConfiguration,
  type EventsStatus
} from '../api/events';

interface EventsFormProps {
  status: EventsStatus;
  configuration: EventDeliveryConfiguration;
  token: string | null;
  tenantSlug: string | null;
}

export function EventsForm({ status, configuration, token, tenantSlug }: EventsFormProps) {
  const t = useTranslations('events');
  const [isPending, startTransition] = useTransition();
  const [profile, setProfile] = useState(configuration.desiredProfile);
  const [showIggyConfiguration, setShowIggyConfiguration] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; msg?: string } | null>(null);

  const profileOptions = [
    { value: 'memory', label: t('transport.memory') },
    { value: 'outbox_local', label: t('transport.outboxLocal') },
    { value: 'outbox_iggy', label: t('transport.outboxIggy') }
  ];
  const restartRequired = profile !== configuration.activeProfile || configuration.restartRequired;

  function selectProfile(nextProfile: string) {
    if (nextProfile === 'outbox_iggy' && !configuration.iggyConfigured) {
      setShowIggyConfiguration(true);
      return;
    }
    setProfile(nextProfile);
  }

  function handleSave() {
    startTransition(async () => {
      try {
        const saved = await saveEventDeliveryProfile(profile, { token, tenantSlug });
        setResult({ ok: true, msg: saved.restartRequired ? t('saved') : undefined });
      } catch (error) {
        setResult({ ok: false, msg: String(error) });
      }
    });
  }

  return (
    <div className='space-y-6'>
      <Card>
        <CardHeader><CardTitle className='text-base'>{t('transport.label')}</CardTitle></CardHeader>
        <CardContent className='space-y-3'>
          <Select value={profile} onValueChange={selectProfile}>
            <SelectTrigger className='w-80'><SelectValue /></SelectTrigger>
            <SelectContent>
              {profileOptions.map((option) => <SelectItem key={option.value} value={option.value}>{option.label}</SelectItem>)}
            </SelectContent>
          </Select>
          <p className='text-muted-foreground text-sm'>{t('transport.help')}</p>
          {restartRequired && <p className='text-xs text-amber-600'>{t('transport.restartRequired')}</p>}
          {profile === 'outbox_iggy' && (
            <p className='text-sm text-amber-700 dark:text-amber-300'>{t('transport.iggyDeployment', { mode: configuration.iggyMode })}</p>
          )}
        </CardContent>
      </Card>

      <div className='flex items-center gap-4'>
        <Button onClick={handleSave} disabled={isPending}>{isPending ? t('saving') : t('save')}</Button>
        {result?.ok && <span className='text-sm text-green-600'>{result.msg ?? t('saved')}</span>}
        {result && !result.ok && <span className='text-destructive text-sm'>{result.msg ?? t('error')}</span>}
      </div>

      <Card>
        <CardHeader><CardTitle className='text-base'>{t('status.title')}</CardTitle></CardHeader>
        <CardContent>
          <dl className='grid max-w-md grid-cols-2 gap-x-4 gap-y-2 text-sm'>
            <dt className='text-muted-foreground'>{t('status.activeProfile')}</dt><dd className='font-mono font-medium'>{status.configuredProfile}</dd>
            <dt className='text-muted-foreground'>{t('status.desiredProfile')}</dt><dd className='font-mono font-medium'>{profile}</dd>
            <dt className='text-muted-foreground'>{t('status.pendingEvents')}</dt><dd className='font-medium'>{status.pendingEvents}</dd>
            <dt className='text-muted-foreground'>{t('status.dlqEvents')}</dt><dd className='font-medium'>{status.dlqEvents}</dd>
          </dl>
        </CardContent>
      </Card>

      <AlertDialog open={showIggyConfiguration} onOpenChange={setShowIggyConfiguration}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('iggyConfiguration.title')}</AlertDialogTitle>
            <AlertDialogDescription>{t('iggyConfiguration.description')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('iggyConfiguration.cancel')}</AlertDialogCancel>
            <AlertDialogAction asChild>
              <a href='/dashboard/iggy-connector'>{t('iggyConfiguration.confirm')}</a>
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
