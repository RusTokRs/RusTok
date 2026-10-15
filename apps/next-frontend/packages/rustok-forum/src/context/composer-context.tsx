'use client';

import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import {
  emptyRichTextDocument,
  type RichTextDocument,
} from '@rustok/richtext';
import type { ForumQuoteReference } from '../api/forum';
import { getClientAuth } from '@/shared/lib/auth';
import { getDraftIdentity, getDraftStorageKey, isCurrentDraftSession, parseTopicDraft, switchTopicDraft, type DraftIdentity, type DraftScope } from './draft-storage';
import { useForumAuth } from '../components/use-forum-auth';

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
  /** Null when this viewer has no safe local draft namespace. */
  draftStorage: 'pending' | 'saved' | 'failed' | null;
}

export type DraftSelectionResult = 'moved' | 'restored' | 'failed';

export interface ComposerContextValue {
  state: ComposerState;
  openReply: (params: { topicId: string; topicTitle: string; parentReplyId?: string | null }) => void;
  openNewTopic: (params?: { categoryId?: string | null }) => void;
  selectCategory: (categoryId: string) => DraftSelectionResult;
  close: () => void;
  minimize: () => void;
  expand: () => void;
  setTitle: (title: string) => void;
  setDocument: (document: RichTextDocument) => void;
  appendQuote: (quote: ForumQuoteReference | null, quoteText: string, authorHandle?: string) => void;
  removeQuote: (index: number) => void;
  setSubmitting: (isSubmitting: boolean) => void;
  setError: (error: string | null) => void;
  reset: () => void;
  isCurrentViewer: () => boolean;
}

const ComposerContext = createContext<ComposerContextValue | null>(null);

/** Remounts the session state on a viewer/tenant switch, before old drafts can
 * render to or be saved under another viewer. Unscoped historical drafts are
 * intentionally neither read nor silently migrated to a new account. */
