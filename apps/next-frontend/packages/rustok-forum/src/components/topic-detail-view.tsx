'use client';

import React, { useCallback, useEffect, useState } from 'react';
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
  fetchStorefrontCategories,
  fetchStorefrontReplies,
  fetchStorefrontTopic,
  markForumTopicRead,
} from '../api/forum';
import { AuthorBadge, MemberCardProvider } from './member-card';
import { TimelineScroller } from './timeline-scroller';
import { Composer } from './composer';
import { QuoteTooltip } from './quote-tooltip';
import { ComposerProvider, useComposer } from '../context/composer-context';

interface TopicDetailViewProps {
  tenantId?: string | null;
  tenantSlug?: string | null;
  locale: string;
  topicId: string;
  initialTopic?: ForumTopicDetail | null;
  initialReplies?: ForumReplyDetail[];
  initialRepliesTotal?: number;
}

function TopicDetailContent({
  tenantId,
  tenantSlug,
  locale,
  topicId,
  initialTopic,
  initialReplies = [],
  initialRepliesTotal = 0,
}: TopicDetailViewProps) {
  const { openReply } = useComposer();

  const [currentPost, setCurrentPost] = useState(1);
  const [topic, setTopic] = useState<ForumTopicDetail | null>(initialTopic ?? null);
  const [replies, setReplies] = useState<ForumReplyDetail[]>(initialReplies);
  const [repliesTotal, setRepliesTotal] = useState(initialRepliesTotal);
  const [categories, setCategories] = useState<ForumCategoryListItem[]>([]);
  const [memberCards, setMemberCards] = useState<ForumMemberCard[]>([]);
  const [isMarkingRead, setIsMarkingRead] = useState(false);
  const [copiedLink, setCopiedLink] = useState(false);

  // Load Categories & Topic Data if not provided initially
  const loadData = useCallback(async () => {
    try {
      const [catsData, topicData, repliesData] = await Promise.all([
        fetchStorefrontCategories({
          tenantId: tenantId ?? undefined,
          tenantSlug: tenantSlug ?? undefined,
          locale,
        }),
        !initialTopic
          ? fetchStorefrontTopic({
              tenantId: tenantId ?? undefined,
              tenantSlug: tenantSlug ?? undefined,
              topicId,
              locale,
            })
          : Promise.resolve(initialTopic),
        initialReplies.length === 0
          ? fetchStorefrontReplies({
              tenantId: tenantId ?? undefined,
              tenantSlug: tenantSlug ?? undefined,
              topicId,
              locale,
            })
          : Promise.resolve({ items: initialReplies, total: initialRepliesTotal }),
      ]);

      setCategories(catsData.items);
      if (topicData) setTopic(topicData);
      setReplies(repliesData.items);
      setRepliesTotal(repliesData.total);

      // Collect author IDs for member cards
      const userIds = Array.from(
        new Set(
          [topicData?.authorId, ...repliesData.items.map((r) => r.authorId)].filter(
            (id): id is string => Boolean(id)
          )
        )
      );
      if (userIds.length > 0) {
        const cards = await fetchForumMemberCards({
          userIds,
          locale,
          tenantSlug: tenantSlug ?? undefined,
        });
        setMemberCards(cards);
      }
    } catch {
      // Ignore load error
    }
  }, [tenantId, tenantSlug, locale, topicId, initialTopic, initialReplies, initialRepliesTotal]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleMarkRead = useCallback(async () => {
    if (!topic || isMarkingRead) return;
    setIsMarkingRead(true);
    try {
      await markForumTopicRead({
        tenantId: tenantId ?? undefined,
        tenantSlug: tenantSlug ?? undefined,
        topicId: topic.id,
        locale,
      });
    } catch {
      // Ignore
    } finally {
      setIsMarkingRead(false);
    }
  }, [topic, isMarkingRead, tenantId, tenantSlug, locale]);

  useEffect(() => {
    if (topic?.id) {
      handleMarkRead();
    }
  }, [topic?.id, handleMarkRead]);

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
    return card?.profile.handle || card?.profile.displayName || undefined;
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

  if (!topic) {
    return (
      <div className="rounded-[1.75rem] border border-dashed border-border p-12 text-center">
        <h3 className="text-xl font-semibold text-card-foreground">Topic not found</h3>
        <p className="mt-2 text-sm text-muted-foreground">
          This discussion thread does not exist or may have been deleted.
        </p>
        <Link
          href={`/${locale}/modules/forum`}
          className="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Discussions
        </Link>
      </div>
    );
  }

  return (
    <MemberCardProvider cards={memberCards}>
      <div className="space-y-6">
        {/* Discourse/NodeBB Breadcrumbs & Header Navigation */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-border/60 pb-4">
          <nav className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <Link
              href={`/${locale}/modules/forum`}
              className="inline-flex items-center gap-1.5 font-medium transition hover:text-foreground"
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              Forum
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
              <span>Discussions</span>
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
              {copiedLink ? 'Copied link!' : 'Share'}
            </button>
            <button
              type="button"
              onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
              className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-1.5 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
            >
              <MessageSquare className="h-3.5 w-3.5" />
              Reply
            </button>
          </div>
        </div>

        {/* Main Thread Content & Timeline Scroller */}
        <div className="relative flex gap-6">
          <div className="flex-1 space-y-6">
            {/* Topic Title and Status Pillbox */}
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
                {topic.isPinned && (
                  <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2.5 py-0.5 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                    <Pin className="h-3 w-3" /> Pinned
                  </span>
                )}
                {topic.isLocked && (
                  <span className="inline-flex items-center gap-1 rounded-full bg-destructive/10 px-2.5 py-0.5 text-[11px] font-medium text-destructive">
                    <Lock className="h-3 w-3" /> Locked
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
                    Posted on {new Date(topic.createdAt).toLocaleDateString()}
                  </span>
                </div>

                <div className="flex items-center gap-3 text-xs text-muted-foreground">
                  <span>
                    <strong className="text-foreground">{repliesTotal}</strong> replies
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
              data-revision-id="1"
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
                <button
                  type="button"
                  onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                  className="inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-3 py-1.5 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
                >
                  <ReplyIcon className="h-3.5 w-3.5" />
                  Quote & Reply
                </button>
              </div>
            </article>

            {/* Replies List */}
            {replies.length > 0 && (
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <h3 className="text-lg font-semibold text-foreground">
                    Replies ({repliesTotal})
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
                    data-revision-id="1"
                    className="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm"
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

                    <div className="mt-4 flex items-center justify-end border-t border-border/40 pt-3">
                      <button
                        type="button"
                        onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                        className="inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted hover:text-foreground"
                      >
                        <ReplyIcon className="h-3 w-3" />
                        Reply
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}

            {/* Quick Reply Trigger Bar */}
            <div className="rounded-[1.75rem] border border-dashed border-border bg-muted/20 p-6 text-center">
              <h4 className="text-base font-semibold text-foreground">Join the conversation</h4>
              <p className="mt-1 text-xs text-muted-foreground">
                Highlight text above to quote directly, or click reply below.
              </p>
              <button
                type="button"
                onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                className="mt-4 inline-flex items-center gap-2 rounded-full bg-primary px-5 py-2 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
              >
                <MessageSquare className="h-4 w-4" />
                Reply to this topic
              </button>
            </div>
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
        <QuoteTooltip />
      </div>
    </MemberCardProvider>
  );
}

export function TopicDetailView(props: TopicDetailViewProps) {
  return (
    <ComposerProvider>
      <TopicDetailContent {...props} />
    </ComposerProvider>
  );
}
