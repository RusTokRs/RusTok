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

function emitSeoTelemetry(
  kind: SeoActionKind,
  phase: 'started' | 'success' | 'failure'
) {
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
  const [status, setStatus] = useState<SeoIndexDeliveryStatusRecord | null>(
    null
  );
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
    [
      confirmRepair,
      confirmReplay,
      limit,
      loadStatus,
      targetType,
      token,
      tenantSlug
    ]
  );

  return (
    <div className='space-y-4'>
      <div className='border-border bg-card rounded-xl border p-4'>
        <h3 className='text-card-foreground text-base font-semibold'>
          Index delivery observability
        </h3>
        <p className='text-muted-foreground mt-1 text-sm'>
          Track SEO → index transitions and run repair/replay operations with
          explicit confirmation.
        </p>
      </div>

      <div className='border-border bg-card grid gap-3 rounded-xl border p-4 md:grid-cols-[1fr_160px_auto]'>
        <select
          className='border-border bg-background rounded-lg border px-3 py-2 text-sm'
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
          className='border-border bg-background rounded-lg border px-3 py-2 text-sm'
          value={limit}
          onChange={(event) =>
            setLimit(Number.parseInt(event.target.value, 10) || 100)
          }
          disabled={loading || busyAction !== null}
        />
        <button
          type='button'
          className='border-border hover:bg-accent rounded-lg border px-3 py-2 text-sm font-medium disabled:opacity-60'
          onClick={() => void loadStatus()}
          disabled={loading || busyAction !== null}
        >
          Refresh
        </button>
      </div>

      <div className='border-border bg-card grid gap-3 rounded-xl border p-4 md:grid-cols-5'>
        <MetricTile label='pending' value={status?.pendingCount ?? 0} />
        <MetricTile label='sent' value={status?.sentCount ?? 0} />
        <MetricTile label='retry' value={status?.retryCount ?? 0} />
        <MetricTile label='failed' value={status?.failedCount ?? 0} />
        <MetricTile label='dead_letter' value={status?.deadLetterCount ?? 0} />
      </div>

      <div className='border-border bg-card grid gap-4 rounded-xl border p-4 md:grid-cols-2'>
        <div className='space-y-3'>
          <label className='text-foreground flex items-start gap-2 text-sm'>
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
            className='border-border hover:bg-accent w-full rounded-lg border px-3 py-2 text-sm font-medium disabled:opacity-60'
            onClick={() => void runAction('repair_only')}
            disabled={busyAction !== null || !confirmRepair}
          >
            Run repair only
          </button>
        </div>

        <div className='space-y-3'>
          <label className='text-foreground flex items-start gap-2 text-sm'>
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
            className='bg-primary text-primary-foreground hover:bg-primary/90 w-full rounded-lg px-3 py-2 text-sm font-medium disabled:opacity-60'
            onClick={() => void runAction('repair_replay')}
            disabled={busyAction !== null || !confirmReplay}
          >
            Run repair + replay
          </button>
        </div>
      </div>

      <div className='border-border bg-card rounded-xl border p-4'>
        <h4 className='text-card-foreground text-sm font-semibold'>
          Cursor timeline
        </h4>
        {status?.cursors?.length ? (
          <ul className='mt-3 space-y-2'>
            {status.cursors.map((cursor) => (
              <li
                key={cursor.targetType}
                className='border-border bg-background text-muted-foreground rounded-lg border px-3 py-2 text-xs'
              >
                <div className='text-foreground font-medium'>
                  {cursor.targetType} · {cursor.replayMode} · forward-only
                </div>
                <div className='mt-1'>initial: {cursor.initialCursorAt}</div>
                <div>high-water: {cursor.highWaterMarkAt}</div>
                <div>
                  last repair: {cursor.lastRepairCursorAt ?? 'n/a'} · replay
                  done: {cursor.replayCompletedAt ?? 'n/a'}
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className='text-muted-foreground mt-2 text-sm'>
            No cursor data yet.
          </p>
        )}
      </div>

      <div className='border-border bg-card rounded-xl border p-4'>
        <h4 className='text-card-foreground text-sm font-semibold'>
          Failure drilldown
        </h4>
        {status?.failureSamples?.length ? (
          <ul className='mt-3 space-y-2'>
            {status.failureSamples.map((sample) => (
              <li
                key={`${sample.targetType}-${sample.targetId ?? 'none'}-${sample.updatedAt}`}
                className='border-border bg-background text-muted-foreground rounded-lg border px-3 py-2 text-xs'
              >
                <div className='text-foreground font-medium'>
                  {sample.targetType} · {sample.status}
                </div>
                <div className='mt-1'>attempts: {sample.attemptCount}</div>
                <div>updated: {sample.updatedAt}</div>
                <div className='text-destructive mt-1 break-words'>
                  {sample.lastError ?? 'n/a'}
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className='text-muted-foreground mt-2 text-sm'>
            No failed/dead-letter samples.
          </p>
        )}
      </div>

      {message ? (
        <div className='border-border bg-secondary/40 text-foreground rounded-xl border px-4 py-3 text-sm'>
          {message}
        </div>
      ) : null}
    </div>
  );
}

function MetricTile({ label, value }: { label: string; value: number }) {
  return (
    <article className='border-border bg-background rounded-lg border px-3 py-2'>
      <p className='text-muted-foreground text-xs tracking-wide uppercase'>
        {label}
      </p>
      <p className='text-card-foreground mt-1 text-lg font-semibold'>{value}</p>
    </article>
  );
}
