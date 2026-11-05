'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslations } from '@rustok/next-fluent';
import { Button } from '@/shared/ui/base/button';
import { getClientAuth } from '@/shared/lib/auth';
import { applyReaction, loadReactionSnapshot } from '../api/reactions';
import { blogReactionSubject, reactionControls, settleReactionMutation, type ReactionSnapshot } from '../utils/reactions';

export function ReactionBar({ postId, version, tenantSlug }: {
  postId: string; version: number; locale?: string; tenantSlug?: string | null;
}) {
  const t = useTranslations('Blog');
  const subject = useMemo(() => blogReactionSubject(postId, version), [postId, version]);
  const [snapshot, setSnapshot] = useState<ReactionSnapshot | null>(null);
  const [failed, setFailed] = useState(false);
  const [writeFailed, setWriteFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const generation = useRef(0);
  const pending = useRef<number | null>(null);
  useEffect(() => {
    const current = ++generation.current;
    setSnapshot(null); setFailed(false); setWriteFailed(false); setBusy(false); pending.current = null;
    const auth = getClientAuth();
    void loadReactionSnapshot({ token: auth?.token ?? undefined, tenant: tenantSlug ?? undefined }, subject)
      .then(value => { if (generation.current === current) setSnapshot(value); })
      .catch(() => { if (generation.current === current) setFailed(true); });
    return () => { generation.current++; };
  }, [subject, tenantSlug]);

  async function toggle(reaction: string, selected: boolean) {
    if (pending.current === generation.current) return;
    const auth = getClientAuth();
    if (!auth?.token || !snapshot?.actorState) return;
    const current = generation.current;
    const context = { token: auth.token, tenant: tenantSlug ?? undefined };
    pending.current = current; setBusy(true); setWriteFailed(false);
    try {
      const result = await settleReactionMutation(
        () => applyReaction(context, subject, reaction, selected),
        () => loadReactionSnapshot(context, subject),
      );
      if (generation.current === current) { setSnapshot(result.snapshot); setWriteFailed(result.writeFailed); setFailed(false); }
    } catch { if (generation.current === current) { setSnapshot(null); setFailed(true); } }
    finally {
      if (pending.current === current) pending.current = null;
      if (generation.current === current) setBusy(false);
    }
  }
  return <section aria-label={t('reactions.title')} className='space-y-2'>
    {failed ? <p role='status'>{t('reactions.unavailable')}</p> : snapshot ? <>
      <div className='flex flex-wrap gap-2'>{reactionControls(snapshot).map(control =>
        <Button key={control.reaction} type='button' variant={control.selected ? 'default' : 'outline'}
          disabled={busy || !control.canMutate} aria-pressed={control.selected} aria-busy={busy}
          onClick={() => void toggle(control.reaction, control.selected)}>
          {control.reaction === 'like' ? t('reactions.like') : control.reaction} <span>{control.count}</span>
        </Button>)}</div>
      {!snapshot.actorState && <p>{t('reactions.signIn')}</p>}
    </> : <p role='status'>{t('reactions.loading')}</p>}
    {writeFailed && <p role='status' aria-live='polite'>{t('reactions.updateError')}</p>}
  </section>;
}
