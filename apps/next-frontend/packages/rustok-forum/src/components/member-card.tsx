'use client';

import React, { createContext, useContext, useMemo } from 'react';
import type { ForumMemberCard } from '../api/forum';

export const MemberCardContext = createContext<Map<string, ForumMemberCard>>(new Map());

export function MemberCardProvider({
  cards,
  children,
}: {
  cards: ForumMemberCard[];
  children: React.ReactNode;
}) {
  const cardMap = useMemo(() => {
    const map = new Map<string, ForumMemberCard>();
    for (const card of cards) {
      map.set(card.userId, card);
    }
    return map;
  }, [cards]);

  return (
    <MemberCardContext.Provider value={cardMap}>
      {children}
    </MemberCardContext.Provider>
  );
}

export function AuthorBadge({ authorId }: { authorId: string | null }) {
  const cardMap = useContext(MemberCardContext);
  if (!authorId) return null;

  const card = cardMap.get(authorId);
  if (!card) return null;

  const displayName = card.profile.displayName || card.profile.handle;
  const initials = displayName
    .split(' ')
    .map((s) => s[0])
    .join('')
    .slice(0, 2)
    .toUpperCase();

  return (
    <div className="forum-member-card inline-flex max-w-full items-center gap-2 rounded-xl border border-border bg-background/70 px-2.5 py-1.5 shadow-xs">
      <div
        role="img"
        aria-label={displayName}
        className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-xs font-bold text-primary"
      >
        <span aria-hidden="true">{initials}</span>
      </div>
      <div className="min-w-0">
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-0.5">
          <span className="truncate text-xs font-semibold text-foreground">
            {displayName}
          </span>
          <span className="truncate text-[11px] text-muted-foreground" dir="ltr">
            @{card.profile.handle}
          </span>
        </div>
        <div className="flex flex-wrap gap-x-2 text-[10px] text-muted-foreground">
          <span>{card.forumStats.topicCount} topics</span>
          <span>•</span>
          <span>{card.forumStats.replyCount} replies</span>
          {card.forumStats.solutionCount > 0 && (
            <>
              <span>•</span>
              <span className="text-emerald-600 dark:text-emerald-400">
                {card.forumStats.solutionCount} solutions
              </span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
