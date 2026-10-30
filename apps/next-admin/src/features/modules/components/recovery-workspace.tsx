'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from '@rustok/next-fluent';
import {
  compensateFailedModuleOperation,
  listFailedModuleOperationRecoveryPlans,
  listTenantModules,
  retryFailedModuleOperationPostHook,
  type GqlOpts,
  type ModuleOperationRecoveryPlan,
  type TenantModule
} from '@/shared/api/modules';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';

interface RecoveryWorkspaceProps {
  apiOpts: GqlOpts;
  onModuleUpdated?: (module: TenantModule) => void;
  onRefresh?: () => void;
}

type PendingCommand = { revision: number; key: string };

/** Operator controls for owner-issued failed lifecycle operations. */
export function RecoveryWorkspace({
  apiOpts,
  onModuleUpdated,
  onRefresh
}: RecoveryWorkspaceProps) {
  const t = useTranslations('modules');
  const [plans, setPlans] = useState<ModuleOperationRecoveryPlan[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busyOperation, setBusyOperation] = useState<string | null>(null);
  const generation = useRef(0);
  const scopeGeneration = useRef(0);
  const pending = useRef(new Map<string, PendingCommand>());

  const refresh = useCallback(async () => {
    const current = ++generation.current;
    setLoading(true);
    setError(null);
    try {
      const next = await listFailedModuleOperationRecoveryPlans(
        undefined,
        10,
        apiOpts
      );
      if (current === generation.current) setPlans(next);
    } catch (cause) {
      if (current === generation.current) {
        setPlans([]);
        setError(
          cause instanceof Error ? cause.message : t('recovery.load.error')
        );
      }
    } finally {
      if (current === generation.current) setLoading(false);
    }
  }, [apiOpts, t]);

  useEffect(() => {
    setBusyOperation(null);
    setPlans([]);
    void refresh();
    return () => {
      generation.current += 1;
      scopeGeneration.current += 1;
      pending.current.clear();
    };
  }, [refresh]);

  const execute = async (
    plan: ModuleOperationRecoveryPlan,
    action: 'retry' | 'compensate'
  ) => {
    if (
      busyOperation ||
      (action === 'retry' && !plan.retryable) ||
      (action === 'compensate' && plan.issue !== 'post_hook_failed')
    )
      return;
    setBusyOperation(plan.operationId);
    setError(null);
    const commandId = `${plan.operationId}:${action}`;
    const scope = scopeGeneration.current;
    try {
      let command = pending.current.get(commandId);
      if (!command) {
        const modules = await listTenantModules(apiOpts);
        if (scope !== scopeGeneration.current) return;
        const current = modules.find(
          (item) => item.moduleSlug === plan.moduleSlug
        );
        if (!current) throw new Error(t('recovery.revision.error'));
        command = { revision: current.revision, key: crypto.randomUUID() };
        pending.current.set(commandId, command);
      }
      if (action === 'retry') {
        await retryFailedModuleOperationPostHook(
          plan.operationId,
          command.revision,
          command.key,
          apiOpts
        );
      } else {
        const module = await compensateFailedModuleOperation(
          plan.operationId,
          command.revision,
          command.key,
          apiOpts
        );
        if (scope !== scopeGeneration.current) return;
        onModuleUpdated?.(module);
      }
      if (scope !== scopeGeneration.current) return;
      pending.current.delete(commandId);
      toast.success(`${t('recovery.action.success')} ${plan.moduleSlug}`);
      await refresh();
      if (scope !== scopeGeneration.current) return;
      try {
        const modules = await listTenantModules(apiOpts);
        if (scope !== scopeGeneration.current) return;
        const current = modules.find(
          (item) => item.moduleSlug === plan.moduleSlug
        );
        if (current) onModuleUpdated?.(current);
      } catch (cause) {
        if (scope !== scopeGeneration.current) return;
        setError(
          cause instanceof Error ? cause.message : t('recovery.refresh.error')
        );
      }
      onRefresh?.();
    } catch (cause) {
      if (scope !== scopeGeneration.current) return;
      const message =
        cause instanceof Error ? cause.message : t('recovery.action.error');
      setError(message);
      toast.error(message);
    } finally {
      if (scope === scopeGeneration.current) setBusyOperation(null);
    }
  };

  return (
    <Card>
      <CardHeader>
        <div className='flex flex-wrap items-center justify-between gap-3'>
          <div>
            <CardTitle className='text-base'>{t('recovery.title')}</CardTitle>
            <CardDescription>{t('recovery.description')}</CardDescription>
          </div>
          <Button
            variant='outline'
            size='sm'
            disabled={loading || Boolean(busyOperation)}
            onClick={() => void refresh()}
          >
            {loading ? t('recovery.refreshing') : t('recovery.refresh')}
          </Button>
        </div>
      </CardHeader>
      <CardContent className='space-y-3'>
        {error && (
          <p className='text-destructive text-sm' role='alert'>
            {error}
          </p>
        )}
        {!loading && !error && plans.length === 0 && (
          <p className='text-muted-foreground text-sm'>{t('recovery.empty')}</p>
        )}
        {plans.map((plan) => (
          <div
            key={plan.operationId}
            className='flex flex-wrap items-start justify-between gap-3 rounded-md border p-3'
          >
            <div className='space-y-1'>
              <p className='text-sm font-medium'>
                {plan.moduleSlug} /{' '}
                {plan.requestedEnabled
                  ? t('recovery.enable')
                  : t('recovery.disable')}
              </p>
              <p className='text-muted-foreground text-xs'>
                {plan.errorMessage || plan.issue}
              </p>
              <div className='flex flex-wrap gap-2'>
                <Badge variant='outline'>{plan.status}</Badge>
                <Badge variant='outline'>{plan.recommendedAction}</Badge>
                {plan.correlationId && (
                  <Badge variant='secondary'>corr {plan.correlationId}</Badge>
                )}
              </div>
            </div>
            <div className='flex gap-2'>
              <Button
                variant='outline'
                size='sm'
                disabled={Boolean(busyOperation) || !plan.retryable}
                onClick={() => void execute(plan, 'retry')}
              >
                {t('recovery.retry')}
              </Button>
              <Button
                size='sm'
                disabled={
                  Boolean(busyOperation) || plan.issue !== 'post_hook_failed'
                }
                onClick={() => void execute(plan, 'compensate')}
              >
                {t('recovery.compensate')}
              </Button>
            </div>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
