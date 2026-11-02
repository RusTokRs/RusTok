/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
'use client';

import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Label } from '@/shared/ui/shadcn/label';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Alert, AlertDescription } from '@/shared/ui/shadcn/alert';
import {
  ProductWriteAttempt,
  type ProductWriteOutcome,
  type ProductWriteScope
} from '../../write-attempt';
import {
  browserAttributeDefinitionJournal,
  ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY,
  type ProductJournalRecord
} from '../../write-journal';
import {
  attributePresentationJson,
  prepareAttributeDefinition,
  isDefinitionId,
  attributeOutcomeKey,
  attributeReferenceKey,
  type AttributeDefinitionAction,
  type AttributeDefinitionWriteRequest,
  type AttributeLifecycleRequest,
  type AttributeLifecycleWorkspace,
  type AttributeReferenceImpact
} from '../../attribute-lifecycle';

export interface AttributeLifecycleCardProps {
  heading?: React.ReactNode;
  children?: React.ReactNode;
  scope: ProductWriteScope;
  locale: string;
  attributeId: string | null;
  optionId?: string | null;
  invalidSelection?: boolean;
  read: (
    scope: ProductWriteScope,
    locale: string,
    request: AttributeLifecycleRequest
  ) => Promise<AttributeLifecycleWorkspace>;
  write: (
    request: AttributeDefinitionWriteRequest,
    key: string
  ) => Promise<ProductWriteOutcome>;
}

