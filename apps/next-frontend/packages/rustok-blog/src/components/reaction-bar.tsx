'use client';

import { useEffect, useState, useTransition } from 'react';
import { Heart } from 'lucide-react';

import { getClientAuth, type AuthSession } from '@/shared/lib/auth';
import { storefrontGraphql } from '@/shared/lib/graphql';

interface ReactionBarProps {
  postId: string;
  version?: number;
  locale?: string;
  tenantSlug?: string | null;
}

const REACTION_SNAPSHOT_QUERY = `
  query ReactionSnapshot($subject: ReactionSubjectInput!) {
    reactionSnapshot(subject: $subject) {
      actorState {
        selected
      }
      aggregates {
        reaction
        count
      }
    }
  }
`;

const APPLY_REACTION_MUTATION = `
  mutation ApplyReaction($input: ApplyReactionInput!) {
    applyReaction(input: $input) {
      commandId
      changed
    }
  }
`;

export function ReactionBar({
  postId,
  version = 1,
  locale = 'ru',
  tenantSlug
}: ReactionBarProps) {
  const [auth, setAuth] = useState<AuthSession | null>(null);
  const [isLiked, setIsLiked] = useState(false);
  const [likeCount, setLikeCount] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  const isRu = locale === 'ru';

  const subject = {
    source: 'blog',
    kind: 'post',
    subjectId: postId,
    subjectRevision: String(version)
  };

  useEffect(() => {
    setAuth(getClientAuth());
  }, []);

  useEffect(() => {
    let active = true;

    async function loadSnapshot() {
      try {
        const res = await storefrontGraphql<{
          reactionSnapshot?: {
            actorState?: { selected: string[] } | null;
            aggregates?: Array<{ reaction: string; count: number }>;
          };
        }, { subject: typeof subject }>({
          query: REACTION_SNAPSHOT_QUERY,
          variables: { subject },
          token: auth?.token ?? undefined,
          tenant: tenantSlug ?? undefined
        });

        if (!active || !res.data?.reactionSnapshot) return;

        const aggregates = res.data.reactionSnapshot.aggregates ?? [];
        const likeAggregate = aggregates.find((a) => a.reaction === 'like');
        setLikeCount(likeAggregate ? Number(likeAggregate.count) : 0);

        const selected = res.data.reactionSnapshot.actorState?.selected ?? [];
        setIsLiked(selected.includes('like'));
      } catch {
        // Degraded mode / reactions unavailable
      }
    }

    loadSnapshot();

    return () => {
      active = false;
    };
  }, [postId, version, auth?.token, tenantSlug]);

  const handleToggleLike = () => {
    if (!auth?.token) {
      setNotice(isRu ? 'Войдите, чтобы поставить лайк' : 'Sign in to react');
      setTimeout(() => setNotice(null), 3000);
      return;
    }

    const nextLiked = !isLiked;
    setIsLiked(nextLiked);
    setLikeCount((prev) => (nextLiked ? prev + 1 : Math.max(0, prev - 1)));

    startTransition(async () => {
      try {
        const commandId = typeof crypto !== 'undefined' && crypto.randomUUID
          ? crypto.randomUUID()
          : `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;

        await storefrontGraphql<
          { applyReaction: { commandId: string; changed: boolean } },
          {
            input: {
              commandId: string;
              subject: typeof subject;
              reaction: string;
              action: 'ADD' | 'REMOVE';
            };
          }
        >({
          query: APPLY_REACTION_MUTATION,
          variables: {
            input: {
              commandId,
              subject,
              reaction: 'like',
              action: nextLiked ? 'ADD' : 'REMOVE'
            }
          },
          token: auth.token,
          tenant: tenantSlug ?? undefined
        });
      } catch {
        // Rollback optimistic state
        setIsLiked(!nextLiked);
        setLikeCount((prev) => (nextLiked ? Math.max(0, prev - 1) : prev + 1));
        setNotice(isRu ? 'Не удалось сохранить реакцию' : 'Could not save reaction');
        setTimeout(() => setNotice(null), 3000);
      }
    });
  };

  return (
    <div className='flex items-center gap-3'>
      <button
        type='button'
        onClick={handleToggleLike}
        disabled={isPending}
        title={
          isLiked
            ? isRu
              ? 'Убрать отметку «Нравится»'
              : 'Unlike'
            : isRu
              ? 'Нравится'
              : 'Like'
        }
        className={`group inline-flex items-center gap-2 rounded-full border px-4 py-2 text-sm font-medium transition-all shadow-sm ${
          isLiked
            ? 'border-red-200 bg-red-50 text-red-600 dark:border-red-900/60 dark:bg-red-950/40 dark:text-red-400'
            : 'border-border bg-card text-muted-foreground hover:border-red-300 hover:text-red-600 dark:hover:border-red-900 dark:hover:text-red-400'
        }`}
      >
        <Heart
          className={`h-4 w-4 transition-transform group-hover:scale-110 ${
            isLiked ? 'fill-red-500 text-red-500' : 'text-current'
          }`}
        />
        <span>{likeCount > 0 ? likeCount : isRu ? 'Нравится' : 'Like'}</span>
      </button>
      {notice && (
        <span className='text-xs text-muted-foreground transition-opacity'>
          {notice}
        </span>
      )}
    </div>
  );
}
