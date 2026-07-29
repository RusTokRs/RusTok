'use client';

import React from 'react';
import { usePathname, useRouter, useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/shadcn/alert';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger
} from '@/shared/ui/shadcn/tabs';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import { registerAdminModule } from '@/modules/registry';

import { executeTranslationOperation } from './api';
import { translationNavItems } from './nav';
import type {
  Glossary,
  GlossaryConcept,
  GlossarySummary,
  MemoryEntry,
  MemoryRetentionPolicy,
  MemorySuggestion,
  TranslationAdminPageProps,
  TranslationOperation,
  TranslationPolicy,
  TranslationResponse,
  TranslationTarget
} from './types';

registerAdminModule({
  id: 'translation',
  name: 'Translation',
  navItems: translationNavItems
});

type TranslationTab =
  'overview' | 'jobs' | 'glossaries' | 'memory' | 'inventory' | 'workflow';

export function TranslationAdminPage({
  graphql,
  token = null,
  tenantSlug = null,
  graphqlUrl
}: TranslationAdminPageProps): React.JSX.Element {
  const t = useTranslations('translation');
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const tab = parseTab(searchParams.get('tab'));
  const selectedGlossaryId = searchParams.get('glossary_id');
  const selectedMemoryEntryId = searchParams.get('memory_entry_id');
  const context = React.useMemo(
    () => ({ graphql, token, tenantSlug, graphqlUrl }),
    [graphql, token, tenantSlug, graphqlUrl]
  );
  const idempotencyKeys = React.useRef<Record<string, string>>({});
  const [policy, setPolicy] = React.useState<TranslationPolicy | null>(null);
  const [targets, setTargets] = React.useState<TranslationTarget[]>([]);
  const [glossaries, setGlossaries] = React.useState<GlossarySummary[]>([]);
  const [glossary, setGlossary] = React.useState<Glossary | null>(null);
  const [loadingGlossaries, setLoadingGlossaries] = React.useState(false);
  const [glossaryError, setGlossaryError] = React.useState<string | null>(null);
  const [bootstrapError, setBootstrapError] = React.useState<string | null>(
    null
  );
  const [loadingBootstrap, setLoadingBootstrap] = React.useState(true);
  const [pending, setPending] = React.useState(false);
  const [outcome, setOutcome] = React.useState<TranslationResponse | null>(
    null
  );
  const [operationError, setOperationError] = React.useState<string | null>(
    null
  );

  const [expectedRevision, setExpectedRevision] = React.useState('0');
  const [requiredLocales, setRequiredLocales] = React.useState('');
  const [sourceLocale, setSourceLocale] = React.useState('en');
  const [targetLocale, setTargetLocale] = React.useState('de');
  const [jobId, setJobId] = React.useState('');
  const [ownerSlug, setOwnerSlug] = React.useState('media');
  const [resourceKind, setResourceKind] = React.useState('asset');
  const [resourceId, setResourceId] = React.useState('');
  const [subresourceId, setSubresourceId] = React.useState('');
  const [limit, setLimit] = React.useState('100');
  const [pageSize, setPageSize] = React.useState('100');
  const [itemId, setItemId] = React.useState('');
  const [proposalId, setProposalId] = React.useState('');
  const [fieldKey, setFieldKey] = React.useState('alt');
  const [fieldValue, setFieldValue] = React.useState('');
  const [jobGlossaryId, setJobGlossaryId] = React.useState('');
  const [jobGlossaryRevision, setJobGlossaryRevision] = React.useState('');
  const [glossaryName, setGlossaryName] = React.useState('');
  const [glossaryDescription, setGlossaryDescription] = React.useState('');
  const [glossarySourceLocale, setGlossarySourceLocale] = React.useState('en');
  const [glossaryTargetLocale, setGlossaryTargetLocale] = React.useState('de');
  const [glossaryOwnerSlug, setGlossaryOwnerSlug] = React.useState('');
  const [glossaryResourceKind, setGlossaryResourceKind] = React.useState('');
  const [glossaryFieldKey, setGlossaryFieldKey] = React.useState('');
  const [editGlossaryName, setEditGlossaryName] = React.useState('');
  const [editGlossaryDescription, setEditGlossaryDescription] =
    React.useState('');
  const [glossaryConcepts, setGlossaryConcepts] = React.useState('[]');
  const applyGlossaryState = React.useCallback((value: Glossary) => {
    setGlossary(value);
    setEditGlossaryName(value.name);
    setEditGlossaryDescription(value.description);
    setGlossaryConcepts(JSON.stringify(value.concepts, null, 2));
  }, []);

  React.useEffect(() => {
    let active = true;
    setLoadingBootstrap(true);
    setBootstrapError(null);
    Promise.all([
      executeTranslationOperation(context, { kind: 'read_policy' }),
      executeTranslationOperation(context, { kind: 'list_targets' })
    ])
      .then(([policyResponse, targetResponse]) => {
        if (!active) return;
        if (
          policyResponse.kind !== 'policy' ||
          targetResponse.kind !== 'targets'
        ) {
          throw new Error(
            'Translation bootstrap returned an unexpected response'
          );
        }
        setPolicy(policyResponse.value);
        setExpectedRevision(String(policyResponse.value.revision));
        setRequiredLocales(
          policyResponse.value.requiredTargetLocales.join(', ')
        );
        setTargets(targetResponse.value);
      })
      .catch((error: unknown) => {
        if (active) setBootstrapError(errorMessage(error));
      })
      .finally(() => {
        if (active) setLoadingBootstrap(false);
      });
    return () => {
      active = false;
    };
  }, [context]);

  React.useEffect(() => {
    if (tab !== 'glossaries') return;
    let active = true;
    setLoadingGlossaries(true);
    setGlossaryError(null);
    const list = executeTranslationOperation(context, {
      kind: 'list_glossaries',
      limit: 200
    });
    const selected = selectedGlossaryId
      ? executeTranslationOperation(context, {
          kind: 'read_glossary',
          glossaryId: selectedGlossaryId
        })
      : Promise.resolve(null);
    Promise.all([list, selected])
      .then(([listResponse, selectedResponse]) => {
        if (!active) return;
        if (listResponse.kind !== 'glossaries') {
          throw new Error('Glossary list returned an unexpected response');
        }
        setGlossaries(listResponse.value);
        if (selectedResponse) {
          if (selectedResponse.kind !== 'glossary') {
            throw new Error('Glossary read returned an unexpected response');
          }
          applyGlossaryState(selectedResponse.value);
        } else {
          setGlossary(null);
        }
      })
      .catch((error: unknown) => {
        if (active) setGlossaryError(errorMessage(error));
      })
      .finally(() => {
        if (active) setLoadingGlossaries(false);
      });
    return () => {
      active = false;
    };
  }, [applyGlossaryState, context, selectedGlossaryId, tab]);

  const commandKey = React.useCallback((operation: string) => {
    const existing = idempotencyKeys.current[operation];
    if (existing) return existing;
    const key = `translation-admin:${operation}:${crypto.randomUUID()}`;
    idempotencyKeys.current[operation] = key;
    return key;
  }, []);

  const run = React.useCallback(
    async (operation: TranslationOperation, keyName?: string) => {
      if (pending) return;
      setPending(true);
      setOperationError(null);
      try {
        const response = await executeTranslationOperation(context, operation);
        setOutcome(response);
        if (response.kind === 'policy') {
          setPolicy(response.value);
          setExpectedRevision(String(response.value.revision));
          setRequiredLocales(response.value.requiredTargetLocales.join(', '));
        }
        if (response.kind === 'glossaries') {
          setGlossaries(response.value);
        }
        if (response.kind === 'glossary') {
          applyGlossaryState(response.value);
          setGlossaries((current) =>
            upsertGlossarySummary(current, response.value)
          );
        }
        if (keyName) delete idempotencyKeys.current[keyName];
      } catch (error: unknown) {
        setOperationError(errorMessage(error));
      } finally {
        setPending(false);
      }
    },
    [applyGlossaryState, context, pending]
  );

  function selectTab(nextTab: string) {
    const selected = parseTab(nextTab);
    const next = new URLSearchParams(searchParams.toString());
    if (selected === 'overview') next.delete('tab');
    else next.set('tab', selected);
    const query = next.toString();
    router.replace(query ? `${pathname}?${query}` : pathname, {
      scroll: false
    });
  }

  function selectGlossary(glossaryId: string | null) {
    const next = new URLSearchParams(searchParams.toString());
    next.set('tab', 'glossaries');
    if (glossaryId) next.set('glossary_id', glossaryId);
    else next.delete('glossary_id');
    router.replace(`${pathname}?${next.toString()}`, { scroll: false });
  }

  function selectMemoryEntry(entryId: string | null) {
    const next = new URLSearchParams(searchParams.toString());
    next.set('tab', 'memory');
    if (entryId) next.set('memory_entry_id', entryId);
    else next.delete('memory_entry_id');
    router.replace(`${pathname}?${next.toString()}`, { scroll: false });
  }

  function safeRun(factory: () => TranslationOperation, keyName?: string) {
    try {
      void run(factory(), keyName);
    } catch (error: unknown) {
      setOperationError(errorMessage(error));
    }
  }

  return (
    <div className='space-y-6' data-testid='translation-admin-next'>
      <Card>
        <CardHeader>
          <div className='mb-2'>
            <Badge variant='outline'>{t('badge')}</Badge>
          </div>
          <CardTitle>{t('title')}</CardTitle>
          <CardDescription>{t('subtitle')}</CardDescription>
        </CardHeader>
      </Card>

      <Tabs value={tab} onValueChange={selectTab}>
        <TabsList aria-label={t('tabs.label')}>
          <TabsTrigger value='overview'>{t('tabs.overview')}</TabsTrigger>
          <TabsTrigger value='jobs'>{t('tabs.jobs')}</TabsTrigger>
          <TabsTrigger value='glossaries'>{t('tabs.glossaries')}</TabsTrigger>
          <TabsTrigger value='memory'>{t('tabs.memory')}</TabsTrigger>
          <TabsTrigger value='inventory'>{t('tabs.inventory')}</TabsTrigger>
          <TabsTrigger value='workflow'>{t('tabs.workflow')}</TabsTrigger>
        </TabsList>

        <TabsContent value='overview' className='space-y-6'>
          {loadingBootstrap ? (
            <LoadingState label={t('loading')} />
          ) : bootstrapError ? (
            <ErrorState title={t('error.load')} message={bootstrapError} />
          ) : (
            <div className='grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)]'>
              <Card>
                <CardHeader>
                  <CardTitle>{t('policy.title')}</CardTitle>
                  <CardDescription>{t('policy.description')}</CardDescription>
                </CardHeader>
                <CardContent className='space-y-4'>
                  <div className='flex flex-wrap items-center gap-2'>
                    <Badge
                      variant={
                        policy?.freshness === 'current'
                          ? 'default'
                          : 'secondary'
                      }
                    >
                      {policy?.freshness ?? 'unknown'}
                    </Badge>
                    <span className='text-muted-foreground text-xs'>
                      {t('field.revision')}: {policy?.revision ?? 0}
                    </span>
                  </div>
                  <div className='flex flex-wrap gap-2'>
                    {policy?.requiredTargetLocales.map((locale) => (
                      <Badge key={locale} variant='secondary'>
                        {locale}
                      </Badge>
                    ))}
                  </div>
                  <div className='space-y-2'>
                    <Label htmlFor='translation-expected-revision'>
                      {t('field.revision')}
                    </Label>
                    <Input
                      id='translation-expected-revision'
                      value={expectedRevision}
                      onChange={(event) =>
                        setExpectedRevision(event.target.value)
                      }
                    />
                  </div>
                  <div className='space-y-2'>
                    <Label htmlFor='translation-required-locales'>
                      {t('field.locales')}
                    </Label>
                    <Input
                      id='translation-required-locales'
                      value={requiredLocales}
                      onChange={(event) =>
                        setRequiredLocales(event.target.value)
                      }
                    />
                  </div>
                  <Button
                    disabled={pending}
                    onClick={() =>
                      safeRun(
                        () => ({
                          kind: 'replace_policy',
                          expectedRevision: integer(
                            expectedRevision,
                            'expected_revision'
                          ),
                          requiredTargetLocales: csv(
                            requiredLocales,
                            'required_target_locales'
                          ),
                          idempotencyKey: commandKey('replace-policy')
                        }),
                        'replace-policy'
                      )
                    }
                  >
                    {t('action.replacePolicy')}
                  </Button>
                </CardContent>
              </Card>

              <Card>
                <CardHeader>
                  <CardTitle>{t('targets.title')}</CardTitle>
                  <CardDescription>{t('targets.description')}</CardDescription>
                </CardHeader>
                <CardContent>
                  {targets.length ? (
                    <div className='overflow-x-auto rounded-xl border'>
                      <table className='w-full text-sm'>
                        <thead className='bg-muted/50 text-muted-foreground text-left text-xs uppercase'>
                          <tr>
                            <th className='px-4 py-3'>
                              {t('targets.provider')}
                            </th>
                            <th className='px-4 py-3'>{t('targets.target')}</th>
                            <th className='px-4 py-3'>
                              {t('targets.capabilities')}
                            </th>
                          </tr>
                        </thead>
                        <tbody className='divide-y'>
                          {targets.map((target) => (
                            <tr
                              key={`${target.ownerSlug}/${target.resourceKind}`}
                            >
                              <td className='px-4 py-3 font-medium'>
                                {target.ownerSlug}
                              </td>
                              <td className='px-4 py-3'>
                                <div className='font-medium'>
                                  {target.displayName}
                                </div>
                                <div className='text-muted-foreground text-xs'>
                                  {target.resourceKind}
                                </div>
                              </td>
                              <td className='px-4 py-3'>
                                <div className='flex flex-wrap gap-1'>
                                  {target.capabilities.map((capability) => (
                                    <Badge key={capability} variant='outline'>
                                      {capability}
                                    </Badge>
                                  ))}
                                </div>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  ) : (
                    <EmptyState message={t('targets.empty')} />
                  )}
                </CardContent>
              </Card>
            </div>
          )}
        </TabsContent>

        <TabsContent value='jobs' className='space-y-6'>
          <div className='grid gap-6 xl:grid-cols-2'>
            <Card>
              <CardHeader>
                <CardTitle>{t('jobs.create')}</CardTitle>
                <CardDescription>{t('jobs.createDescription')}</CardDescription>
              </CardHeader>
              <CardContent className='space-y-4'>
                <div className='grid gap-4 sm:grid-cols-2'>
                  <div className='space-y-2'>
                    <Label htmlFor='translation-source-locale'>
                      {t('field.sourceLocale')}
                    </Label>
                    <Input
                      id='translation-source-locale'
                      value={sourceLocale}
                      onChange={(event) => setSourceLocale(event.target.value)}
                    />
                  </div>
                  <div className='space-y-2'>
                    <Label htmlFor='translation-target-locale'>
                      {t('field.targetLocale')}
                    </Label>
                    <Input
                      id='translation-target-locale'
                      value={targetLocale}
                      onChange={(event) => setTargetLocale(event.target.value)}
                    />
                  </div>
                  <div className='space-y-2'>
                    <Label htmlFor='translation-job-glossary-id'>
                      {t('field.glossaryId')}
                    </Label>
                    <Input
                      id='translation-job-glossary-id'
                      value={jobGlossaryId}
                      onChange={(event) => setJobGlossaryId(event.target.value)}
                    />
                  </div>
                  <div className='space-y-2'>
                    <Label htmlFor='translation-job-glossary-revision'>
                      {t('field.glossaryRevision')}
                    </Label>
                    <Input
                      id='translation-job-glossary-revision'
                      value={jobGlossaryRevision}
                      onChange={(event) =>
                        setJobGlossaryRevision(event.target.value)
                      }
                    />
                  </div>
                </div>
                <Button
                  disabled={pending}
                  onClick={() =>
                    safeRun(
                      () => ({
                        kind: 'create_job',
                        sourceLocale: required(sourceLocale, 'source_locale'),
                        targetLocale: required(targetLocale, 'target_locale'),
                        glossary: optionalGlossaryBinding(
                          jobGlossaryId,
                          jobGlossaryRevision
                        ),
                        idempotencyKey: commandKey('create-job')
                      }),
                      'create-job'
                    )
                  }
                >
                  {t('action.createJob')}
                </Button>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>{t('jobs.inspect')}</CardTitle>
                <CardDescription>
                  {t('jobs.inspectDescription')}
                </CardDescription>
              </CardHeader>
              <CardContent className='space-y-4'>
                <div className='space-y-2'>
                  <Label htmlFor='translation-job-id'>{t('field.jobId')}</Label>
                  <Input
                    id='translation-job-id'
                    value={jobId}
                    onChange={(event) => setJobId(event.target.value)}
                  />
                </div>
                <div className='flex flex-wrap gap-2'>
                  <Button
                    variant='outline'
                    disabled={pending}
                    onClick={() =>
                      safeRun(() => ({
                        kind: 'read_job_progress',
                        jobId: required(jobId, 'job_id')
                      }))
                    }
                  >
                    {t('action.readProgress')}
                  </Button>
                  <Button
                    variant='secondary'
                    disabled={pending}
                    onClick={() =>
                      safeRun(
                        () => ({
                          kind: 'rebuild_job_progress',
                          jobId: required(jobId, 'job_id'),
                          idempotencyKey: commandKey('rebuild-job-progress')
                        }),
                        'rebuild-job-progress'
                      )
                    }
                  >
                    {t('action.rebuildProgress')}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value='glossaries' className='space-y-6'>
          {loadingGlossaries ? (
            <LoadingState label={t('glossary.loading')} />
          ) : glossaryError ? (
            <ErrorState title={t('glossary.error')} message={glossaryError} />
          ) : (
            <>
              <div className='grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.3fr)]'>
                <Card>
                  <CardHeader>
                    <CardTitle>{t('glossary.create')}</CardTitle>
                    <CardDescription>
                      {t('glossary.createDescription')}
                    </CardDescription>
                  </CardHeader>
                  <CardContent className='space-y-4'>
                    <TextInput
                      id='translation-glossary-name'
                      label={t('field.name')}
                      value={glossaryName}
                      onChange={setGlossaryName}
                    />
                    <div className='space-y-2'>
                      <Label htmlFor='translation-glossary-description'>
                        {t('field.description')}
                      </Label>
                      <Textarea
                        id='translation-glossary-description'
                        value={glossaryDescription}
                        onChange={(event) =>
                          setGlossaryDescription(event.target.value)
                        }
                      />
                    </div>
                    <div className='grid gap-4 sm:grid-cols-2'>
                      <TextInput
                        id='translation-glossary-source'
                        label={t('field.sourceLocale')}
                        value={glossarySourceLocale}
                        onChange={setGlossarySourceLocale}
                      />
                      <TextInput
                        id='translation-glossary-target'
                        label={t('field.targetLocale')}
                        value={glossaryTargetLocale}
                        onChange={setGlossaryTargetLocale}
                      />
                      <TextInput
                        id='translation-glossary-owner'
                        label={t('field.ownerSlug')}
                        value={glossaryOwnerSlug}
                        onChange={setGlossaryOwnerSlug}
                      />
                      <TextInput
                        id='translation-glossary-kind'
                        label={t('field.resourceKind')}
                        value={glossaryResourceKind}
                        onChange={setGlossaryResourceKind}
                      />
                      <TextInput
                        id='translation-glossary-field'
                        label={t('field.fieldKey')}
                        value={glossaryFieldKey}
                        onChange={setGlossaryFieldKey}
                      />
                    </div>
                    <Button
                      disabled={pending}
                      onClick={() =>
                        safeRun(
                          () => ({
                            kind: 'create_glossary',
                            name: required(glossaryName, 'name'),
                            description: glossaryDescription.trim(),
                            sourceLocale: required(
                              glossarySourceLocale,
                              'source_locale'
                            ),
                            targetLocale: required(
                              glossaryTargetLocale,
                              'target_locale'
                            ),
                            scope: glossaryScope(
                              glossaryOwnerSlug,
                              glossaryResourceKind,
                              glossaryFieldKey
                            ),
                            idempotencyKey: commandKey('create-glossary')
                          }),
                          'create-glossary'
                        )
                      }
                    >
                      {t('action.createGlossary')}
                    </Button>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader>
                    <CardTitle>{t('glossary.list')}</CardTitle>
                    <CardDescription>
                      {t('glossary.listDescription')}
                    </CardDescription>
                  </CardHeader>
                  <CardContent className='space-y-3'>
                    {glossaries.length ? (
                      glossaries.map((item) => (
                        <button
                          key={item.id}
                          type='button'
                          className={`w-full rounded-xl border p-4 text-left transition-colors ${
                            selectedGlossaryId === item.id
                              ? 'border-primary bg-primary/5'
                              : 'hover:bg-muted/50'
                          }`}
                          onClick={() => selectGlossary(item.id)}
                        >
                          <div className='flex flex-wrap items-center justify-between gap-2'>
                            <span className='font-medium'>{item.name}</span>
                            <div className='flex gap-2'>
                              <Badge
                                variant={
                                  item.isActive ? 'default' : 'secondary'
                                }
                              >
                                {item.isActive
                                  ? t('glossary.active')
                                  : t('glossary.inactive')}
                              </Badge>
                              <Badge variant='outline'>v{item.revision}</Badge>
                            </div>
                          </div>
                          <p className='text-muted-foreground mt-2 text-xs'>
                            {item.sourceLocale} → {item.targetLocale}
                          </p>
                        </button>
                      ))
                    ) : (
                      <EmptyState message={t('glossary.empty')} />
                    )}
                  </CardContent>
                </Card>
              </div>

              {glossary ? (
                <div className='grid gap-6 xl:grid-cols-2'>
                  <Card>
                    <CardHeader>
                      <CardTitle>{t('glossary.metadata')}</CardTitle>
                      <CardDescription>
                        {t('glossary.metadataDescription')}
                      </CardDescription>
                    </CardHeader>
                    <CardContent className='space-y-4'>
                      <div className='flex flex-wrap gap-2'>
                        <Badge variant='outline'>v{glossary.revision}</Badge>
                        <Badge
                          variant={glossary.isActive ? 'default' : 'secondary'}
                        >
                          {glossary.isActive
                            ? t('glossary.active')
                            : t('glossary.inactive')}
                        </Badge>
                        <Badge variant='outline'>
                          {glossary.concepts.length} {t('field.concepts')}
                        </Badge>
                      </div>
                      <TextInput
                        id='translation-glossary-edit-name'
                        label={t('field.name')}
                        value={editGlossaryName}
                        onChange={setEditGlossaryName}
                      />
                      <div className='space-y-2'>
                        <Label htmlFor='translation-glossary-edit-description'>
                          {t('field.description')}
                        </Label>
                        <Textarea
                          id='translation-glossary-edit-description'
                          value={editGlossaryDescription}
                          onChange={(event) =>
                            setEditGlossaryDescription(event.target.value)
                          }
                        />
                      </div>
                      <div className='flex flex-wrap gap-2'>
                        <Button
                          disabled={pending}
                          onClick={() =>
                            safeRun(
                              () => ({
                                kind: 'update_glossary',
                                glossaryId: glossary.id,
                                expectedRevision: glossary.revision,
                                name: required(editGlossaryName, 'name'),
                                description: editGlossaryDescription.trim(),
                                idempotencyKey: commandKey('update-glossary')
                              }),
                              'update-glossary'
                            )
                          }
                        >
                          {t('action.updateGlossary')}
                        </Button>
                        <Button
                          variant='secondary'
                          disabled={pending}
                          onClick={() =>
                            safeRun(
                              () => ({
                                kind: 'set_glossary_active',
                                glossaryId: glossary.id,
                                expectedRevision: glossary.revision,
                                isActive: !glossary.isActive,
                                idempotencyKey: commandKey(
                                  'set-glossary-active'
                                )
                              }),
                              'set-glossary-active'
                            )
                          }
                        >
                          {glossary.isActive
                            ? t('action.deactivateGlossary')
                            : t('action.activateGlossary')}
                        </Button>
                        <Button
                          variant='outline'
                          onClick={() => selectGlossary(null)}
                        >
                          {t('action.clearSelection')}
                        </Button>
                      </div>
                    </CardContent>
                  </Card>

                  <Card>
                    <CardHeader>
                      <CardTitle>{t('glossary.terms')}</CardTitle>
                      <CardDescription>
                        {t('glossary.termsDescription')}
                      </CardDescription>
                    </CardHeader>
                    <CardContent className='space-y-4'>
                      <div className='space-y-2'>
                        <Label htmlFor='translation-glossary-concepts'>
                          {t('field.conceptsJson')}
                        </Label>
                        <Textarea
                          id='translation-glossary-concepts'
                          className='min-h-80 font-mono text-xs'
                          value={glossaryConcepts}
                          onChange={(event) =>
                            setGlossaryConcepts(event.target.value)
                          }
                        />
                      </div>
                      <Button
                        disabled={pending || !glossary.isActive}
                        onClick={() =>
                          safeRun(
                            () => ({
                              kind: 'replace_glossary_terms',
                              glossaryId: glossary.id,
                              expectedRevision: glossary.revision,
                              concepts: parseGlossaryConcepts(glossaryConcepts),
                              idempotencyKey: commandKey(
                                'replace-glossary-terms'
                              )
                            }),
                            'replace-glossary-terms'
                          )
                        }
                      >
                        {t('action.replaceGlossaryTerms')}
                      </Button>
                    </CardContent>
                  </Card>
                </div>
              ) : (
                <EmptyState message={t('glossary.select')} />
              )}
            </>
          )}
        </TabsContent>

        <TabsContent value='memory' className='space-y-6'>
          <MemoryPanel
            active={tab === 'memory'}
            context={context}
            selectedEntryId={selectedMemoryEntryId}
            onSelectEntry={selectMemoryEntry}
          />
        </TabsContent>

        <TabsContent value='inventory' className='space-y-6'>
          <Card>
            <CardHeader>
              <CardTitle>{t('inventory.title')}</CardTitle>
              <CardDescription>{t('inventory.description')}</CardDescription>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-4'>
                <TextInput
                  id='translation-owner'
                  label={t('field.ownerSlug')}
                  value={ownerSlug}
                  onChange={setOwnerSlug}
                />
                <TextInput
                  id='translation-kind'
                  label={t('field.resourceKind')}
                  value={resourceKind}
                  onChange={setResourceKind}
                />
                <TextInput
                  id='translation-inventory-source'
                  label={t('field.sourceLocale')}
                  value={sourceLocale}
                  onChange={setSourceLocale}
                />
                <TextInput
                  id='translation-inventory-target'
                  label={t('field.targetLocale')}
                  value={targetLocale}
                  onChange={setTargetLocale}
                />
                <TextInput
                  id='translation-limit'
                  label={t('field.limit')}
                  value={limit}
                  onChange={setLimit}
                />
                <TextInput
                  id='translation-page-size'
                  label={t('field.pageSize')}
                  value={pageSize}
                  onChange={setPageSize}
                />
              </div>
              <div className='flex flex-wrap gap-2'>
                <Button
                  disabled={pending}
                  onClick={() =>
                    safeRun(() => ({
                      kind: 'sync_inventory',
                      ownerSlug: required(ownerSlug, 'owner_slug'),
                      resourceKind: required(resourceKind, 'resource_kind'),
                      limit: positiveInteger(limit, 'limit')
                    }))
                  }
                >
                  {t('action.syncInventory')}
                </Button>
                <Button
                  variant='secondary'
                  disabled={pending}
                  onClick={() =>
                    safeRun(() => ({
                      kind: 'rebuild_inventory',
                      ownerSlug: required(ownerSlug, 'owner_slug'),
                      resourceKind: required(resourceKind, 'resource_kind'),
                      sourceLocale: required(sourceLocale, 'source_locale'),
                      targetLocale: required(targetLocale, 'target_locale'),
                      pageSize: positiveInteger(pageSize, 'page_size')
                    }))
                  }
                >
                  {t('action.rebuildInventory')}
                </Button>
                <Button
                  variant='outline'
                  disabled={pending}
                  onClick={() =>
                    safeRun(() => ({
                      kind: 'read_provider_progress',
                      ownerSlug: required(ownerSlug, 'owner_slug'),
                      resourceKind: required(resourceKind, 'resource_kind'),
                      sourceLocale: required(sourceLocale, 'source_locale'),
                      targetLocale: required(targetLocale, 'target_locale')
                    }))
                  }
                >
                  {t('action.readCoverage')}
                </Button>
                <Button
                  variant='outline'
                  disabled={pending}
                  onClick={() =>
                    safeRun(() => ({
                      kind: 'read_required_progress',
                      ownerSlug: required(ownerSlug, 'owner_slug'),
                      resourceKind: required(resourceKind, 'resource_kind'),
                      sourceLocale: required(sourceLocale, 'source_locale')
                    }))
                  }
                >
                  {t('action.readRequiredCoverage')}
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value='workflow' className='space-y-6'>
          <div className='grid gap-6 xl:grid-cols-2'>
            <Card>
              <CardHeader>
                <CardTitle>{t('workflow.admit')}</CardTitle>
                <CardDescription>
                  {t('workflow.admitDescription')}
                </CardDescription>
              </CardHeader>
              <CardContent className='space-y-4'>
                <div className='grid gap-4 sm:grid-cols-2'>
                  <TextInput
                    id='translation-workflow-job'
                    label={t('field.jobId')}
                    value={jobId}
                    onChange={setJobId}
                  />
                  <TextInput
                    id='translation-resource-id'
                    label={t('field.resourceId')}
                    value={resourceId}
                    onChange={setResourceId}
                  />
                  <TextInput
                    id='translation-workflow-owner'
                    label={t('field.ownerSlug')}
                    value={ownerSlug}
                    onChange={setOwnerSlug}
                  />
                  <TextInput
                    id='translation-workflow-kind'
                    label={t('field.resourceKind')}
                    value={resourceKind}
                    onChange={setResourceKind}
                  />
                  <TextInput
                    id='translation-subresource'
                    label={t('field.subresourceId')}
                    value={subresourceId}
                    onChange={setSubresourceId}
                  />
                </div>
                <Button
                  disabled={pending}
                  onClick={() =>
                    safeRun(
                      () => ({
                        kind: 'add_item',
                        jobId: required(jobId, 'job_id'),
                        ownerSlug: required(ownerSlug, 'owner_slug'),
                        resourceKind: required(resourceKind, 'resource_kind'),
                        resourceId: required(resourceId, 'resource_id'),
                        subresourceId: subresourceId.trim() || undefined,
                        idempotencyKey: commandKey('add-item')
                      }),
                      'add-item'
                    )
                  }
                >
                  {t('action.addItem')}
                </Button>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>{t('workflow.proposal')}</CardTitle>
                <CardDescription>
                  {t('workflow.proposalDescription')}
                </CardDescription>
              </CardHeader>
              <CardContent className='space-y-4'>
                <TextInput
                  id='translation-item'
                  label={t('field.itemId')}
                  value={itemId}
                  onChange={setItemId}
                />
                <TextInput
                  id='translation-field-key'
                  label={t('field.fieldKey')}
                  value={fieldKey}
                  onChange={setFieldKey}
                />
                <div className='space-y-2'>
                  <Label htmlFor='translation-field-value'>
                    {t('field.value')}
                  </Label>
                  <Textarea
                    id='translation-field-value'
                    value={fieldValue}
                    onChange={(event) => setFieldValue(event.target.value)}
                  />
                </div>
                <Button
                  disabled={pending}
                  onClick={() =>
                    safeRun(
                      () => ({
                        kind: 'save_proposal',
                        itemId: required(itemId, 'item_id'),
                        fieldKey: required(fieldKey, 'field_key'),
                        value: required(fieldValue, 'value', false),
                        idempotencyKey: commandKey('save-proposal')
                      }),
                      'save-proposal'
                    )
                  }
                >
                  {t('action.saveProposal')}
                </Button>
              </CardContent>
            </Card>
          </div>
          <Card>
            <CardHeader>
              <CardTitle>{t('workflow.review')}</CardTitle>
              <CardDescription>
                {t('workflow.reviewDescription')}
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='grid gap-4 sm:grid-cols-2'>
                <TextInput
                  id='translation-review-item'
                  label={t('field.itemId')}
                  value={itemId}
                  onChange={setItemId}
                />
                <TextInput
                  id='translation-proposal'
                  label={t('field.proposalId')}
                  value={proposalId}
                  onChange={setProposalId}
                />
              </div>
              <div className='flex flex-wrap gap-2'>
                {(['submit', 'approve', 'apply'] as const).map((command) => (
                  <Button
                    key={command}
                    variant={
                      command === 'submit'
                        ? 'outline'
                        : command === 'approve'
                          ? 'secondary'
                          : 'default'
                    }
                    disabled={pending}
                    onClick={() =>
                      safeRun(
                        () => ({
                          kind: `${command}_proposal`,
                          itemId: required(itemId, 'item_id'),
                          proposalId: required(proposalId, 'proposal_id'),
                          idempotencyKey: commandKey(`${command}-proposal`)
                        }),
                        `${command}-proposal`
                      )
                    }
                  >
                    {t(`action.${command}Proposal`)}
                  </Button>
                ))}
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      {pending && <LoadingState label={t('loading')} />}
      {operationError && (
        <ErrorState title={t('error.operation')} message={operationError} />
      )}
      {outcome && <OperationReceipt response={outcome} />}
    </div>
  );
}

function MemoryPanel({
  active,
  context,
  selectedEntryId,
  onSelectEntry
}: {
  active: boolean;
  context: TranslationAdminPageProps;
  selectedEntryId: string | null;
  onSelectEntry: (entryId: string | null) => void;
}): React.JSX.Element {
  const t = useTranslations('translation');
  const idempotencyKeys = React.useRef<Record<string, string>>({});
  const [entries, setEntries] = React.useState<MemoryEntry[]>([]);
  const [entry, setEntry] = React.useState<MemoryEntry | null>(null);
  const [suggestions, setSuggestions] = React.useState<MemorySuggestion[]>([]);
  const [loading, setLoading] = React.useState(false);
  const [pending, setPending] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [outcome, setOutcome] = React.useState<TranslationResponse | null>(
    null
  );
  const [refreshRevision, setRefreshRevision] = React.useState(0);

  const [listSourceLocale, setListSourceLocale] = React.useState('');
  const [listTargetLocale, setListTargetLocale] = React.useState('');
  const [includeTombstoned, setIncludeTombstoned] = React.useState(false);
  const [listLimit, setListLimit] = React.useState('200');
  const [filters, setFilters] = React.useState({
    sourceLocale: '',
    targetLocale: '',
    includeTombstoned: false,
    limit: 200
  });

  const [lookupSourceLocale, setLookupSourceLocale] = React.useState('en');
  const [lookupTargetLocale, setLookupTargetLocale] = React.useState('de');
  const [lookupOwnerSlug, setLookupOwnerSlug] = React.useState('media');
  const [lookupResourceKind, setLookupResourceKind] = React.useState('asset');
  const [lookupResourceId, setLookupResourceId] = React.useState('');
  const [lookupSubresourceId, setLookupSubresourceId] = React.useState('');
  const [lookupFieldKey, setLookupFieldKey] = React.useState('alt');
  const [lookupSourceText, setLookupSourceText] = React.useState('');
  const [lookupMinimumScore, setLookupMinimumScore] = React.useState('8500');
  const [lookupLimit, setLookupLimit] = React.useState('10');

  const [expectedRevision, setExpectedRevision] = React.useState('1');
  const [retentionPolicy, setRetentionPolicy] =
    React.useState<MemoryRetentionPolicy>('OWNER_LIFECYCLE');
  const [retainUntil, setRetainUntil] = React.useState('');

  React.useEffect(() => {
    if (!active) return;
    let current = true;
    setLoading(true);
    setError(null);
    const list = executeTranslationOperation(context, {
      kind: 'list_memory_entries',
      ...(filters.sourceLocale ? { sourceLocale: filters.sourceLocale } : {}),
      ...(filters.targetLocale ? { targetLocale: filters.targetLocale } : {}),
      includeTombstoned: filters.includeTombstoned,
      limit: filters.limit
    });
    const selected = selectedEntryId
      ? executeTranslationOperation(context, {
          kind: 'read_memory_entry',
          entryId: selectedEntryId
        })
      : Promise.resolve(null);
    Promise.all([list, selected])
      .then(([listResponse, selectedResponse]) => {
        if (!current) return;
        if (listResponse.kind !== 'memory_entries') {
          throw new Error('Memory list returned an unexpected response');
        }
        setEntries(listResponse.value);
        if (selectedResponse) {
          if (selectedResponse.kind !== 'memory_entry') {
            throw new Error(
              'Memory entry read returned an unexpected response'
            );
          }
          const value = selectedResponse.value;
          setEntry(value);
          setExpectedRevision(String(value.revision));
          setRetentionPolicy(value.retentionPolicy);
          setRetainUntil(value.retainUntil ?? '');
        } else {
          setEntry(null);
          setExpectedRevision('1');
          setRetentionPolicy('OWNER_LIFECYCLE');
          setRetainUntil('');
        }
      })
      .catch((cause: unknown) => {
        if (current) setError(errorMessage(cause));
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [active, context, filters, refreshRevision, selectedEntryId]);

  const commandKey = React.useCallback((operation: string): string => {
    const existing = idempotencyKeys.current[operation];
    if (existing) return existing;
    const key = `translation-admin:${operation}:${crypto.randomUUID()}`;
    idempotencyKeys.current[operation] = key;
    return key;
  }, []);

  const runMemory = React.useCallback(
    async (operation: TranslationOperation, keyName?: string) => {
      if (pending) return;
      setPending(true);
      setError(null);
      try {
        const response = await executeTranslationOperation(context, operation);
        setOutcome(response);
        if (response.kind === 'memory_suggestions') {
          setSuggestions(response.value);
        }
        if (response.kind === 'memory_mutation') {
          if (response.value.state === 'purged') {
            onSelectEntry(null);
          }
          setRefreshRevision((current) => current + 1);
        }
        if (keyName) delete idempotencyKeys.current[keyName];
      } catch (cause: unknown) {
        setError(errorMessage(cause));
      } finally {
        setPending(false);
      }
    },
    [context, onSelectEntry, pending]
  );

  function safeMemoryRun(
    factory: () => TranslationOperation,
    keyName?: string
  ) {
    try {
      void runMemory(factory(), keyName);
    } catch (cause: unknown) {
      setError(errorMessage(cause));
    }
  }

  function applyFilters() {
    try {
      setFilters({
        sourceLocale: listSourceLocale.trim(),
        targetLocale: listTargetLocale.trim(),
        includeTombstoned,
        limit: positiveInteger(listLimit, 'limit')
      });
    } catch (cause: unknown) {
      setError(errorMessage(cause));
    }
  }

  return (
    <div className='space-y-6'>
      <Card>
        <CardHeader>
          <CardTitle>{t('memory.lookup')}</CardTitle>
          <CardDescription>{t('memory.lookupDescription')}</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-4'>
            <TextInput
              id='memory-lookup-source-locale'
              label={t('field.sourceLocale')}
              value={lookupSourceLocale}
              onChange={setLookupSourceLocale}
            />
            <TextInput
              id='memory-lookup-target-locale'
              label={t('field.targetLocale')}
              value={lookupTargetLocale}
              onChange={setLookupTargetLocale}
            />
            <TextInput
              id='memory-lookup-owner-slug'
              label={t('field.ownerSlug')}
              value={lookupOwnerSlug}
              onChange={setLookupOwnerSlug}
            />
            <TextInput
              id='memory-lookup-resource-kind'
              label={t('field.resourceKind')}
              value={lookupResourceKind}
              onChange={setLookupResourceKind}
            />
            <TextInput
              id='memory-lookup-resource-id'
              label={t('field.resourceId')}
              value={lookupResourceId}
              onChange={setLookupResourceId}
            />
            <TextInput
              id='memory-lookup-subresource-id'
              label={t('field.subresourceId')}
              value={lookupSubresourceId}
              onChange={setLookupSubresourceId}
            />
            <TextInput
              id='memory-lookup-field-key'
              label={t('field.fieldKey')}
              value={lookupFieldKey}
              onChange={setLookupFieldKey}
            />
            <TextInput
              id='memory-lookup-score'
              label={t('field.minimumScore')}
              value={lookupMinimumScore}
              onChange={setLookupMinimumScore}
            />
          </div>
          <div className='space-y-2'>
            <Label htmlFor='memory-lookup-source-text'>
              {t('field.sourceText')}
            </Label>
            <Textarea
              id='memory-lookup-source-text'
              value={lookupSourceText}
              onChange={(event) => setLookupSourceText(event.target.value)}
            />
          </div>
          <div className='flex flex-wrap items-end gap-3'>
            <div className='w-32'>
              <TextInput
                id='memory-lookup-limit'
                label={t('field.lookupLimit')}
                value={lookupLimit}
                onChange={setLookupLimit}
              />
            </div>
            <Button
              disabled={pending}
              onClick={() =>
                safeMemoryRun(() => ({
                  kind: 'lookup_memory',
                  sourceLocale: required(lookupSourceLocale, 'source_locale'),
                  targetLocale: required(lookupTargetLocale, 'target_locale'),
                  identity: {
                    ownerSlug: required(lookupOwnerSlug, 'owner_slug'),
                    resourceKind: required(lookupResourceKind, 'resource_kind'),
                    resourceId: required(lookupResourceId, 'resource_id'),
                    subresourceId: lookupSubresourceId.trim() || null
                  },
                  fieldKey: required(lookupFieldKey, 'field_key'),
                  sourceText: required(lookupSourceText, 'source_text', false),
                  minimumSimilarityBasisPoints: basisPoints(
                    lookupMinimumScore,
                    'minimum_similarity_basis_points'
                  ),
                  limit: positiveInteger(lookupLimit, 'limit')
                }))
              }
            >
              {t('action.lookupMemory')}
            </Button>
          </div>
          {suggestions.length === 0 ? (
            <EmptyState message={t('memory.noSuggestions')} />
          ) : (
            <div className='grid gap-3'>
              {suggestions.map((suggestion) => (
                <div key={suggestion.entryId} className='rounded-xl border p-4'>
                  <div className='flex flex-wrap items-center justify-between gap-2'>
                    <span className='font-medium'>{suggestion.targetText}</span>
                    <Badge variant='outline'>
                      {suggestion.evidence.finalSimilarityBasisPoints} bp
                    </Badge>
                  </div>
                  <p className='text-muted-foreground mt-2 text-sm'>
                    {suggestion.sourceText}
                  </p>
                  <p className='text-muted-foreground mt-2 text-xs'>
                    {suggestion.ownerSlug}/{suggestion.resourceKind} |{' '}
                    {suggestion.fieldKey} | {suggestion.evidence.kind}
                  </p>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t('memory.list')}</CardTitle>
          <CardDescription>{t('memory.listDescription')}</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='grid gap-4 md:grid-cols-2 xl:grid-cols-4'>
            <TextInput
              id='memory-list-source-locale'
              label={t('field.sourceLocaleFilter')}
              value={listSourceLocale}
              onChange={setListSourceLocale}
            />
            <TextInput
              id='memory-list-target-locale'
              label={t('field.targetLocaleFilter')}
              value={listTargetLocale}
              onChange={setListTargetLocale}
            />
            <TextInput
              id='memory-list-limit'
              label={t('field.listLimit')}
              value={listLimit}
              onChange={setListLimit}
            />
            <label className='flex items-center gap-2 pt-8 text-sm'>
              <Checkbox
                checked={includeTombstoned}
                onCheckedChange={(checked) =>
                  setIncludeTombstoned(checked === true)
                }
              />
              {t('field.includeTombstoned')}
            </label>
          </div>
          <Button variant='outline' onClick={applyFilters}>
            {t('action.refreshMemory')}
          </Button>
        </CardContent>
      </Card>

      {loading ? (
        <LoadingState label={t('memory.loading')} />
      ) : (
        <div className='grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)]'>
          <Card>
            <CardHeader>
              <CardTitle>{t('memory.entries')}</CardTitle>
              <CardDescription>
                {t('memory.selectionDescription')}
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-3'>
              {entries.length === 0 ? (
                <EmptyState message={t('memory.empty')} />
              ) : (
                entries.map((value) => (
                  <button
                    key={value.id}
                    type='button'
                    className={
                      selectedEntryId === value.id
                        ? 'border-primary bg-primary/5 w-full rounded-xl border p-4 text-left'
                        : 'hover:bg-muted/50 w-full rounded-xl border p-4 text-left'
                    }
                    onClick={() => onSelectEntry(value.id)}
                  >
                    <div className='flex flex-wrap items-center justify-between gap-2'>
                      <span className='font-medium'>
                        {value.ownerSlug}/{value.resourceKind} |{' '}
                        {value.fieldKey}
                      </span>
                      <div className='flex gap-2'>
                        <Badge
                          variant={value.tombstonedAt ? 'secondary' : 'default'}
                        >
                          {value.tombstonedAt
                            ? t('memory.tombstoned')
                            : t('memory.active')}
                        </Badge>
                        <Badge variant='outline'>v{value.revision}</Badge>
                      </div>
                    </div>
                    <p className='text-muted-foreground mt-2 text-xs'>
                      {value.sourceLocale} -&gt; {value.targetLocale} |{' '}
                      {value.resourceId}
                    </p>
                  </button>
                ))
              )}
            </CardContent>
          </Card>

          {entry ? (
            <div className='space-y-6'>
              <Card>
                <CardHeader>
                  <CardTitle>{t('memory.selected')}</CardTitle>
                  <CardDescription>{entry.id}</CardDescription>
                </CardHeader>
                <CardContent className='space-y-4'>
                  <div className='flex flex-wrap gap-2'>
                    <Badge variant='outline'>
                      {entry.sourceLocale} -&gt; {entry.targetLocale}
                    </Badge>
                    <Badge variant='outline'>{entry.qualityState}</Badge>
                    <Badge variant='outline'>v{entry.revision}</Badge>
                  </div>
                  <MemoryText
                    label={t('field.sourceText')}
                    value={entry.sourceText}
                  />
                  <MemoryText
                    label={t('field.targetText')}
                    value={entry.targetText}
                  />
                  <dl className='grid gap-3 text-xs sm:grid-cols-2'>
                    <MemoryFact
                      label={t('field.resource')}
                      value={`${entry.ownerSlug}/${entry.resourceKind}/${entry.resourceId}`}
                    />
                    <MemoryFact
                      label={t('field.reviewer')}
                      value={`${entry.reviewerActorKind}:${entry.reviewerActorId}`}
                    />
                    <MemoryFact
                      label={t('field.proposalId')}
                      value={entry.proposalId}
                    />
                    <MemoryFact
                      label={t('field.providerReceipt')}
                      value={entry.applyReceiptId}
                    />
                  </dl>
                </CardContent>
              </Card>

              <Card>
                <CardHeader>
                  <CardTitle>{t('memory.lifecycle')}</CardTitle>
                  <CardDescription>
                    {t('memory.lifecycleDescription')}
                  </CardDescription>
                </CardHeader>
                <CardContent className='space-y-4'>
                  <div className='grid gap-4 md:grid-cols-2'>
                    <div className='space-y-2'>
                      <Label>{t('field.retentionPolicy')}</Label>
                      <Select
                        value={retentionPolicy}
                        onValueChange={(value) =>
                          setRetentionPolicy(value as MemoryRetentionPolicy)
                        }
                      >
                        <SelectTrigger className='w-full'>
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value='OWNER_LIFECYCLE'>
                            {t('memory.retention.ownerLifecycle')}
                          </SelectItem>
                          <SelectItem value='RETAIN_UNTIL'>
                            {t('memory.retention.retainUntil')}
                          </SelectItem>
                          <SelectItem value='LEGAL_HOLD'>
                            {t('memory.retention.legalHold')}
                          </SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                    <TextInput
                      id='memory-retain-until'
                      label={t('field.retainUntil')}
                      value={retainUntil}
                      onChange={setRetainUntil}
                    />
                  </div>
                  <div className='flex flex-wrap gap-2'>
                    <Button
                      disabled={pending}
                      onClick={() =>
                        safeMemoryRun(
                          () => ({
                            kind: 'set_memory_retention',
                            entryId: entry.id,
                            expectedRevision: positiveRevision(
                              expectedRevision,
                              'expected_revision'
                            ),
                            policy: retentionPolicy,
                            retainUntil: retentionTimestamp(
                              retentionPolicy,
                              retainUntil
                            ),
                            idempotencyKey: commandKey('set-memory-retention')
                          }),
                          'set-memory-retention'
                        )
                      }
                    >
                      {t('action.updateMemoryRetention')}
                    </Button>
                    <Button
                      variant='secondary'
                      disabled={pending || Boolean(entry.tombstonedAt)}
                      onClick={() =>
                        safeMemoryRun(
                          () => ({
                            kind: 'tombstone_memory_entry',
                            entryId: entry.id,
                            expectedRevision: positiveRevision(
                              expectedRevision,
                              'expected_revision'
                            ),
                            idempotencyKey: commandKey('tombstone-memory-entry')
                          }),
                          'tombstone-memory-entry'
                        )
                      }
                    >
                      {t('action.tombstoneMemory')}
                    </Button>
                    {entry.tombstonedAt && (
                      <Button
                        variant='destructive'
                        disabled={pending}
                        onClick={() =>
                          safeMemoryRun(
                            () => ({
                              kind: 'purge_memory_entry',
                              entryId: entry.id,
                              expectedRevision: positiveRevision(
                                expectedRevision,
                                'expected_revision'
                              ),
                              idempotencyKey: commandKey('purge-memory-entry')
                            }),
                            'purge-memory-entry'
                          )
                        }
                      >
                        {t('action.purgeMemory')}
                      </Button>
                    )}
                    <Button
                      variant='outline'
                      onClick={() => onSelectEntry(null)}
                    >
                      {t('action.clearSelection')}
                    </Button>
                  </div>
                </CardContent>
              </Card>
            </div>
          ) : (
            <Card>
              <CardContent className='p-8'>
                <p className='text-muted-foreground text-sm'>
                  {t('memory.select')}
                </p>
              </CardContent>
            </Card>
          )}
        </div>
      )}

      {pending && <LoadingState label={t('loading')} />}
      {error && <ErrorState title={t('memory.error')} message={error} />}
      {outcome && <OperationReceipt response={outcome} />}
    </div>
  );
}

function MemoryText({
  label,
  value
}: {
  label: string;
  value: string;
}): React.JSX.Element {
  return (
    <div>
      <p className='text-muted-foreground text-xs font-medium tracking-wide uppercase'>
        {label}
      </p>
      <p className='mt-1 text-sm whitespace-pre-wrap'>{value}</p>
    </div>
  );
}

function MemoryFact({
  label,
  value
}: {
  label: string;
  value: string;
}): React.JSX.Element {
  return (
    <div>
      <dt className='text-muted-foreground'>{label}</dt>
      <dd className='mt-1 font-mono break-all'>{value}</dd>
    </div>
  );
}

function OperationReceipt({
  response
}: {
  response: TranslationResponse;
}): React.JSX.Element {
  const t = useTranslations('translation');
  const facts = responseFacts(response);
  return (
    <Alert className='border-emerald-200 bg-emerald-50 text-emerald-900'>
      <AlertTitle>{t(`receipt.${receiptKey(response.kind)}`)}</AlertTitle>
      <AlertDescription>
        <dl className='grid gap-3 sm:grid-cols-2 xl:grid-cols-3'>
          {facts.map(([label, value]) => (
            <div key={label}>
              <dt className='text-xs font-medium tracking-wide uppercase opacity-70'>
                {label}
              </dt>
              <dd className='mt-1 font-mono text-xs break-all'>{value}</dd>
            </div>
          ))}
        </dl>
      </AlertDescription>
    </Alert>
  );
}

function TextInput({
  id,
  label,
  value,
  onChange
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
}): React.JSX.Element {
  return (
    <div className='space-y-2'>
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function LoadingState({ label }: { label: string }): React.JSX.Element {
  return (
    <div className='bg-muted/30 text-muted-foreground rounded-xl border border-dashed p-8 text-center text-sm'>
      {label}
    </div>
  );
}

function ErrorState({
  title,
  message
}: {
  title: string;
  message: string;
}): React.JSX.Element {
  return (
    <Alert variant='destructive'>
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription>{message}</AlertDescription>
    </Alert>
  );
}

function EmptyState({ message }: { message: string }): React.JSX.Element {
  return (
    <div className='text-muted-foreground rounded-xl border border-dashed p-8 text-center text-sm'>
      {message}
    </div>
  );
}

function parseTab(value: string | null): TranslationTab {
  return value === 'jobs' ||
    value === 'glossaries' ||
    value === 'memory' ||
    value === 'inventory' ||
    value === 'workflow'
    ? value
    : 'overview';
}

function optionalGlossaryBinding(
  glossaryId: string,
  revision: string
): { glossaryId: string; revision: number } | undefined {
  if (!glossaryId.trim() && !revision.trim()) return undefined;
  const parsedRevision = positiveRevision(revision, 'glossary_revision');
  return {
    glossaryId: required(glossaryId, 'glossary_id'),
    revision: parsedRevision
  };
}

function glossaryScope(
  ownerSlug: string,
  resourceKind: string,
  fieldKey: string
): { ownerSlug?: string; resourceKind?: string; fieldKey?: string } {
  const owner = ownerSlug.trim();
  const resource = resourceKind.trim();
  const field = fieldKey.trim();
  if (resource && !owner) {
    throw new Error('owner_slug: required when resource_kind is set');
  }
  if (field && !resource) {
    throw new Error('resource_kind: required when field_key is set');
  }
  return {
    ...(owner ? { ownerSlug: owner } : {}),
    ...(resource ? { resourceKind: resource } : {}),
    ...(field ? { fieldKey: field } : {})
  };
}

function parseGlossaryConcepts(value: string): GlossaryConcept[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(required(value, 'concepts_json', false));
  } catch {
    throw new Error('concepts_json: must be valid JSON');
  }
  if (!Array.isArray(parsed)) {
    throw new Error('concepts_json: root value must be an array');
  }
  return parsed.map((entry, conceptIndex) => {
    const concept = objectValue(entry, `concepts[${conceptIndex}]`);
    const variants = concept.variants;
    if (!Array.isArray(variants) || variants.length === 0) {
      throw new Error(
        `concepts[${conceptIndex}].variants: must be a non-empty array`
      );
    }
    const matchKind = stringValue(
      concept.matchKind,
      `concepts[${conceptIndex}].matchKind`
    );
    if (!['EXACT', 'WHOLE_WORD', 'SUBSTRING'].includes(matchKind)) {
      throw new Error(`concepts[${conceptIndex}].matchKind: unsupported value`);
    }
    if (typeof concept.caseSensitive !== 'boolean') {
      throw new Error(
        `concepts[${conceptIndex}].caseSensitive: must be a boolean`
      );
    }
    return {
      conceptKey: stringValue(
        concept.conceptKey,
        `concepts[${conceptIndex}].conceptKey`
      ),
      sourceTerm: stringValue(
        concept.sourceTerm,
        `concepts[${conceptIndex}].sourceTerm`
      ),
      variants: variants.map((entry, variantIndex) => {
        const variant = objectValue(
          entry,
          `concepts[${conceptIndex}].variants[${variantIndex}]`
        );
        const policy = stringValue(
          variant.policy,
          `concepts[${conceptIndex}].variants[${variantIndex}].policy`
        );
        if (
          !['PREFERRED', 'ALLOWED', 'FORBIDDEN', 'DO_NOT_TRANSLATE'].includes(
            policy
          )
        ) {
          throw new Error(
            `concepts[${conceptIndex}].variants[${variantIndex}].policy: unsupported value`
          );
        }
        return {
          value: stringValue(
            variant.value,
            `concepts[${conceptIndex}].variants[${variantIndex}].value`
          ),
          policy: policy as GlossaryConcept['variants'][number]['policy']
        };
      }),
      matchKind: matchKind as GlossaryConcept['matchKind'],
      caseSensitive: concept.caseSensitive,
      notes:
        concept.notes === undefined
          ? ''
          : stringValue(concept.notes, `concepts[${conceptIndex}].notes`)
    };
  });
}

function objectValue(value: unknown, field: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${field}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function stringValue(value: unknown, field: string): string {
  if (typeof value !== 'string' || !value.trim()) {
    throw new Error(`${field}: must be a non-empty string`);
  }
  return value;
}

function upsertGlossarySummary(
  current: GlossarySummary[],
  value: Glossary
): GlossarySummary[] {
  const summary: GlossarySummary = {
    id: value.id,
    name: value.name,
    description: value.description,
    sourceLocale: value.sourceLocale,
    targetLocale: value.targetLocale,
    scope: value.scope,
    isActive: value.isActive,
    revision: value.revision
  };
  const next = current.filter((item) => item.id !== value.id);
  next.push(summary);
  return next.sort((left, right) => left.name.localeCompare(right.name));
}

function required(value: string, field: string, trim = true): string {
  if (!value.trim()) throw new Error(`${field}: value is required`);
  return trim ? value.trim() : value;
}

function integer(value: string, field: string): number {
  const parsed = Number(required(value, field));
  if (!Number.isInteger(parsed))
    throw new Error(`${field}: must be an integer`);
  return parsed;
}

function positiveInteger(value: string, field: string): number {
  const parsed = integer(value, field);
  if (parsed < 1 || parsed > 65535) {
    throw new Error(`${field}: must be between 1 and 65535`);
  }
  return parsed;
}

function positiveRevision(value: string, field: string): number {
  const parsed = integer(value, field);
  if (parsed < 1 || !Number.isSafeInteger(parsed)) {
    throw new Error(`${field}: must be a positive safe integer`);
  }
  return parsed;
}

function basisPoints(value: string, field: string): number {
  const parsed = integer(value, field);
  if (parsed < 0 || parsed > 10_000) {
    throw new Error(`${field}: must be between 0 and 10000`);
  }
  return parsed;
}

function retentionTimestamp(
  policy: MemoryRetentionPolicy,
  value: string
): string | null {
  const normalized = value.trim();
  if (policy === 'RETAIN_UNTIL') {
    return required(normalized, 'retain_until');
  }
  if (normalized) {
    throw new Error(
      'retain_until: value is valid only for RETAIN_UNTIL policy'
    );
  }
  return null;
}

function csv(value: string, field: string): string[] {
  const values = value
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
  if (!values.length)
    throw new Error(`${field}: at least one locale is required`);
  return values;
}

function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : 'Translation operation failed';
}

function receiptKey(kind: TranslationResponse['kind']): string {
  return kind === 'job_progress'
    ? 'jobProgress'
    : kind === 'provider_progress'
      ? 'providerProgress'
      : kind === 'required_progress'
        ? 'requiredProgress'
        : kind === 'memory_entries'
          ? 'memoryEntries'
          : kind === 'memory_entry'
            ? 'memoryEntry'
            : kind === 'memory_suggestions'
              ? 'memorySuggestions'
              : kind === 'memory_mutation'
                ? 'memoryMutation'
                : kind === 'machine_proposal'
                  ? 'machineProposal'
                  : kind === 'machine_cancellation'
                    ? 'machineCancellation'
                    : kind;
}

function responseFacts(response: TranslationResponse): Array<[string, string]> {
  switch (response.kind) {
    case 'policy':
      return [
        ['Revision', String(response.value.revision)],
        ['Required locales', response.value.requiredTargetLocales.join(', ')],
        ['Freshness', response.value.freshness]
      ];
    case 'targets':
      return [['Targets', String(response.value.length)]];
    case 'glossaries':
      return [['Glossaries', String(response.value.length)]];
    case 'glossary':
      return [
        ['Glossary', response.value.name],
        ['Revision', String(response.value.revision)],
        ['Concepts', String(response.value.concepts.length)],
        ['Status', response.value.isActive ? 'active' : 'inactive']
      ];
    case 'memory_entries':
      return [['Memory entries', String(response.value.length)]];
    case 'memory_entry':
      return [
        ['Memory entry ID', response.value.id],
        [
          'Locale pair',
          `${response.value.sourceLocale} -> ${response.value.targetLocale}`
        ],
        ['Revision', String(response.value.revision)]
      ];
    case 'memory_suggestions':
      return [['Suggestions', String(response.value.length)]];
    case 'memory_mutation':
      return [
        ['Memory entry ID', response.value.entryId],
        ['Status', response.value.state],
        ['Revision', String(response.value.revision)]
      ];
    case 'job':
      return [
        ['Job ID', response.value.id],
        [
          'Locale pair',
          `${response.value.sourceLocale} → ${response.value.targetLocale}`
        ],
        [
          'Glossary revision',
          response.value.glossary
            ? `${response.value.glossary.glossaryId}@${response.value.glossary.revision}`
            : 'none'
        ],
        ['Status', response.value.status],
        ['Revision', String(response.value.revision)]
      ];
    case 'job_progress':
      return [
        ['Job ID', response.value.jobId],
        ['Total items', String(response.value.totalItems)],
        ['Applied items', String(response.value.appliedItems)],
        ['Blocked items', String(response.value.blockedItems)]
      ];
    case 'provider_progress':
      return [
        [
          'Provider',
          `${response.value.ownerSlug}/${response.value.resourceKind}`
        ],
        [
          'Complete resources',
          `${response.value.completeResources}/${response.value.resources}`
        ],
        ['Freshness', response.value.freshness]
      ];
    case 'required_progress':
      return [
        [
          'Provider',
          `${response.value.ownerSlug}/${response.value.resourceKind}`
        ],
        ['Required locales', response.value.requiredTargetLocales.join(', ')],
        [
          'Complete resource-locale pairs',
          `${response.value.completeResourceLocalePairs}/${response.value.resourceLocalePairs}`
        ],
        ['Freshness', response.value.freshness]
      ];
    case 'inventory':
      return [
        ['Observed resources', String(response.value.observedResources)],
        ['Checkpoint', response.value.checkpoint ?? 'none'],
        ['Revision', String(response.value.checkpointRevision)]
      ];
    case 'item':
      return [
        ['Item ID', response.value.id],
        ['Job ID', response.value.jobId],
        ['Status', response.value.status],
        ['Revision', String(response.value.revision)]
      ];
    case 'proposal':
      return [
        ['Proposal ID', response.value.id],
        ['Item ID', response.value.itemId],
        ['Status', response.value.status],
        ['QA issues', String(response.value.qaIssues.length)]
      ];
    case 'machine_proposal':
      return [
        ['Proposal ID', response.value.proposalId],
        ['Provider', response.value.providerSlug],
        ['Execution ID', response.value.executionId],
        ['Review required', String(response.value.reviewRequired)]
      ];
    case 'machine_cancellation':
      return [
        ['Operation ID', response.value.operationId],
        ['Status', response.value.status],
        ['Provider status', response.value.providerStatus],
        ['Provider execution ID', response.value.providerExecutionId ?? '—']
      ];
    case 'apply':
      return [
        ['Operation ID', response.value.operationId],
        ['Provider receipt', response.value.providerReceiptId],
        ['Target revision', response.value.targetRevision]
      ];
  }
}

export { executeTranslationOperation } from './api';
export { translationNavItems } from './nav';
export type * from './types';
export default TranslationAdminPage;