export function ComposerProvider({ children, tenantId, tenantSlug }: {
  children: React.ReactNode; tenantId?: string | null; tenantSlug?: string | null;
}) {
  const auth = useForumAuth();
  const [identity, setIdentity] = useState<{
    token: string; tenant: string; draft: DraftIdentity | null;
  } | null>(null);

  useEffect(() => {
    const sync = () => {
      const current = getClientAuth();
      const draft = getDraftIdentity(current.token);
      setIdentity((previous) => previous?.token === current.token &&
        previous.tenant === current.tenant && previous.draft?.userId === draft?.userId &&
        previous.draft?.tenantId === draft?.tenantId
        ? previous : { token: current.token, tenant: current.tenant, draft });
    };
    const onVisible = () => { if (!document.hidden) sync(); };
    sync();
    window.addEventListener('focus', sync);
    window.addEventListener('pageshow', sync);
    window.addEventListener('storage', sync);
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      window.removeEventListener('focus', sync);
      window.removeEventListener('pageshow', sync);
      window.removeEventListener('storage', sync);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, []);

  const tenant = tenantSlug ?? tenantId ?? auth?.tenant ?? '';
  const signedIn = Boolean(auth?.token && (!tenantSlug || auth.tenant === tenantSlug));
  const viewer = signedIn && identity?.token === auth?.token && identity?.tenant === auth?.tenant
    ? identity : null;
  const scope = useMemo<DraftScope | null>(
    () => viewer?.draft ? { tenant, ...viewer.draft } : null,
    [tenant, viewer?.draft]
  );
  const sessionKey = JSON.stringify([tenant, auth?.tenant, auth?.token, viewer?.draft?.tenantId, viewer?.draft?.userId]);

  return (
    <ComposerSession key={sessionKey} scope={scope} sessionToken={auth?.token ?? ''} sessionTenant={auth?.tenant ?? ''}>
      {children}
    </ComposerSession>
  );
}

function ComposerSession({ children, scope, sessionToken, sessionTenant }: {
  children: React.ReactNode; scope: DraftScope | null;
  sessionToken: string; sessionTenant: string;
}) {
  const isCurrentViewer = useCallback(() => {
    const current = getClientAuth();
    return isCurrentDraftSession(
      { token: sessionToken, tenant: sessionTenant, scope },
      { ...current, identity: scope ? getDraftIdentity(current.token) : null }
    );
  }, [sessionToken, sessionTenant, scope]);

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
  const [draftStorageStatus, setDraftStorageStatus] = useState<'pending' | 'saved' | 'failed'>('pending');

  // Load draft from localStorage when opening
  const loadDraft = useCallback((draftMode: 'reply' | 'topic', tId: string | null, cId: string | null) => {
    if (!scope || !isCurrentViewer()) return;
    try {
      const key = getDraftStorageKey(scope, draftMode, tId, cId);
      const saved = localStorage.getItem(key);
      if (saved) {
        if (draftMode === 'topic') {
          const parsed = parseTopicDraft(saved);
          setDocument(parsed.document);
          setTitle(parsed.title);
          setQuotes(parsed.quotes);
        } else {
          const parsed = JSON.parse(saved);
          if (parsed.document) setDocument(parsed.document);
          if (parsed.title) setTitle(parsed.title);
          if (Array.isArray(parsed.quotes)) setQuotes(parsed.quotes);
        }
      }
    } catch {
      // Ignore localStorage errors
    }
  }, [scope, isCurrentViewer]);

  const persistDraft = useCallback((): boolean => {
    if (!scope || !isCurrentViewer()) return false;
    try {
      const key = getDraftStorageKey(scope, mode, topicId, categoryId);
      localStorage.setItem(key, JSON.stringify({ title, document, quotes }));
      setDraftStorageStatus('saved');
      return true;
    } catch {
      setDraftStorageStatus('failed');
      // The in-memory editor remains usable when storage is unavailable.
      return false;
    }
  }, [scope, isCurrentViewer, mode, topicId, categoryId, title, document, quotes]);

  // Autosave while editing. Closing flushes synchronously below, before this
  // timer is cancelled by the `isOpen` transition.
  useEffect(() => {
    if (!isOpen || !scope) return;
    setDraftStorageStatus('pending');
    const timeout = setTimeout(persistDraft, 600);
    return () => clearTimeout(timeout);
  }, [isOpen, scope, persistDraft]);

  // SPA navigation unmounts without `pagehide`; leaving the page can also
  // cancel a pending timer. Always flush the newest render's snapshot, not the
  // one captured when the listener was first installed. Reset disables the
  // flush synchronously so a successful post cannot resurrect its draft.
  const activeDraftRef = useRef(isOpen);
  const persistDraftRef = useRef(persistDraft);
  activeDraftRef.current = isOpen;
  persistDraftRef.current = persistDraft;
  useEffect(() => {
    const flush = () => { if (activeDraftRef.current) persistDraftRef.current(); };
    window.addEventListener('pagehide', flush);
    return () => {
      flush();
      window.removeEventListener('pagehide', flush);
    };
  }, []);

  const clearDraft = useCallback((draftMode: 'reply' | 'topic', tId: string | null, cId: string | null) => {
    if (!scope || !isCurrentViewer()) return;
    try {
      const key = getDraftStorageKey(scope, draftMode, tId, cId);
      localStorage.removeItem(key);
    } catch {
      // Ignore
    }
  }, [scope, isCurrentViewer]);

  const openReply = useCallback((params: { topicId: string; topicTitle: string; parentReplyId?: string | null }) => {
    if (isSubmitting) return;
    if (isOpen && scope && !persistDraft()) return;
    setMode('reply');
    setTopicId(params.topicId);
    setTopicTitle(params.topicTitle);
    setParentReplyId(params.parentReplyId ?? null);
    setTitle('');
    setQuotes([]);
    setError(null);
    setDocument(emptyRichTextDocument());
    loadDraft('reply', params.topicId, null);
    activeDraftRef.current = true;
    setIsOpen(true);
    setIsMinimized(false);
  }, [loadDraft, isOpen, isSubmitting, scope, persistDraft]);

  const openNewTopic = useCallback((params?: { categoryId?: string | null }) => {
    if (isSubmitting) return;
    if (isOpen && scope && !persistDraft()) return;
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
    activeDraftRef.current = true;
    setIsOpen(true);
    setIsMinimized(false);
  }, [loadDraft, isOpen, isSubmitting, scope, persistDraft]);

  const selectCategory = useCallback((nextCategoryId: string): DraftSelectionResult => {
    if (mode !== 'topic' || !nextCategoryId || isSubmitting) return 'failed';
    if (categoryId === nextCategoryId) return 'moved';
    if (scope && !isCurrentViewer()) return 'failed';
    // Save before switching keys: an occupied destination has its own draft.
    // Restore that one while keeping this editor's outgoing text at its old key.
    let restored = false;
    if (scope) {
      try {
        const existing = switchTopicDraft(localStorage, scope, categoryId, nextCategoryId, {
          title, document, quotes,
        });
        if (existing) {
          setTitle(existing.title);
          setDocument(existing.document);
          setQuotes(existing.quotes);
          restored = true;
        }
        setDraftStorageStatus('saved');
      } catch {
        setDraftStorageStatus('failed');
        // A quota/read error or corrupt destination must never silently replace
        // an existing draft. Keep the current category and edits in memory.
        return 'failed';
      }
    }
    setCategoryId(nextCategoryId);
    setError(null);
    return restored ? 'restored' : 'moved';
  }, [mode, categoryId, isSubmitting, scope, isCurrentViewer, title, document, quotes]);

  const close = useCallback(() => {
    // Closing cancels the debounced autosave. Persist the latest in-memory
    // snapshot first, including text typed less than 600 ms ago.
    if (isOpen) persistDraft();
    activeDraftRef.current = false;
    setIsOpen(false);
    setIsMinimized(false);
  }, [isOpen, persistDraft]);

  const minimize = useCallback(() => {
    setIsMinimized(true);
  }, []);

  const expand = useCallback(() => {
    setIsMinimized(false);
  }, []);

  const reset = useCallback(() => {
    activeDraftRef.current = false;
    clearDraft(mode, topicId, categoryId);
    setTitle('');
    setDocument(emptyRichTextDocument());
    setQuotes([]);
    setError(null);
    setIsOpen(false);
    setIsMinimized(false);
  }, [clearDraft, mode, topicId, categoryId]);

  const appendQuote = useCallback((quote: ForumQuoteReference | null, quoteText: string, authorHandle?: string) => {
    if (quote) {
      setQuotes((prev) => {
        // A persisted relation pins a real global relation-revision ID only.
        if (prev.some((q) => q.targetId === quote.targetId && q.targetKind === quote.targetKind && q.revisionId === quote.revisionId)) {
          return prev;
        }
        return [...prev, quote];
      });
    }

    if (quoteText) {
      setDocument((prev) => {
        const textToInsert = authorHandle
          ? `@${authorHandle}: "${quoteText}"`
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

    activeDraftRef.current = true;
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
    draftStorage: scope ? draftStorageStatus : null,
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
    scope,
    draftStorageStatus,
  ]);

  const value = useMemo<ComposerContextValue>(() => ({
    state,
    openReply,
    openNewTopic,
    selectCategory,
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
    isCurrentViewer,
  }), [
    state,
    openReply,
    openNewTopic,
    selectCategory,
    close,
    minimize,
    expand,
    appendQuote,
    removeQuote,
    reset,
    isCurrentViewer,
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
