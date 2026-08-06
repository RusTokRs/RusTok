import { graphqlRequest } from '@/lib/graphql';
import type { RichTextDocument } from '@rustok/richtext';
import type {
  ForumTopicForkCommand,
  ForumTopicForkReceipt
} from '../core/topic-fork';
import type {
  ForumTopicMergeCandidate,
  ForumTopicMergeCommand,
  ForumTopicMergeReceipt
} from '../core/topic-merge';
import type {
  ForumTopicSlugRenameCommand,
  ForumTopicSlugRenameReceipt
} from '../core/topic-slug-rename';
import type {
  ForumTopicSplitCommand,
  ForumTopicSplitReceipt,
  ForumTopicSplitReplyPage
} from '../core/topic-split';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export interface ForumTopicSummary extends ForumTopicMergeCandidate {
  locale: string;
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
          locale
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

export async function listForumTopicReplies(
  topicId: string,
  opts: GqlOpts = {},
  input: { locale?: string; first?: number } = {}
): Promise<ForumTopicSplitReplyPage> {
  const query = `
    query ForumTopicSplitReplies(
      $tenantId: UUID!
      $topicId: UUID!
      $locale: String
      $pagination: PaginationInput!
    ) {
      forumReplies(
        tenantId: $tenantId
        topicId: $topicId
        locale: $locale
        pagination: $pagination
      ) {
        total
        items {
          id
          contentPreview: contentPlainText
          status
          parentReplyId
          createdAt
        }
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId: string;
      topicId: string;
      locale?: string;
      pagination: { first: number };
    },
    { forumReplies: ForumTopicSplitReplyPage }
  >(
    query,
    {
      tenantId: opts.tenantId!,
      topicId,
      locale: input.locale,
      pagination: { first: input.first ?? 500 }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.forumReplies;
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

export async function renameForumTopicSlug(
  command: ForumTopicSlugRenameCommand,
  opts: GqlOpts = {}
): Promise<ForumTopicSlugRenameReceipt> {
  const mutation = `
    mutation RenameForumTopicSlug(
      $tenantId: UUID
      $topicId: UUID!
      $input: RenameForumTopicSlugGraphqlInput!
    ) {
      renameForumTopicSlug(
        tenantId: $tenantId
        topicId: $topicId
        input: $input
      ) {
        topicId
        locale
        previousSlug
        slug
        previousPath
        canonical {
          topicId
          locale
          shortId
          slug
          path
        }
        aliasId
        changed
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId?: string | null;
      topicId: string;
      input: { locale: string; slug: string };
    },
    { renameForumTopicSlug: ForumTopicSlugRenameReceipt }
  >(
    mutation,
    {
      tenantId: opts.tenantId,
      topicId: command.topicId,
      input: {
        locale: command.locale,
        slug: command.slug
      }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.renameForumTopicSlug;
}

export async function splitForumTopicReplies(
  command: ForumTopicSplitCommand,
  opts: GqlOpts = {}
): Promise<ForumTopicSplitReceipt> {
  const mutation = `
    mutation SplitForumTopicReplies(
      $tenantId: UUID
      $sourceTopicId: UUID!
      $input: SplitForumTopicRepliesGraphqlInput!
    ) {
      splitForumTopicReplies(
        tenantId: $tenantId
        sourceTopicId: $sourceTopicId
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
        sourceResultingPublishedReplyCount
        targetResultingPublishedReplyCount
        solutionReplyId
        splitAt
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
        replyIds: string[];
        locale: string;
        title: string;
        slug?: string;
        reason: string;
      };
    },
    { splitForumTopicReplies: ForumTopicSplitReceipt }
  >(
    mutation,
    {
      tenantId: opts.tenantId,
      sourceTopicId: command.sourceTopicId,
      input: {
        operationId: command.operationId,
        targetTopicId: command.targetTopicId,
        replyIds: command.replyIds,
        locale: command.locale,
        title: command.title,
        slug: command.slug,
        reason: command.reason
      }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.splitForumTopicReplies;
}

export async function forkForumTopicReplyBranch(
  command: ForumTopicForkCommand,
  opts: GqlOpts = {}
): Promise<ForumTopicForkReceipt> {
  const mutation = `
    mutation ForkForumTopicReplyBranch(
      $tenantId: UUID
      $sourceTopicId: UUID!
      $input: ForkForumTopicReplyBranchGraphqlInput!
    ) {
      forkForumTopicReplyBranch(
        tenantId: $tenantId
        sourceTopicId: $sourceTopicId
        input: $input
      ) {
        operationId
        eventId
        sourceTopicId
        targetTopicId
        rootReplyId
        categoryId
        actorId
        reason
        copiedReplyCount
        copiedPublishedReplyCount
        copiedBodyCount
        copiedReplyRevisionCount
        copiedRelationRevisionCount
        copiedMentionCount
        copiedQuoteCount
        forkedAt
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
        rootReplyId: string;
        locale: string;
        title: string;
        slug?: string;
        reason: string;
      };
    },
    { forkForumTopicReplyBranch: ForumTopicForkReceipt }
  >(
    mutation,
    {
      tenantId: opts.tenantId,
      sourceTopicId: command.sourceTopicId,
      input: {
        operationId: command.operationId,
        targetTopicId: command.targetTopicId,
        rootReplyId: command.rootReplyId,
        locale: command.locale,
        title: command.title,
        slug: command.slug,
        reason: command.reason
      }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.forkForumTopicReplyBranch;
}
