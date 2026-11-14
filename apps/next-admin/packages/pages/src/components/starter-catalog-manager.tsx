'use client';

import { useTranslations } from '@rustok/next-fluent/client';
import { useRouter } from 'next/navigation';
import { useRef, useState, useTransition } from 'react';

import { deleteStarter, updateStarterMetadata } from '../api/pages';
import type { GqlOpts, PageStarterSummary } from '../api/pages';

type Selection = {
  key: string;
  revision: string;
  token: string | null | undefined;
  tenantId: string | null | undefined;
  tenantSlug: string | null | undefined;
};

export function StarterCatalogManager({
  starters,
  error: catalogError,
  opts
}: {
  starters: PageStarterSummary[];
  error: string | null;
  opts: GqlOpts;
}) {
  const t = useTranslations('pages');
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const inFlight = useRef(false);
  const latestOpts = useRef(opts);
  latestOpts.current = opts;
  const [selection, setSelection] = useState<Selection | null>(null);
  const [name, setName] = useState('');
  const [category, setCategory] = useState('');
  const [description, setDescription] = useState('');
  const [layout, setLayout] = useState('');
  const [confirming, setConfirming] = useState(false);
  const [feedback, setFeedback] = useState<{ ok: boolean; message: string } | null>(null);

  const matchesContext = (selected: Selection) =>
    selected.token === latestOpts.current.token &&
    selected.tenantId === latestOpts.current.tenantId &&
    selected.tenantSlug === latestOpts.current.tenantSlug;

  function select(item: PageStarterSummary) {
    if (!item.revision || inFlight.current) return;
    setSelection({ key: item.key, revision: item.revision, token: opts.token, tenantId: opts.tenantId, tenantSlug: opts.tenantSlug });
    setName(item.name);
    setCategory(item.category);
    setDescription(item.description ?? '');
    setLayout(item.defaultTemplateKey);
    setConfirming(false);
    setFeedback(null);
  }

  function run(selected: Selection, action: () => Promise<void>, success: string) {
    if (inFlight.current) return;
    if (!matchesContext(selected)) {
      setFeedback({ ok: false, message: t('starter.contextError') });
      return;
    }
    inFlight.current = true;
    setFeedback(null);
    startTransition(async () => {
      try {
        await action();
        if (!matchesContext(selected)) {
          setFeedback({ ok: false, message: t('starter.contextError') });
          return;
        }
        setSelection(null);
        setConfirming(false);
        setFeedback({ ok: true, message: success });
        router.refresh();
      } catch (cause) {
        setFeedback({ ok: false, message: matchesContext(selected)
          ? `${t('starter.operationError')}: ${cause instanceof Error ? cause.message : t('starter.unavailable')}`
          : t('starter.contextError') });
      } finally {
        inFlight.current = false;
      }
    });
  }

  return (
    <section className='my-4 rounded-lg border p-4'>
      <h2>{t('starter.manageTitle')}</h2>
      <p>{t('starter.manageHint')}</p>
      <button type='button' disabled={pending} onClick={() => {
        setSelection(null);
        setConfirming(false);
        setFeedback(null);
        router.refresh();
      }}>{t('starter.reload')}</button>
      {catalogError ? <p role='alert'>{t('starter.unavailable')}: {catalogError}</p> : (
        <ul>
          {starters.map((item) => (
            <li key={item.key}>
              {item.name} ({item.key}) — {item.revision ? t('starter.tenant') : t('starter.builtin')}{' '}
              {item.revision ? <button type='button' disabled={pending} onClick={() => select(item)}>{t('starter.edit')}</button> : null}
            </li>
          ))}
        </ul>
      )}
      {selection ? (
        <form onSubmit={(event) => {
          event.preventDefault();
          const chosen = selection;
          run(chosen, async () => {
            await updateStarterMetadata({
              key: chosen.key, expectedRevision: chosen.revision, name, category,
              description: description.trim() ? description : null, defaultTemplateKey: layout
            }, { token: chosen.token, tenantId: chosen.tenantId, tenantSlug: chosen.tenantSlug });
          }, t('starter.updated'));
        }}>
          <p>{selection.key}</p>
          <label>{t('starter.name')} <input required disabled={pending} value={name} onChange={(event) => setName(event.target.value)} /></label>{' '}
          <label>{t('starter.category')} <input required disabled={pending} value={category} onChange={(event) => setCategory(event.target.value)} /></label>{' '}
          <label>{t('starter.description')} <textarea disabled={pending} value={description} onChange={(event) => setDescription(event.target.value)} /></label>{' '}
          <label>{t('starter.layoutKey')} <input required disabled={pending} value={layout} onChange={(event) => setLayout(event.target.value)} /></label>{' '}
          <button type='submit' disabled={pending}>{t('starter.saveChanges')}</button>{' '}
          <button type='button' disabled={pending} onClick={() => setConfirming(true)}>{t('starter.delete')}</button>
          {confirming ? (
            <div role='alert'>
              <p>{t('starter.deleteWarning')}</p>
              <button type='button' disabled={pending} onClick={() => {
                const chosen = selection;
                run(chosen, async () => {
                  const deleted = await deleteStarter(chosen.key, chosen.revision, { token: chosen.token, tenantId: chosen.tenantId, tenantSlug: chosen.tenantSlug });
                  if (!deleted) throw new Error(t('starter.notDeleted'));
                }, t('starter.deleted'));
              }}>{t('starter.confirmDelete')}</button>{' '}
              <button type='button' disabled={pending} onClick={() => setConfirming(false)}>{t('starter.cancel')}</button>
            </div>
          ) : null}
        </form>
      ) : null}
      {feedback ? <p role={feedback.ok ? 'status' : 'alert'}>{feedback.message}</p> : null}
    </section>
  );
}
