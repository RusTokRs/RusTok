'use client';

import { useRef, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from '@rustok/next-fluent';
import {
  IconAlertTriangle,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconShieldLock
} from '@tabler/icons-react';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import {
  finalizeModuleTransition,
  rollbackModuleTransition,
  type GqlOpts,
  type ModuleTransitionCheckpoint,
  type RetentionHold
} from '@/shared/api/modules';

function shortDigest(digest?: string | null): string {
  if (!digest) return 'None (Initial Install)';
  if (digest.length > 19) {
    return `${digest.slice(0, 10)}...${digest.slice(-6)}`;
  }
  return digest;
}

interface TransitionControlCardProps {
  checkpoint: ModuleTransitionCheckpoint;
  retentionHolds?: RetentionHold[];
  onRefresh?: () => void;
  apiOpts?: GqlOpts;
}

export function TransitionControlCard({
  checkpoint,
  retentionHolds,
  onRefresh,
  apiOpts = {}
}: TransitionControlCardProps) {
  const t = useTranslations('modules');
  const [isBusy, setIsBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [showHolds, setShowHolds] = useState(false);
  const [showRollback, setShowRollback] = useState(false);
  const [rollbackReason, setRollbackReason] = useState('');
  const pendingKeys = useRef(new Map<string, string>());

  const isObserving = checkpoint.state === 'OBSERVING';
  const isInterventionRequired = checkpoint.state === 'INTERVENTION_REQUIRED';
  const isConverged = checkpoint.state === 'CONVERGED';
  const recoveryLimitReached = checkpoint.recoveryAttemptCount >= 1;
  const isPastPointOfNoReturn = checkpoint.state === 'POINT_OF_NO_RETURN';
  const canRollback =
    (isObserving || isInterventionRequired) &&
    Boolean(checkpoint.predecessorDigest) &&
    !recoveryLimitReached;

  const stateBadgeVariant = (() => {
    switch (checkpoint.state) {
      case 'OBSERVING':
        return 'outline';
      case 'CONVERGED':
        return 'default';
      case 'RECOVERED_TO_PREDECESSOR':
        return 'secondary';
      case 'INTERVENTION_REQUIRED':
        return 'destructive';
      default:
        return 'outline';
    }
  })();

  const handleFinalizeTransition = async () => {
    setIsBusy(true);
    setActionError(null);
    const signature = JSON.stringify([
      'finalize',
      checkpoint.operationId,
      checkpoint.revision,
      apiOpts.tenantId,
      apiOpts.tenantSlug,
      apiOpts.token
    ]);
    const key = pendingKeys.current.get(signature) ?? crypto.randomUUID();
    pendingKeys.current.set(signature, key);

    try {
      await finalizeModuleTransition(
        checkpoint.operationId,
        checkpoint.revision,
        key,
        apiOpts
      );
      pendingKeys.current.delete(signature);
      toast.success('Module transition finalized successfully');
      onRefresh?.();
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Finalization failed';
      setActionError(message);
      toast.error(message);
    } finally {
      setIsBusy(false);
    }
  };

  const handleRollback = async () => {
    const reason = rollbackReason.trim();
    if (!canRollback || !reason) return;
    setIsBusy(true);
    setActionError(null);
    const signature = JSON.stringify([
      'rollback',
      checkpoint.operationId,
      reason,
      apiOpts.tenantId,
      apiOpts.tenantSlug,
      apiOpts.token
    ]);
    const key = pendingKeys.current.get(signature) ?? crypto.randomUUID();
    pendingKeys.current.set(signature, key);
    try {
      await rollbackModuleTransition(
        checkpoint.operationId,
        reason,
        key,
        apiOpts
      );
      pendingKeys.current.delete(signature);
      setShowRollback(false);
      toast.success(t('rollback.success'));
      onRefresh?.();
    } catch (error) {
      const message =
        error instanceof Error ? error.message : t('rollback.error');
      setActionError(message);
      toast.error(message);
    } finally {
      setIsBusy(false);
    }
  };

  return (
    <Card className='border-primary/20 bg-card shadow-sm'>
      <CardHeader className='pb-3'>
        <div className='flex flex-wrap items-center justify-between gap-3'>
          <div className='space-y-1'>
            <div className='flex items-center gap-2'>
              <CardTitle className='text-base font-semibold'>
                Module Transition: {checkpoint.moduleSlug}
              </CardTitle>
              <Badge variant={stateBadgeVariant}>
                {checkpoint.state.replace(/_/g, ' ')}
              </Badge>
            </div>
            <CardDescription className='text-xs'>
              Operation ID:{' '}
              <span className='text-foreground font-mono font-medium'>
                {checkpoint.operationId}
              </span>
            </CardDescription>
          </div>

          <div className='flex items-center gap-2'>
            <Badge variant='outline' className='text-xs'>
              Epoch #{checkpoint.securityEpoch}
            </Badge>
            <Badge variant='secondary' className='text-xs'>
              Recovery Attempts: {checkpoint.recoveryAttemptCount} / 1
            </Badge>
          </div>
        </div>
      </CardHeader>

      <CardContent className='space-y-4'>
        {/* Predecessor vs Candidate digests */}
        <div className='grid grid-cols-1 gap-3 text-xs sm:grid-cols-2'>
          <div className='bg-background/50 space-y-1 rounded-md border p-2.5'>
            <span className='text-muted-foreground font-medium'>
              Direct Predecessor (Standby N):
            </span>
            <div className='text-foreground font-mono break-all'>
              {shortDigest(checkpoint.predecessorDigest)}
            </div>
          </div>

          <div className='bg-background/50 space-y-1 rounded-md border p-2.5'>
            <span className='text-muted-foreground font-medium'>
              Candidate Artifact (N+1):
            </span>
            <div className='text-foreground font-mono break-all'>
              {shortDigest(checkpoint.candidateDigest)}
            </div>
          </div>
        </div>

        {/* State Details Note */}
        {checkpoint.stateDetails && (
          <div className='rounded-md border border-blue-500/20 bg-blue-500/10 p-3 text-xs text-blue-600 dark:text-blue-400'>
            <span className='font-semibold'>Transition Detail: </span>
            {checkpoint.stateDetails}
          </div>
        )}

        {/* Anti-Flapping Warning */}
        {recoveryLimitReached && !isConverged && (
          <div className='flex items-start gap-2 rounded-md border border-amber-500/20 bg-amber-500/10 p-3 text-xs text-amber-600 dark:text-amber-400'>
            <IconAlertTriangle className='mt-0.5 h-4 w-4 shrink-0' />
            <div>
              <span className='font-semibold'>Zero-Flapping Invariant: </span>
              Single automatic recovery attempt already executed. Automated
              bouncing is disabled to protect persistent state.
            </div>
          </div>
        )}

        {/* Intervention Required Notice */}
        {isInterventionRequired && (
          <div className='border-destructive/30 bg-destructive/10 text-destructive flex items-start gap-2 rounded-md border p-3 text-xs'>
            <IconShieldLock className='mt-0.5 h-4 w-4 shrink-0' />
            <div>
              <span className='font-semibold'>Intervention Required: </span>
              Automatic transition action stopped, but serving selection was not
              changed. Explicitly roll back or deactivate the candidate if
              traffic must move.
            </div>
          </div>
        )}

        {/* Action Error Message */}
        {actionError && (
          <div className='border-destructive/30 bg-destructive/10 text-destructive rounded-md border p-3 text-xs'>
            {actionError}
          </div>
        )}

        {/* Retention Holds Section */}
        <div className='border-t pt-3'>
          <button
            type='button'
            className='text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-xs font-medium'
            onClick={() => setShowHolds((prev) => !prev)}
          >
            {showHolds ? (
              <IconChevronDown className='h-3.5 w-3.5' />
            ) : (
              <IconChevronRight className='h-3.5 w-3.5' />
            )}
            <span>
              {t('holds.tenant.title')} (
              {retentionHolds?.length ?? t('governance.unavailable')})
            </span>
          </button>

          {showHolds && (
            <div className='bg-background/50 mt-2.5 rounded-md border p-3 text-xs'>
              {!retentionHolds ? (
                <p className='text-muted-foreground'>
                  {t('holds.unavailable')}
                </p>
              ) : retentionHolds.length === 0 ? (
                <p className='text-muted-foreground'>{t('holds.empty')}</p>
              ) : (
                <div className='space-y-2'>
                  {retentionHolds.map((hold) => (
                    <div
                      key={hold.holdId}
                      className='flex items-center justify-between border-b pb-1 font-mono text-[11px] last:border-b-0'
                    >
                      <span className='text-foreground'>
                        {hold.targetType}: {shortDigest(hold.targetIdentity)}
                      </span>
                      <Badge variant='secondary' className='text-[10px]'>
                        {hold.kind}
                      </Badge>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Transition action */}
        <div className='flex flex-wrap items-center justify-between gap-3 border-t pt-3'>
          <div className='flex items-center gap-2'>
            {(isObserving || isPastPointOfNoReturn) && (
              <Button
                variant='default'
                size='sm'
                disabled={isBusy}
                onClick={handleFinalizeTransition}
                className='bg-emerald-600 text-white hover:bg-emerald-500'
              >
                <IconCheck className='mr-1.5 h-3.5 w-3.5' />
                {isBusy ? 'Finalizing...' : 'Finalize Convergence'}
              </Button>
            )}
            {canRollback && (
              <Button
                variant='destructive'
                size='sm'
                disabled={isBusy}
                onClick={() => setShowRollback((value) => !value)}
              >
                {showRollback ? t('rollback.cancel') : t('rollback.action')}
              </Button>
            )}
          </div>

          <p className='text-muted-foreground text-[11px]'>
            {t('rollback.owner.detail')}
          </p>
        </div>
        {showRollback && canRollback && (
          <div className='space-y-2 rounded-md border p-3'>
            <label
              htmlFor={`rollback-reason-${checkpoint.operationId}`}
              className='text-xs font-medium'
            >
              {t('rollback.reason')}
            </label>
            <Input
              id={`rollback-reason-${checkpoint.operationId}`}
              value={rollbackReason}
              onChange={(event) => setRollbackReason(event.target.value)}
            />
            <Button
              variant='destructive'
              size='sm'
              disabled={isBusy || !rollbackReason.trim()}
              onClick={() => void handleRollback()}
            >
              {t('rollback.confirm')}
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
