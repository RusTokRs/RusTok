import { isForumUuid, newForumUuid } from './identity';

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

export function newForumTopicForkIdentity(): ForumTopicForkIdentity {
  return {
    operationId: newForumUuid(),
    targetTopicId: newForumUuid()
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

  if (!isForumUuid(operationId) || !isForumUuid(targetTopicId)) {
    throw new Error('Fork retry identity is invalid.');
  }
  if (!isForumUuid(sourceTopicId)) {
    throw new Error('Choose the source topic to fork.');
  }
  if (sourceTopicId === targetTopicId) {
    throw new Error('The new topic identity must differ from the source topic.');
  }
  if (!isForumUuid(rootReplyId)) {
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
