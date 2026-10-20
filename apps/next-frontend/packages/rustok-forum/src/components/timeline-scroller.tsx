'use client';

import React from 'react';
import { ChevronUp, ChevronDown } from 'lucide-react';
import { useTranslations } from '@rustok/next-fluent';
import type { ThreadPostTarget } from './thread-navigation';

interface TimelineScrollerProps {
  currentPost: number;
  totalPosts: number;
  onJumpToPost?: (postNumber: ThreadPostTarget) => void;
  isNavigating?: boolean;
}

export function TimelineScroller({
  currentPost,
  totalPosts,
  onJumpToPost,
  isNavigating = false,
}: TimelineScrollerProps) {
  const forum = useTranslations('Forum');
  if (totalPosts <= 1) return null;

  const percentage = Math.min(
    100,
    Math.max(0, Math.round((currentPost / totalPosts) * 100))
  );

  return (
    <aside aria-busy={isNavigating} className="sticky top-20 flex flex-col items-center gap-2 rounded-2xl border border-border bg-card/80 p-2 shadow-sm backdrop-blur">
      <button
        type="button"
        disabled={isNavigating || !onJumpToPost}
        onClick={() => onJumpToPost?.(1)}
        className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:opacity-50"
        title={forum('jumpToBeginning')}
      >
        <ChevronUp className="h-4 w-4" />
      </button>

      <div className="flex flex-col items-center py-1">
        <span className="text-xs font-bold text-foreground">{currentPost}</span>
        <div role="progressbar" aria-label={forum('replies')} aria-valuemin={1} aria-valuemax={totalPosts} aria-valuenow={currentPost} className="my-1.5 h-16 w-1 overflow-hidden rounded-full bg-muted">
          <svg
            aria-hidden="true"
            className="block h-full w-full"
            preserveAspectRatio="none"
            viewBox="0 0 4 100"
          >
            <rect
              className="fill-primary transition-all duration-300"
              x="0"
              y="0"
              width="4"
              height={`${percentage}`}
            />
          </svg>
        </div>
        <span className="text-[10px] text-muted-foreground">{totalPosts}</span>
      </div>

      <button
        type="button"
        disabled={isNavigating || !onJumpToPost}
        onClick={() => onJumpToPost?.('last')}
        className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:opacity-50"
        title={forum('jumpToEnd')}
      >
        <ChevronDown className="h-4 w-4" />
      </button>
    </aside>
  );
}
