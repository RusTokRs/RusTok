import { isForumUuid, newForumUuid } from './identity';

export const MAX_FORUM_REPLY_RANGE_MOVE_REASON_LENGTH = 500;

export interface ForumReplyRangeMoveIdentity {
  operationId: string;
}

export interface ForumReplyRangeMoveCommand {
  operationId: string;
  sourceTopicId: string;
  targetTopicId: string;
  startPosition: number;
  endPosition: number;
  reason: string;
}

export interface ForumReplyRangeMoveReceipt {
  operationId: string;
  eventId: string;
  sourceTopicId: string;
  targetTopicId: string;
  sourceCategoryId: string;
  targetCategoryId: string;
  actorId: string;
  reason: string;
  sourceStartPosition: number;
  sourceEndPosition: number;
  targetStartPosition: number;
  targetEndPosition: number;
  movedReplyCount: number;
  movedPublishedReplyCount: number;
  sourceResultingPublishedReplyCount: number;
  targetResultingPublishedReplyCount: number;
  movedSolutionReplyId?: string | null;
  sourceResultingSolutionReplyId?: string | null;
  targetResultingSolutionReplyId?: string | null;
  movedAt: string;
}

export function newForumReplyRangeMoveIdentity(): ForumReplyRangeMoveIdentity {
  return { operationId: newForumUuid() };
}

export function buildForumReplyRangeMoveCommand(input: {
  identity: ForumReplyRangeMoveIdentity;
  sourceTopicId: string;
  targetTopicId: string;
  startPosition: number;
  endPosition: number;
  reason: string;
}): ForumReplyRangeMoveCommand {
  const operationId = input.identity.operationId.trim();
  const sourceTopicId = input.sourceTopicId.trim();
  const targetTopicId = input.targetTopicId.trim();

  if (!isForumUuid(operationId)) {
    throw new Error('Reply-range retry identity is invalid.');
  }
  if (!isForumUuid(sourceTopicId)) {
    throw new Error('Choose the source topic.');
  }
  if (!isForumUuid(targetTopicId)) {
    throw new Error('Choose the target topic.');
  }
  if (sourceTopicId === targetTopicId) {
    throw new Error('Source and target topics must differ.');
  }
  if (!Number.isSafeInteger(input.startPosition) || input.startPosition < 1) {
    throw new Error('Start position must be a positive integer.');
  }
  if (!Number.isSafeInteger(input.endPosition) || input.endPosition < 1) {
    throw new Error('End position must be a positive integer.');
  }
  if (input.startPosition > input.endPosition) {
    throw new Error('Start position must not exceed end position.');
  }

  const reason = validateText(
    input.reason,
    'Move reason',
    MAX_FORUM_REPLY_RANGE_MOVE_REASON_LENGTH
  );

  return {
    operationId,
    sourceTopicId,
    targetTopicId,
    startPosition: input.startPosition,
    endPosition: input.endPosition,
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
