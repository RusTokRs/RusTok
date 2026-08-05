export const MAX_FORUM_TOPIC_FORK_REPLIES = 500;
export const MAX_FORUM_TOPIC_FORK_REASON_LENGTH = 500;
export const MAX_FORUM_TOPIC_FORK_TITLE_LENGTH = 500;

export interface ForumTopicForkReply {
  id: string;
  contentPreview: string;
  status: string;
  parentReplyId?: string | null;
  createdAt: string;
}

export interface ForumTopicForkReplyPage {
  total: number;
  items: ForumTopicForkReply[];
}

export interface ForumTopicForkIdentity {
  operationId: string;
  targetTopicId: string;
}

export interface ForumTopicForkCommand {
  operationId: string;
  sourceTopicId: string;
  targetTopicId: string;
  rootReplyId: string;
  locale: string;
  title: string;
  slug?: string;
  reason: string;
}

export interface ForumTopicForkReceipt {
  operationId: string;
  eventId: string;
  sourceTopicId: string;
  targetTopicId: string;
  rootReplyId: string;
  categoryId: string;
  actorId: string;
  reason: string;
  copiedReplyCount: number;
  copiedPublishedReplyCount: number;
  copiedBodyCount: number;
  copiedReplyRevisionCount: number;
  copiedRelationRevisionCount: number;
  copiedMentionCount: number;
  copiedQuoteCount: number;
  forkedAt: string;
}

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function newUuidV4(): string {
  if (globalThis.crypto?.randomUUID) {
    return globalThis.crypto.randomUUID();
  }
  const now = BigInt(Date.now());
  const entropy = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
  const value = ((now << 64n) ^ entropy).toString(16).padStart(32, '0').slice(-32);
  return `${value.slice(0, 8)}-${value.slice(8, 12)}-4${value.slice(13, 16)}-8${value.slice(17, 20)}-${value.slice(20)}`;
}

export function newForumTopicForkIdentity(): ForumTopicForkIdentity {
  return {
    operationId: newUuidV4(),
    targetTopicId: newUuidV4()
  };
}

export function forumTopicForkReplyLabel(reply: ForumTopicForkReply): string {
  const preview = reply.contentPreview.trim() || '(empty reply)';
  return `${preview} · ${reply.status}${reply.parentReplyId ? ' · child' : ''}`;
}

export function buildForumTopicForkCommand(input: {
  identity: ForumTopicForkIdentity;
  sourceTopicId: string;
  replies: ForumTopicForkReplyPage;
  rootReplyId: string;
  locale: string;
  title: string;
  slug: string;
  reason: string;
}): ForumTopicForkCommand {
  const operationId = input.identity.operationId.trim();
  const targetTopicId = input.identity.targetTopicId.trim();
  const sourceTopicId = input.sourceTopicId.trim();
  const rootReplyId = input.rootReplyId.trim();

  if (!UUID_PATTERN.test(operationId) || !UUID_PATTERN.test(targetTopicId)) {
    throw new Error('Fork retry identity is invalid.');
  }
  if (!UUID_PATTERN.test(sourceTopicId)) {
    throw new Error('Choose the source topic to fork.');
  }
  if (sourceTopicId === targetTopicId) {
    throw new Error('The new topic identity must differ from the source topic.');
  }
  if (!UUID_PATTERN.test(rootReplyId)) {
    throw new Error('Choose the root reply to fork.');
  }
  if (!input.replies.items.some((reply) => reply.id === rootReplyId)) {
    throw new Error('The selected root reply is not loaded for this topic.');
  }

  const locale = validateText(input.locale, 'Target locale', 64);
  const title = validateText(
    input.title,
    'Target title',
    MAX_FORUM_TOPIC_FORK_TITLE_LENGTH
  );
  const reason = validateText(
    input.reason,
    'Fork reason',
    MAX_FORUM_TOPIC_FORK_REASON_LENGTH
  );
  const slug = normalizeOptionalText(input.slug, 'Target slug', 255);

  return {
    operationId,
    sourceTopicId,
    targetTopicId,
    rootReplyId,
    locale,
    title,
    slug,
    reason
  };
}

function validateText(value: string, label: string, maximum: number): string {
  const normalized = value.trim();
  if (!normalized) {
    throw new Error(`${label} is required.`);
  }
  if ([...normalized].length > maximum) {
    throw new Error(`${label} must not exceed ${maximum} characters.`);
  }
  if (/[\u0000-\u001f\u007f]/u.test(normalized)) {
    throw new Error(`${label} must not contain control characters.`);
  }
  return normalized;
}

function normalizeOptionalText(
  value: string,
  label: string,
  maximum: number
): string | undefined {
  const normalized = value.trim();
  if (!normalized) {
    return undefined;
  }
  if ([...normalized].length > maximum) {
    throw new Error(`${label} must not exceed ${maximum} characters.`);
  }
  if (/[\u0000-\u001f\u007f]/u.test(normalized)) {
    throw new Error(`${label} must not contain control characters.`);
  }
  return normalized;
}
