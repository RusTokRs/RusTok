'use client';

import React, { useRef } from 'react';
import { RichTextHtml } from '@rustok/richtext/view';
import { CheckCheck, MessageSquare, Reply as ReplyIcon, Pin, Lock } from 'lucide-react';
import type { ForumTopicDetail, ForumReplyDetail } from '../api/forum';
import { AuthorBadge } from './member-card';
import { TimelineScroller } from './timeline-scroller';
import { useComposer } from '../context/composer-context';

interface ThreadPanelProps {
  topic: ForumTopicDetail | null;
  replies: ForumReplyDetail[];
  repliesTotal: number;
  onMarkRead?: (topicId: string) => void;
  isMarkingRead?: boolean;
}

export function ThreadPanel({
  topic,
  replies,
  repliesTotal,
  onMarkRead,
  isMarkingRead = false,
}: ThreadPanelProps) {
  const { openReply } = useComposer();
  const repliesContainerRef = useRef<HTMLDivElement>(null);

  if (!topic) {
    return (
      <aside className="rounded-[1.75rem] border border-dashed border-border p-8 text-center xl:sticky xl:top-6 xl:self-start">
        <h3 className="text-lg font-semibold text-card-foreground">Open a thread</h3>
        <p className="mt-2 text-sm text-muted-foreground">
          Pick a topic from the feed to read the opening post and latest replies.
        </p>
      </aside>
    );
  }

  function handleJumpToPost(postNumber: number) {
    if (postNumber === 1) {
      window.scrollTo({ top: 0, behavior: 'smooth' });
    } else {
      const el = document.getElementById(`reply-${postNumber - 2}`);
      el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  }

  return (
    <aside className="relative flex gap-3 xl:sticky xl:top-6 xl:self-start">
      <div className="flex-1 space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm overflow-hidden">
        {/* Topic Header & Meta */}
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
                  {isMarkingRead ? 'Marking read…' : 'Mark read'}
                </button>
              )}

              <button
                type="button"
                onClick={() => openReply({ topicId: topic.id, topicTitle: topic.title })}
                className="inline-flex items-center gap-1.5 rounded-xl bg-primary px-3.5 py-1.5 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
              >
                <MessageSquare className="h-3.5 w-3.5" />
                Reply
              </button>
            </div>
          </div>

          {/* Opening post body */}
          <div
            data-forum-post
            data-target-kind="TOPIC"
            data-target-id={topic.id}
            data-revision-id="1"
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
        <div ref={repliesContainerRef} className="space-y-3 pt-2">
          <div className="flex items-center justify-between border-t border-border pt-4">
            <h4 className="text-sm font-semibold text-foreground">Replies</h4>
            <span className="text-xs text-muted-foreground">
              {repliesTotal} total
            </span>
          </div>

          {replies.length === 0 ? (
            <p className="rounded-xl border border-dashed border-border p-5 text-center text-xs text-muted-foreground">
              No replies yet. Be the first to join the conversation!
            </p>
          ) : (
            <div className="space-y-3">
              {replies.map((reply, index) => (
                <article
                  key={reply.id}
                  id={`reply-${index}`}
                  data-forum-post
                  data-target-kind="REPLY"
                  data-target-id={reply.id}
                  data-revision-id="1"
                  className="rounded-[1.25rem] border border-border bg-card p-4 transition hover:border-border/80"
                >
                  <div className="flex items-center justify-between gap-3">
                    <AuthorBadge authorId={reply.authorId} />

                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-muted-foreground">
                        #{index + 1}
                      </span>
                      <button
                        type="button"
                        onClick={() =>
                          openReply({
                            topicId: topic.id,
                            topicTitle: topic.title,
                            parentReplyId: reply.id,
                          })
                        }
                        className="rounded-lg p-1 text-muted-foreground transition hover:bg-muted hover:text-foreground"
                        title="Reply to this message"
                      >
                        <ReplyIcon className="h-3.5 w-3.5" />
                      </button>
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
        </div>
      </div>

      {/* Discourse Timeline Scroller on right */}
      <TimelineScroller
        currentPost={1}
        totalPosts={repliesTotal + 1}
        onJumpToPost={handleJumpToPost}
      />
    </aside>
  );
}
