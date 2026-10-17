'use client';

import React, { useContext, useEffect } from 'react';
import { RichTextHtml } from '@rustok/richtext/view';
import { CheckCheck, MessageSquare, Reply as ReplyIcon, Pin, Lock } from 'lucide-react';
import type { ForumTopicDetail, ForumReplyDetail } from '../api/forum';
import { AuthorBadge, MemberCardContext } from './member-card';
import { TimelineScroller } from './timeline-scroller';
import { useComposer } from '../context/composer-context';
import { TopicStatusLabel } from './topic-status-label';
import { canReplyToTopic } from './topic-reply-availability';
import { focusForumReplyPost } from './reply-anchor';
import { useTranslations } from '@rustok/next-fluent';
import { threadPostTotal, type ThreadJump, type ThreadPostTarget } from './thread-navigation';
import { useThreadNavigation } from './use-thread-navigation';

interface ThreadPanelProps {
  topic: ForumTopicDetail | null;
  replies: ForumReplyDetail[];
  repliesTotal: number;
  onMarkRead?: (topicId: string) => void;
  isMarkingRead?: boolean;
  hasMoreReplies?: boolean;
  isLoadingReplies?: boolean;
  onLoadMoreReplies?: () => void;
  onNavigateReplies?: (target: ThreadPostTarget) => Promise<ThreadJump | null>;
  /** Reply id the incoming link addressed; scrolled to and marked once posts render. */
  focusReplyId?: string | null;
  /** The linked reply stayed outside the loaded pages. */
  focusReplyMissing?: boolean;
  /** Reports that the anchor was applied, so re-renders do not scroll again. */
  onFocusHandled?: () => void;
}