/** The recovery controller is deliberately independent of either dictionary GET. */
export function AttributeLifecycleCard({
  scope: suppliedScope,
  locale,
  attributeId,
  optionId: initialOptionId = null,
  invalidSelection = false,
  read,
  write,
  heading,
  children
}: AttributeLifecycleCardProps) {
  const t = useTranslations('product');
  const scope = React.useMemo(
    () => suppliedScope,
    [suppliedScope.tenantId, suppliedScope.actorId]
  );
  const actions = React.useRef({ read, write });
  actions.current = { read, write };
  const writer = React.useRef<ProductWriteAttempt | null>(null);
  const generation = React.useRef(0);
  const mounted = React.useRef(false);
  const [record, setRecord] = React.useState<ProductJournalRecord | null>(null);
  const [storageError, setStorageError] = React.useState(false);
  const [recoveryReady, setRecoveryReady] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [directory, setDirectory] =
    React.useState<AttributeLifecycleWorkspace | null>(null);
  const [loadedWorkspace, setWorkspace] =
    React.useState<AttributeLifecycleWorkspace | null>(null);
  const [workspaceStamp, setWorkspaceStamp] = React.useState<string | null>(
    null
  );
  const [readError, setReadError] = React.useState(false);
  const [message, setMessage] = React.useState<string | null>(null);
  const [optionId, setOptionId] = React.useState<string | null>(
    initialOptionId
  );
  const [patch, setPatch] = React.useState('');
  const [readImpact, setReadImpact] =
    React.useState<AttributeReferenceImpact | null>(null);

  const refresh = React.useCallback(
    async (selectedOption: string | null = null) => {
      const requestGeneration = ++generation.current;
      setReadError(false);
      const results = await Promise.allSettled([
        actions.current.read(scope, locale, {
          attributeId: null,
          optionId: null,
          includeArchived: true
        }),
        attributeId && isDefinitionId(attributeId)
          ? actions.current.read(scope, locale, {
              attributeId,
              optionId: selectedOption,
              includeArchived: true
            })
          : Promise.resolve(null)
      ]);
      if (!mounted.current || requestGeneration !== generation.current) return;
      if (results[0].status === 'fulfilled') setDirectory(results[0].value);
      else {
        setDirectory(null);
        setReadError(true);
      }
      if (results[1].status === 'fulfilled') {
        const value = results[1].value;
        setWorkspace(value);
        setWorkspaceStamp(
          JSON.stringify([
            scope.tenantId,
            scope.actorId,
            attributeId,
            locale,
            selectedOption
          ])
        );
        setReadImpact(value?.impact ?? null);
        setPatch(
          value ? (attributePresentationJson(value, selectedOption) ?? '') : ''
        );
      } else {
        setWorkspace(null);
        setReadImpact(null);
        setReadError(true);
      }
    },
    [scope, locale, attributeId]
  );

  React.useEffect(() => {
    mounted.current = true;
    setBusy(false);
    setRecord(null);
    setStorageError(false);
    setRecoveryReady(false);
    setMessage(null);
    setDirectory(null);
    setWorkspace(null);
    setWorkspaceStamp(null);
    setReadImpact(null);
    setPatch('');
    setReadError(false);
    if (
      invalidSelection ||
      (attributeId !== null && !isDefinitionId(attributeId)) ||
      (initialOptionId !== null &&
        (!isDefinitionId(initialOptionId) || attributeId === null))
    ) {
      setStorageError(true);
    } else if (attributeId) {
      try {
        const attempt = new ProductWriteAttempt(
          browserAttributeDefinitionJournal(
            scope,
            attributeId,
            false,
            initialOptionId
          )
        );
        const restored = attempt.restore();
        writer.current = attempt;
        setRecord(restored);
        setRecoveryReady(true);
        const retained =
          restored?.phase === 'pending' &&
          restored.attempt.operation === 'definition'
            ? restored.attempt.request
            : restored?.phase === 'settled' &&
                restored.operation === 'definition'
              ? restored.request
              : null;
        const targetId = retained ? retained.input.optionId : initialOptionId;
        setOptionId(targetId);
        void refresh(targetId);
      } catch {
        setStorageError(true);
        void refresh(initialOptionId);
      }
    } else {
      setRecoveryReady(true);
      void refresh();
    }
    return () => {
      mounted.current = false;
      ++generation.current;
      writer.current = null;
    };
  }, [scope, attributeId, initialOptionId, invalidSelection, refresh]);

  const locked = busy || storageError || !recoveryReady || record !== null;
  const workspace =
    workspaceStamp ===
    JSON.stringify([
      scope.tenantId,
      scope.actorId,
      attributeId,
      locale,
      optionId
    ])
      ? loadedWorkspace
      : null;
  const selected = workspace?.attributes[0];
  const target =
    optionId === null
      ? selected
      : workspace?.options?.find((option) => option.id === optionId);
  const sync = (attempt: ProductWriteAttempt) => {
    if (writer.current !== attempt || !mounted.current) return;
    setRecord(attempt.recovery);
    setStorageError(!!attempt.recoveryError);
    const saved = attempt.recovery;
    if (saved?.phase === 'settled' && saved.operation === 'definition') {
      setMessage(t(attributeOutcomeKey(saved.outcome)));
    } else if (saved?.phase === 'pending')
      setMessage(t('attributeLifecycle-unknown'));
  };
  const submit = async (action: AttributeDefinitionAction | 'recover') => {
    const attempt = writer.current;
    if (!attempt || busy || attempt.isBusy || storageError || !recoveryReady)
      return;
    setBusy(true);
    setMessage(null);
    try {
      if (action === 'recover')
        await attempt.resume({ definition: actions.current.write });
      else {
        if (locked || !workspace || !selected) return;
        const request = prepareAttributeDefinition(
          scope,
          workspace,
          optionId,
          action,
          locale,
          patch
        );
        await attempt.execute('definition', request, actions.current.write);
      }
      sync(attempt);
    } catch {
      if (mounted.current && writer.current === attempt) {
        setMessage(t('attributeLifecycle-invalid'));
        sync(attempt);
      }
    } finally {
      if (mounted.current && writer.current === attempt) setBusy(false);
    }
  };
  const recoverStorage = () => {
    try {
      if (!attributeId || !writer.current || record?.phase === 'settled') {
        window.location.reload();
        return;
      }
      writer.current.restore();
      sync(writer.current);
      setRecoveryReady(true);
    } catch {
      setStorageError(true);
    }
  };
  const fresh = () => {
    if (!attributeId || busy || storageError || record?.phase !== 'settled')
      return;
    try {
      if (record.operation !== 'definition')
        throw new Error('Invalid dictionary journal');
      setBusy(true);
      browserAttributeDefinitionJournal(
        scope,
        attributeId,
        true,
        initialOptionId
      );
      const url = new URL(window.location.href);
      url.searchParams.set('attribute_id', record.request.id);
      if (record.request.input.optionId === null)
        url.searchParams.delete('option_id');
      else url.searchParams.set('option_id', record.request.input.optionId);
      window.location.assign(url); // Keep the exact target; never rebase a dirty JSON draft.
    } catch {
      setBusy(false);
      setStorageError(true);
    }
  };
  const selectAttribute = (id: string) => {
    if (
      busy ||
      storageError ||
      record?.phase === 'pending' ||
      !isDefinitionId(id)
    )
      return;
    try {
      if (writer.current?.restore()?.phase === 'pending') {
        sync(writer.current);
        return;
      }
      const url = new URL(window.location.href);
      url.searchParams.set('attribute_id', id);
      url.searchParams.delete('option_id');
      url.searchParams.delete(ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY);
      setBusy(true);
      window.location.assign(url);
    } catch {
      setBusy(false);
      setStorageError(true);
    }
  };
  const selectOption = (id: string) => {
    if (locked) return;
    const next = id === 'attribute' ? null : id;
    if (
      next !== null &&
      !workspace?.options?.some((option) => option.id === next)
    )
      return;
    try {
      const attempt = writer.current;
      if (!attributeId || !attempt || attempt.restore() !== null) {
        if (attempt) sync(attempt);
        return;
      }
      const url = new URL(window.location.href);
      url.searchParams.set('attribute_id', attributeId);
      if (next === null) url.searchParams.delete('option_id');
      else url.searchParams.set('option_id', next);
      // A normal target change is an explicit navigation while the journal is Ready.
      // Preserve its recovery marker and never choose a replacement for a missing UUID.
      setBusy(true);
      window.location.assign(url);
    } catch {
      setBusy(false);
      setStorageError(true);
    }
  };

  const retainedImpact =
    record?.phase === 'settled' &&
    record.operation === 'definition' &&
    record.outcome.status === 'rejected'
      ? (record.outcome.definition?.impact ?? null)
      : null;
  const impact = retainedImpact ?? readImpact;
  const displayMessage =
    message ??
    (record?.phase === 'pending'
      ? t('attributeLifecycle-unknown')
      : record?.phase === 'settled' && record.operation === 'definition'
        ? t(attributeOutcomeKey(record.outcome))
        : null);
  const peerLocked =
    busy ||
    storageError ||
    (attributeId !== null && (!recoveryReady || record !== null));
  return (
    <>
      {heading && (
        <fieldset disabled={peerLocked} className='contents'>
          {heading}
        </fieldset>
      )}
      <Card>
        <CardHeader>
          <CardTitle>{t('attributeLifecycle-title')}</CardTitle>
          <CardDescription>{t('attributeLifecycle-help')}</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4' aria-busy={busy}>
          {(readError || invalidSelection) && (
            <Alert role='alert'>
              <AlertDescription>
                {t(
                  invalidSelection
                    ? 'attributeLifecycle-selection'
                    : 'attributeLifecycle-unavailable'
                )}
              </AlertDescription>
            </Alert>
          )}
          {storageError && (
            <Alert role='alert'>
              <AlertDescription>
                {t('attributeLifecycle-storage')}
              </AlertDescription>
            </Alert>
          )}
          {displayMessage && <p role='status'>{displayMessage}</p>}
          <div className='flex flex-wrap items-end gap-3'>
            <div className='min-w-64 flex-1 space-y-2'>
              <Label htmlFor='definition-directory'>
                {t('attributeLifecycle-directory')}
              </Label>
              <Select
                value={attributeId ?? ''}
                onValueChange={selectAttribute}
                disabled={busy || storageError || record?.phase === 'pending'}
              >
                <SelectTrigger id='definition-directory'>
                  <SelectValue placeholder={t('attributeLifecycle-choose')} />
                </SelectTrigger>
                <SelectContent>
                  {directory?.attributes.map((attribute) => (
                    <SelectItem key={attribute.id} value={attribute.id}>
                      {attribute.translation?.label ?? attribute.code} ·{' '}
                      {attribute.code} ·{' '}
                      {t(
                        attribute.isArchived
                          ? 'attributeLifecycle-archived'
                          : 'attributeLifecycle-active'
                      )}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              variant='outline'
              onClick={() => void refresh(optionId)}
              disabled={locked}
            >
              {t('attributeLifecycle-refresh')}
            </Button>
          </div>
          {attributeId && (
            <>
              <p className='text-muted-foreground font-mono text-xs break-all'>
                {attributeId}
                {optionId ? ` / ${optionId}` : ''}
              </p>
              {selected && record === null && (
                <>
                  <p className='text-sm'>
                    {selected.code} · {selected.valueType} · {selected.scope} ·{' '}
                    {t('attributeLifecycle-locale')} {locale}
                  </p>
                  <Label htmlFor='definition-option'>
                    {t('attributeLifecycle-target')}
                  </Label>
                  <Select
                    value={optionId ?? 'attribute'}
                    onValueChange={selectOption}
                    disabled={locked}
                  >
                    <SelectTrigger id='definition-option'>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value='attribute'>
                        {t('attributeLifecycle-attribute')}
                      </SelectItem>
                      {optionId !== null && !target && (
                        <SelectItem value={optionId} disabled>
                          {t('attributeLifecycle-notFound')} · {optionId}
                        </SelectItem>
                      )}
                      {workspace?.options?.map((option) => (
                        <SelectItem key={option.id} value={option.id}>
                          {option.label ?? option.code} · {option.code} ·{' '}
                          {t(
                            option.isArchived
                              ? 'attributeLifecycle-archived'
                              : 'attributeLifecycle-active'
                          )}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  {target && (
                    <>
                      <Label htmlFor='definition-patch'>
                        {t('attributeLifecycle-patch')}
                      </Label>
                      <Textarea
                        id='definition-patch'
                        className='min-h-64 font-mono text-xs'
                        value={patch}
                        onChange={(event) => setPatch(event.target.value)}
                        disabled={locked}
                        aria-describedby='definition-patch-help'
                      />
                      <p
                        id='definition-patch-help'
                        className='text-muted-foreground text-sm'
                      >
                        {t('attributeLifecycle-patchHelp')}
                      </p>
                    </>
                  )}
                </>
              )}
              {!target && !readError && (
                <p role='status'>{t('attributeLifecycle-notFound')}</p>
              )}
              {impact && (
                <div className='space-y-2 text-sm'>
                  <p>{t('attributeLifecycle-impact')}</p>
                  <ul className='list-inside list-disc'>
                    {impact.references.map((reference) => (
                      <li key={`${reference.kind}:${reference.resourceId}`}>
                        {t(attributeReferenceKey(reference.kind))} ·{' '}
                        <code className='break-all'>
                          {reference.resourceId}
                        </code>
                      </li>
                    ))}
                  </ul>
                  {impact.hasMore && <p>{t('attributeLifecycle-more')}</p>}
                  {!impact.inspectionComplete && (
                    <p>{t('attributeLifecycle-limit')}</p>
                  )}
                </div>
              )}
              <div className='flex flex-wrap gap-2'>
                <Button
                  onClick={() => void submit('update')}
                  disabled={locked || !target}
                >
                  {t('attributeLifecycle-update')}
                </Button>
                <Button
                  variant='outline'
                  onClick={() => void submit('archive')}
                  disabled={locked || !target || target.isArchived}
                >
                  {t('attributeLifecycle-archive')}
                </Button>
                <Button
                  variant='outline'
                  onClick={() => void submit('restore')}
                  disabled={locked || !target || !target.isArchived}
                >
                  {t('attributeLifecycle-restore')}
                </Button>
                <Button
                  variant='outline'
                  onClick={() => void submit('recover')}
                  disabled={
                    busy ||
                    storageError ||
                    !recoveryReady ||
                    record?.phase !== 'pending'
                  }
                >
                  {t('attributeLifecycle-recover')}
                </Button>
                <Button
                  variant='outline'
                  onClick={fresh}
                  disabled={busy || storageError || record?.phase !== 'settled'}
                >
                  {t('attributeLifecycle-fresh')}
                </Button>
                {storageError && (
                  <Button
                    variant='outline'
                    onClick={recoverStorage}
                    disabled={busy}
                  >
                    {t('attributeLifecycle-recoverStorage')}
                  </Button>
                )}
              </div>
              {record && (
                <details>
                  <summary className='cursor-pointer'>
                    {t('attributeLifecycle-stored')}
                  </summary>
                  <pre className='max-h-96 overflow-auto text-xs break-all whitespace-pre-wrap'>
                    {JSON.stringify(record, null, 2)}
                  </pre>
                </details>
              )}
            </>
          )}
        </CardContent>
      </Card>
      <fieldset disabled={peerLocked} className='contents'>
        {children}
      </fieldset>
    </>
  );
}
