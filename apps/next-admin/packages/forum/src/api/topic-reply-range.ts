import { graphqlRequest } from '@/lib/graphql';
import type { GqlOpts } from './forum';
import type {
  ForumReplyRangeMoveCommand,
  ForumReplyRangeMoveReceipt
} from '../core/topic-reply-range';

export async function moveForumTopicReplyRange(
  command: ForumReplyRangeMoveCommand,
  opts: GqlOpts = {}
): Promise<ForumReplyRangeMoveReceipt> {
  const mutation = `
    mutation MoveForumTopicReplyRange(
      $tenantId: UUID
      $sourceTopicId: UUID!
      $input: MoveForumTopicReplyRangeGraphqlInput!
    ) {
      moveForumTopicReplyRange(
        tenantId: $tenantId
        sourceTopicId: $sourceTopicId
        input: $input
      ) {
        operationId
        eventId
        sourceTopicId
        targetTopicId
        sourceCategoryId
        targetCategoryId
        actorId
        reason
        sourceStartPosition
        sourceEndPosition
        targetStartPosition
        targetEndPosition
        movedReplyCount
        movedPublishedReplyCount
        sourceResultingPublishedReplyCount
        targetResultingPublishedReplyCount
        movedSolutionReplyId
        sourceResultingSolutionReplyId
        targetResultingSolutionReplyId
        movedAt
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId?: string | null;
      sourceTopicId: string;
      input: {
        operationId: string;
        targetTopicId: string;
        startPosition: number;
        endPosition: number;
        reason: string;
      };
    },
    { moveForumTopicReplyRange: ForumReplyRangeMoveReceipt }
  >(
    mutation,
    {
      tenantId: opts.tenantId,
      sourceTopicId: command.sourceTopicId,
      input: {
        operationId: command.operationId,
        targetTopicId: command.targetTopicId,
        startPosition: command.startPosition,
        endPosition: command.endPosition,
        reason: command.reason
      }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.moveForumTopicReplyRange;
}
