'use client';

import React, { useEffect, useMemo, useState } from 'react';
import {
  richTextDocumentHasText,
  type RichTextDocument,
} from '@rustok/richtext';
import { RichTextEditor } from '@rustok/richtext/react';
import { useTranslations } from '@rustok/next-fluent';
import {
  X,
  Minus,
  Maximize2,
  Send,
  MessageSquare,
  Quote,
  Loader2,
  FileEdit,
} from 'lucide-react';
import { useComposer } from '../context/composer-context';
import {
  createForumReply,
  createForumTopic,
  type ForumCategoryListItem,
} from '../api/forum';

interface ComposerProps {
  tenantId?: string;
  tenantSlug?: string;
  locale: string;
  categories?: ForumCategoryListItem[];
  onSubmitted?: (targetTopicId: string) => void;
}

export function Composer({
  tenantId,
  tenantSlug,
  locale,
  categories = [],
  onSubmitted,
}: ComposerProps) {
  const {
    state,
    close,
    minimize,
    expand,
    setTitle,
    setDocument,
    removeQuote,
    setSubmitting,
    setError,
    reset,
  } = useComposer();

  const [activeTab, setActiveTab] = useState<'write' | 'preview'>('write');
  const [selectedCategoryId, setSelectedCategoryId] = useState<string>(
    state.categoryId ?? categories[0]?.id ?? ''
  );

  useEffect(() => {
    if (state.categoryId) {
      setSelectedCategoryId(state.categoryId);
    } else if (!selectedCategoryId && categories.length > 0) {
      setSelectedCategoryId(categories[0].id);
    }
  }, [state.categoryId, categories, selectedCategoryId]);

  const richText = useTranslations('richText');

  const richTextMessages = useMemo(
    () => ({
      bold: richText('bold'),
      italic: richText('italic'),
      strike: richText('strike'),
      code: richText('code'),
      heading: richText('heading'),
      bullet_list: richText('bullet_list'),
      ordered_list: richText('ordered_list'),
      blockquote: richText('blockquote'),
      code_block: richText('code_block'),
      horizontal_rule: richText('horizontal_rule'),
      link: richText('link'),
      link_url: richText('link_url'),
      apply_link: richText('apply_link'),
      remove_link: richText('remove_link'),
      clear_formatting: richText('clear_formatting'),
      undo: richText('undo'),
      redo: richText('redo'),
      editor: richText('editor'),
    }),
    [richText]
  );

  if (!state.isOpen) return null;

  // Minimized drawer (Floating pill)
  if (state.isMinimized) {
    return (
      <div className="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-full border border-border bg-card px-4 py-2.5 shadow-xl transition-all hover:bg-muted/80">
        <MessageSquare className="h-4 w-4 text-primary" />
        <button
          type="button"
          onClick={expand}
          className="text-xs font-medium text-foreground hover:underline"
        >
          {state.mode === 'reply'
            ? `Draft: ${state.topicTitle ?? 'Reply'}`
            : `Draft: ${state.title || 'New Topic'}`}
        </button>
        <button
          type="button"
          onClick={expand}
          className="rounded-full p-1 text-muted-foreground hover:bg-background hover:text-foreground"
          title="Expand"
        >
          <Maximize2 className="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          onClick={close}
          className="rounded-full p-1 text-muted-foreground hover:bg-background hover:text-foreground"
          title="Close"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
    );
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();

    if (!richTextDocumentHasText(state.document)) {
      setError('Please enter your message before posting.');
      return;
    }

    if (state.mode === 'topic' && !state.title.trim()) {
      setError('Please provide a topic title.');
      return;
    }

    setSubmitting(true);
    setError(null);

    try {
      if (state.mode === 'reply' && state.topicId) {
        const replyResult = await createForumReply({
          tenantId,
          tenantSlug,
          topicId: state.topicId,
          input: {
            locale,
            content: state.document,
            parentReplyId: state.parentReplyId,
            quotes: state.quotes.map((q) => ({
              targetKind: q.targetKind,
              targetId: q.targetId,
              revisionId: q.revisionId,
            })),
          },
        });

        if (replyResult) {
          const tId = state.topicId;
          reset();
          onSubmitted?.(tId);
        } else {
          setError('Failed to create reply.');
        }
      } else if (state.mode === 'topic') {
        const cId = state.categoryId || selectedCategoryId || categories[0]?.id;
        if (!cId) {
          setError('Please select a category.');
          setSubmitting(false);
          return;
        }

        const topicResult = await createForumTopic({
          tenantId,
          tenantSlug,
          input: {
            locale,
            categoryId: cId,
            title: state.title.trim(),
            body: state.document,
            tags: [],
            quotes: state.quotes.map((q) => ({
              targetKind: q.targetKind,
              targetId: q.targetId,
              revisionId: q.revisionId,
            })),
          },
        });

        if (topicResult) {
          const createdTopicId = topicResult.id;
          reset();
          onSubmitted?.(createdTopicId);
        } else {
          setError('Failed to create topic.');
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred while posting.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="fixed inset-x-0 bottom-0 z-50 flex justify-center p-2 sm:p-4 pointer-events-none">
      <div className="pointer-events-auto flex max-h-[85vh] w-full max-w-4xl flex-col overflow-hidden rounded-t-2xl sm:rounded-2xl border border-border bg-card shadow-2xl transition-all">
        {/* Header bar */}
        <div className="flex items-center justify-between border-b border-border bg-muted/40 px-4 py-3">
          <div className="flex items-center gap-2">
            <span className="flex h-6 w-6 items-center justify-center rounded-full bg-primary/10 text-primary">
              <FileEdit className="h-3.5 w-3.5" />
            </span>
            <span className="text-sm font-semibold text-foreground">
              {state.mode === 'reply'
                ? `Replying to: ${state.topicTitle ?? 'Topic'}`
                : 'Create New Topic'}
            </span>
          </div>

          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={minimize}
              className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-background hover:text-foreground"
              title="Minimize"
            >
              <Minus className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={close}
              className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-background hover:text-foreground"
              title="Close"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="flex flex-1 flex-col overflow-hidden">
          {/* New Topic Metadata Inputs */}
          {state.mode === 'topic' && (
            <div className="grid gap-3 border-b border-border p-4 sm:grid-cols-[1fr_200px]">
              <input
                type="text"
                placeholder="Topic title..."
                value={state.title}
                onChange={(e) => setTitle(e.target.value)}
                className="rounded-xl border border-border bg-background px-3.5 py-2 text-sm font-medium text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
                required
              />
              <select
                value={selectedCategoryId}
                onChange={(e) => setSelectedCategoryId(e.target.value)}
                className="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
              >
                {categories.map((cat) => (
                  <option key={cat.id} value={cat.id}>
                    {cat.name}
                  </option>
                ))}
              </select>
            </div>
          )}

          {/* Attached quotes pills */}
          {state.quotes.length > 0 && (
            <div className="flex flex-wrap gap-2 border-b border-border bg-muted/20 px-4 py-2">
              <span className="inline-flex items-center gap-1 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                <Quote className="h-3 w-3" /> Quotes:
              </span>
              {state.quotes.map((quote, idx) => (
                <span
                  key={`${quote.targetId}-${idx}`}
                  className="inline-flex items-center gap-1.5 rounded-full border border-border bg-background px-2.5 py-0.5 text-xs text-foreground"
                >
                  <span className="max-w-[150px] truncate text-muted-foreground">
                    {quote.authorHandle ? `@${quote.authorHandle}` : `#${quote.targetId.slice(0, 6)}`}
                  </span>
                  <button
                    type="button"
                    onClick={() => removeQuote(idx)}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </span>
              ))}
            </div>
          )}

          {/* Editor / Preview Tabs */}
          <div className="flex items-center gap-2 border-b border-border px-4 py-1.5 text-xs">
            <button
              type="button"
              onClick={() => setActiveTab('write')}
              className={`rounded-lg px-3 py-1 font-medium transition ${
                activeTab === 'write'
                  ? 'bg-primary/10 text-primary'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              Write
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('preview')}
              className={`rounded-lg px-3 py-1 font-medium transition ${
                activeTab === 'preview'
                  ? 'bg-primary/10 text-primary'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              Preview
            </button>
          </div>

          {/* Editor content container */}
          <div className="flex-1 overflow-y-auto p-4 min-h-[180px]">
            {activeTab === 'write' ? (
              <RichTextEditor
                frameUrl="/richtext/frame"
                label={state.mode === 'reply' ? 'Reply content' : 'Topic content'}
                profile="discussion"
                value={state.document}
                onChange={setDocument}
                messages={richTextMessages}
                contentLocale={locale}
                disabled={state.isSubmitting}
                className="min-h-[160px]"
              />
            ) : (
              <div className="prose dark:prose-invert max-w-none text-sm text-foreground">
                <p className="italic text-muted-foreground">Live preview of your post:</p>
                <div className="mt-2 rounded-xl border border-dashed border-border p-4">
                  {state.quotes.length > 0 && (
                    <div className="mb-3 space-y-2">
                      {state.quotes.map((q, i) => (
                        <blockquote key={i} className="border-l-2 border-primary/40 pl-3 italic text-muted-foreground">
                          {q.snippet ?? `Quote from ${q.authorHandle ?? 'author'}`}
                        </blockquote>
                      ))}
                    </div>
                  )}
                  {richTextDocumentHasText(state.document) ? (
                    <div className="space-y-2 whitespace-pre-wrap text-foreground">
                      {state.document.content.map((node, i) => {
                        const text = (node.content ?? [])
                          .map((child) => child.text ?? '')
                          .join('');
                        return text ? <p key={i}>{text}</p> : null;
                      })}
                    </div>
                  ) : (
                    <span className="text-muted-foreground">Nothing to preview yet.</span>
                  )}
                </div>
              </div>
            )}
          </div>

          {/* Error notice */}
          {state.error && (
            <div className="border-t border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive">
              {state.error}
            </div>
          )}

          {/* Footer action bar */}
          <div className="flex items-center justify-between border-t border-border bg-muted/20 px-4 py-3">
            <span className="text-xs text-muted-foreground">
              Draft saved locally
            </span>

            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={close}
                disabled={state.isSubmitting}
                className="rounded-xl px-4 py-2 text-xs font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={state.isSubmitting}
                className="inline-flex items-center gap-2 rounded-xl bg-primary px-5 py-2 text-xs font-semibold text-primary-foreground shadow transition hover:bg-primary/90 disabled:opacity-60"
              >
                {state.isSubmitting ? (
                  <>
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    Posting…
                  </>
                ) : (
                  <>
                    <Send className="h-3.5 w-3.5" />
                    {state.mode === 'reply' ? 'Post Reply' : 'Create Topic'}
                  </>
                )}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
