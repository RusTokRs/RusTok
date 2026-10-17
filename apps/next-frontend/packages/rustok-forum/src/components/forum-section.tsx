'use client';

import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
  fetchAllStorefrontCategories,
  fetchStorefrontTopics,
  fetchStorefrontUnreadTopics,
  fetchStorefrontTopic,
  fetchStorefrontReplies,
  fetchForumMemberCards,
  markForumTopicRead,
  type ForumCategoryListItem,
  type ForumTopicListItem,
  type ForumTopicDetail,
  type ForumReplyDetail,
  type ForumMemberCard,
} from '../api/forum';
import { ComposerProvider } from '../context/composer-context';
import { MemberCardProvider } from './member-card';
import { CategoryRail } from './category-rail';
import { TopicFeed } from './topic-feed';
import { topicFeedSelection, type TopicFilterTab } from './topic-feed-selection';
import { useForumAuth } from './use-forum-auth';
import { ThreadPanel } from './thread-panel';
import { Composer } from './composer';
import { QuoteTooltip } from './quote-tooltip';
import { canReplyToTopic } from './topic-reply-availability';
import { CategoryOverview } from './category-overview';
import { isForumReplyId, MAX_ANCHOR_REPLY_PAGES } from './reply-anchor';
import { useTranslations } from '@rustok/next-fluent';
import { loadThreadReplyPages, threadJumpResult, type ThreadPostTarget } from './thread-navigation';

interface ForumSectionProps {
  tenantId?: string | null;
  tenantSlug?: string | null;
  locale: string;
  initialCategoryId?: string | null;
  initialTopicId?: string | null;
  /** Reply id from `?reply=` on a notification or mention deep link. */
  initialReplyId?: string | null;
  initialView?: 'topics' | 'categories';
}

