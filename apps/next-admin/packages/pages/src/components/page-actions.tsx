'use client';

import { useTranslations } from '@rustok/next-fluent/client';
import { useRouter } from 'next/navigation';
import { useRef, useState, useTransition } from 'react';

import type { GqlOpts, PageStarterSummary } from '../api/pages';
import { createPage, deletePage, instantiatePageStarter, listPageLayoutKeys, saveStarterFromPage, unpublishPage } from '../api/pages';
import { createPageFromStarter, LayoutSelectionError } from '../model/create-page';

interface PageActionsProps {
  opts: GqlOpts;
  pageId?: string;
  status?: string;
  /** Locale used for newly created pages. */
  locale?: string;
  starters?: PageStarterSummary[];
  starterError?: string | null;
  layoutKeys?: string[];
  layoutError?: string | null;
  canManageStarters?: boolean;
}

/**
 * Row/create actions for the Pages surface. Visual editing and reviewed publish
 * intentionally live in the Rust FFA builder; here we only create drafts and
 * reverse publication through the same GraphQL both admins consume.
 */
export function PageActions({
  opts,
  pageId,
  status,
  locale = 'en',
  starters = [],
  starterError = null,
  layoutKeys = [],
  layoutError = null,
  canManageStarters = false
}: PageActionsProps) {
  const t = useTranslations('pages');
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const inFlight = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [title, setTitle] = useState('');
  const [starterKey, setStarterKey] = useState('blank');
  const [layoutKey, setLayoutKey] = useState('default');
  const latestContext = useRef({ opts, locale });
  latestContext.current = { opts, locale };
  const [sourceLocale, setSourceLocale] = useState(locale);
  const [saveKey, setSaveKey] = useState('');
  const [saveName, setSaveName] = useState('');
  const [saveCategory, setSaveCategory] = useState('custom');
  const [savedSource, setSavedSource] = useState<string | null>(null);
  const selectedStarter = starters.find((item) => item.key === starterKey);

  function run(action: () => Promise<unknown>) {
    if (inFlight.current) return;
    inFlight.current = true;
    setError(null);
    startTransition(async () => {
      try {
        await action();
        router.refresh();
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : 'Request failed');
      } finally {
        inFlight.current = false;
      }
    });
  }

  if (!pageId) {
    return (
      <form
        onSubmit={(event) => {
          event.preventDefault();
          const value = title.trim();
          if (!value) return;
          const starter = selectedStarter;
          if (!starter || starterError) {
            setError(t('starter.selectionError'));
            return;
          }
          if (layoutError || !layoutKeys.includes(layoutKey)) {
            setError(t('layout.selectionError'));
            return;
          }
          const selectedLayout = layoutKey;
          const chosenLocale = locale;
          const chosenOpts = { ...opts };
          const contextMatches = () => {
            const current = latestContext.current;
            return current.locale === chosenLocale && current.opts.token === chosenOpts.token &&
              current.opts.tenantSlug === chosenOpts.tenantSlug && current.opts.tenantId === chosenOpts.tenantId;
          };
          run(async () => {
            try {
              await createPageFromStarter(
                { starter, locale: chosenLocale, title: value, layoutKey: selectedLayout, visibleLayoutKeys: layoutKeys },
                {
                  instantiate: (item, pageLocale) => instantiatePageStarter(item, pageLocale, chosenOpts),
                  listLayouts: (pageLocale) => listPageLayoutKeys(pageLocale, chosenOpts),
                  create: (input) => createPage(input, chosenOpts),
                  contextMatches
                },
                { layout: t('layout.selectionError'), context: t('starter.contextError'), contextAfterCreate: t('layout.contextAfterCreate') }
              );
            } catch (cause) {
              if (cause instanceof LayoutSelectionError) router.refresh();
              throw cause;
            }
            setTitle('');
            setStarterKey('blank');
            setLayoutKey('default');
          });
        }}
      >
        <fieldset disabled={pending}>
        <label>
          New page title{' '}
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder='Landing page'
          />
        </label>{' '}
        <label>
          {t('starter.label')}{' '}
          <select
            value={starterKey}
            onChange={(event) => setStarterKey(event.target.value)}
            disabled={pending || !!starterError || starters.length === 0}
          >
            {starters.map((starter) => (
              <option key={starter.key} value={starter.key}>
                {starter.name} ({starter.category})
              </option>
            ))}
          </select>
        </label>{' '}
        <label>
          {t('layout.label')}{' '}
          <select value={layoutKey} onChange={(event) => setLayoutKey(event.target.value)} disabled={!!layoutError || layoutKeys.length === 0}>
            {!layoutKeys.includes(layoutKey) ? <option value={layoutKey} disabled>{t('layout.selectionError')}</option> : null}
            {layoutKeys.map((key) => <option key={key} value={key}>{key}</option>)}
          </select>
        </label>{' '}
        <button type='button' onClick={() => router.refresh()}>{t('layout.reload')}</button>
        <p>{t('layout.permissionsHint')}</p>
        {layoutError ? <p role='alert'>{t('layout.unavailable')}: {layoutError}</p> : null}
        {!layoutError && !layoutKeys.includes(layoutKey) ? <p role='alert'>{t('layout.selectionError')}</p> : null}
        {selectedStarter?.description ? <p>{selectedStarter.description}</p> : null}
        {selectedStarter ? <p>{t('starter.layoutHint')}: {selectedStarter.defaultTemplateKey}{!layoutError ? ` — ${layoutKeys.includes(selectedStarter.defaultTemplateKey) ? t('layout.suggestionAvailable') : t('layout.suggestionMissing')}` : ''}</p> : null}
        {starterError ? <p role='alert'>{t('starter.unavailable')}: {starterError}</p> : null}
        {!starterError && starters.length === 0 ? <p role='alert'>{t('starter.unavailable')}</p> : null}
        <button type='submit' disabled={pending || !title.trim() || !!starterError || !!layoutError || !layoutKeys.includes(layoutKey) || !starters.some((item) => item.key === starterKey)}>
          Create draft
        </button>
        {error ? <p role='alert'>{error}</p> : null}
        </fieldset>
      </form>
    );
  }

  return (
    <div>
      {canManageStarters ? <details>
        <summary>{t('starter.saveTitle')}</summary>
        <p>{t('starter.savedWarning')}</p>
        <form onSubmit={(event) => {
          event.preventDefault();
          setSavedSource(null);
          run(async () => {
            const snapshot = await saveStarterFromPage({ pageId, locale: sourceLocale, key: saveKey, name: saveName, category: saveCategory }, opts);
            setSavedSource(`${t('starter.saved')}: ${snapshot.sourceLocale} / ${snapshot.sourceState} / ${snapshot.sourceRevision}`);
          });
        }}>
          <label>{t('starter.sourceLocale')} <input required value={sourceLocale} onChange={(event) => setSourceLocale(event.target.value)} /></label>{' '}
          <label>{t('starter.key')} <input required value={saveKey} onChange={(event) => setSaveKey(event.target.value)} /></label>{' '}
          <label>{t('starter.name')} <input required value={saveName} onChange={(event) => setSaveName(event.target.value)} /></label>{' '}
          <label>{t('starter.category')} <input required value={saveCategory} onChange={(event) => setSaveCategory(event.target.value)} /></label>{' '}
          <button disabled={pending} type='submit'>{t('starter.saveAction')}</button>
        </form>
        {savedSource ? <p role='status'>{savedSource}</p> : null}
      </details> : null}
      {status === 'published' ? (
        <button
          type='button'
          disabled={pending}
          onClick={() => run(() => unpublishPage(pageId, undefined, opts))}
        >
          Unpublish
        </button>
      ) : null}{' '}
      <button
        type='button'
        disabled={pending}
        onClick={() => {
          if (window.confirm('Delete this page permanently?')) {
            run(() => deletePage(pageId, opts));
          }
        }}
      >
        Delete
      </button>
      {error ? <span role='alert'> {error}</span> : null}
    </div>
  );
}
