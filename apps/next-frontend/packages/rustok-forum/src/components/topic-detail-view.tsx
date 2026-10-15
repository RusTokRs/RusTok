'use client';

import React, { useCallback, useEffect, useRef, useState } from 'react';
import Link from 'next/link';
import {
  ArrowLeft,
  CheckCircle2,
  Lock,
  MessageSquare,
  Pin,
  Reply as ReplyIcon,
  Share2,
} from 'lucide-react';
import { RichTextHtml } from '@rustok/richtext/view';
import type {
  ForumCategoryListItem,
  ForumMemberCard,
  ForumReplyDetail,
  ForumTopicDetail,
} from '../api/forum';
import {
  fetchForumMemberCards,
  fetchAllStorefrontCategories,
  fetchStorefrontReplies,
  fetchStorefrontTopic,
  markForumTopicRead,
} from '../api/forum';
import { AuthorBadge, MemberCardProvider } from './member-card';
import { TimelineScroller } from './timeline-scroller';
import { Composer } from './composer';
import { QuoteTooltip } from './quote-tooltip';
import { ComposerProvider, useComposer } from '../context/composer-context';
import { TopicStatusLabel } from './topic-status-label';
import { canReplyToTopic } from './topic-reply-availability';
import { useForumAuth } from './use-forum-auth';
import {
  focusForumReplyPost,
  isForumReplyId,
  MAX_ANCHOR_REPLY_PAGES,
} from './reply-anchor';
import { useTranslations } from '@rustok/next-fluent';

interface TopicDetailViewProps {
  tenantId?: string | null;
  tenantSlug?: string | null;
  locale: string;
  topicId: string;
  initialTopic?: ForumTopicDetail | null;
  initialReplies?: ForumReplyDetail[];
  initialRepliesTotal?: number;
  /** Cursor after `initialReplies`, so a reply deep link continues paging without refetching. */
  initialRepliesCursor?: string | null;
  /** Reply id addressed by `?reply=` on a search-result or notification link. */
  focusReplyId?: string | null;
}

