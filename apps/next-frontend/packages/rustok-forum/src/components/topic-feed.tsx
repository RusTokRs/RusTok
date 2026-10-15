'use client';

import React, { useMemo, useState } from 'react';
import Link from 'next/link';
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
import { TopicStatusLabel } from './topic-status-label';
import { useTranslations } from '@rustok/next-fluent';

export type TopicFilterTab = 'latest' | 'top' | 'unread' | 'solved';

interface TopicFeedProps {
  /** The first page is still in flight; the empty state must not claim there is nothing. */
  isLoading?: boolean;
  /** A failed owner read must not masquerade as an empty or all-read forum. */
  hasError?: boolean;
  topics: ForumTopicListItem[];
  /** Count of loaded, audience-visible items, not a global category aggregate. */
  total: number;
  activeTab: TopicFilterTab;
  onTabChange: (tab: TopicFilterTab) => void;
  isAuthenticated: boolean;
  hasMore: boolean;
  onLoadMore: () => void;
  selectedTopicId: string | null;
  onSelectTopic?: (topicId: string) => void;
  selectedCategoryId: string | null;
  locale?: string;
}

export function TopicFeed({
  isLoading = false,
  hasError = false,
  topics,
  total,
  activeTab,
  onTabChange,
  isAuthenticated,
  hasMore,
  onLoadMore,
  selectedTopicId,
  onSelectTopic,
  selectedCategoryId,
  locale,
}: TopicFeedProps) {
  const forum = useTranslations('Forum');
  const { openNewTopic } = useComposer();
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
        // Already selected by the authenticated owner query, not from the public page.
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

  const unreadCountTotal = activeTab === 'unread' ? topics.length : 0;

  const solvedCountTotal = useMemo(() => {
    return topics.filter((t) => Boolean(t.solutionReplyId)).length;
  }, [topics]);

  const handleResetFilters = () => {
    onTabChange('latest');
    setSearchQuery('');
  };

  return (
    <section className="space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
            {forum('topicFeed')}
          </p>
          <h3 className="mt-2 text-2xl font-semibold text-card-foreground">
            {forum('latestDiscussions')}
          </h3>
        </div>

        <div className="flex items-center gap-2">
          <span className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
            {forum('loadedThreads', { count: total })}
          </span>
          <button
            type="button"
            onClick={() => openNewTopic({ categoryId: selectedCategoryId })}
            className="inline-flex items-center gap-1.5 rounded-full bg-primary px-3.5 py-1 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
          >
            <Plus className="h-3.5 w-3.5" />
            {forum('newTopic')}
          </button>
        </div>
      </div>

      {/* Filter Tabs and Search Bar */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        {/* Discourse/NodeBB Filter Tabs */}
        <div className="flex flex-wrap items-center gap-1 rounded-2xl border border-border bg-muted/30 p-1">
          <button
            type="button"
            onClick={() => onTabChange('latest')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'latest'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Sparkles className="h-3.5 w-3.5 text-primary" />
            {forum('latest')}
          </button>

          <button
            type="button"
            onClick={() => onTabChange('top')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'top'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Flame className="h-3.5 w-3.5 text-amber-500" />
            {forum('top')}
          </button>

          <button
            type="button"
            onClick={() => onTabChange('unread')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'unread'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <MessageSquare className="h-3.5 w-3.5 text-blue-500" />
            {forum('unread')}
            {unreadCountTotal > 0 && (
              <span className="ml-0.5 rounded-full bg-blue-500/20 px-1.5 py-0.2 text-[10px] font-bold text-blue-600 dark:text-blue-400">
                {unreadCountTotal}
              </span>
            )}
          </button>

          <button
            type="button"
            onClick={() => onTabChange('solved')}
            className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition ${
              activeTab === 'solved'
                ? 'bg-background text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
            {forum('solved')}
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
            placeholder={forum('searchPlaceholder')}
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

      {/* These filters and text search apply to loaded pages, not the whole forum. */}
      {(searchQuery || activeTab === 'top' || activeTab === 'solved') && (
        <p className="text-xs text-muted-foreground">{forum('filtersLoadedOnly')}</p>
      )}

      {/* Topic List */}
      {isLoading && topics.length === 0 ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
          {forum('loading')}
        </div>
      ) : activeTab === 'unread' && !isAuthenticated ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
          {forum('signInForUnread')}
        </div>
      ) : hasError && topics.length === 0 ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
          {forum('errorLoad')}
        </div>
      ) : activeTab === 'unread' && topics.length === 0 ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center">
          <h4 className="text-base font-semibold text-card-foreground">{forum('noUnreadTopics')}</h4>
          <p className="mt-2 text-sm text-muted-foreground">{forum('noUnreadTopicsHint')}</p>
        </div>
      ) : topics.length === 0 ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center">
          <h4 className="text-base font-semibold text-card-foreground">{forum('noTopicsYet')}</h4>
          <p className="mt-2 text-sm text-muted-foreground">
            {forum('noTopicsYetHint')}
          </p>
          <button
            type="button"
            onClick={() => openNewTopic({ categoryId: selectedCategoryId })}
            className="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-primary/30 bg-primary/5 px-4 py-2 text-sm font-semibold text-primary transition hover:bg-primary/10"
          >
            <Plus className="h-4 w-4" />
            {forum('startDiscussion')}
          </button>
        </div>
      ) : filteredTopics.length === 0 ? (
        <div className="rounded-[1.5rem] border border-dashed border-border p-8 text-center">
          <h4 className="text-base font-semibold text-card-foreground">
            {forum('noMatchingDiscussions')}
          </h4>
          <p className="mt-2 text-sm text-muted-foreground">
            {forum('noMatchingDiscussionsHint')}
          </p>
          <button
            type="button"
            onClick={handleResetFilters}
            className="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
          >
            {forum('clearFilters')}
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
                        <TopicStatusLabel status={topic.status} />
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
                          <CheckCircle2 className="h-3 w-3" /> {forum('solved')}
                        </span>
                      )}

                      {/* Unread Badge */}
                      {topic.isUnread && (
                        <span className="rounded-full bg-blue-500/15 px-2.5 py-0.5 text-[11px] font-medium text-blue-700 dark:text-blue-300">
                          {topic.unreadCount ? `${topic.unreadCount} ${forum('unread')}` : forum('fresh')}
                        </span>
                      )}

                      {/* Pinned Badge */}
                      {topic.isPinned && (
                        <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2.5 py-0.5 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                          <Pin className="h-3 w-3" /> {forum('pinned')}
                        </span>
                      )}

                      {/* Locked Badge */}
                      {topic.isLocked && (
                        <span className="inline-flex items-center gap-1 rounded-full bg-destructive/10 px-2.5 py-0.5 text-[11px] font-medium text-destructive">
                          <Lock className="h-3 w-3" /> {forum('locked')}
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
                      {locale ? (
                        <Link
                          // The topic page is mounted at the owner's canonical route prefix
                          // `/[locale]/forum/t/…`. This read model carries the topic id and slug but
                          // not the owner's short identity, so the link uses the UUID form the route
                          // resolves instead of re-implementing route identity in the widget.
                          href={`/${locale}/forum/t/${topic.id}`}
                          onClick={(e) => e.stopPropagation()}
                          className="mt-1 inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground hover:underline"
                          dir="ltr"
                          title={forum('openTopicFullPage')}
                        >
                          t/{topic.slug} ↗
                        </Link>
                      ) : (
                        <p className="mt-1 text-xs text-muted-foreground" dir="ltr">
                          t/{topic.slug}
                        </p>
                      )}
                    </div>

                    <AuthorBadge authorId={topic.authorId} />
                  </div>

                  <div className="text-right">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                      {forum('replies')}
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
      {hasMore && (
        <button
          type="button"
          onClick={onLoadMore}
          disabled={isLoading}
          className="w-full rounded-xl border border-border bg-background px-4 py-2 text-sm font-medium text-foreground transition hover:bg-muted disabled:opacity-50"
        >
          {isLoading ? forum('loading') : forum('loadMoreTopics')}
        </button>
      )}
    </section>
  );
}
