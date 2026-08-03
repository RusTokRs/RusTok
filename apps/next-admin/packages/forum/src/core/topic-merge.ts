export const MAX_FORUM_TOPIC_MERGE_REASON_LENGTH = 500;

export interface ForumTopicMergeCandidate {
  id: string;
  title: string;
  categoryId: string;
  replyCount: number;
  solutionReplyId?: string | null;
}

export type ForumTopicMergeWinner = 'source' | 'target';

export interface ForumTopicMergeCommand {
  operationId: string;
  sourceTopicId: string;
  targetTopicId: string;
  reason: string;
  selectedSolutionReplyId?: string;
}

export interface ForumTopicMergeReceipt {
  operationId: string;
  eventId: string;
  sourceTopicId: string;
  targetTopicId: string;
  categoryId: string;
  actorId: string;
  reason: string;
  movedReplyCount: number;
  movedPublishedReplyCount: number;
  resultingPublishedReplyCount: number;
  positionOffset: number;
  mergedAt: string;
}

export function forumTopicMergeNeedsSolutionChoice(
  source: ForumTopicMergeCandidate,
  target: ForumTopicMergeCandidate
): boolean {
  return Boolean(source.solutionReplyId && target.solutionReplyId);
}

export function forumTopicMergeCandidateLabel(
  candidate: ForumTopicMergeCandidate
): string {
  const solved = candidate.solutionReplyId ? ' · solved' : '';
  return `${candidate.title} · ${candidate.replyCount} replies${solved}`;
}

export function newForumTopicMergeOperationId(): string {
  if (globalThis.crypto?.randomUUID) {
    return globalThis.crypto.randomUUID();
  }
  const now = BigInt(Date.now());
  const entropy = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
  const value = ((now << 64n) ^ entropy).toString(16).padStart(32, '0').slice(-32);
  return `${value.slice(0, 8)}-${value.slice(8, 12)}-4${value.slice(13, 16)}-8${value.slice(17, 20)}-${value.slice(20)}`;
}

export function buildForumTopicMergeCommand(input: {
  operationId: string;
  source: ForumTopicMergeCandidate;
  target: ForumTopicMergeCandidate;
  reason: string;
  winner?: ForumTopicMergeWinner;
}): ForumTopicMergeCommand {
  const operationId = input.operationId.trim();
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(operationId)) {
    throw new Error('Merge operation identity is invalid.');
  }
  if (input.source.id === input.target.id) {
    throw new Error('Source and retained target topics must be different.');
  }
  const reason = input.reason.trim();
  if (!reason) {
    throw new Error('Merge reason is required.');
  }
  if ([...reason].length > MAX_FORUM_TOPIC_MERGE_REASON_LENGTH) {
    throw new Error(
      `Merge reason must not exceed ${MAX_FORUM_TOPIC_MERGE_REASON_LENGTH} characters.`
    );
  }
  if(/[\u0000-\u001f\u007f]/u.test(reason)) {
    throw new Error('Merge reason must not contain control characters.');
  }

  const needsChoice = forumTopicMergeNeedsSolutionChoice(input.source, input.target);
  if (needsChoice && !input.winner) {
    throw new Error('Choose which accepted solution must remain after the merge.');
  }
  if (!needsChoice && input.winner) {
    throw new Error(
      'A solution winner can be selected only when both topics are solved.'
    );
  }

  const selectedSolutionReplyId =
    input.winner === 'source'
      ? input.source.solutionReplyId ?? undefined
      : input.winner === 'target'
        ? input.target.solutionReplyId ?? undefined
        : undefined;

  return {
    operationId,
    sourceTopicId: input.source.id,
    targetTopicId: input.target.id,
    reason,
    selectedSolutionReplyId
  };
}
