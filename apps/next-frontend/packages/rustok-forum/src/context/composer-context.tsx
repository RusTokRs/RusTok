'use client';

import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import {
  emptyRichTextDocument,
  type RichTextDocument,
} from '@rustok/richtext';
import type { ForumQuoteReference } from '../api/forum';
import { getClientAuth } from '@/shared/lib/auth';
import {
  getDraftIdentity,
  getDraftStorageKey,
  isCurrentDraftSession,
  parseTopicDraft,
  parseReplyDraft,
  switchTopicDraft,
  type DraftIdentity,
  type DraftScope,
  type TopicDraft,
} from './draft-storage';
import { useForumAuth } from '../components/use-forum-auth';

export interface ComposerState {
  /** Render projection of the composer-owned editing lifetime. */
  editorSession: number;
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
  draftStorage: 'pending' | 'saved' | 'failed' | 'cleanup_failed' | null;
}

export type DraftSelectionResult = 'moved' | 'restored' | 'failed';

export interface ComposerContextValue {
  state: ComposerState;
  openReply: (params: { topicId: string; topicTitle: string; parentReplyId?: string | null }) => boolean;
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
  /** Capture an editor lifetime, not merely its target IDs. */
  captureEditorSession: () => () => boolean;
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
  // The ref is the synchronous owner lock; isSubmitting is its render projection.
  const submissionPendingRef = useRef(false);
  // Target IDs can repeat after a close/reopen; asynchronous edits need a lifetime too.
  const editorSessionRef = useRef(0);
  const [editorSession, setEditorSession] = useState(0);
  const advanceEditorSession = useCallback(() => {
    editorSessionRef.current += 1;
    setEditorSession(editorSessionRef.current);
  }, []);
  const setSubmitting = useCallback((pending: boolean) => {
    submissionPendingRef.current = pending;
    setIsSubmitting(pending);
  }, []);
  const [error, setError] = useState<string | null>(null);
  const [draftStorageStatus, setDraftStorageStatus] = useState<'pending' | 'saved' | 'failed' | 'cleanup_failed'>('pending');

  // A successful post must not resurrect its stored payload after a failed removeItem.
  // This is session-only recovery state; it is never an alternate persisted draft store.
  const pendingDraftCleanupRef = useRef(new Set<string>());

  const retryDraftCleanup = useCallback((key: string): boolean => {
    if (!pendingDraftCleanupRef.current.has(key)) return false;
    // Fail closed while storage is blocked. Retry deletion before any restoration.
    localStorage.removeItem(key);
    pendingDraftCleanupRef.current.delete(key);
    return true;
  }, []);

  // Parse before changing the editor. A corrupt or unreadable destination
  // must not replace the outgoing in-memory draft or be silently overwritten.
  const loadDraft = useCallback((draftMode: 'reply' | 'topic', tId: string | null, cId: string | null, pId: string | null): TopicDraft | null => {
    if (!scope || !isCurrentViewer()) return null;
    const key = getDraftStorageKey(scope, draftMode, tId, cId, pId);
    if (retryDraftCleanup(key)) return null;
    const saved = localStorage.getItem(key);
    return saved === null ? null : draftMode === 'topic'
      ? parseTopicDraft(saved) : parseReplyDraft(saved, pId);
  }, [scope, isCurrentViewer, retryDraftCleanup]);

  const persistDraft = useCallback((): boolean => {
    if (!scope || !isCurrentViewer()) return false;
    try {
      const key = getDraftStorageKey(scope, mode, topicId, categoryId, parentReplyId);
      localStorage.setItem(key, JSON.stringify(mode === 'reply'
        ? { title, document, quotes, parentReplyId } : { title, document, quotes }));
      pendingDraftCleanupRef.current.delete(key);
      setDraftStorageStatus('saved');
      return true;
    } catch {
      setDraftStorageStatus('failed');
      // The in-memory editor remains usable when storage is unavailable.
      return false;
    }
  }, [scope, isCurrentViewer, mode, topicId, categoryId, parentReplyId, title, document, quotes]);

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

  const captureEditorSession = useCallback(() => {
    const generation = editorSessionRef.current;
    return () => generation === editorSessionRef.current && activeDraftRef.current &&
      !submissionPendingRef.current && isCurrentViewer();
  }, [isCurrentViewer]);

  const updateTitle = useCallback((value: string) => {
    if (!activeDraftRef.current || submissionPendingRef.current || !isCurrentViewer()) return;
    setTitle(value);
  }, [isCurrentViewer]);

  const updateDocument = useCallback((value: RichTextDocument) => {
    if (!activeDraftRef.current || submissionPendingRef.current || !isCurrentViewer()) return;
    setDocument(value);
  }, [isCurrentViewer]);

  const clearDraft = useCallback((draftMode: 'reply' | 'topic', tId: string | null, cId: string | null, pId: string | null): boolean => {
    if (!scope) return true;
    if (!isCurrentViewer()) return false;
    const key = getDraftStorageKey(scope, draftMode, tId, cId, pId);
    pendingDraftCleanupRef.current.add(key);
    try {
      localStorage.removeItem(key);
      pendingDraftCleanupRef.current.delete(key);
      setDraftStorageStatus('saved');
      return true;
    } catch {
      setDraftStorageStatus('cleanup_failed');
      return false;
    }
  }, [scope, isCurrentViewer]);

