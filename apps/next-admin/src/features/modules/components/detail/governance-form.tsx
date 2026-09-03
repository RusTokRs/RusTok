'use client';

import { useState } from 'react';
import { toast } from 'sonner';
import { IconAlertTriangle, IconCheck, IconShield } from '@tabler/icons-react';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import { Input } from '@/shared/ui/shadcn/input';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import {
  approveRegistryPublishRequest,
  holdRegistryPublishRequest,
  rejectRegistryPublishRequest,
  requestChangesRegistryPublishRequest,
  resumeRegistryPublishRequest,
  transferRegistryOwner,
  validateRegistryPublishRequest,
  yankRegistryRelease,
  type GqlOpts,
  type MarketplaceModule,
  type RegistryMutationResult
} from '@/shared/api/modules';

interface GovernanceFormProps {
  module: MarketplaceModule;
  apiOpts?: GqlOpts;
  onSuccess?: () => void;
}

export function GovernanceForm({ module, apiOpts = {}, onSuccess }: GovernanceFormProps) {
  const [dryRun, setDryRun] = useState(true);
  const [reasonCode, setReasonCode] = useState('');
  const [reason, setReason] = useState('');
  const [newOwnerUserId, setNewOwnerUserId] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [result, setResult] = useState<RegistryMutationResult | null>(null);
  const [confirmAction, setConfirmAction] = useState<string | null>(null);

  const activeRelease = module.versions?.[0]?.version ?? module.latestVersion;
  const requestId = module.slug; // Default to slug or published request identifier

  const handleAction = async (action: string) => {
    // Check confirmation for destructive actions if not dry-run
    if (!dryRun && ['reject', 'yank', 'owner_transfer'].includes(action)) {
      if (confirmAction !== action) {
        setConfirmAction(action);
        return;
      }
    }

    setConfirmAction(null);
    setIsSubmitting(true);
    setResult(null);

    try {
      let res: RegistryMutationResult;
      switch (action) {
        case 'validate':
          res = await validateRegistryPublishRequest(requestId, dryRun, apiOpts);
          break;
        case 'approve':
          res = await approveRegistryPublishRequest(
            requestId,
            reason || undefined,
            reasonCode || undefined,
            dryRun,
            apiOpts
          );
          break;
        case 'reject':
          if (!reason || !reasonCode) {
            toast.error('Reason and Reason Code are required for Reject');
            setIsSubmitting(false);
            return;
          }
          res = await rejectRegistryPublishRequest(
            requestId,
            reason,
            reasonCode,
            dryRun,
            apiOpts
          );
          break;
        case 'request_changes':
          if (!reason || !reasonCode) {
            toast.error('Reason and Reason Code are required to request changes');
            setIsSubmitting(false);
            return;
          }
          res = await requestChangesRegistryPublishRequest(
            requestId,
            reason,
            reasonCode,
            dryRun,
            apiOpts
          );
          break;
        case 'hold':
          if (!reason || !reasonCode) {
            toast.error('Reason and Reason Code are required to place on Hold');
            setIsSubmitting(false);
            return;
          }
          res = await holdRegistryPublishRequest(
            requestId,
            reason,
            reasonCode,
            dryRun,
            apiOpts
          );
          break;
        case 'resume':
          if (!reason || !reasonCode) {
            toast.error('Reason and Reason Code are required to Resume');
            setIsSubmitting(false);
            return;
          }
          res = await resumeRegistryPublishRequest(
            requestId,
            reason,
            reasonCode,
            dryRun,
            apiOpts
          );
          break;
        case 'owner_transfer':
          if (!newOwnerUserId.trim() || !reason || !reasonCode) {
            toast.error('New Owner User ID, Reason, and Reason Code are required');
            setIsSubmitting(false);
            return;
          }
          res = await transferRegistryOwner(
            module.slug,
            newOwnerUserId.trim(),
            reason,
            reasonCode,
            dryRun,
            apiOpts
          );
          break;
        case 'yank':
          if (!reason || !reasonCode) {
            toast.error('Reason and Reason Code are required to Yank release');
            setIsSubmitting(false);
            return;
          }
          res = await yankRegistryRelease(
            module.slug,
            activeRelease,
            reason,
            reasonCode,
            dryRun,
            apiOpts
          );
          break;
        default:
          throw new Error(`Unknown governance action: ${action}`);
      }

      setResult(res);
      if (res.accepted) {
        toast.success(
          `${dryRun ? '[Dry-run] ' : ''}Action "${action}" completed successfully`
        );
        if (!dryRun) {
          onSuccess?.();
        }
      } else {
        toast.warning(
          `${dryRun ? '[Dry-run] ' : ''}Action completed with warnings or non-accepted status`
        );
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Governance action failed';
      toast.error(msg);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className='rounded-lg border bg-background/80 p-4 space-y-4'>
      <div className='flex flex-wrap items-center justify-between gap-3 border-b pb-3'>
        <div className='flex items-center gap-2'>
          <IconShield className='h-4 w-4 text-primary' />
          <h4 className='text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
            Registry Governance & Moderation
          </h4>
        </div>
        <div className='flex items-center space-x-2'>
          <Checkbox
            id='gov-dry-run'
            checked={dryRun}
            onCheckedChange={(checked) => setDryRun(Boolean(checked))}
          />
          <label
            htmlFor='gov-dry-run'
            className='text-xs font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 cursor-pointer'
          >
            Dry run preview
          </label>
        </div>
      </div>

      <div className='grid gap-3 sm:grid-cols-2'>
        <div className='space-y-1.5'>
          <label className='text-xs font-medium text-muted-foreground'>
            New Owner User ID (UUID)
          </label>
          <Input
            placeholder='00000000-0000-0000-0000-000000000000'
            value={newOwnerUserId}
            onChange={(e) => setNewOwnerUserId(e.target.value)}
            className='text-xs'
          />
        </div>

        <div className='space-y-1.5'>
          <label className='text-xs font-medium text-muted-foreground'>
            Reason Code
          </label>
          <Input
            placeholder='e.g. security_emergency, governance_override, critical_regression'
            value={reasonCode}
            onChange={(e) => setReasonCode(e.target.value)}
            className='text-xs'
          />
        </div>

        <div className='space-y-1.5 sm:col-span-2'>
          <label className='text-xs font-medium text-muted-foreground'>
            Detailed Reason / Reviewer Notes
          </label>
          <Textarea
            placeholder='State the explicit rationale for this governance decision...'
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            rows={2}
            className='text-xs'
          />
        </div>
      </div>

      <div className='flex flex-wrap items-center gap-2 border-t pt-3'>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('validate')}
          className='text-xs'
        >
          Validate
        </Button>
        <Button
          variant='default'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('approve')}
          className='text-xs bg-emerald-600 hover:bg-emerald-500'
        >
          Approve
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('request_changes')}
          className='text-xs'
        >
          Request Changes
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('hold')}
          className='text-xs'
        >
          Hold
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('resume')}
          className='text-xs'
        >
          Resume
        </Button>
        <Button
          variant='destructive'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('reject')}
          className='text-xs'
        >
          {confirmAction === 'reject' ? 'Confirm Reject?' : 'Reject'}
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('owner_transfer')}
          className='text-xs'
        >
          {confirmAction === 'owner_transfer'
            ? 'Confirm Transfer?'
            : 'Transfer Owner'}
        </Button>
        <Button
          variant='destructive'
          size='sm'
          disabled={isSubmitting}
          onClick={() => handleAction('yank')}
          className='text-xs'
        >
          {confirmAction === 'yank' ? 'Confirm Yank?' : `Yank v${activeRelease}`}
        </Button>
      </div>

      {result && (
        <div className='rounded-md border bg-background p-3 text-xs space-y-2 mt-3'>
          <div className='flex items-center justify-between'>
            <span className='font-semibold text-foreground'>
              Action: {result.action} {result.dry_run ? '(dry-run)' : ''}
            </span>
            <Badge variant={result.accepted ? 'default' : 'destructive'}>
              {result.accepted ? 'Accepted' : 'Pending / Rejected'}
            </Badge>
          </div>
          {result.next_step && (
            <p className='text-muted-foreground'>
              <span className='font-medium text-foreground'>Next step: </span>
              {result.next_step}
            </p>
          )}
          {result.warnings.length > 0 && (
            <div className='text-amber-600 dark:text-amber-400'>
              <p className='font-medium'>Warnings:</p>
              <ul className='list-disc pl-4 space-y-0.5'>
                {result.warnings.map((w, idx) => (
                  <li key={idx}>{w}</li>
                ))}
              </ul>
            </div>
          )}
          {result.errors.length > 0 && (
            <div className='text-destructive'>
              <p className='font-medium'>Errors:</p>
              <ul className='list-disc pl-4 space-y-0.5'>
                {result.errors.map((e, idx) => (
                  <li key={idx}>{e}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
