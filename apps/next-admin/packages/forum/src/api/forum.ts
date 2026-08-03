import { graphqlRequest } from '@/lib/graphql';
import type { RichTextDocument } from '@rustok/richtext';
import type {
  ForumTopicMergeCandidate,
  ForumTopicMergeCommand,
  ForumTopicMergeReceipt
} from '../core/topic-merge';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export interface ForumTopicSummary extends ForumTopicMergeCandidate {
  slug: string;
}

interface CreateForumReplyInput {
  locale: string;
  content: RichTextDocument;
  parentReplyId?: string;
}

export async function listForumTopics(
  opts: GqlOpts = {},
  input: { locale?: string; first?: number } = {}
): Promise<ForumTopicSummary[]> {
  const query = `
    query ForumTopics($tenantId: UUID!, $locale: String, $pagination: PaginationInput!) {
      forumTopics(tenantId: $tenantId, locale: $locale, pagination: $pagination) {
        items {
          id
          title
          slug
          categoryId
          replyCount
          solutionReplyId
        }
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId: string;
      locale?: string;
      pagination: { first: number };
    },
    {
      forumTopics: { items: ForumTopicSummary[] };
    }
  >(
    query,
    {
      tenantId: opts.tenantId!,
      locale: input.locale,
      pagination: { first: input.first ?? 100 }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.forumTopics.items;
}

export async function createForumReply(
  topicId: string,
  input: CreateForumReplyInput,
  opts: GqlOpts = {}
): Promise<string> {
  const mutation = `
    mutation CreateForumReply($tenantId: UUID!, $topicId: UUID!, $input: CreateForumReplyInput!) {
      createForumReply(tenantId: $tenantId, topicId: $topicId, input: $input) {
        id
      }
    }
  `;

  const data = await graphqlRequest<
    { tenantId: string; topicId: string; input: CreateForumReplyInput },
    { createForumReply: { id: string } }
  >(
    mutation,
    { tenantId: opts.tenantId!, topicId, input },
    opts.token,
    opts.tenantSlug
  );

  return data.createForumReply.id;
}

export async function mergeForumTopics(
  command: ForumTopicMergeCommand,
  opts: GqlOpts = {}
): Promise<ForumTopicMergeReceipt> {
  if (command.selectedSolutionReplyId) {
    const mutation = `
      mutation MergeForumTopicResolvingSolution(
        $tenantId: UUID
        $targetTopicId: UUID!
        $input: ResolveForumTopicMergeSolutionGraphqlInput!
      ) {
        mergeForumTopicResolvingSolution(
          tenantId: $tenantId
          targetTopicId: $targetTopicId
          input: $input
        ) {
          merge {
            operationId
            eventId
            sourceTopicId
            targetTopicId
            categoryId
            actorId
            reason
            movedReplyCount
            movedPublishedReplyCount
            resultingPublishedReplyCount
            positionOffset
            mergedAt
          }
        }
      }
    `;
    const data = await graphqlRequest<
      {
        tenantId?: string | null;
        targetTopicId: string;
        input: {
          operationId: string;
          sourceTopicId: string;
          selectedSolutionReplyId: string;
          reason: string;
        };
      },
      {
        mergeForumTopicResolvingSolution: { merge: ForumTopicMergeReceipt };
      }
    >(
      mutation,
      {
        tenantId: opts.tenantId,
        targetTopicId: command.targetTopicId,
        input: {
          operationId: command.operationId,
          sourceTopicId: command.sourceTopicId,
          selectedSolutionReplyId: command.selectedSolutionReplyId,
          reason: command.reason
        }
      },
      opts.token,
      opts.tenantSlug
    );
    return data.mergeForumTopicResolvingSolution.merge;
  }

  const mutation = `
    mutation MergeForumTopic(
      $tenantId: UUID
      $targetTopicId: UUID!
      $input: MergeForumTopicGraphqlInput!
    ) {
      mergeForumTopic(
        tenantId: $tenantId
        targetTopicId: $targetTopicId
        input: $input
      ) {
        operationId
        eventId
        sourceTopicId
        targetTopicId
        categoryId
        actorId
        reason
        movedReplyCount
        movedPublishedReplyCount
        resultingPublishedReplyCount
        positionOffset
        mergedAt
      }
    }
  `;
  const data = await graphqlRequest<
    {
      tenantId?: string | null;
      targetTopicId: string;
      input: {
        operationId: string;
        sourceTopicId: string;
        reason: string;
      };
    },
    { mergeForumTopic: ForumTopicMergeReceipt }
  >(
    mutation,
    {
      tenantId: opts.tenantId,
      targetTopicId: command.targetTopicId,
      input: {
        operationId: command.operationId,
        sourceTopicId: command.sourceTopicId,
        reason: command.reason
      }
    },
    opts.token,
    opts.tenantSlug
  );
  return data.mergeForumTopic;
}