function TopicDetailContent({
  tenantId,
  tenantSlug,
  locale,
  topicId,
  initialTopic,
  initialReplies = [],
  initialRepliesTotal = 0,
  initialRepliesCursor = null,
  focusReplyId = null,
}: TopicDetailViewProps) {
  const forum = useTranslations('Forum');
  const { openReply } = useComposer();
  const auth = useForumAuth();
  const token = auth && (!tenantSlug || auth.tenant === tenantSlug) ? auth.token : '';
  const contentKey = `${tenantId ?? ''}:${tenantSlug ?? ''}:${locale}:${topicId}:${token}`;
  const categoryKey = `${tenantId ?? ''}:${tenantSlug ?? ''}:${locale}:${token}`;

  const [currentPost, setCurrentPost] = useState(1);
  const [content, setContent] = useState<{
    key: string; topic: ForumTopicDetail | null; replies: ForumReplyDetail[];
    nextCursor: string | null;
  } | null>(null);
  // Server-provided content is rendered only until the browser auth state is known.
  // It is request-bound and never cached across viewers by the route.
  const topic = content?.key === contentKey ? content.topic : auth === null ? initialTopic ?? null : null;
  const replies = content?.key === contentKey ? content.replies : auth === null ? initialReplies : [];
  const repliesCursor = content?.key === contentKey ? content.nextCursor : auth === null ? initialRepliesCursor : null;
  const repliesTotal = topic?.replyCount ?? initialRepliesTotal;
  const [categoryPage, setCategoryPage] = useState<{
    key: string; items: ForumCategoryListItem[];
  } | null>(null);
  // A previous viewer's private category names must disappear in the same
  // render as an account/tenant change, not after the next request resolves.
  const categories = categoryPage?.key === categoryKey ? categoryPage.items : [];
  const categoryKeyRef = useRef(categoryKey);
  categoryKeyRef.current = categoryKey;
  const [cards, setCards] = useState<{ key: string; items: ForumMemberCard[] } | null>(null);
  const memberCards = cards?.key === contentKey ? cards.items : [];
  const [copiedLink, setCopiedLink] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadingReplies, setLoadingReplies] = useState(false);
  const [contentError, setContentError] = useState<string | null>(null);
  const [categoriesError, setCategoriesError] = useState<string | null>(null);
  const [cardsError, setCardsError] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const loadError = contentError ?? categoriesError ?? cardsError ?? readError;
  const [anchorMissing, setAnchorMissing] = useState(false);
  const anchorAppliedRef = useRef(false);
  const attemptedReadRef = useRef<string | null>(null);
  const contentRequest = useRef(0);
  const replyRequest = useRef(0);
  const contentKeyRef = useRef(contentKey);
  contentKeyRef.current = contentKey;

  const loadData = useCallback(async () => {
    if (!auth) return;
    const request = ++contentRequest.current;
    // The first page of a new snapshot supersedes any in-flight older page.
    ++replyRequest.current;
    setLoadingReplies(false);
    setLoading(true);
    setContentError(null);
    try {
      const options = {
        tenantId: tenantId ?? undefined,
        tenantSlug: tenantSlug ?? undefined,
        token: token || undefined,
        topicId,
        locale,
      };
      const topicData = await fetchStorefrontTopic(options);
      if (request !== contentRequest.current || contentKeyRef.current !== contentKey) return;
      if (!topicData) {
        setContent({ key: contentKey, topic: null, replies: [], nextCursor: null });
        return;
      }
      const first = await fetchStorefrontReplies(options);
      if (request !== contentRequest.current || contentKeyRef.current !== contentKey) return;
      let items = first.items;
      let cursor = first.nextCursor;
      const anchorId = isForumReplyId(focusReplyId) ? focusReplyId : null;
      if (anchorId) {
        let pages = 1;
        while (cursor && pages < MAX_ANCHOR_REPLY_PAGES &&
               !items.some((reply) => reply.id === anchorId)) {
          const next = await fetchStorefrontReplies({ ...options, after: cursor });
          items = items.concat(next.items);
          cursor = next.nextCursor;
          pages += 1;
        }
        if (request !== contentRequest.current || contentKeyRef.current !== contentKey) return;
        const anchoredIndex = items.findIndex((reply) => reply.id === anchorId);
        setAnchorMissing(anchoredIndex < 0);
        if (anchoredIndex >= 0) setCurrentPost(anchoredIndex + 2);
      }
      setContent({ key: contentKey, topic: topicData, replies: items, nextCursor: cursor });
      const userIds = Array.from(new Set(
        [topicData.authorId, ...items.map((reply) => reply.authorId)].filter(
          (id): id is string => Boolean(id)
        )
      ));
      // Member cards are private supplementary data; a card error must not
      // discard an otherwise successfully loaded topic and its replies.
      if (token && userIds.length) {
        // Do not delay the thread (or its one-shot read receipt) for profiles.
        void fetchForumMemberCards({
          userIds, token, locale, tenantSlug: tenantSlug ?? undefined,
        }).then((loaded) => {
          if (request === contentRequest.current && contentKeyRef.current === contentKey) {
            setCards((previous) => {
              const prior = previous?.key === contentKey ? previous.items : [];
              const merged = new Map([...prior, ...loaded].map((card) => [card.userId, card]));
              return { key: contentKey, items: Array.from(merged.values()) };
            });
            setCardsError(null);
          }
        }).catch(() => {
          if (request === contentRequest.current && contentKeyRef.current === contentKey) {
            setCardsError(forum('errorMemberCards'));
          }
        });
      }
    } catch {
      if (request === contentRequest.current && contentKeyRef.current === contentKey) setContentError(forum('errorLoad'));
    } finally {
      if (request === contentRequest.current && contentKeyRef.current === contentKey) setLoading(false);
    }
  }, [auth, tenantId, tenantSlug, token, topicId, locale, contentKey, focusReplyId, forum]);

  useEffect(() => {
    if (!auth) return;
    let active = true;
    fetchAllStorefrontCategories({
      tenantId: tenantId ?? undefined, tenantSlug: tenantSlug ?? undefined,
      token: token || undefined, locale,
    }).then((page) => {
      if (!active || categoryKeyRef.current !== categoryKey) return;
      setCategoryPage({ key: categoryKey, items: page.items });
      setCategoriesError(null);
    }).catch(() => {
      if (active && categoryKeyRef.current === categoryKey) setCategoriesError(forum('errorLoad'));
    });
    return () => { active = false; };
  }, [auth, tenantId, tenantSlug, token, locale, categoryKey, forum]);

  const loadMoreReplies = useCallback(async () => {
    if (!auth || !repliesCursor || loadingReplies) return;
    const request = ++replyRequest.current;
    setLoadingReplies(true);
    try {
      const next = await fetchStorefrontReplies({
        tenantId: tenantId ?? undefined, tenantSlug: tenantSlug ?? undefined,
        token: token || undefined, topicId, locale, after: repliesCursor,
      });
      if (request !== replyRequest.current || contentKeyRef.current !== contentKey) return;
      setContent((previous) => {
        if (previous?.key !== contentKey) return previous;
        const items = new Map([...previous.replies, ...next.items].map((reply) => [reply.id, reply]));
        return { ...previous, replies: Array.from(items.values()), nextCursor: next.nextCursor };
      });
      if (isForumReplyId(focusReplyId) && next.items.some((reply) => reply.id === focusReplyId)) {
        setAnchorMissing(false);
      }
      setContentError(null);
      if (token) {
        const known = new Set(memberCards.map((card) => card.userId));
        const userIds = Array.from(new Set(next.items.map((reply) => reply.authorId).filter(
          (id): id is string => typeof id === 'string' && !known.has(id)
        )));
        if (userIds.length) {
          void fetchForumMemberCards({
            userIds, token, locale, tenantSlug: tenantSlug ?? undefined,
          }).then((loaded) => {
            if (request !== replyRequest.current || contentKeyRef.current !== contentKey) return;
            setCards((previous) => {
              const prior = previous?.key === contentKey ? previous.items : [];
              const merged = new Map([...prior, ...loaded].map((card) => [card.userId, card]));
              return { key: contentKey, items: Array.from(merged.values()) };
            });
            setCardsError(null);
          }).catch(() => {
            if (request === replyRequest.current && contentKeyRef.current === contentKey) {
              setCardsError(forum('errorMemberCards'));
            }
          });
        }
      }
    } catch {
      if (request === replyRequest.current && contentKeyRef.current === contentKey) setContentError(forum('errorLoad'));
    } finally {
      if (request === replyRequest.current && contentKeyRef.current === contentKey) setLoadingReplies(false);
    }
  }, [auth, repliesCursor, loadingReplies, tenantId, tenantSlug, token, topicId, locale, contentKey, focusReplyId, memberCards, forum]);

  useEffect(() => {
    anchorAppliedRef.current = false;
    setAnchorMissing(false);
  }, [focusReplyId, contentKey]);

  // Scroll to the linked post once, after it exists in the document.
  useEffect(() => {
    if (anchorAppliedRef.current || !isForumReplyId(focusReplyId)) {
      return undefined;
    }
    const clearHighlight = focusForumReplyPost(focusReplyId);
    if (!clearHighlight) {
      return undefined;
    }
    anchorAppliedRef.current = true;
    return clearHighlight;
  }, [focusReplyId, replies]);

  useEffect(() => {
    setCardsError(null);
    setCategoriesError(null);
    setReadError(null);
    setContentError(null);
    setLoadingReplies(false);
  }, [contentKey]);

  useEffect(() => {
    if (auth) void loadData();
  }, [auth, loadData]);

  // Mark exactly once per viewer and topic on opening, including after errors.
  // A ref is set before the network call; request completion cannot retrigger it.
  useEffect(() => {
    if (!token || !topic?.id || loading) return;
    const key = JSON.stringify([token, tenantId, tenantSlug, topic.id]);
    if (attemptedReadRef.current === key) return;
    attemptedReadRef.current = key;
    void markForumTopicRead({
      tenantId: tenantId ?? undefined,
      tenantSlug: tenantSlug ?? undefined,
      token,
      topicId: topic.id,
      locale,
    }).then(() => {
      if (contentKeyRef.current === contentKey) setReadError(null);
    }).catch(() => {
      if (contentKeyRef.current === contentKey) setReadError(forum('errorMarkRead'));
    });
  }, [token, topic?.id, loading, tenantId, tenantSlug, locale, contentKey, forum]);

  // Track active post position as user scrolls
  useEffect(() => {
    if (!topic) return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const el = entry.target as HTMLElement;
            if (el.getAttribute('data-target-kind') === 'TOPIC') {
              setCurrentPost(1);
            } else {
              const id = el.id;
              const match = id.match(/reply-(\d+)/);
              if (match) {
                const replyIndex = parseInt(match[1], 10);
                setCurrentPost(replyIndex + 2);
              }
            }
          }
        }
      },
      { rootMargin: '-10% 0px -70% 0px' }
    );

    const opEl = document.querySelector('[data-forum-post][data-target-kind="TOPIC"]');
    if (opEl) observer.observe(opEl);

    replies.forEach((_, idx) => {
      const el = document.getElementById(`reply-${idx}`);
      if (el) observer.observe(el);
    });

    return () => observer.disconnect();
  }, [topic, replies]);

  const getAuthorHandle = (authorId: string | null | undefined): string | undefined => {
    if (!authorId) return undefined;
    const card = memberCards.find((c) => c.userId === authorId);
    return card?.profile.handle;
  };

  const handleShare = async () => {
    try {
      await navigator.clipboard.writeText(window.location.href);
      setCopiedLink(true);
      setTimeout(() => setCopiedLink(false), 2000);
    } catch {
      // Ignore clipboard error
    }
  };

  const handleJumpToPost = (postNumber: number) => {
    if (postNumber === 1) {
      window.scrollTo({ top: 0, behavior: 'smooth' });
    } else {
      const el = document.getElementById(`reply-${postNumber - 2}`);
      el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  const category = categories.find((c) => c.id === topic?.categoryId);

  if (!topic && (auth === null || loading)) {
    return (
      <div className="rounded-[1.75rem] border border-dashed border-border p-12 text-center text-sm text-muted-foreground">
        {forum('loading')}
      </div>
    );
  }

  if (!topic && loadError) {
    return (
      <div className="rounded-[1.75rem] border border-dashed border-destructive/40 p-12 text-center">
        <h3 className="text-xl font-semibold text-card-foreground">{forum('errorLoad')}</h3>
        <p className="mt-2 text-sm text-muted-foreground">{loadError}</p>
        <button
          type="button"
          onClick={() => void loadData()}
          className="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
        >
          {forum('retry')}
        </button>
      </div>
    );
  }

  if (!topic) {
    return (
      <div className="rounded-[1.75rem] border border-dashed border-border p-12 text-center">
        <h3 className="text-xl font-semibold text-card-foreground">{forum('topicNotFound')}</h3>
        <p className="mt-2 text-sm text-muted-foreground">
          {forum('topicNotFoundHint')}
        </p>
        <Link
          href={`/${locale}/modules/forum`}
          className="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
        >
          <ArrowLeft className="h-4 w-4" />
          {forum('backToDiscussions')}
        </Link>
      </div>
    );
  }

  const canReply = canReplyToTopic(topic);

  return (
    <MemberCardProvider cards={memberCards}>
      <div className="space-y-6">
        {loadError && (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-2 text-xs text-destructive">
            <span>{loadError}</span>
            <button
              type="button"
              onClick={() => void loadData()}
              className="rounded-xl border border-destructive/30 px-3 py-1 font-semibold transition hover:bg-destructive/15"
            >
              {forum('retry')}
            </button>
          </div>
        )}
        {/* Discourse/NodeBB Breadcrumbs & Header Navigation */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-border/60 pb-4">
          <nav className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <Link
              href={`/${locale}/modules/forum`}
              className="inline-flex items-center gap-1.5 font-medium transition hover:text-foreground"
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              {forum('forumIndex')}
            </Link>
            <span>/</span>
            {category ? (
              <Link
                href={`/${locale}/modules/forum?category=${category.id}`}
                className="font-medium text-foreground transition hover:underline"
              >
                {category.name}
              </Link>
            ) : (
              <span>{forum('discussions')}</span>
            )}
            <span>/</span>
            <span className="max-w-xs truncate font-semibold text-foreground sm:max-w-md">
              {topic.title}
            </span>
          </nav>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleShare}
              className="inline-flex items-center gap-1.5 rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-muted-foreground shadow-xs transition hover:bg-muted hover:text-foreground"
            >
              <Share2 className="h-3.5 w-3.5" />
              {copiedLink ? forum('copiedLink') : forum('share')}
            </button>
            {canReply && (
              <button
                type="button"
                onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-1.5 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
              >
                <MessageSquare className="h-3.5 w-3.5" />
                {forum('reply')}
              </button>
            )}
          </div>
        </div>

        {/* Main Thread Content & Timeline Scroller */}
        <div className="relative flex gap-6">
          <div className="flex-1 space-y-6">
            {/* Topic Title and Status Pillbox */}
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
                {topic.isPinned && (
                  <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2.5 py-0.5 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                    <Pin className="h-3 w-3" /> {forum('pinned')}
                  </span>
                )}
                {topic.isLocked && (
                  <span className="inline-flex items-center gap-1 rounded-full bg-destructive/10 px-2.5 py-0.5 text-[11px] font-medium text-destructive">
                    <Lock className="h-3 w-3" /> {forum('locked')}
                  </span>
                )}
              </div>

              <h1 className="text-3xl font-bold tracking-tight text-foreground sm:text-4xl">
                {topic.title}
              </h1>

              <div className="flex flex-wrap items-center justify-between gap-4 pt-2">
                <div className="flex items-center gap-3">
                  <AuthorBadge authorId={topic.authorId} />
                  <span className="text-xs text-muted-foreground">
                    {forum('postedOn', { date: new Date(topic.createdAt).toLocaleDateString() })}
                  </span>
                </div>

                <div className="flex items-center gap-3 text-xs text-muted-foreground">
                  <span>
                    <strong className="text-foreground">{repliesTotal}</strong> {forum('repliesCount')}
                  </span>
                </div>
              </div>
            </div>

            {/* Original Post (OP) */}
            <article
              data-forum-post
              data-target-kind="TOPIC"
              data-target-id={topic.id}
              data-author-handle={getAuthorHandle(topic.authorId)}
                className="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm"
            >
              <div className="prose prose-sm max-w-none text-foreground dark:prose-invert">
                <RichTextHtml view={topic.body} contentLocale={topic.effectiveLocale} />
              </div>

              <div className="mt-6 flex items-center justify-between border-t border-border/60 pt-4">
                <div className="flex items-center gap-2">
                  <span className="rounded-full bg-muted/60 px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
                    #1 OP
                  </span>
                </div>
                {canReply && (
                  <button
                    type="button"
                    onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                    className="inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-3 py-1.5 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
                  >
                    <ReplyIcon className="h-3.5 w-3.5" />
                    {forum('replyToTopic')}
                  </button>
                )}
              </div>
            </article>

            {/* Replies List */}
            {anchorMissing && (
              <p className="rounded-xl border border-dashed border-border px-4 py-2 text-center text-[11px] text-muted-foreground">
                {forum('linkedPostUnavailable')}
              </p>
            )}
            {replies.length > 0 && (
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <h3 className="text-lg font-semibold text-foreground">
                    {forum('replies')} ({repliesTotal})
                  </h3>
                </div>

                {replies.map((reply, idx) => (
                  <article
                    key={reply.id}
                    id={`reply-${idx}`}
                    data-forum-post
                    data-target-kind="REPLY"
                    data-target-id={reply.id}
                    data-author-handle={getAuthorHandle(reply.authorId)}
                            className="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm data-[focused=true]:border-primary/60 data-[focused=true]:ring-2 data-[focused=true]:ring-primary/40"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/60 pb-3">
                      <div className="flex items-center gap-3">
                        <AuthorBadge authorId={reply.authorId} />
                        <span className="text-xs text-muted-foreground">
                          {new Date(reply.createdAt).toLocaleDateString()}
                        </span>
                      </div>
                      <span className="rounded-full bg-muted/60 px-2.5 py-0.5 text-[11px] font-medium text-muted-foreground">
                        #{idx + 2}
                      </span>
                    </div>

                    <div className="prose prose-sm mt-4 max-w-none text-foreground dark:prose-invert">
                      <RichTextHtml view={reply.content} contentLocale={reply.effectiveLocale} />
                    </div>

                    {canReply && (
                      <div className="mt-4 flex items-center justify-end border-t border-border/40 pt-3">
                        <button
                          type="button"
                          onClick={() => openReply({
                            topicId: topic.id, topicTitle: topic.title, parentReplyId: reply.id,
                          })}
                          className="inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted hover:text-foreground"
                        >
                          <ReplyIcon className="h-3 w-3" />
                          {forum('reply')}
                        </button>
                      </div>
                    )}
                  </article>
                ))}
              </div>
            )}

            {repliesCursor && (
              <button
                type="button"
                onClick={() => void loadMoreReplies()}
                disabled={loadingReplies}
                className="w-full rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground transition hover:bg-muted disabled:opacity-50"
              >
                {loadingReplies ? forum('loading') : forum('loadMoreReplies')}
              </button>
            )}

            {/* Quick Reply Trigger Bar */}
            {canReply ? (
              <div className="rounded-[1.75rem] border border-dashed border-border bg-muted/20 p-6 text-center">
                <h4 className="text-base font-semibold text-foreground">{forum('joinTheConversation')}</h4>
                <p className="mt-1 text-xs text-muted-foreground">
                  {forum('joinTheConversationHint')}
                </p>
                <button
                  type="button"
                  onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                  className="mt-4 inline-flex items-center gap-2 rounded-full bg-primary px-5 py-2 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
                >
                  <MessageSquare className="h-4 w-4" />
                  {forum('replyToTopic')}
                </button>
              </div>
            ) : (
              <p className="rounded-[1.75rem] border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
                {forum('repliesClosed')}
              </p>
            )}
          </div>

          {/* Timeline Scroller */}
          {repliesTotal > 0 && (
            <div className="hidden xl:block">
              <div className="sticky top-20">
                <TimelineScroller
                  currentPost={currentPost}
                  totalPosts={repliesTotal + 1}
                  onJumpToPost={handleJumpToPost}
                />
              </div>
            </div>
          )}
        </div>

        {/* Global Floating Composer Drawer and Tooltip */}
        <Composer
          tenantId={tenantId ?? undefined}
          tenantSlug={tenantSlug ?? undefined}
          locale={locale}
          categories={categories}
          onSubmitted={loadData}
        />
        <QuoteTooltip tenantId={tenantId ?? undefined} tenantSlug={tenantSlug ?? undefined} locale={locale}
          replyTopic={canReply ? { id: topic.id, title: topic.title } : null} />
      </div>
    </MemberCardProvider>
  );
}

export function TopicDetailView(props: TopicDetailViewProps) {
  const auth = useForumAuth();
  return (
    <ComposerProvider tenantId={props.tenantId} tenantSlug={props.tenantSlug}>
      <TopicDetailContent
        {...props}
        // Initial HTML belongs to the server request viewer only. After mount
        // never reuse it when a different browser account reopens this tree.
        initialTopic={auth === null ? props.initialTopic : null}
        initialReplies={auth === null ? props.initialReplies : []}
        initialRepliesCursor={auth === null ? props.initialRepliesCursor : null}
      />
    </ComposerProvider>
  );
}
