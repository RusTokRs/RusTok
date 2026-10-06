'use client';

import React from 'react';
import { ChevronUp, ChevronDown } from 'lucide-react';

interface TimelineScrollerProps {
  currentPost: number;
  totalPosts: number;
  onJumpToPost?: (postNumber: number) => void;
}

export function TimelineScroller({
  currentPost,
  totalPosts,
  onJumpToPost,
}: TimelineScrollerProps) {
  if (totalPosts <= 1) return null;

  const percentage = Math.min(
    100,
    Math.max(0, Math.round((currentPost / totalPosts) * 100))
  );

  return (
    <aside className="sticky top-20 flex flex-col items-center gap-2 rounded-2xl border border-border bg-card/80 p-2 shadow-sm backdrop-blur">
      <button
        type="button"
        onClick={() => onJumpToPost?.(1)}
        className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
        title="Jump to beginning"
      >
        <ChevronUp className="h-4 w-4" />
      </button>

      <div className="flex flex-col items-center py-1">
        <span className="text-xs font-bold text-foreground">{currentPost}</span>
        <div className="my-1.5 h-16 w-1 rounded-full bg-muted">
          <div
            className="w-full rounded-full bg-primary transition-all duration-300"
            style={{ height: `${percentage}%` }}
          />
        </div>
        <span className="text-[10px] text-muted-foreground">{totalPosts}</span>
      </div>

      <button
        type="button"
        onClick={() => onJumpToPost?.(totalPosts)}
        className="rounded-xl p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
        title="Jump to end"
      >
        <ChevronDown className="h-4 w-4" />
      </button>
    </aside>
  );
}
