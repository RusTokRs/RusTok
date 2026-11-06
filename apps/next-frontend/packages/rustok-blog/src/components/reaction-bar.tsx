'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslations } from '@rustok/next-fluent';
import { Button } from '@/shared/ui/base/button';
import { useStorefrontAuth } from '@/shared/lib/auth-session';
import { applyReaction, loadReactionSnapshot } from '../api/reactions';
import { blogReactionSubject, reactionControls, settleReactionMutation, type ReactionSnapshot } from '../utils/reactions';

export function ReactionBar({ postId, version, tenantSlug }: {
  postId: string; version: number; locale?: string; tenantSlug?: string | null;
}) {
  const t = useTranslations('Blog');
  const auth = useStorefrontAuth();
  const subject = useMemo(() => blogReactionSubject(postId, version), [postId, version]);
  const scope = useMemo(() => ({ subject, tenantSlug, auth }), [subject, tenantSlug, auth]);
  const empty = { snapshot: null as ReactionSnapshot | null, failed: false, writeFailed: false, busy: false };
  const [state, setState] = useState({ scope, ...empty });
  // Hide an old principal's state during the render before effects reset it.
  const { snapshot, failed, writeFailed, busy } = state.scope === scope ? state : empty;
  const generation = useRef(0);
  const pending = useRef<number | null>(null);
  useEffect(() => {
    const current = ++generation.current;
    setState({ scope, snapshot: null, failed: auth.refused, writeFailed: false, busy: false });
    pending.current = null;
    if (auth.ready && !auth.refused) {
      void loadReactionSnapshot({ token: auth.token ?? undefined, tenant: tenantSlug ?? undefined }, subject)
        .then(value => {
          if (generation.current === current && auth.isCurrent())
            setState({ scope, snapshot: value, failed: false, writeFailed: false, busy: false });
        })
        .catch(() => {
          if (generation.current === current && auth.isCurrent())
            setState({ scope, snapshot: null, failed: true, writeFailed: false, busy: false });
        });
    }
    return () => { generation.current++; };
  }, [scope, subject, tenantSlug, auth]);

  async function toggle(reaction: string, selected: boolean) {
    if (pending.current === generation.current) return;
    if (!auth.token || !auth.isCurrent() || !snapshot?.actorState) return;
    const current = generation.current;
    const context = { token: auth.token, tenant: tenantSlug ?? undefined };
    const isCurrent = () => generation.current === current && auth.isCurrent();
    pending.current = current;
    setState({ scope, snapshot, failed: false, writeFailed: false, busy: true });
    try {
      const result = await settleReactionMutation(
        () => applyReaction(context, subject, reaction, selected),
        () => loadReactionSnapshot(context, subject),
        isCurrent,
      );
      if (result && isCurrent())
        setState({ scope, snapshot: result.snapshot, writeFailed: result.writeFailed, failed: false, busy: false });
    } catch {
      if (isCurrent()) setState({ scope, snapshot: null, failed: true, writeFailed: false, busy: false });
    }
    finally {
      if (pending.current === current) pending.current = null;
      if (isCurrent()) setState(previous => ({ ...previous, busy: false }));
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