export function ForumSection({
  tenantId,
  tenantSlug,
  locale,
  initialCategoryId = null,
  initialTopicId = null,
  initialReplyId = null,
  initialView = 'topics',
}: ForumSectionProps) {
  const forum = useTranslations('Forum');
  const auth = useForumAuth();
  const token = auth && (!tenantSlug || auth.tenant === tenantSlug) ? auth.token : '';
  // The key is kept in component memory only. It never becomes an HTML attribute or cache key.
  const viewerKey = `${tenantId ?? ''}:${tenantSlug ?? ''}:${locale}:${token}`;
  const [activeView, setActiveView] = useState<'topics' | 'categories'>(initialView);
  const [activeTab, setActiveTab] = useState<TopicFilterTab>('latest');
  const [searchQuery, setSearchQuery] = useState('');
  const normalizedSearchQuery = searchQuery.trim();
  const [selectedCategoryId, setSelectedCategoryId] = useState<string | null>(initialCategoryId);
  const [selectedTopicId, setSelectedTopicId] = useState<string | null>(initialTopicId);
  const feedKey = JSON.stringify([viewerKey, selectedCategoryId, activeTab, normalizedSearchQuery]);
  const detailKey = `${viewerKey}:${selectedTopicId ?? ''}`;

  const [categoryPage, setCategoryPage] = useState<{
    key: string; items: ForumCategoryListItem[]; total: number;
  } | null>(null);
  const categories = categoryPage?.key === viewerKey ? categoryPage.items : [];
  const categoriesTotal = categoryPage?.key === viewerKey ? categoryPage.total : 0;
  const [topicPage, setTopicPage] = useState<{
    key: string; items: ForumTopicListItem[]; nextCursor: string | null;
  } | null>(null);
  const topics = topicPage?.key === feedKey ? topicPage.items : [];
  const topicsCursor = topicPage?.key === feedKey ? topicPage.nextCursor : null;
  const [detailPage, setDetailPage] = useState<{
    key: string; topic: ForumTopicDetail | null; replies: ForumReplyDetail[];
    nextCursor: string | null;
  } | null>(null);
  const selectedTopic = detailPage?.key === detailKey ? detailPage.topic : null;
  const replies = detailPage?.key === detailKey ? detailPage.replies : [];
  const repliesTotal = selectedTopic?.replyCount ?? 0;
  const repliesCursor = detailPage?.key === detailKey ? detailPage.nextCursor : null;
  const [cardsPage, setCardsPage] = useState<{
    key: string; cards: ForumMemberCard[];
  } | null>(null);
  const memberCards = cardsPage?.key === viewerKey ? cardsPage.cards : [];

  const [isMarkingRead, setIsMarkingRead] = useState(false);
  const [loadingTopics, setLoadingTopics] = useState(true);
  const [loadingReplies, setLoadingReplies] = useState(false);
  const [categoriesError, setCategoriesError] = useState<string | null>(null);
  const [topicsError, setTopicsError] = useState<string | null>(null);
  const [detailsError, setDetailsError] = useState<string | null>(null);
  const [cardsError, setCardsError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const loadError = categoriesError ?? topicsError ?? detailsError ?? cardsError ?? actionError;
  const categoriesRequest = useRef(0);
  const topicsRequest = useRef(0);
  const detailsRequest = useRef(0);
  const repliesRequest = useRef(0);
  const repliesPendingRef = useRef(false);
  // Do not keep walking reply pages after this surface is unmounted.
  useEffect(() => () => {
    ++repliesRequest.current;
    repliesPendingRef.current = false;
  }, []);
  // Refs are updated during render, not in an effect: a response that finishes
  // between a viewer/tab change and the next effect must never mutate its UI.
  const viewerKeyRef = useRef(viewerKey);
  const feedKeyRef = useRef(feedKey);
  const detailKeyRef = useRef(detailKey);
  viewerKeyRef.current = viewerKey;
  feedKeyRef.current = feedKey;
  detailKeyRef.current = detailKey;

  // One-shot deep-link anchor: `?reply=<uuid>` addresses one post inside the opened topic.
  const [replyAnchor, setReplyAnchor] = useState<{
    topicId: string; replyId: string; visible: boolean;
  } | null>(null);
  const pendingAnchorRef = useRef<{ topicId: string; replyId: string } | null>(
    initialTopicId && isForumReplyId(initialReplyId)
      ? { topicId: initialTopicId, replyId: initialReplyId }
      : null
  );

  const loadCategories = useCallback(async () => {
    const request = ++categoriesRequest.current;
    try {
      const data = await fetchAllStorefrontCategories({
        tenantId: tenantId ?? undefined,
        tenantSlug: tenantSlug ?? undefined,
        token: token || undefined,
        locale,
      });
      if (request !== categoriesRequest.current || viewerKeyRef.current !== viewerKey) return;
      setCategoryPage({ key: viewerKey, items: data.items, total: data.total });
      setCategoriesError(null);
    } catch {
      if (request === categoriesRequest.current && viewerKeyRef.current === viewerKey) setCategoriesError(forum('errorLoad'));
    }
  }, [tenantId, tenantSlug, token, locale, viewerKey, forum]);

  // Profile cards require a signed-in reader and must not turn a successful
  // anonymous or personalized topic read into a failed forum page.
  const loadMemberCards = useCallback(async (items: Array<{ authorId: string | null }>) => {
    if (!token) return;
    const userIds = Array.from(new Set(items.map((item) => item.authorId).filter(
      (id): id is string => Boolean(id)
    )));
    if (!userIds.length) return;
    try {
      const cards = await fetchForumMemberCards({
        userIds, token, locale, tenantSlug: tenantSlug ?? undefined,
      });
      if (viewerKeyRef.current !== viewerKey) return;
      setCardsPage((previous) => {
        const merged = new Map(
          (previous?.key === viewerKey ? previous.cards : []).map((card) => [card.userId, card])
        );
        for (const card of cards) merged.set(card.userId, card);
        return { key: viewerKey, cards: Array.from(merged.values()) };
      });
      setCardsError(null);
    } catch {
      if (viewerKeyRef.current === viewerKey) setCardsError(forum('errorMemberCards'));
    }
  }, [token, locale, tenantSlug, viewerKey, forum]);

  const loadTopics = useCallback(async (after?: string) => {
    // A receipt/post can hold this callback across a selection change. Reject it
    // before incrementing the shared generation, or it cancels the new feed read.
    if (auth === null || feedKeyRef.current !== feedKey) return;
    const request = ++topicsRequest.current;
    if (activeTab === 'unread' && !token) {
      setTopicPage({ key: feedKey, items: [], nextCursor: null });
      setTopicsError(null);
      setLoadingTopics(false);
      return;
    }
    setLoadingTopics(true);
    setTopicsError(null);
    try {
      const options = {
        tenantId: tenantId ?? undefined,
        tenantSlug: tenantSlug ?? undefined,
        categoryId: selectedCategoryId ?? undefined,
        locale,
        after,
        ...topicFeedSelection(activeTab, normalizedSearchQuery),
      };
      const data = activeTab === 'unread' && token
        ? await fetchStorefrontUnreadTopics({ ...options, token })
        : await fetchStorefrontTopics({ ...options, token: token || undefined });
      if (request !== topicsRequest.current || feedKeyRef.current !== feedKey) return;
      setTopicPage((previous) => {
        const prior = after && previous?.key === feedKey ? previous.items : [];
        const items = new Map([...prior, ...data.items].map((item) => [item.id, item]));
        return { key: feedKey, items: Array.from(items.values()), nextCursor: data.nextCursor };
      });
      setTopicsError(null);
      void loadMemberCards(data.items);
    } catch {
      if (request === topicsRequest.current && feedKeyRef.current === feedKey) setTopicsError(forum('errorLoad'));
    } finally {
      if (request === topicsRequest.current && feedKeyRef.current === feedKey) setLoadingTopics(false);
    }
  }, [auth, tenantId, tenantSlug, selectedCategoryId, locale, activeTab, normalizedSearchQuery, token, feedKey, loadMemberCards, forum]);

  const loadTopicDetails = useCallback(async (topicId: string) => {
    const expectedKey = `${viewerKey}:${topicId}`;
    if (auth === null || detailKeyRef.current !== expectedKey) return;
    const request = ++detailsRequest.current;
    // A late "load more" from the old snapshot must not overwrite this
    // refresh's replies or cursor (notably after posting a new reply).
    ++repliesRequest.current;
    repliesPendingRef.current = false;
    setLoadingReplies(false);
    setDetailsError(null);
    try {
      const [topicData, repliesData] = await Promise.all([
        fetchStorefrontTopic({
          tenantId: tenantId ?? undefined, tenantSlug: tenantSlug ?? undefined,
          token: token || undefined, topicId, locale,
        }),
        fetchStorefrontReplies({
          tenantId: tenantId ?? undefined, tenantSlug: tenantSlug ?? undefined,
          token: token || undefined, topicId, locale,
        }),
      ]);
      if (request !== detailsRequest.current || detailKeyRef.current !== expectedKey) return;
      const pending = pendingAnchorRef.current;
      let items = repliesData.items;
      let cursor = repliesData.nextCursor;
      if (pending?.topicId === topicId) {
        let pages = 1;
        while (cursor && pages < MAX_ANCHOR_REPLY_PAGES &&
               !items.some((reply) => reply.id === pending.replyId)) {
          const next = await fetchStorefrontReplies({
            tenantId: tenantId ?? undefined, tenantSlug: tenantSlug ?? undefined,
            token: token || undefined, topicId, locale, after: cursor,
          });
          items = items.concat(next.items);
          cursor = next.nextCursor;
          pages += 1;
        }
        if (request !== detailsRequest.current || detailKeyRef.current !== expectedKey) return;
        pendingAnchorRef.current = null;
        setReplyAnchor({
          topicId, replyId: pending.replyId,
          visible: items.some((reply) => reply.id === pending.replyId),
        });
      }
      setDetailPage({ key: expectedKey, topic: topicData, replies: items, nextCursor: cursor });
      setDetailsError(null);
      void loadMemberCards([...(topicData ? [topicData] : []), ...items]);
    } catch {
      if (request === detailsRequest.current && detailKeyRef.current === expectedKey) setDetailsError(forum('errorLoad'));
    }
  }, [auth, tenantId, tenantSlug, token, locale, viewerKey, loadMemberCards, forum]);

  const loadMoreReplies = useCallback(async (target?: ThreadPostTarget) => {
    if (!selectedTopicId || !repliesCursor || repliesPendingRef.current) return null;
    const request = ++repliesRequest.current;
    const isCurrent = () => request === repliesRequest.current && detailKeyRef.current === detailKey;
    repliesPendingRef.current = true;
    setLoadingReplies(true);
    try {
      const data = await loadThreadReplyPages({
        items: replies, nextCursor: repliesCursor, target, isCurrent,
        loadPage: (after) => fetchStorefrontReplies({
          tenantId: tenantId ?? undefined, tenantSlug: tenantSlug ?? undefined,
          token: token || undefined, topicId: selectedTopicId, locale, after,
        }),
      });
      if (!data || !isCurrent()) return null;
      setDetailPage((previous) => {
        if (previous?.key !== detailKey) return previous;
        const items = new Map([...previous.replies, ...data.items].map((item) => [item.id, item]));
        return { ...previous, replies: Array.from(items.values()), nextCursor: data.nextCursor };
      });
      setDetailsError(null);
      void loadMemberCards(data.items.slice(replies.length));
      return target === undefined ? null : threadJumpResult(data, target);
    } catch {
      if (request === repliesRequest.current && detailKeyRef.current === detailKey) setDetailsError(forum('errorLoad'));
      return null;
    } finally {
      if (isCurrent()) {
        repliesPendingRef.current = false;
        setLoadingReplies(false);
      }
    }
  }, [selectedTopicId, replies, repliesCursor, tenantId, tenantSlug, token, locale, detailKey, loadMemberCards, forum]);

  useEffect(() => {
    setCategoriesError(null);
    setTopicsError(null);
    setDetailsError(null);
    setCardsError(null);
    setActionError(null);
    setIsMarkingRead(false);
    setLoadingReplies(false);
  }, [viewerKey]);

  useEffect(() => { if (auth) void loadCategories(); }, [auth, loadCategories]);
  // A category from another account, a removed category or a now-hidden link
  // must not leave this visitor trapped in a stale empty/forbidden feed.
  useEffect(() => {
    if (!selectedCategoryId || categoryPage?.key !== viewerKey) return;
    if (categoryPage.items.some((category) => category.id === selectedCategoryId)) return;
    setSelectedCategoryId(null);
  }, [categoryPage, viewerKey, selectedCategoryId]);
  useEffect(() => {
    if (!auth) return;
    setTopicsError(null);
    // Hide the previous selection immediately via feedKey, but coalesce typing requests.
    const timer = window.setTimeout(() => void loadTopics(), normalizedSearchQuery ? 250 : 0);
    return () => window.clearTimeout(timer);
  }, [auth, loadTopics, normalizedSearchQuery]);
  useEffect(() => {
    if (auth && selectedTopicId) void loadTopicDetails(selectedTopicId);
  }, [auth, selectedTopicId, loadTopicDetails]);

  const handleMarkTopicRead = useCallback(async (topicId: string) => {
    if (!token) return;
    setIsMarkingRead(true);
    try {
      await markForumTopicRead({
        tenantId: tenantId ?? undefined, tenantSlug: tenantSlug ?? undefined,
        token, topicId, locale,
      });
      if (viewerKeyRef.current !== viewerKey) return;
      void loadTopics();
      setActionError(null);
    } catch {
      if (viewerKeyRef.current === viewerKey) setActionError(forum('errorMarkRead'));
    } finally {
      if (viewerKeyRef.current === viewerKey) setIsMarkingRead(false);
    }
  }, [tenantId, tenantSlug, token, locale, viewerKey, loadTopics, forum]);

  const retryLoad = useCallback(() => {
    void loadCategories();
    void loadTopics();
    if (selectedTopicId) void loadTopicDetails(selectedTopicId);
  }, [loadCategories, loadTopics, loadTopicDetails, selectedTopicId]);

  const handleSubmitted = useCallback((targetTopicId: string) => {
    void loadTopics();
    if (selectedTopicId === targetTopicId) void loadTopicDetails(targetTopicId);
    else setSelectedTopicId(targetTopicId);
  }, [selectedTopicId, loadTopics, loadTopicDetails]);

  return (
    <ComposerProvider tenantId={tenantId} tenantSlug={tenantSlug}>
      <MemberCardProvider cards={memberCards}>
        <section className="overflow-hidden rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/35 p-6 sm:p-8 shadow-sm">
          {/* Header & View Switcher */}
          <div className="flex flex-col gap-6 sm:flex-row sm:items-end sm:justify-between">
            <div className="max-w-3xl space-y-3">
              <span className="inline-flex items-center gap-2 rounded-full border border-border bg-background/80 px-3 py-1 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                <span className="h-2 w-2 rounded-full bg-amber-500" />
                {forum('forumIndex')}
              </span>
              <h2 className="text-3xl font-semibold text-card-foreground">
                {forum('sectionSubtitle')}
              </h2>
              <p className="text-sm leading-6 text-muted-foreground">
                {forum('sectionDescription')}
              </p>
            </div>

            {/* View Mode Switcher (Topics vs Categories) */}
            <div className="flex items-center rounded-2xl border border-border bg-background p-1 shadow-xs">
              <button
                type="button"
                onClick={() => setActiveView('topics')}
                className={`flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-semibold transition ${
                  activeView === 'topics'
                    ? 'bg-primary text-primary-foreground shadow-xs'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                <span>💬</span>
                <span>{forum('discussions')}</span>
              </button>
              <button
                type="button"
                onClick={() => setActiveView('categories')}
                className={`flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-semibold transition ${
                  activeView === 'categories'
                    ? 'bg-primary text-primary-foreground shadow-xs'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                <span>📁</span>
                <span>{forum('categories')}</span>
              </button>
            </div>
          </div>

          {/* Load failures stay visible instead of masquerading as an empty forum */}
          {loadError && (
            <div className="mt-6 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-2 text-xs text-destructive">
              <span>{loadError}</span>
              <button
                type="button"
                onClick={retryLoad}
                className="rounded-xl border border-destructive/30 px-3 py-1 font-semibold transition hover:bg-destructive/15"
              >
                {forum('retry')}
              </button>
            </div>
          )}

          {/* Body: Categories Overview vs 3-Column Feed */}
          {activeView === 'categories' ? (
            <div className="mt-8">
              <CategoryOverview
                categories={categories}
                total={categoriesTotal}
                onSelectCategory={(cId) => {
                  setSelectedCategoryId(cId);
                  setSelectedTopicId(null);
                  setActiveView('topics');
                }}
              />
            </div>
          ) : (
            <div className="mt-8 grid gap-6 xl:grid-cols-[16rem_minmax(0,1fr)_24rem]">
              <CategoryRail
                categories={categories}
                total={categoriesTotal}
                selectedCategoryId={selectedCategoryId}
                onSelectCategory={(cId) => {
                  setSelectedCategoryId(cId);
                  setSelectedTopicId(null);
                }}
                onSwitchToOverview={() => setActiveView('categories')}
              />

              <TopicFeed
                isLoading={loadingTopics || (auth !== null && topicPage?.key !== feedKey && !topicsError)}
                hasError={Boolean(topicsError)}
                topics={topics}
                total={topics.length}
                activeTab={activeTab}
                searchQuery={searchQuery}
                onSearchChange={setSearchQuery}
                onTabChange={setActiveTab}
                isAuthenticated={Boolean(token)}
                hasMore={Boolean(topicsCursor)}
                onLoadMore={() => { if (topicsCursor) void loadTopics(topicsCursor); }}
                selectedTopicId={selectedTopicId}
                onSelectTopic={(tId) => {
                  setSelectedTopicId(tId);
                  setReplyAnchor(null);
                }}
                selectedCategoryId={selectedCategoryId}
                locale={locale}
              />

              <ThreadPanel
                key={detailKey}
                topic={selectedTopic}
                replies={replies}
                repliesTotal={repliesTotal}
                onMarkRead={token ? handleMarkTopicRead : undefined}
                isMarkingRead={isMarkingRead}
                hasMoreReplies={Boolean(repliesCursor)}
                isLoadingReplies={loadingReplies}
                onLoadMoreReplies={() => void loadMoreReplies()}
                onNavigateReplies={loadMoreReplies}
                focusReplyId={
                  replyAnchor && replyAnchor.topicId === selectedTopicId && replyAnchor.visible
                    ? replyAnchor.replyId
                    : null
                }
                focusReplyMissing={
                  replyAnchor !== null &&
                  !replyAnchor.visible &&
                  replyAnchor.topicId === selectedTopicId
                }
                onFocusHandled={() => setReplyAnchor(null)}
              />
            </div>
          )}

          {/* Global Composer and Floating Tooltip */}
          <Composer
            tenantId={tenantId ?? undefined}
            tenantSlug={tenantSlug ?? undefined}
            locale={locale}
            categories={categories}
            onSubmitted={handleSubmitted}
          />
          <QuoteTooltip tenantId={tenantId ?? undefined} tenantSlug={tenantSlug ?? undefined} locale={locale}
            replyTopic={selectedTopic && canReplyToTopic(selectedTopic)
              ? { id: selectedTopic.id, title: selectedTopic.title } : null} />
        </section>
      </MemberCardProvider>
    </ComposerProvider>
  );
}
