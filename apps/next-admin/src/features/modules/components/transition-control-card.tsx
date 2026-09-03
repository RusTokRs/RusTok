'use client';

import { useState } from 'react';
import { toast } from 'sonner';
import {
  IconAlertTriangle,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconRotate,
  IconShieldLock
} from '@tabler/icons-react';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import {
  finalizeModuleTransition,
  triggerModuleRecovery,
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
  retentionHolds = [],
  onRefresh,
  apiOpts = {}
}: TransitionControlCardProps) {
  const [showRollbackPrompt, setShowRollbackPrompt] = useState(false);
  const [rollbackReason, setRollbackReason] = useState('');
  const [isBusy, setIsBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [showHolds, setShowHolds] = useState(false);

  const isObserving = checkpoint.state === 'OBSERVING';
  const isRecovering = checkpoint.state === 'ROLLBACK_TRIGGERED';
  const isFailed = checkpoint.state === 'FAILED_CLOSED';
  const isConverged = checkpoint.state === 'CONVERGED';
  const recoveryLimitReached = checkpoint.recoveryAttemptCount >= 1;

  const stateBadgeVariant = (() => {
    switch (checkpoint.state) {
      case 'OBSERVING':
        return 'outline';
      case 'CONVERGED':
        return 'default';
      case 'RECOVERED_TO_PREDECESSOR':
        return 'secondary';
      case 'FAILED_CLOSED':
      case 'ROLLBACK_TRIGGERED':
        return 'destructive';
      default:
        return 'outline';
    }
  })();

  const handleTriggerRollback = async () => {
    if (!rollbackReason.trim()) {
      setActionError('Please specify a reason for emergency rollback.');
      return;
    }

    setIsBusy(true);
    setActionError(null);

    try {
      await triggerModuleRecovery(
        checkpoint.operationId,
        rollbackReason.trim(),
        apiOpts
      );
      toast.success('Emergency rollback initiated');
      setShowRollbackPrompt(false);
      setRollbackReason('');
      onRefresh?.();
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Rollback trigger failed';
      setActionError(message);
      toast.error(message);
    } finally {
      setIsBusy(false);
    }
  };

  const handleFinalizeTransition = async () => {
    setIsBusy(true);
    setActionError(null);

    try {
      await finalizeModuleTransition(checkpoint.operationId, apiOpts);
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
              <span className='font-mono font-medium text-foreground'>
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
        <div className='grid grid-cols-1 gap-3 sm:grid-cols-2 text-xs'>
          <div className='space-y-1 rounded-md border p-2.5 bg-background/50'>
            <span className='font-medium text-muted-foreground'>
              Direct Predecessor (Standby N):
            </span>
            <div className='font-mono break-all text-foreground'>
              {shortDigest(checkpoint.predecessorDigest)}
            </div>
          </div>

          <div className='space-y-1 rounded-md border p-2.5 bg-background/50'>
            <span className='font-medium text-muted-foreground'>
              Candidate Artifact (N+1):
            </span>
            <div className='font-mono break-all text-foreground'>
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
            <IconAlertTriangle className='h-4 w-4 shrink-0 mt-0.5' />
            <div>
              <span className='font-semibold'>Zero-Flapping Invariant: </span>
              Single automatic recovery attempt already executed. Automated
              bouncing is disabled to protect persistent state.
            </div>
          </div>
        )}

        {/* Failed Closed Containment Notice */}
        {isFailed && (
          <div className='flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive'>
            <IconShieldLock className='h-4 w-4 shrink-0 mt-0.5' />
            <div>
              <span className='font-semibold'>
                Permanent Containment (Failed Closed):{' '}
              </span>
              Transition failed closed to protect persistent data and fleet
              state. Manual operator intervention is required.
            </div>
          </div>
        )}

        {/* Action Error Message */}
        {actionError && (
          <div className='rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive'>
            {actionError}
          </div>
        )}

        {/* Retention Holds Section */}
        <div className='border-t pt-3'>
          <button
            type='button'
            className='inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground hover:text-foreground'
            onClick={() => setShowHolds((prev) => !prev)}
          >
            {showHolds ? (
              <IconChevronDown className='h-3.5 w-3.5' />
            ) : (
              <IconChevronRight className='h-3.5 w-3.5' />
            )}
            <span>Active Retention Holds ({retentionHolds.length})</span>
          </button>

          {showHolds && (
            <div className='mt-2.5 rounded-md border bg-background/50 p-3 text-xs'>
              {retentionHolds.length === 0 ? (
                <p className='text-muted-foreground'>
                  No active GC retention holds.
                </p>
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

        {/* Controls / Actions */}
        <div className='flex flex-wrap items-center justify-between gap-3 border-t pt-3'>
          <div className='flex items-center gap-2'>
            {isObserving && (
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
          </div>

          <div className='flex items-center gap-2'>
            {(isObserving || isRecovering || !isConverged) &&
              !recoveryLimitReached && (
                <Button
                  variant='destructive'
                  size='sm'
                  disabled={isBusy}
                  onClick={() => setShowRollbackPrompt((prev) => !prev)}
                >
                  <IconRotate className='mr-1.5 h-3.5 w-3.5' />
                  Emergency Rollback
                </Button>
              )}
          </div>
        </div>

        {/* Rollback Confirmation Form */}
        {showRollbackPrompt && (
          <div className='rounded-md border border-destructive/30 bg-destructive/5 p-3 space-y-3'>
            <div className='space-y-1'>
              <h4 className='text-xs font-semibold text-destructive'>
                Confirm Single-Attempt Rollback
              </h4>
              <p className='text-[11px] text-muted-foreground'>
                This will immediately demote candidate N+1, return traffic to
                direct predecessor N, and advance the security epoch.
              </p>
            </div>

            <Input
              placeholder='Reason for emergency rollback (e.g. Memory leak on node 2)...'
              value={rollbackReason}
              onChange={(e) => setRollbackReason(e.target.value)}
              className='text-xs'
              disabled={isBusy}
            />

            <div className='flex justify-end gap-2'>
              <Button
                type='button'
                variant='outline'
                size='sm'
                onClick={() => setShowRollbackPrompt(false)}
                disabled={isBusy}
              >
                Cancel
              </Button>
              <Button
                type='button'
                variant='destructive'
                size='sm'
                disabled={isBusy}
                onClick={handleTriggerRollback}
              >
                {isBusy ? 'Executing...' : 'Confirm & Revert'}
              </Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
