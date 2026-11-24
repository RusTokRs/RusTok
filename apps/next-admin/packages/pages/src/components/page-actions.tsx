'use client';

import { useTranslations } from '@rustok/next-fluent/client';
import { useRouter } from 'next/navigation';
import { useRef, useState, useTransition } from 'react';

import type { GqlOpts, PageStarterSummary } from '../api/pages';
import { createPage, deletePage, instantiatePageStarter, listPageLayoutKeys, saveStarterFromPage, unpublishPage } from '../api/pages';
import { createPageFromStarter } from '../model/create-page';

async function createFingerprint(parts: readonly unknown[]): Promise<string> {
  if (typeof crypto === 'undefined' || !crypto.subtle || typeof TextEncoder === 'undefined') {
    throw new Error('Secure browser hashing is unavailable for this operation');
  }
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify(parts)));
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function createRandomOperationKey(): string {
  if (typeof crypto === 'undefined' || typeof crypto.getRandomValues !== 'function') {
    throw new Error('Secure browser randomness is unavailable for this operation');
  }
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function loadOrCreateOperationKey(fingerprint: string, memory: Map<string, string>): string {
  const storageKey = `rustok:pages:operation:${fingerprint}`;
  let key = memory.get(fingerprint);
  if (!key) {
    try {
      key = sessionStorage.getItem(storageKey) ?? undefined;
    } catch {
      // Storage can be unavailable in restricted browser contexts; the in-memory
      // key still protects retries while this component remains mounted.
    }
  }
  if (!key || !/^[A-Za-z0-9._:-]{1,128}$/.test(key)) {
    key = createRandomOperationKey();
  }
  memory.set(fingerprint, key);
  try {
    sessionStorage.setItem(storageKey, key);
  } catch {
    // Keep the in-memory copy when session storage is unavailable.
  }
  return key;
}

function clearOperationKey(fingerprint: string, memory: Map<string, string>): void {
  memory.delete(fingerprint);
  try {
    sessionStorage.removeItem(`rustok:pages:operation:${fingerprint}`);
  } catch {
    // The successful operation is still acknowledged in memory.
  }
}

interface PageActionsProps {
  opts: GqlOpts;
  pageId?: string;
  version?: number;
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
  version,
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
  const pendingOperationKeys = useRef(new Map<string, string>());
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
        // Reconcile server state after ambiguous responses while retaining the pending key.
        try {
          router.refresh();
        } catch {
          // The operation key remains available for an explicit retry.
        }
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
            const fingerprint = await createFingerprint([
              'create',
              chosenOpts.tenantId ?? null,
              chosenOpts.tenantSlug ?? null,
              chosenLocale,
              value,
              starter.key,
              // Keep pending identity stable across catalog refreshes. The server hashes
              // the instantiated document and rejects changed content under the same key.
              selectedLayout
            ]);
            const idempotencyKey = loadOrCreateOperationKey(fingerprint, pendingOperationKeys.current);
            await createPageFromStarter(
                {
                  starter,
                  locale: chosenLocale,
                  title: value,
                  layoutKey: selectedLayout,
                  visibleLayoutKeys: layoutKeys,
                  idempotencyKey
                },
                {
                  instantiate: (item, pageLocale) => instantiatePageStarter(item, pageLocale, chosenOpts),
                  listLayouts: (pageLocale) => listPageLayoutKeys(pageLocale, chosenOpts),
                  create: (input) => createPage(input, chosenOpts),
                  contextMatches
                },
                { layout: t('layout.selectionError'), context: t('starter.contextError'), contextAfterCreate: t('layout.contextAfterCreate') }
            );
            clearOperationKey(fingerprint, pendingOperationKeys.current);
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
          onClick={() => {
            const operationOpts = { ...opts };
            run(async () => {
              const fingerprint = await createFingerprint([
                operationOpts.tenantId ?? null,
                operationOpts.tenantSlug ?? null,
                'unpublish',
                pageId,
                version ?? null
              ]);
              const idempotencyKey = loadOrCreateOperationKey(fingerprint, pendingOperationKeys.current);
              await unpublishPage(pageId, version, operationOpts, idempotencyKey);
              clearOperationKey(fingerprint, pendingOperationKeys.current);
            });
          }}
        >
          Unpublish
        </button>
      ) : null}{' '}
      <button
        type='button'
        disabled={pending}
        onClick={() => {
          if (window.confirm('Delete this page permanently?')) {
            const operationOpts = { ...opts };
            run(async () => {
              const fingerprint = await createFingerprint([
                operationOpts.tenantId ?? null,
                operationOpts.tenantSlug ?? null,
                'delete',
                pageId,
                version ?? null
              ]);
              const idempotencyKey = loadOrCreateOperationKey(fingerprint, pendingOperationKeys.current);
              await deletePage(pageId, operationOpts, idempotencyKey, version);
              clearOperationKey(fingerprint, pendingOperationKeys.current);
            });
          }
        }}
      >
        Delete
      </button>
      {error ? <span role='alert'> {error}</span> : null}
    </div>
  );
}
