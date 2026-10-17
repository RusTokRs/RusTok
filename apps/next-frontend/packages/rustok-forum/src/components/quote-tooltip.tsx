'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { Quote } from 'lucide-react';
import { getClientAuth } from '@/shared/lib/auth';
import { fetchStorefrontQuoteRevision } from '../api/forum';
import { useComposer } from '../context/composer-context';
import { useTranslations } from '@rustok/next-fluent';

interface QuoteTooltipProps {
  tenantId?: string;
  tenantSlug?: string;
  locale: string;
  /** The currently rendered thread. A quote from a closed composer replies here. */
  replyTopic: { id: string; title: string } | null;
}

export function QuoteTooltip({ tenantId, tenantSlug, locale, replyTopic }: QuoteTooltipProps) {
  const { state, openReply, appendQuote, captureEditorSession } = useComposer();
  const forum = useTranslations('Forum');
  const [position, setPosition] = useState<{ top: number; left: number } | null>(null);
  const [selectedText, setSelectedText] = useState('');
  const [textOnlyNotice, setTextOnlyNotice] = useState(false);
  const [isQuoting, setIsQuoting] = useState(false);
  const quoteInFlight = useRef(false);
  const selectionVersionRef = useRef(0);
  const quoteActiveRef = useRef(true);
  useEffect(() => {
    quoteActiveRef.current = true;
    return () => { quoteActiveRef.current = false; };
  }, []);
  const composerTarget = useRef({
    isOpen: state.isOpen, isSubmitting: state.isSubmitting,
    mode: state.mode, topicId: state.topicId, categoryId: state.categoryId,
    parentReplyId: state.parentReplyId,
  });
  composerTarget.current = {
    isOpen: state.isOpen, isSubmitting: state.isSubmitting,
    mode: state.mode, topicId: state.topicId, categoryId: state.categoryId,
    parentReplyId: state.parentReplyId,
  };
  const [targetContext, setTargetContext] = useState<{
    targetKind: 'TOPIC' | 'REPLY';
    targetId: string;
    authorHandle?: string;
  } | null>(null);

  const handleSelectionChange = useCallback(() => {
    selectionVersionRef.current += 1;
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) {
      setPosition(null);
      return;
    }

    const text = selection.toString().trim();
    if (text.length < 2) {
      setPosition(null);
      return;
    }

    const anchorNode = selection.anchorNode;
    const element = anchorNode?.nodeType === Node.ELEMENT_NODE
      ? (anchorNode as HTMLElement)
      : anchorNode?.parentElement;
    const postContainer = element?.closest('[data-forum-post]');
    // Never attach a pinned revision to text that spans multiple posts.
    if (!postContainer || !selection.focusNode || !postContainer.contains(selection.focusNode)) {
      setPosition(null);
      return;
    }

    const targetKind = postContainer.getAttribute('data-target-kind');
    const targetId = postContainer.getAttribute('data-target-id');
    if ((targetKind !== 'TOPIC' && targetKind !== 'REPLY') || !targetId) {
      setPosition(null);
      return;
    }

    try {
      const rect = selection.getRangeAt(0).getBoundingClientRect();
      setPosition({
        top: Math.max(10, rect.top - 38),
        left: rect.left + rect.width / 2,
      });
      setSelectedText(text);
      setTargetContext({
        targetKind,
        targetId,
        authorHandle: postContainer.getAttribute('data-author-handle') || undefined,
      });
    } catch {
      setPosition(null);
    }
  }, []);

  useEffect(() => {
    document.addEventListener('selectionchange', handleSelectionChange);
    return () => document.removeEventListener('selectionchange', handleSelectionChange);
  }, [handleSelectionChange]);

  useEffect(() => {
    if (!textOnlyNotice) return;
    const timeout = window.setTimeout(() => setTextOnlyNotice(false), 5000);
    return () => window.clearTimeout(timeout);
  }, [textOnlyNotice]);

  async function onQuoteClick() {
    if (!targetContext || quoteInFlight.current || state.isSubmitting) return;
    const auth = getClientAuth();
    const parentReplyId = targetContext.targetKind === 'REPLY' ? targetContext.targetId : null;
    const expectedTarget = state.isOpen
      ? { mode: state.mode, topicId: state.topicId, categoryId: state.categoryId, parentReplyId: state.parentReplyId }
      : { mode: 'reply' as const, topicId: replyTopic?.id ?? null, categoryId: null, parentReplyId };
    // Bind the reply before awaiting the revision lookup, even when it falls back to
    // text-only. Keep an already-open composer (including a new-topic draft).
    if (!state.isOpen) {
      if (!replyTopic) return;
      if (!openReply({ topicId: replyTopic.id, topicTitle: replyTopic.title, parentReplyId })) return;
      composerTarget.current = { ...composerTarget.current, ...expectedTarget, isOpen: true };
    }
    const isEditingSession = captureEditorSession();
    // The lookup can finish after the viewer closes or switches composers.
    // Never reopen a closed editor or attach this quote to another draft.
    const stillEditingTarget = () => {
      const currentAuth = getClientAuth();
      const current = composerTarget.current;
      return quoteActiveRef.current && isEditingSession() && currentAuth.token === auth.token && currentAuth.tenant === auth.tenant &&
        current.isOpen && !current.isSubmitting &&
        current.mode === expectedTarget.mode && current.topicId === expectedTarget.topicId &&
        current.categoryId === expectedTarget.categoryId &&
        current.parentReplyId === expectedTarget.parentReplyId;
    };
    quoteInFlight.current = true;
    setIsQuoting(true);
    const { targetKind, targetId, authorHandle } = targetContext;
    const snippet = selectedText;
    const selectionVersion = selectionVersionRef.current;
    try {
      const revisionId = await fetchStorefrontQuoteRevision({
        tenantId,
        tenantSlug,
        token: !tenantSlug || auth.tenant === tenantSlug ? auth.token || undefined : undefined,
        targetKind,
        targetId,
        locale,
      });
      if (stillEditingTarget()) {
        appendQuote(
          revisionId === null
            ? null
            : { targetKind, targetId, revisionId, authorHandle, snippet },
          snippet,
          authorHandle,
        );
        setTextOnlyNotice(revisionId === null);
      }
    } catch {
      // A quote can still be posted as text when an exact revision cannot be verified.
      // Do not fabricate a relation ID or send one to the write owner.
      if (stillEditingTarget()) {
        appendQuote(null, snippet, authorHandle);
        setTextOnlyNotice(true);
      }
    } finally {
      if (quoteActiveRef.current) {
        if (stillEditingTarget() && selectionVersionRef.current === selectionVersion) {
          window.getSelection()?.removeAllRanges();
          setPosition(null);
        }
        setIsQuoting(false);
      }
      quoteInFlight.current = false;
    }
  }

  return (
    <>
      {textOnlyNotice && (
        <p role="status" className="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl bg-foreground px-4 py-2 text-xs text-background shadow-lg">
          {forum('quoteTextOnly')}
        </p>
      )}
      {position && targetContext && (state.isOpen || replyTopic !== null) && (
        <svg className="fixed inset-0 z-50 pointer-events-none h-full w-full overflow-visible">
          <foreignObject
            x={Math.round(position.left - 48)}
            y={Math.round(position.top)}
            width="96"
            height="36"
            className="overflow-visible pointer-events-auto"
          >
            <button
              type="button"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => void onQuoteClick()}
              disabled={isQuoting || state.isSubmitting || (!state.isOpen && !replyTopic)}
              className="flex items-center gap-1.5 rounded-full border border-border bg-foreground px-3 py-1 text-xs font-semibold text-background shadow-lg transition hover:scale-105 active:scale-95 disabled:opacity-60"
            >
              <Quote className="h-3 w-3" />
              {forum('quote')}
            </button>
          </foreignObject>
        </svg>
      )}
    </>
  );
}
