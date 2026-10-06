'use client';

import React, { useMemo, useState } from 'react';
import {
  Pin,
  Lock,
  Plus,
  Search,
  X,
  CheckCircle2,
  Flame,
  Sparkles,
  MessageSquare,
} from 'lucide-react';
import type { ForumTopicListItem } from '../api/forum';
import { AuthorBadge } from './member-card';
import { useComposer } from '../context/composer-context';

export type TopicFilterTab = 'latest' | 'top' | 'unread' | 'solved';

interface TopicFeedProps {
  topics: ForumTopicListItem[];
  total: number;
  selectedTopicId: string | null;
  onSelectTopic?: (topicId: string) => void;
  selectedCategoryId: string | null;
}

export function TopicFeed({
  topics,
  total,
  selectedTopicId,
  onSelectTopic,
  selectedCategoryId,
}: TopicFeedProps) {
  const { openNewTopic } = useComposer();
  const [activeTab, setActiveTab] = useState<TopicFilterTab>('latest');
  const [searchQuery, setSearchQuery] = useState('');

  // Filter and sort topics based on active tab and search query
  const filteredTopics = useMemo(() => {
    let result = [...topics];

    // 1. Text search filter
    const query = searchQuery.trim().toLowerCase();
    if (query) {
      result = result.filter(
        (t) =>
          t.title.toLowerCase().includes(query) ||
          t.slug.toLowerCase().includes(query)
      );
    }

    // 2. Tab filter & sorting
    switch (activeTab) {
      case 'top':
        result.sort((a, b) => (b.replyCount ?? 0) - (a.replyCount ?? 0));
        break;
      case 'unread':
        result = result.filter(
          (t) => Boolean(t.isUnread) || (t.unreadCount !== undefined && t.unreadCount > 0)
        );
        break;
      case 'solved':
        result = result.filter((t) => Boolean(t.solutionReplyId));
        break;
      case 'latest':
      default:
        // Default sort keeps pinned topics on top, then preserves arrival/created date order
        result.sort((a, b) => {
          if (a.isPinned && !b.isPinned) return -1;
          if (!a.isPinned && b.isPinned) return 1;
          return 0;
        });
        break;
    }

    return result;
  }, [topics, searchQuery, activeTab]);

  const unreadCountTotal = useMemo(() => {
    return topics.filter(
      (t) => Boolean(t.isUnread) || (t.unreadCount !== undefined && t.unreadCount > 0)
    ).length;
  }, [topics]);

  const solvedCountTotal = useMemo(() => {
    return topics.filter((t) => Boolean(t.solutionReplyId)).length;
  }, [topics]);

  const handleResetFilters = () => {
    setActiveTab('latest');
    setSearchQuery('');
  };

  return (
    <section className="space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
            Topic feed
          </p>
          <h3 className="mt-2 text-2xl font-semibold text-card-foreground">
            Latest discussions
          </h3>
        </div>

        <div className="flex items-center gap-2">
          <span className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
            {total} threads
          </span>
          <button
            type="button"
            onClick={() => openNewTopic({ categoryId: selectedCategoryId })}
            className="inline-flex items-center gap-1.5 rounded-full bg-primary px-3.5 py-1 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
          >
            <Plus className="h-3.5 w-3.5" />
            New Topic
          </button>
        </div>
      </div>

      {/* Filter Tabs and Search Bar */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        {/* Discourse/NodeBB Filter Tabs */}
        <div className="flex flex-wrap items-center gap-1 rounded-2xl border border-border bg-muted/30 p-1">
          <button
            type="button"
            onClick={() => setActiveTab('latest')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'latest'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Sparkles className="h-3.5 w-3.5 text-primary" />
            Latest
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('top')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'top'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Flame className="h-3.5 w-3.5 text-amber-500" />
            Top
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('unread')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'unread'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <MessageSquare className="h-3.5 w-3.5 text-blue-500" />
            Unread
            {unreadCountTotal > 0 && (
              <span className="ml-0.5 rounded-full bg-blue-500/20 px-1.5 py-0.2 text-[10px] font-bold text-blue-600 dark:text-blue-400">
                {unreadCountTotal}
              </span>
            )}
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('solved')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'solved'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
            Solved
            {solvedCountTotal > 0 && (
              <span className="ml-0.5 rounded-full bg-emerald-500/20 px-1.5 py-0.2 text-[10px] font-bold text-emerald-600 dark:text-emerald-400">
                {solvedCountTotal}
              </span>
            )}
          </button>
        </div>

        {/* Real-time Search Box */}
        <div className="relative flex-1 sm:max-w-xs">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search discussions..."
            className="w-full rounded-2xl border border-border bg-background/80 py-1.5 pl-9 pr-8 text-xs text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-hidden"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery('')}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-full p-0.5 text-muted-foreground transition hover:text-foreground"
            >
              <X className="h-3 w-3" />
            </button>
          )}
        </div>
      </div>

      {/* Topic List */}
      {topics.length === 0 ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center">
          <h4 className="text-base font-semibold text-card-foreground">No topics yet</h4>
          <p className="mt-2 text-sm text-muted-foreground">
            Be the first to start a conversation in this section!
          </p>
          <button
            type="button"
            onClick={() => openNewTopic({ categoryId: selectedCategoryId })}
            className="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-primary/30 bg-primary/5 px-4 py-2 text-sm font-semibold text-primary transition hover:bg-primary/10"
          >
            <Plus className="h-4 w-4" />
            Start Discussion
          </button>
        </div>
      ) : filteredTopics.length === 0 ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center">
          <h4 className="text-base font-semibold text-card-foreground">
            No matching discussions found
          </h4>
          <p className="mt-2 text-sm text-muted-foreground">
            No topics match your current filter tab or search keyword.
          </p>
          <button
            type="button"
            onClick={handleResetFilters}
            className="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
          >
            Clear filters
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          {filteredTopics.map((topic) => {
            const isSelected = selectedTopicId === topic.id;
            return (
              <div
                key={topic.id}
                onClick={() => onSelectTopic?.(topic.id)}
                className={`block cursor-pointer rounded-[1.5rem] border p-5 transition ${
                  isSelected
                    ? 'border-primary/50 bg-primary/5 shadow-xs'
                    : 'border-border bg-background/60 hover:border-border/80 hover:bg-muted/30'
                }`}
              >
                <div className="flex flex-wrap items-start justify-between gap-4">
                  <div className="space-y-3">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="rounded-full bg-muted px-2.5 py-0.5 text-[11px] font-medium text-muted-foreground">
                        {topic.status}
                      </span>
                      <span
                        dir="ltr"
                        className="rounded-full border border-border px-2.5 py-0.5 text-[11px] font-medium text-muted-foreground"
                      >
                        {topic.effectiveLocale}
                      </span>

                      {/* Solved Badge */}
                      {Boolean(topic.solutionReplyId) && (
                        <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                          <CheckCircle2 className="h-3 w-3" /> Solved
                        </span>
                      )}

                      {/* Unread Badge */}
                      {topic.isUnread && (
                        <span className="rounded-full bg-blue-500/15 px-2.5 py-0.5 text-[11px] font-medium text-blue-700 dark:text-blue-300">
                          {topic.unreadCount ? `${topic.unreadCount} unread` : 'New'}
                        </span>
                      )}

                      {/* Pinned Badge */}
                      {topic.isPinned && (
                        <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2.5 py-0.5 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                          <Pin className="h-3 w-3" /> Pinned
                        </span>
                      )}

                      {/* Locked Badge */}
                      {topic.isLocked && (
                        <span className="inline-flex items-center gap-1 rounded-full bg-destructive/10 px-2.5 py-0.5 text-[11px] font-medium text-destructive">
                          <Lock className="h-3 w-3" /> Locked
                        </span>
                      )}

                      {/* Vote Score Badge */}
                      {topic.voteScore !== undefined && topic.voteScore > 0 && (
                        <span className="inline-flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                          ▲ {topic.voteScore}
                        </span>
                      )}
                    </div>

                    <div>
                      <h4 className="text-lg font-semibold text-foreground">
                        {topic.title}
                      </h4>
                      <p className="mt-1 text-xs text-muted-foreground" dir="ltr">
                        t/{topic.slug}
                      </p>
                    </div>

                    <AuthorBadge authorId={topic.authorId} />
                  </div>

                  <div className="text-right">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                      Replies
                    </p>
                    <p className="mt-1 text-2xl font-semibold text-foreground">
                      {topic.replyCount}
                    </p>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