export function ThreadPanel({
  topic,
  replies,
  repliesTotal,
  onMarkRead,
  isMarkingRead = false,
  hasMoreReplies = false,
  isLoadingReplies = false,
  onLoadMoreReplies,
  onNavigateReplies,
  focusReplyId = null,
  focusReplyMissing = false,
  onFocusHandled,
}: ThreadPanelProps) {
  const forum = useTranslations('Forum');
  const { openReply } = useComposer();
  const memberCards = useContext(MemberCardContext);
  const navigation = useThreadNavigation({
    scopeKey: topic?.id ?? '', replies,
    hasMore: hasMoreReplies, loadReplies: onNavigateReplies,
  });

  // A deep link addressed one reply: scroll to it once the posts are rendered. A reply outside
  // the loaded pages is reported below rather than left to look like the whole thread.
  useEffect(() => {
    if (!focusReplyId) {
      return undefined;
    }
    const clearHighlight = focusForumReplyPost(focusReplyId);
    if (clearHighlight) {
      onFocusHandled?.();
    }
    return clearHighlight ?? undefined;
  }, [focusReplyId, replies, onFocusHandled]);

  if (!topic) {
    return (
      <aside className="rounded-[1.75rem] border border-dashed border-border p-8 text-center xl:sticky xl:top-6 xl:self-start">
        <h3 className="text-lg font-semibold text-card-foreground">{forum('openAThread')}</h3>
        <p className="mt-2 text-sm text-muted-foreground">
          {forum('openAThreadHint')}
        </p>
      </aside>
    );
  }

  const canReply = canReplyToTopic(topic);

  return (
    <aside className="relative flex gap-3 xl:sticky xl:top-6 xl:self-start">
      <div ref={navigation.postsRef} className="flex-1 space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm overflow-hidden">
        {/* Topic Header & Meta */}
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

          <div>
            <h3 className="text-2xl font-semibold text-card-foreground">
              {topic.title}
            </h3>
            <p className="mt-1 text-xs text-muted-foreground" dir="ltr">
              t/{topic.slug}
            </p>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
            <AuthorBadge authorId={topic.authorId} />

            <div className="flex items-center gap-2">
              {onMarkRead && (
                <button
                  type="button"
                  onClick={() => onMarkRead(topic.id)}
                  disabled={isMarkingRead}
                  className="inline-flex items-center gap-1.5 rounded-xl border border-primary/30 bg-primary/5 px-3 py-1.5 text-xs font-semibold text-primary transition hover:bg-primary/10 disabled:opacity-50"
                >
                  <CheckCheck className="h-3.5 w-3.5" />
                  {isMarkingRead ? forum('markingRead') : forum('markRead')}
                </button>
              )}

              {canReply ? (
                <button
                  type="button"
                  onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                  className="inline-flex items-center gap-1.5 rounded-xl bg-primary px-3.5 py-1.5 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
                >
                  <MessageSquare className="h-3.5 w-3.5" />
                  {forum('reply')}
                </button>
              ) : <span className="text-xs text-muted-foreground">{forum('repliesClosed')}</span>}
            </div>
          </div>

          {/* Opening post body */}
          <div
            data-forum-post
            data-thread-post-number={1}
            data-target-kind="TOPIC"
            data-target-id={topic.id}
            data-author-handle={topic.authorId ? memberCards.get(topic.authorId)?.profile.handle : undefined}
            className="rounded-2xl border border-border/80 bg-background/60 p-4"
          >
            <RichTextHtml
              view={topic.body}
              contentLocale={topic.effectiveLocale}
              className="richtext text-sm leading-7 text-foreground"
            />
          </div>

          {/* Tags */}
          {topic.tags.length > 0 && (
            <div className="flex flex-wrap gap-1.5 pt-1">
              {topic.tags.map((tag) => (
                <span
                  key={tag}
                  className="rounded-full border border-border px-2.5 py-0.5 text-xs text-muted-foreground"
                >
                  #{tag}
                </span>
              ))}
            </div>
          )}
        </div>

        {/* Replies Section */}
        <div className="space-y-3 pt-2">
          {focusReplyMissing && (
            <p className="rounded-xl border border-dashed border-border px-4 py-2 text-center text-[11px] text-muted-foreground">
              {forum('linkedPostUnavailable')}
            </p>
          )}
          <div className="flex items-center justify-between border-t border-border pt-4">
            <h4 className="text-sm font-semibold text-foreground">{forum('replies')}</h4>
            <span className="text-xs text-muted-foreground">
              {repliesTotal} {forum('total')}
            </span>
          </div>

          {replies.length === 0 ? (
            <p className="rounded-xl border border-dashed border-border p-5 text-center text-xs text-muted-foreground">
              {canReply ? forum('noRepliesYet') : forum('repliesClosed')}
            </p>
          ) : (
            <div className="space-y-3">
              {replies.map((reply, index) => (
                <article
                  key={reply.id}
                  id={`reply-${index}`}
                  data-forum-post
                  data-thread-post-number={index + 2}
                  data-target-kind="REPLY"
                  data-target-id={reply.id}
                  data-author-handle={reply.authorId ? memberCards.get(reply.authorId)?.profile.handle : undefined}
                        className="rounded-[1.25rem] border border-border bg-card p-4 transition hover:border-border/80 data-[focused=true]:border-primary/60 data-[focused=true]:ring-2 data-[focused=true]:ring-primary/40"
                >
                  <div className="flex items-center justify-between gap-3">
                    <AuthorBadge authorId={reply.authorId} />

                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-muted-foreground">
                        #{index + 2}
                      </span>
                      {canReply && (
                        <button
                          type="button"
                          onClick={() => openReply({
                            topicId: topic.id,
                            topicTitle: topic.title,
                            parentReplyId: reply.id,
                          })}
                          className="rounded-lg p-1 text-muted-foreground transition hover:bg-muted hover:text-foreground"
                          title={forum('replyToMessage')}
                        >
                          <ReplyIcon className="h-3.5 w-3.5" />
                        </button>
                      )}
                    </div>
                  </div>

                  <div className="mt-3">
                    <RichTextHtml
                      view={reply.content}
                      contentLocale={reply.effectiveLocale}
                      className="richtext text-sm leading-6 text-foreground"
                    />
                  </div>
                </article>
              ))}
            </div>
          )}
          {navigation.navigationIncomplete && (
            <p role="status" className="text-xs text-muted-foreground">{forum('morePostsToEnd')}</p>
          )}
          {hasMoreReplies && onLoadMoreReplies && (
            <button
              type="button"
              onClick={onLoadMoreReplies}
              disabled={isLoadingReplies}
              className="w-full rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground transition hover:bg-muted disabled:opacity-50"
            >
              {isLoadingReplies ? forum('loading') : forum('loadMoreReplies')}
            </button>
          )}
        </div>
      </div>

      {/* Discourse Timeline Scroller on right */}
      <TimelineScroller
        currentPost={navigation.currentPost}
        totalPosts={threadPostTotal(replies.length, repliesTotal, hasMoreReplies)}
        onJumpToPost={navigation.jumpToPost}
        isNavigating={navigation.isNavigating || isLoadingReplies}
      />
    </aside>
  );
}
