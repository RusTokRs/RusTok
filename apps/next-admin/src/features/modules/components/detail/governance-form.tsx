'use client';

import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from '@rustok/next-fluent';
import { IconAlertTriangle, IconCheck, IconShield } from '@tabler/icons-react';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import { Input } from '@/shared/ui/shadcn/input';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import {
  approveRegistryPublishRequest,
  fetchRegistryPublishRequestStatus,
  holdRegistryPublishRequest,
  rejectRegistryPublishRequest,
  requestChangesRegistryPublishRequest,
  resumeRegistryPublishRequest,
  transferRegistryOwner,
  validateRegistryPublishRequest,
  yankRegistryRelease,
  type GqlOpts,
  type MarketplaceModule,
  type RegistryMutationResult,
  type RegistryPublishStatusContract
} from '@/shared/api/modules';

interface GovernanceFormProps {
  module: MarketplaceModule;
  apiOpts?: GqlOpts;
  onSuccess?: () => void;
}

const EMPTY_API_OPTIONS: GqlOpts = {};

export function GovernanceForm({
  module,
  apiOpts = EMPTY_API_OPTIONS,
  onSuccess
}: GovernanceFormProps) {
  const t = useTranslations('modules');
  const [dryRun, setDryRun] = useState(true);
  const [reasonCode, setReasonCode] = useState('');
  const [reason, setReason] = useState('');
  const [newOwnerUserId, setNewOwnerUserId] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [result, setResult] = useState<RegistryMutationResult | null>(null);
  const [confirmAction, setConfirmAction] = useState<string | null>(null);

  const requestId = module.registryLifecycle?.latestRequest?.id ?? null;
  const activeRelease =
    module.registryLifecycle?.latestRelease?.version ?? null;
  const [statusEvidence, setStatusEvidence] = useState<{
    status: RegistryPublishStatusContract;
    scope: GqlOpts;
  } | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [statusRefresh, setStatusRefresh] = useState(0);
  const commandKeys = useRef(new Map<string, string>());

  useEffect(() => {
    let cancelled = false;
    setStatusEvidence(null);
    setStatusError(null);
    if (!requestId) {
      setStatusLoading(false);
      return;
    }
    setStatusLoading(true);
    void fetchRegistryPublishRequestStatus(requestId, apiOpts)
      .then((status) => {
        if (status.request_id !== requestId || status.slug !== module.slug) {
          throw new Error(t('governance.status.mismatch'));
        }
        if (!cancelled) setStatusEvidence({ status, scope: apiOpts });
      })
      .catch((error: unknown) => {
        if (!cancelled)
          setStatusError(
            error instanceof Error
              ? error.message
              : t('governance.status.error')
          );
      })
      .finally(() => {
        if (!cancelled) setStatusLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [requestId, module.slug, apiOpts, statusRefresh, t]);

  const status =
    statusEvidence?.scope === apiOpts &&
    statusEvidence.status.request_id === requestId
      ? statusEvidence.status
      : null;
  const requestActions = new Set([
    'validate',
    'approve',
    'reject',
    'request_changes',
    'hold',
    'resume'
  ]);
  const actionFor = (action: string) =>
    requestActions.has(action)
      ? status?.governanceActions.find((item) => item.key === action)
      : module.registryLifecycle?.governanceActions.find(
          (item) => item.key === action
        );
  const signatureFor = (action: string) =>
    JSON.stringify([
      action,
      module.slug,
      requestId,
      activeRelease,
      reason.trim(),
      reasonCode.trim(),
      newOwnerUserId.trim(),
      dryRun,
      apiOpts.tenantSlug,
      apiOpts.tenantId,
      apiOpts.token
    ]);

  const handleAction = async (action: string) => {
    const policy = actionFor(action);
    if (!policy || (requestActions.has(action) && !requestId)) return;
    const cleanReason = reason.trim();
    const cleanCode = reasonCode.trim();
    if (policy.reasonRequired && !cleanReason) {
      toast.error(t('governance.reason.required'));
      return;
    }
    if (policy.reasonCodeRequired && !cleanCode) {
      toast.error(t('governance.reason.code.required'));
      return;
    }
    if (
      cleanCode &&
      policy.reasonCodes.length > 0 &&
      !policy.reasonCodes.includes(cleanCode)
    ) {
      toast.error(
        `${t('governance.reason.code.options')} ${policy.reasonCodes.join(', ')}`
      );
      return;
    }
    if (action === 'owner_transfer' && !newOwnerUserId.trim()) {
      toast.error(t('governance.owner.required'));
      return;
    }
    if (action === 'yank' && !activeRelease) {
      toast.error(t('governance.release.unavailable'));
      return;
    }
    const signature = signatureFor(action);
    if (!dryRun && policy.destructive && confirmAction !== signature) {
      setConfirmAction(signature);
      return;
    }
    setConfirmAction(null);
    setIsSubmitting(true);
    setResult(null);
    const key = commandKeys.current.get(signature) ?? crypto.randomUUID();
    commandKeys.current.set(signature, key);
    try {
      let res: RegistryMutationResult;
      switch (action) {
        case 'validate':
          res = await validateRegistryPublishRequest(
            requestId!,
            dryRun,
            apiOpts,
            key
          );
          break;
        case 'approve':
          res = await approveRegistryPublishRequest(
            requestId!,
            cleanReason || undefined,
            cleanCode || undefined,
            dryRun,
            apiOpts,
            key
          );
          break;
        case 'reject':
          res = await rejectRegistryPublishRequest(
            requestId!,
            cleanReason,
            cleanCode,
            dryRun,
            apiOpts,
            key
          );
          break;
        case 'request_changes':
          res = await requestChangesRegistryPublishRequest(
            requestId!,
            cleanReason,
            cleanCode,
            dryRun,
            apiOpts,
            key
          );
          break;
        case 'hold':
          res = await holdRegistryPublishRequest(
            requestId!,
            cleanReason,
            cleanCode,
            dryRun,
            apiOpts,
            key
          );
          break;
        case 'resume':
          res = await resumeRegistryPublishRequest(
            requestId!,
            cleanReason,
            cleanCode,
            dryRun,
            apiOpts,
            key
          );
          break;
        case 'owner_transfer':
          res = await transferRegistryOwner(
            module.slug,
            newOwnerUserId.trim(),
            cleanReason,
            cleanCode,
            dryRun,
            apiOpts,
            key
          );
          break;
        case 'yank':
          res = await yankRegistryRelease(
            module.slug,
            activeRelease!,
            cleanReason,
            cleanCode,
            dryRun,
            apiOpts,
            key
          );
          break;
        default:
          throw new Error(`Unknown governance action: ${action}`);
      }
      commandKeys.current.delete(signature);
      setResult(res);
      if (res.accepted) {
        toast.success(
          `${dryRun ? '[Dry-run] ' : ''}Action "${action}" completed successfully`
        );
        if (!dryRun) {
          setStatusRefresh((value) => value + 1);
          onSuccess?.();
        }
      } else {
        toast.warning(
          `${dryRun ? '[Dry-run] ' : ''}Action completed with warnings or non-accepted status`
        );
      }
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : 'Governance action failed'
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className='bg-background/80 space-y-4 rounded-lg border p-4'>
      <div className='flex flex-wrap items-center justify-between gap-3 border-b pb-3'>
        <div className='flex items-center gap-2'>
          <IconShield className='text-primary h-4 w-4' />
          <h4 className='text-muted-foreground text-xs font-semibold tracking-wider uppercase'>
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
            className='cursor-pointer text-xs leading-none font-medium peer-disabled:cursor-not-allowed peer-disabled:opacity-70'
          >
            Dry run preview
          </label>
        </div>
      </div>

      {requestId && (
        <p className='text-muted-foreground text-xs'>
          {t('governance.request')} {requestId}:{' '}
          {statusLoading
            ? t('governance.loading')
            : (status?.status ?? t('governance.unavailable'))}
        </p>
      )}
      {statusError && (
        <p className='text-destructive text-xs' role='alert'>
          {statusError}
        </p>
      )}
      {!requestId && (
        <p className='text-muted-foreground text-xs'>
          {t('governance.no.request')}
        </p>
      )}
      <div className='grid gap-3 sm:grid-cols-2'>
        <div className='space-y-1.5'>
          <label className='text-muted-foreground text-xs font-medium'>
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
          <label className='text-muted-foreground text-xs font-medium'>
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
          <label className='text-muted-foreground text-xs font-medium'>
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
          disabled={isSubmitting || !actionFor('validate')}
          onClick={() => handleAction('validate')}
          className='text-xs'
        >
          Validate
        </Button>
        <Button
          variant='default'
          size='sm'
          disabled={isSubmitting || !actionFor('approve')}
          onClick={() => handleAction('approve')}
          className='bg-emerald-600 text-xs hover:bg-emerald-500'
        >
          Approve
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting || !actionFor('request_changes')}
          onClick={() => handleAction('request_changes')}
          className='text-xs'
        >
          Request Changes
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting || !actionFor('hold')}
          onClick={() => handleAction('hold')}
          className='text-xs'
        >
          Hold
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting || !actionFor('resume')}
          onClick={() => handleAction('resume')}
          className='text-xs'
        >
          Resume
        </Button>
        <Button
          variant='destructive'
          size='sm'
          disabled={isSubmitting || !actionFor('reject')}
          onClick={() => handleAction('reject')}
          className='text-xs'
        >
          {confirmAction === signatureFor('reject')
            ? 'Confirm Reject?'
            : 'Reject'}
        </Button>
        <Button
          variant='outline'
          size='sm'
          disabled={isSubmitting || !actionFor('owner_transfer')}
          onClick={() => handleAction('owner_transfer')}
          className='text-xs'
        >
          {confirmAction === signatureFor('owner_transfer')
            ? 'Confirm Transfer?'
            : 'Transfer Owner'}
        </Button>
        <Button
          variant='destructive'
          size='sm'
          disabled={isSubmitting || !actionFor('yank')}
          onClick={() => handleAction('yank')}
          className='text-xs'
        >
          {confirmAction === signatureFor('yank')
            ? 'Confirm Yank?'
            : `Yank v${activeRelease}`}
        </Button>
      </div>

      {result && (
        <div className='bg-background mt-3 space-y-2 rounded-md border p-3 text-xs'>
          <div className='flex items-center justify-between'>
            <span className='text-foreground font-semibold'>
              Action: {result.action} {result.dry_run ? '(dry-run)' : ''}
            </span>
            <Badge variant={result.accepted ? 'default' : 'destructive'}>
              {result.accepted ? 'Accepted' : 'Pending / Rejected'}
            </Badge>
          </div>
          {result.next_step && (
            <p className='text-muted-foreground'>
              <span className='text-foreground font-medium'>Next step: </span>
              {result.next_step}
            </p>
          )}
          {result.warnings.length > 0 && (
            <div className='text-amber-600 dark:text-amber-400'>
              <p className='font-medium'>Warnings:</p>
              <ul className='list-disc space-y-0.5 pl-4'>
                {result.warnings.map((w, idx) => (
                  <li key={idx}>{w}</li>
                ))}
              </ul>
            </div>
          )}
          {result.errors.length > 0 && (
            <div className='text-destructive'>
              <p className='font-medium'>Errors:</p>
              <ul className='list-disc space-y-0.5 pl-4'>
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
