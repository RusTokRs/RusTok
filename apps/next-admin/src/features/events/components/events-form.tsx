'use client';

import { useTranslations } from 'next-intl';
import { useTransition, useState, type ChangeEvent } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { saveEventsSettings, type EventsSettings, type EventsStatus } from '../api/events';

interface EventsFormProps {
  status: EventsStatus;
  initialSettings: EventsSettings | null;
  token: string | null;
  tenantSlug: string | null;
}

const DEFAULT_SETTINGS: EventsSettings = {
  transport: 'memory',
  relay_interval_ms: 1000,
  max_attempts: 5,
  dlq_enabled: true,
  iggy_addresses: '127.0.0.1:8090',
  iggy_protocol: 'tcp',
  iggy_username: 'iggy',
  iggy_password: '',
  iggy_tls: false,
  iggy_stream: 'rustok',
  iggy_partitions: 8,
  iggy_replication: 1
};

export function EventsForm({ status, initialSettings, token, tenantSlug }: EventsFormProps) {
  const t = useTranslations('events');
  const [isPending, startTransition] = useTransition();
  const [result, setResult] = useState<{ ok: boolean; msg?: string } | null>(null);

  const merged = { ...DEFAULT_SETTINGS, ...initialSettings };
  const [transport, setTransport] = useState(merged.transport || status.configuredTransport);
  const [relayIntervalMs, setRelayIntervalMs] = useState(String(merged.relay_interval_ms));
  const [maxAttempts, setMaxAttempts] = useState(String(merged.max_attempts));
  const [dlqEnabled, setDlqEnabled] = useState(merged.dlq_enabled);
  const [iggyAddresses, setIggyAddresses] = useState(merged.iggy_addresses);
  const [iggyProtocol, setIggyProtocol] = useState(merged.iggy_protocol);
  const [iggyUsername, setIggyUsername] = useState(merged.iggy_username);
  const [iggyPassword, setIggyPassword] = useState(merged.iggy_password);
  const [iggyTls, setIggyTls] = useState(merged.iggy_tls);
  const [iggyStream, setIggyStream] = useState(merged.iggy_stream);
  const [iggyPartitions, setIggyPartitions] = useState(String(merged.iggy_partitions));
  const [iggyReplication, setIggyReplication] = useState(String(merged.iggy_replication));

  // All 4 transports always shown; warn when iggy is selected but module not registered
  const iggyAvailable = status.availableTransports.some(t => t.startsWith('iggy'));
  const transportOptions = [
    { value: 'memory', label: t('transport.memory') },
    { value: 'outbox', label: t('transport.outbox') },
    { value: 'iggy_embedded', label: t('transport.iggyEmbedded') },
    { value: 'iggy_external', label: t('transport.iggyExternal') }
  ];
  const showIggyWarning = transport.startsWith('iggy') && !iggyAvailable;

  const showOutboxSettings = transport === 'outbox' || transport.startsWith('iggy');
  const showIggyExternal = transport === 'iggy_external';
  const transportChanged = transport !== status.configuredTransport;

  function handleSave() {
    startTransition(async () => {
      try {
        await saveEventsSettings(
          {
            transport,
            relay_interval_ms: parseInt(relayIntervalMs) || 1000,
            max_attempts: parseInt(maxAttempts) || 5,
            dlq_enabled: dlqEnabled,
            iggy_addresses: iggyAddresses,
            iggy_protocol: iggyProtocol,
            iggy_username: iggyUsername,
            iggy_password: iggyPassword,
            iggy_tls: iggyTls,
            iggy_stream: iggyStream,
            iggy_partitions: parseInt(iggyPartitions) || 8,
            iggy_replication: parseInt(iggyReplication) || 1
          },
          { token, tenantSlug }
        );
        setResult({ ok: true });
      } catch (e) {
        setResult({ ok: false, msg: String(e) });
      }
    });
  }

  return (
    <div className='space-y-6'>
      {/* Transport selector */}
      <Card>
        <CardHeader>
          <CardTitle className='text-base'>{t('transport.label')}</CardTitle>
        </CardHeader>
        <CardContent className='space-y-3'>
          <Select value={transport} onValueChange={setTransport}>
            <SelectTrigger className='w-72'>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {transportOptions.map(opt => (
                <SelectItem key={opt.value} value={opt.value}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {transportChanged && (
            <p className='text-xs text-amber-600'>{t('transport.restartRequired')}</p>
          )}
          {showIggyWarning && (
            <div className='flex items-start gap-2 rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-200'>
              <svg
                className='mt-0.5 h-4 w-4 shrink-0'
                fill='none'
                viewBox='0 0 24 24'
                stroke='currentColor'
                strokeWidth={2}
              >
                <path
                  strokeLinecap='round'
                  strokeLinejoin='round'
                  d='M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z'
                />
              </svg>
              <span>{t('transport.moduleDisabledWarning')}</span>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Outbox settings */}
      {showOutboxSettings && (
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>{t('outbox.title')}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className='grid gap-4 sm:grid-cols-2 max-w-xl'>
              <div className='space-y-1.5'>
                <Label>{t('outbox.relayIntervalMs')}</Label>
                <Input
                  value={relayIntervalMs}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setRelayIntervalMs(e.target.value)
                  }
                  placeholder='1000'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('outbox.maxAttempts')}</Label>
                <Input
                  value={maxAttempts}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setMaxAttempts(e.target.value)
                  }
                  placeholder='5'
                />
              </div>
              <div className='flex items-center gap-2 pt-2'>
                <input
                  type='checkbox'
                  id='dlq-enabled'
                  className='h-4 w-4 rounded border-input'
                  checked={dlqEnabled}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setDlqEnabled(e.target.checked)
                  }
                />
                <Label htmlFor='dlq-enabled'>{t('outbox.dlqEnabled')}</Label>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* External Iggy form */}
      {showIggyExternal && (
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>{t('iggy.title')}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className='grid gap-4 sm:grid-cols-2 max-w-xl'>
              <div className='space-y-1.5 sm:col-span-2'>
                <Label>{t('iggy.addresses')}</Label>
                <Input
                  value={iggyAddresses}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setIggyAddresses(e.target.value)
                  }
                  placeholder='127.0.0.1:8090'
                />
                <p className='text-xs text-muted-foreground'>Comma-separated list of addresses</p>
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.protocol')}</Label>
                <Select value={iggyProtocol} onValueChange={setIggyProtocol}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='tcp'>TCP</SelectItem>
                    <SelectItem value='http'>HTTP</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.username')}</Label>
                <Input
                  value={iggyUsername}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setIggyUsername(e.target.value)
                  }
                  placeholder='iggy'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.password')}</Label>
                <Input
                  type='password'
                  value={iggyPassword}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setIggyPassword(e.target.value)
                  }
                  placeholder='••••••••'
                />
              </div>
              <div className='flex items-center gap-2 pt-2'>
                <input
                  type='checkbox'
                  id='iggy-tls'
                  className='h-4 w-4 rounded border-input'
                  checked={iggyTls}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setIggyTls(e.target.checked)
                  }
                />
                <Label htmlFor='iggy-tls'>{t('iggy.tlsEnabled')}</Label>
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.stream')}</Label>
                <Input
                  value={iggyStream}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setIggyStream(e.target.value)
                  }
                  placeholder='rustok'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.partitions')}</Label>
                <Input
                  value={iggyPartitions}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setIggyPartitions(e.target.value)
                  }
                  placeholder='8'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.replication')}</Label>
                <Input
                  value={iggyReplication}
                  onChange={(e: ChangeEvent<HTMLInputElement>) =>
                    setIggyReplication(e.target.value)
                  }
                  placeholder='1'
                />
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Save */}
      <div className='flex items-center gap-4'>
        <Button onClick={handleSave} disabled={isPending}>
          {isPending ? t('saving') : t('save')}
        </Button>
        {result?.ok && (
          <span className='text-sm text-green-600'>{t('saved')}</span>
        )}
        {result && !result.ok && (
          <span className='text-sm text-destructive'>{result.msg ?? t('error')}</span>
        )}
      </div>

      {/* Runtime status */}
      <Card>
        <CardHeader>
          <CardTitle className='text-base'>{t('status.title')}</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className='grid grid-cols-2 gap-x-4 gap-y-2 text-sm max-w-sm'>
            <dt className='text-muted-foreground'>{t('status.transport')}</dt>
            <dd className='font-mono font-medium'>{status.configuredTransport}</dd>
            <dt className='text-muted-foreground'>{t('status.pendingEvents')}</dt>
            <dd className='font-medium'>{status.pendingEvents}</dd>
            <dt className='text-muted-foreground'>{t('status.dlqEvents')}</dt>
            <dd className='font-medium'>{status.dlqEvents}</dd>
            <dt className='text-muted-foreground'>{t('status.relayInterval')}</dt>
            <dd className='font-medium'>{status.relayIntervalMs} ms</dd>
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}
