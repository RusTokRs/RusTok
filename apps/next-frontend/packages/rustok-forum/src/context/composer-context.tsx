'use client';

import React, { createContext, useContext, useState, useEffect, useCallback, useMemo } from 'react';
import {
  emptyRichTextDocument,
  type RichTextDocument,
} from '@rustok/richtext';
import type { ForumQuoteReference } from '../api/forum';

export interface ComposerState {
  isOpen: boolean;
  isMinimized: boolean;
  mode: 'reply' | 'topic';
  topicId: string | null;
  topicTitle: string | null;
  categoryId: string | null;
  parentReplyId: string | null;
  title: string;
  document: RichTextDocument;
  quotes: ForumQuoteReference[];
  isSubmitting: boolean;
  error: string | null;
}

export interface ComposerContextValue {
  state: ComposerState;
  openReply: (params: { topicId: string; topicTitle: string; parentReplyId?: string | null }) => void;
  openNewTopic: (params?: { categoryId?: string | null }) => void;
  close: () => void;
  minimize: () => void;
  expand: () => void;
  setTitle: (title: string) => void;
  setDocument: (document: RichTextDocument) => void;
  appendQuote: (quote: ForumQuoteReference, quoteText?: string) => void;
  removeQuote: (index: number) => void;
  setSubmitting: (isSubmitting: boolean) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const ComposerContext = createContext<ComposerContextValue | null>(null);

const STORAGE_KEY_PREFIX = 'rustok_forum_draft_';

function getStorageKey(mode: 'reply' | 'topic', topicId: string | null, categoryId: string | null): string {
  if (mode === 'reply' && topicId) {
    return `${STORAGE_KEY_PREFIX}reply_${topicId}`;
  }
  return `${STORAGE_KEY_PREFIX}topic_${categoryId ?? 'general'}`;
}

export function ComposerProvider({ children }: { children: React.ReactNode }) {
  const [isOpen, setIsOpen] = useState(false);
  const [isMinimized, setIsMinimized] = useState(false);
  const [mode, setMode] = useState<'reply' | 'topic'>('reply');
  const [topicId, setTopicId] = useState<string | null>(null);
  const [topicTitle, setTopicTitle] = useState<string | null>(null);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [parentReplyId, setParentReplyId] = useState<string | null>(null);
  const [title, setTitle] = useState('');
  const [document, setDocument] = useState<RichTextDocument>(emptyRichTextDocument());
  const [quotes, setQuotes] = useState<ForumQuoteReference[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load draft from localStorage when opening
  const loadDraft = useCallback((draftMode: 'reply' | 'topic', tId: string | null, cId: string | null) => {
    if (typeof window === 'undefined') return;
    try {
      const key = getStorageKey(draftMode, tId, cId);
      const saved = localStorage.getItem(key);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (parsed.document) setDocument(parsed.document);
        if (parsed.title) setTitle(parsed.title);
        if (Array.isArray(parsed.quotes)) setQuotes(parsed.quotes);
      }
    } catch {
      // Ignore localStorage errors
    }
  }, []);

  // Save draft to localStorage
  useEffect(() => {
    if (!isOpen || typeof window === 'undefined') return;
    const timeout = setTimeout(() => {
      try {
        const key = getStorageKey(mode, topicId, categoryId);
        const data = { title, document, quotes };
        localStorage.setItem(key, JSON.stringify(data));
      } catch {
        // Ignore localStorage quota errors
      }
    }, 600);
    return () => clearTimeout(timeout);
  }, [isOpen, mode, topicId, categoryId, title, document, quotes]);

  const clearDraft = useCallback((draftMode: 'reply' | 'topic', tId: string | null, cId: string | null) => {
    if (typeof window === 'undefined') return;
    try {
      const key = getStorageKey(draftMode, tId, cId);
      localStorage.removeItem(key);
    } catch {
      // Ignore
    }
  }, []);

  const openReply = useCallback((params: { topicId: string; topicTitle: string; parentReplyId?: string | null }) => {
    setMode('reply');
    setTopicId(params.topicId);
    setTopicTitle(params.topicTitle);
    setParentReplyId(params.parentReplyId ?? null);
    setTitle('');
    setQuotes([]);
    setError(null);
    setDocument(emptyRichTextDocument());
    loadDraft('reply', params.topicId, null);
    setIsOpen(true);
    setIsMinimized(false);
  }, [loadDraft]);

  const openNewTopic = useCallback((params?: { categoryId?: string | null }) => {
    const cId = params?.categoryId ?? null;
    setMode('topic');
    setTopicId(null);
    setTopicTitle(null);
    setCategoryId(cId);
    setParentReplyId(null);
    setTitle('');
    setQuotes([]);
    setError(null);
    setDocument(emptyRichTextDocument());
    loadDraft('topic', null, cId);
    setIsOpen(true);
    setIsMinimized(false);
  }, [loadDraft]);

  const close = useCallback(() => {
    setIsOpen(false);
    setIsMinimized(false);
  }, []);

  const minimize = useCallback(() => {
    setIsMinimized(true);
  }, []);

  const expand = useCallback(() => {
    setIsMinimized(false);
  }, []);

  const reset = useCallback(() => {
    clearDraft(mode, topicId, categoryId);
    setTitle('');
    setDocument(emptyRichTextDocument());
    setQuotes([]);
    setError(null);
    setIsOpen(false);
    setIsMinimized(false);
  }, [clearDraft, mode, topicId, categoryId]);

  const appendQuote = useCallback((quote: ForumQuoteReference, quoteText?: string) => {
    setQuotes((prev) => {
      // Prevent duplicate quotes
      if (prev.some((q) => q.targetId === quote.targetId && q.targetKind === quote.targetKind)) {
        return prev;
      }
      return [...prev, quote];
    });

    if (quoteText) {
      setDocument((prev) => {
        const textToInsert = quote.authorHandle
          ? `@${quote.authorHandle}: "${quoteText}"`
          : `"${quoteText}"`;

        const quoteNode = {
          type: 'blockquote',
          content: [
            {
              type: 'paragraph',
              content: [{ type: 'text', text: textToInsert }],
            },
          ],
        };

        const responseParagraph = {
          type: 'paragraph',
        };

        return {
          type: 'doc',
          content: [...prev.content, quoteNode, responseParagraph],
        };
      });
    }

    setIsOpen(true);
    setIsMinimized(false);
  }, []);

  const removeQuote = useCallback((index: number) => {
    setQuotes((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const state = useMemo<ComposerState>(() => ({
    isOpen,
    isMinimized,
    mode,
    topicId,
    topicTitle,
    categoryId,
    parentReplyId,
    title,
    document,
    quotes,
    isSubmitting,
    error,
  }), [
    isOpen,
    isMinimized,
    mode,
    topicId,
    topicTitle,
    categoryId,
    parentReplyId,
    title,
    document,
    quotes,
    isSubmitting,
    error,
  ]);

  const value = useMemo<ComposerContextValue>(() => ({
    state,
    openReply,
    openNewTopic,
    close,
    minimize,
    expand,
    setTitle,
    setDocument,
    appendQuote,
    removeQuote,
    setSubmitting: setIsSubmitting,
    setError,
    reset,
  }), [
    state,
    openReply,
    openNewTopic,
    close,
    minimize,
    expand,
    appendQuote,
    removeQuote,
    reset,
  ]);

  return (
    <ComposerContext.Provider value={value}>
      {children}
    </ComposerContext.Provider>
  );
}

export function useComposer() {
  const context = useContext(ComposerContext);
  if (!context) {
    throw new Error('useComposer must be used within a ComposerProvider');
  }
  return context;
}