  const openReply = useCallback((params: { topicId: string; topicTitle: string; parentReplyId?: string | null }) => {
    if (isSubmitting || submissionPendingRef.current || !isCurrentViewer()) return false;
    if (isOpen && scope && !persistDraft()) return false;
    const pId = params.parentReplyId ?? null;
    let draft: TopicDraft | null;
    try {
      draft = loadDraft('reply', params.topicId, null, pId);
    } catch {
      setDraftStorageStatus('failed');
      return false;
    }
    advanceEditorSession();
    setMode('reply');
    setTopicId(params.topicId);
    setTopicTitle(params.topicTitle);
    setParentReplyId(pId);
    setCategoryId(null);
    setTitle(draft?.title ?? '');
    setQuotes(draft?.quotes ?? []);
    setError(null);
    setDocument(draft?.document ?? emptyRichTextDocument());
    activeDraftRef.current = true;
    setIsOpen(true);
    setIsMinimized(false);
    return true;
  }, [loadDraft, isOpen, isSubmitting, scope, persistDraft, isCurrentViewer, advanceEditorSession]);

  const openNewTopic = useCallback((params?: { categoryId?: string | null }) => {
    if (isSubmitting || submissionPendingRef.current || !isCurrentViewer()) return;
    if (isOpen && scope && !persistDraft()) return;
    const cId = params?.categoryId ?? null;
    let draft: TopicDraft | null;
    try {
      draft = loadDraft('topic', null, cId, null);
    } catch {
      setDraftStorageStatus('failed');
      return;
    }
    advanceEditorSession();
    setMode('topic');
    setTopicId(null);
    setTopicTitle(null);
    setCategoryId(cId);
    setParentReplyId(null);
    setTitle(draft?.title ?? '');
    setQuotes(draft?.quotes ?? []);
    setError(null);
    setDocument(draft?.document ?? emptyRichTextDocument());
    activeDraftRef.current = true;
    setIsOpen(true);
    setIsMinimized(false);
  }, [loadDraft, isOpen, isSubmitting, scope, persistDraft, isCurrentViewer, advanceEditorSession]);

  const selectCategory = useCallback((nextCategoryId: string): DraftSelectionResult => {
    if (mode !== 'topic' || !nextCategoryId || isSubmitting || submissionPendingRef.current) return 'failed';
    if (categoryId === nextCategoryId) return 'moved';
    if (scope && !isCurrentViewer()) return 'failed';
    // Save before switching keys: an occupied destination has its own draft.
    // Restore that one while keeping this editor's outgoing text at its old key.
    let restored = false;
    if (scope) {
      try {
        retryDraftCleanup(getDraftStorageKey(scope, 'topic', null, nextCategoryId));
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
    advanceEditorSession();
    setCategoryId(nextCategoryId);
    setError(null);
    return restored ? 'restored' : 'moved';
  }, [mode, categoryId, isSubmitting, scope, isCurrentViewer, title, document, quotes, retryDraftCleanup, advanceEditorSession]);

  const close = useCallback(() => {
    // Closing cancels the debounced autosave. Persist the latest in-memory
    // snapshot first, including text typed less than 600 ms ago.
    if (isOpen && scope && !persistDraft()) return;
    advanceEditorSession();
    activeDraftRef.current = false;
    setIsOpen(false);
    setIsMinimized(false);
  }, [isOpen, scope, persistDraft, advanceEditorSession]);

  const minimize = useCallback(() => {
    setIsMinimized(true);
  }, []);

  const expand = useCallback(() => {
    setIsMinimized(false);
  }, []);

  const reset = useCallback(() => {
    activeDraftRef.current = false;
    clearDraft(mode, topicId, categoryId, parentReplyId);
    advanceEditorSession();
    setTitle('');
    setDocument(emptyRichTextDocument());
    setQuotes([]);
    setError(null);
    setIsOpen(false);
    setIsMinimized(false);
  }, [clearDraft, mode, topicId, categoryId, parentReplyId, advanceEditorSession]);

  const appendQuote = useCallback((quote: ForumQuoteReference | null, quoteText: string, authorHandle?: string) => {
    if (!activeDraftRef.current || submissionPendingRef.current || !isCurrentViewer()) return;
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
  }, [isCurrentViewer]);

  const removeQuote = useCallback((index: number) => {
    if (!activeDraftRef.current || submissionPendingRef.current || !isCurrentViewer()) return;
    setQuotes((prev) => prev.filter((_, i) => i !== index));
  }, [isCurrentViewer]);

  const state = useMemo<ComposerState>(() => ({
    editorSession,
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
    editorSession,
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
    setTitle: updateTitle,
    setDocument: updateDocument,
    appendQuote,
    removeQuote,
    setSubmitting,
    setError,
    reset,
    isCurrentViewer,
    captureEditorSession,
  }), [
    state,
    updateTitle,
    updateDocument,
    setSubmitting,
    captureEditorSession,
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
