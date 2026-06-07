'use client';

import { useCallback, useEffect, useState } from 'react';

import {
  fetchSeoIndexDeliveryStatus,
  formatSeoReplayErrorMessage,
  runSeoIndexRepairReplay,
  type SeoIndexDeliveryStatusRecord
} from '@/shared/api/seo';

type SeoOperatorPanelProps = {
  token: string | null;
  tenantSlug: string | null;
};

type SeoActionKind = 'repair_only' | 'repair_replay';

function emitSeoTelemetry(kind: SeoActionKind, phase: 'started' | 'success' | 'failure') {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(
      new CustomEvent('seo-operator-action', {
        detail: {
          kind,
          phase,
          happenedAt: new Date().toISOString()
        }
      })
    );
  }
}

export function SeoOperatorPanel({ token, tenantSlug }: SeoOperatorPanelProps) {
  const [targetType, setTargetType] = useState<string>('');
  const [limit, setLimit] = useState<number>(100);
  const [confirmRepair, setConfirmRepair] = useState(false);
  const [confirmReplay, setConfirmReplay] = useState(false);
  const [status, setStatus] = useState<SeoIndexDeliveryStatusRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [busyAction, setBusyAction] = useState<SeoActionKind | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    setLoading(true);
    try {
      const nextStatus = await fetchSeoIndexDeliveryStatus({
        token,
        tenantSlug,
        preferRest: true,
        targetType: targetType || null
      });
      setStatus(nextStatus);
      setMessage(null);
    } catch (error) {
      setMessage(formatSeoReplayErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [targetType, token, tenantSlug]);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const runAction = useCallback(
    async (kind: SeoActionKind) => {
      const replayHistorical = kind === 'repair_replay';
      if (!replayHistorical && !confirmRepair) {
        setMessage('Confirm repair-only execution first.');
        return;
      }
      if (replayHistorical && !confirmReplay) {
        setMessage('Confirm historical replay execution first.');
        return;
      }

      emitSeoTelemetry(kind, 'started');
      setBusyAction(kind);
      setMessage(null);
      try {
        const result = await runSeoIndexRepairReplay(
          {
            targetType: targetType || null,
            limit,
            replayHistorical
          },
          {
            token,
            tenantSlug,
            preferRest: true
          }
        );
        emitSeoTelemetry(kind, 'success');
        setMessage(
          `Done: repaired=${result.repairedCount}, replayed=${result.replayedCount}, scanned=${result.historicalEventsScanned}`
        );
        await loadStatus();
      } catch (error) {
        emitSeoTelemetry(kind, 'failure');
        setMessage(formatSeoReplayErrorMessage(error));
      } finally {
        setBusyAction(null);
      }
    },
    [confirmRepair, confirmReplay, limit, loadStatus, targetType, token, tenantSlug]
  );

  return (
    <div className='space-y-4'>
      <div className='rounded-xl border border-border bg-card p-4'>
        <h3 className='text-base font-semibold text-card-foreground'>
          Index delivery observability
        </h3>
        <p className='mt-1 text-sm text-muted-foreground'>
          Track SEO → index transitions and run repair/replay operations with
          explicit confirmation.
        </p>
      </div>

      <div className='grid gap-3 rounded-xl border border-border bg-card p-4 md:grid-cols-[1fr_160px_auto]'>
        <select
          className='rounded-lg border border-border bg-background px-3 py-2 text-sm'
          value={targetType}
          onChange={(event) => setTargetType(event.target.value)}
          disabled={loading || busyAction !== null}
        >
          <option value=''>all</option>
          <option value='content'>content</option>
          <option value='product'>product</option>
        </select>
        <input
          type='number'
          min={1}
          max={500}
          className='rounded-lg border border-border bg-background px-3 py-2 text-sm'
          value={limit}
          onChange={(event) =>
            setLimit(Number.parseInt(event.target.value, 10) || 100)
          }
          disabled={loading || busyAction !== null}
        />
        <button
          type='button'
          className='rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-accent disabled:opacity-60'
          onClick={() => void loadStatus()}
          disabled={loading || busyAction !== null}
        >
          Refresh
        </button>
      </div>

      <div className='grid gap-3 rounded-xl border border-border bg-card p-4 md:grid-cols-5'>
        <MetricTile label='pending' value={status?.pendingCount ?? 0} />
        <MetricTile label='sent' value={status?.sentCount ?? 0} />
        <MetricTile label='retry' value={status?.retryCount ?? 0} />
        <MetricTile label='failed' value={status?.failedCount ?? 0} />
        <MetricTile label='dead_letter' value={status?.deadLetterCount ?? 0} />
      </div>

      <div className='grid gap-4 rounded-xl border border-border bg-card p-4 md:grid-cols-2'>
        <div className='space-y-3'>
          <label className='flex items-start gap-2 text-sm text-foreground'>
            <input
              type='checkbox'
              checked={confirmRepair}
              onChange={(event) => setConfirmRepair(event.target.checked)}
              disabled={busyAction !== null}
            />
            Confirm repair-only execution
          </label>
          <button
            type='button'
            className='w-full rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-accent disabled:opacity-60'
            onClick={() => void runAction('repair_only')}
            disabled={busyAction !== null || !confirmRepair}
          >
            Run repair only
          </button>
        </div>

        <div className='space-y-3'>
          <label className='flex items-start gap-2 text-sm text-foreground'>
            <input
              type='checkbox'
              checked={confirmReplay}
              onChange={(event) => setConfirmReplay(event.target.checked)}
              disabled={busyAction !== null}
            />
            Confirm repair + historical replay
          </label>
          <button
            type='button'
            className='w-full rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-60'
            onClick={() => void runAction('repair_replay')}
            disabled={busyAction !== null || !confirmReplay}
          >
            Run repair + replay
          </button>
        </div>
      </div>

      <div className='rounded-xl border border-border bg-card p-4'>
        <h4 className='text-sm font-semibold text-card-foreground'>Cursor timeline</h4>
        {status?.cursors?.length ? (
          <ul className='mt-3 space-y-2'>
            {status.cursors.map((cursor) => (
              <li
                key={cursor.targetType}
                className='rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground'
              >
                <div className='font-medium text-foreground'>
                  {cursor.targetType} · {cursor.replayMode} · forward-only
                </div>
                <div className='mt-1'>initial: {cursor.initialCursorAt}</div>
                <div>high-water: {cursor.highWaterMarkAt}</div>
                <div>
                  last repair: {cursor.lastRepairCursorAt ?? 'n/a'} · replay done:{' '}
                  {cursor.replayCompletedAt ?? 'n/a'}
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className='mt-2 text-sm text-muted-foreground'>No cursor data yet.</p>
        )}
      </div>

      <div className='rounded-xl border border-border bg-card p-4'>
        <h4 className='text-sm font-semibold text-card-foreground'>Failure drilldown</h4>
        {status?.failureSamples?.length ? (
          <ul className='mt-3 space-y-2'>
            {status.failureSamples.map((sample) => (
              <li
                key={`${sample.targetType}-${sample.targetId ?? 'none'}-${sample.updatedAt}`}
                className='rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground'
              >
                <div className='font-medium text-foreground'>
                  {sample.targetType} · {sample.status}
                </div>
                <div className='mt-1'>attempts: {sample.attemptCount}</div>
                <div>updated: {sample.updatedAt}</div>
                <div className='mt-1 break-words text-destructive'>
                  {sample.lastError ?? 'n/a'}
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className='mt-2 text-sm text-muted-foreground'>No failed/dead-letter samples.</p>
        )}
      </div>

      {message ? (
        <div className='rounded-xl border border-border bg-secondary/40 px-4 py-3 text-sm text-foreground'>
          {message}
        </div>
      ) : null}
    </div>
  );
}

function MetricTile({ label, value }: { label: string; value: number }) {
  return (
    <article className='rounded-lg border border-border bg-background px-3 py-2'>
      <p className='text-xs uppercase tracking-wide text-muted-foreground'>{label}</p>
      <p className='mt-1 text-lg font-semibold text-card-foreground'>{value}</p>
    </article>
  );
}
