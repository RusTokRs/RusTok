import { isForumUuid, newForumUuid } from './identity';

export const MAX_FORUM_TOPIC_SPLIT_REPLIES = 500;
export const MAX_FORUM_TOPIC_SPLIT_REASON_LENGTH = 500;
export const MAX_FORUM_TOPIC_SPLIT_TITLE_LENGTH = 500;

export interface ForumTopicSplitReply {
  id: string;
  contentPreview: string;
  status: string;
  parentReplyId?: string | null;
  createdAt: string;
}

export interface ForumTopicSplitReplyPage {
  total: number;
  items: ForumTopicSplitReply[];
}

export interface ForumTopicSplitIdentity {
  operationId: string;
  targetTopicId: string;
}

export interface ForumTopicSplitCommand {
  operationId: string;
  sourceTopicId: string;
  targetTopicId: string;
  replyIds: string[];
  locale: string;
  title: string;
  slug?: string;
  reason: string;
}

export interface ForumTopicSplitReceipt {
  operationId: string;
  eventId: string;
  sourceTopicId: string;
  targetTopicId: string;
  categoryId: string;
  actorId: string;
  reason: string;
  movedReplyCount: number;
  movedPublishedReplyCount: number;
  sourceResultingPublishedReplyCount: number;
  targetResultingPublishedReplyCount: number;
  solutionReplyId?: string | null;
  splitAt: string;
}

export function newForumTopicSplitIdentity(): ForumTopicSplitIdentity {
  return {
    operationId: newForumUuid(),
    targetTopicId: newForumUuid()
  };
}

export function forumTopicSplitReplyLabel(reply: ForumTopicSplitReply): string {
  const preview = reply.contentPreview.trim() || '(empty reply)';
  return `${preview} · ${reply.status}${reply.parentReplyId ? ' · child' : ''}`;
}

export function buildForumTopicSplitCommand(input: {
  identity: ForumTopicSplitIdentity;
  sourceTopicId: string;
  replies: ForumTopicSplitReplyPage;
  selectedReplyIds: string[];
  locale: string;
  title: string;
  slug: string;
  reason: string;
}): ForumTopicSplitCommand {
  const operationId = input.identity.operationId.trim();
  const targetTopicId = input.identity.targetTopicId.trim();
  const sourceTopicId = input.sourceTopicId.trim();
  if (!isForumUuid(operationId) || !isForumUuid(targetTopicId)) {
    throw new Error('Split retry identity is invalid.');
  }
  if (!isForumUuid(sourceTopicId)) {
    throw new Error('Choose the source topic to split.');
  }
  if (sourceTopicId === targetTopicId) {
    throw new Error(
      'The new topic identity must differ from the source topic.'
    );
  }

  const locale = validateText(input.locale, 'Target locale', 64);
  const title = validateText(
    input.title,
    'Target title',
    MAX_FORUM_TOPIC_SPLIT_TITLE_LENGTH
  );
  const reason = validateText(
    input.reason,
    'Split reason',
    MAX_FORUM_TOPIC_SPLIT_REASON_LENGTH
  );
  const slug = normalizeOptionalText(input.slug, 'Target slug', 255);

  if (!input.selectedReplyIds.length) {
    throw new Error('Select at least one reply to move.');
  }
  if (input.selectedReplyIds.length > MAX_FORUM_TOPIC_SPLIT_REPLIES) {
    throw new Error(
      `A split may move at most ${MAX_FORUM_TOPIC_SPLIT_REPLIES} replies.`
    );
  }

  const replyIds = input.selectedReplyIds.map((value) => value.trim()).sort();
  if (replyIds.some((value) => !isForumUuid(value))) {
    throw new Error('Every selected reply identity must be a UUID.');
  }
  if (new Set(replyIds).size !== replyIds.length) {
    throw new Error('Selected reply identities must be unique.');
  }
  if (replyIds.length >= input.replies.total) {
    throw new Error('The source topic must retain at least one reply.');
  }

  validateParentClosedSelection(input.replies, replyIds);

  return {
    operationId,
    sourceTopicId,
    targetTopicId,
    replyIds,
    locale,
    title,
    slug,
    reason
  };
}

function validateParentClosedSelection(
  replies: ForumTopicSplitReplyPage,
  replyIds: string[]
): void {
  const byId = new Map(replies.items.map((reply) => [reply.id, reply]));
  const selected = new Set(replyIds);

  for (const replyId of selected) {
    const reply = byId.get(replyId);
    if (!reply) {
      throw new Error(`Selected reply is not loaded: ${replyId}`);
    }
    if (
      reply.parentReplyId &&
      byId.has(reply.parentReplyId) &&
      !selected.has(reply.parentReplyId)
    ) {
      throw new Error(
        'A selected child reply requires its parent to be selected.'
      );
    }
  }

  for (const reply of replies.items) {
    if (
      reply.parentReplyId &&
      selected.has(reply.parentReplyId) &&
      !selected.has(reply.id)
    ) {
      throw new Error(
        'Selecting a parent requires every loaded child to be selected.'
      );
    }
  }
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
