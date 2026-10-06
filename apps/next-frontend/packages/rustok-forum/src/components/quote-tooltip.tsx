'use client';

import React, { useEffect, useState, useCallback } from 'react';
import { Quote } from 'lucide-react';
import { useComposer } from '../context/composer-context';

export function QuoteTooltip() {
  const { appendQuote } = useComposer();
  const [position, setPosition] = useState<{ top: number; left: number } | null>(null);
  const [selectedText, setSelectedText] = useState('');
  const [targetContext, setTargetContext] = useState<{
    targetKind: 'TOPIC' | 'REPLY';
    targetId: string;
    revisionId: number;
    authorHandle?: string;
  } | null>(null);

  const handleSelectionChange = useCallback(() => {
    if (typeof window === 'undefined') return;
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) {
      setPosition(null);
      return;
    }

    const text = selection.toString().trim();
    if (!text || text.length < 2) {
      setPosition(null);
      return;
    }

    // Check if selection is within forum content
    const anchorNode = selection.anchorNode;
    if (!anchorNode) {
      setPosition(null);
      return;
    }

    const element = anchorNode.nodeType === Node.ELEMENT_NODE
      ? (anchorNode as HTMLElement)
      : anchorNode.parentElement;

    const postContainer = element?.closest('[data-forum-post]');
    if (!postContainer) {
      setPosition(null);
      return;
    }

    const targetKind = (postContainer.getAttribute('data-target-kind') as 'TOPIC' | 'REPLY') || 'REPLY';
    const targetId = postContainer.getAttribute('data-target-id') || '';
    const revisionId = parseInt(postContainer.getAttribute('data-revision-id') || '1', 10);
    const authorHandle = postContainer.getAttribute('data-author-handle') || undefined;

    if (!targetId) {
      setPosition(null);
      return;
    }

    try {
      const range = selection.getRangeAt(0);
      const rect = range.getBoundingClientRect();

      setPosition({
        top: Math.max(10, rect.top - 38),
        left: rect.left + rect.width / 2,
      });
      setSelectedText(text);
      setTargetContext({
        targetKind,
        targetId,
        revisionId,
        authorHandle,
      });
    } catch {
      setPosition(null);
    }
  }, []);

  useEffect(() => {
    document.addEventListener('selectionchange', handleSelectionChange);
    return () => {
      document.removeEventListener('selectionchange', handleSelectionChange);
    };
  }, [handleSelectionChange]);

  if (!position || !targetContext) return null;

  function onQuoteClick() {
    if (!targetContext) return;
    appendQuote(
      {
        targetKind: targetContext.targetKind,
        targetId: targetContext.targetId,
        revisionId: targetContext.revisionId,
        authorHandle: targetContext.authorHandle,
        snippet: selectedText,
      },
      selectedText
    );
    window.getSelection()?.removeAllRanges();
    setPosition(null);
  }

  return (
    <div
      style={{
        top: `${position.top}px`,
        left: `${position.left}px`,
      }}
      className="fixed z-50 -translate-x-1/2 animate-in fade-in zoom-in-95 duration-150"
    >
      <button
        type="button"
        onMouseDown={(e) => e.preventDefault()}
        onClick={onQuoteClick}
        className="flex items-center gap-1.5 rounded-full border border-border bg-foreground px-3 py-1 text-xs font-semibold text-background shadow-lg transition hover:scale-105 active:scale-95"
      >
        <Quote className="h-3 w-3" />
        Quote
      </button>
    </div>
  );
}
