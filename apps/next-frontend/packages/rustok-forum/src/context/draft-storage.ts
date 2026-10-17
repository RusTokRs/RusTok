import { getRichTextProfile, validateRichTextDocument, type RichTextDocument } from '@rustok/richtext';
import type { ForumQuoteReference } from '../api/forum';

export type DraftIdentity = { userId: string; tenantId: string };
export type DraftScope = DraftIdentity & { tenant: string };
export type TopicDraft = { title: string; document: RichTextDocument; quotes: ForumQuoteReference[] };
export type ReplyDraft = TopicDraft & { parentReplyId: string | null };

const STORAGE_KEY_PREFIX = 'rustok_forum_draft_';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** This identifies a *local* draft namespace, not an authorization decision.
 * The server validates JWT signatures, tenant and session on every request.
 * Never infer a viewer from the independently persisted user cookie/localStorage:
 * it can still describe the previous user after the access token changes. */
export function getDraftIdentity(accessToken: string): DraftIdentity | null {
  if (!accessToken) return null;
  try {
    const parts = accessToken.split('.');
    if (parts.length !== 3 || !parts[1]) return null;
    const encoded = parts[1].replace(/-/g, '+').replace(/_/g, '/');
    const value: unknown = JSON.parse(atob(encoded));
    if (!value || typeof value !== 'object') return null;
    const claims = value as Record<string, unknown>;
    // Direct sessions and delegated OAuth user tokens carry a user subject.
    // Client-credentials and non-access tokens must never own a person's draft.
    if (claims.grant_type !== undefined &&
        claims.grant_type !== 'direct' && claims.grant_type !== 'authorization_code') return null;
    if (typeof claims.sub !== 'string' || !UUID.test(claims.sub) ||
        typeof claims.tenant_id !== 'string' || !UUID.test(claims.tenant_id) ||
        typeof claims.exp !== 'number' || claims.exp <= Date.now() / 1000) return null;
    return { userId: claims.sub.toLowerCase(), tenantId: claims.tenant_id.toLowerCase() };
  } catch {
    return null;
  }
}

export function isCurrentDraftSession(
  expected: { token: string; tenant: string; scope: DraftScope | null },
  current: { token: string; tenant: string; identity: DraftIdentity | null }
): boolean {
  return Boolean(expected.token && current.token === expected.token &&
    current.tenant === expected.tenant &&
    (!expected.scope ||
      (current.identity?.userId === expected.scope.userId &&
        current.identity.tenantId === expected.scope.tenantId)));
}

/** Historical unscoped keys are intentionally never read or migrated to an
 * account whose identity the old key did not record. */
export function getDraftStorageKey(
  scope: DraftScope,
  mode: 'reply' | 'topic',
  topicId: string | null,
  categoryId: string | null,
  parentReplyId: string | null = null,
): string {
  return `${STORAGE_KEY_PREFIX}${encodeURIComponent(JSON.stringify([
    scope.tenant, scope.tenantId, scope.userId, mode,
    mode === 'reply' ? [topicId, parentReplyId] : categoryId,
  ]))}`;
}

/** Parse a saved draft before restoring it; a corrupt destination must not be
 * silently overwritten by another category's text. */
export function parseTopicDraft(raw: string): TopicDraft {
  const parsed: unknown = JSON.parse(raw);
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('Invalid saved topic draft');
  }
  const draft = parsed as Record<string, unknown>;
  const document = draft.document;
  if (typeof draft.title !== 'string' || !document || typeof document !== 'object' ||
      !validateRichTextDocument(document, getRichTextProfile('discussion'), { allowEmpty: true }).valid ||
      !Array.isArray(draft.quotes) ||
      !draft.quotes.every((value: unknown) => {
        if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
        const quote = value as Record<string, unknown>;
        return (quote.targetKind === 'TOPIC' || quote.targetKind === 'REPLY') &&
          typeof quote.targetId === 'string' &&
          typeof quote.revisionId === 'number' && Number.isSafeInteger(quote.revisionId) &&
          quote.revisionId > 0 &&
          (quote.authorHandle === undefined || typeof quote.authorHandle === 'string') &&
          (quote.snippet === undefined || typeof quote.snippet === 'string');
      })) {
    throw new Error('Invalid saved topic draft');
  }
  return {
    title: draft.title,
    document: document as RichTextDocument,
    quotes: draft.quotes as ForumQuoteReference[],
  };
}

/** A reply draft must identify the same parent as its storage namespace.
 * Old topic-only reply keys are deliberately not read: their parent was never
 * recorded, so restoring them to a selected reply would silently retarget text. */
export function parseReplyDraft(raw: string, parentReplyId: string | null): ReplyDraft {
  const draft = parseTopicDraft(raw);
  const saved = JSON.parse(raw) as Record<string, unknown>;
  if (saved.parentReplyId !== parentReplyId) {
    throw new Error('Saved reply draft parent does not match the selected reply');
  }
  return { ...draft, parentReplyId };
}

/** Switch to a category without losing either draft. An empty destination
 * moves the current text (write before delete). An existing destination keeps
 * its saved draft and stores the outgoing edits under their original key. */
export function switchTopicDraft(
  storage: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>,
  scope: DraftScope,
  previousCategoryId: string | null,
  nextCategoryId: string,
  draft: TopicDraft,
): TopicDraft | null {
  if (previousCategoryId === nextCategoryId) return null;
  const sourceKey = getDraftStorageKey(scope, 'topic', null, previousCategoryId);
  const destinationKey = getDraftStorageKey(scope, 'topic', null, nextCategoryId);
  const existing = storage.getItem(destinationKey);
  if (existing !== null) {
    const restored = parseTopicDraft(existing);
    storage.setItem(sourceKey, JSON.stringify(draft));
    return restored;
  }
  storage.setItem(destinationKey, JSON.stringify(draft));
  storage.removeItem(sourceKey);
  return null;
}
