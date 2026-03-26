'use client';

import { useTranslations } from 'next-intl';
import { useTransition, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card';
import { Input } from '@/shared/ui/input';
import { Label } from '@/shared/ui/label';
import { Button } from '@/shared/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/select';
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

  // Filter available transports: always show memory + outbox; iggy options only if in availableTransports
  const iggyAvailable = status.availableTransports.some(t => t.startsWith('iggy'));
  const transportOptions = [
    { value: 'memory', label: t('transport.memory') },
    { value: 'outbox', label: t('transport.outbox') },
    ...(iggyAvailable
      ? [
          { value: 'iggy_embedded', label: t('transport.iggyEmbedded') },
          { value: 'iggy_external', label: t('transport.iggyExternal') }
        ]
      : [])
  ];

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
                  onChange={e => setRelayIntervalMs(e.target.value)}
                  placeholder='1000'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('outbox.maxAttempts')}</Label>
                <Input
                  value={maxAttempts}
                  onChange={e => setMaxAttempts(e.target.value)}
                  placeholder='5'
                />
              </div>
              <div className='flex items-center gap-2 pt-2'>
                <input
                  type='checkbox'
                  id='dlq-enabled'
                  className='h-4 w-4 rounded border-input'
                  checked={dlqEnabled}
                  onChange={e => setDlqEnabled(e.target.checked)}
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
                  onChange={e => setIggyAddresses(e.target.value)}
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
                  onChange={e => setIggyUsername(e.target.value)}
                  placeholder='iggy'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.password')}</Label>
                <Input
                  type='password'
                  value={iggyPassword}
                  onChange={e => setIggyPassword(e.target.value)}
                  placeholder='••••••••'
                />
              </div>
              <div className='flex items-center gap-2 pt-2'>
                <input
                  type='checkbox'
                  id='iggy-tls'
                  className='h-4 w-4 rounded border-input'
                  checked={iggyTls}
                  onChange={e => setIggyTls(e.target.checked)}
                />
                <Label htmlFor='iggy-tls'>{t('iggy.tlsEnabled')}</Label>
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.stream')}</Label>
                <Input
                  value={iggyStream}
                  onChange={e => setIggyStream(e.target.value)}
                  placeholder='rustok'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.partitions')}</Label>
                <Input
                  value={iggyPartitions}
                  onChange={e => setIggyPartitions(e.target.value)}
                  placeholder='8'
                />
              </div>
              <div className='space-y-1.5'>
                <Label>{t('iggy.replication')}</Label>
                <Input
                  value={iggyReplication}
                  onChange={e => setIggyReplication(e.target.value)}
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
