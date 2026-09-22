import { graphqlRequest } from '@/lib/graphql';
import type { RichTextDocument, RichTextView } from '@rustok/richtext';
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
  effectiveLocale: string;
  slug: string;
}

export interface ForumCategoryOption {
  id: string;
  name: string;
  effectiveLocale: string;
}

export interface ForumTopicDetail {
  id: string;
  requestedLocale: string;
  locale: string;
  effectiveLocale: string;
  availableLocales: string[];
  categoryId: string;
  authorId?: string | null;
  title: string;
  slug: string;
  body: RichTextView;
  bodyPlainText: string;
  status: string;
  tags: string[];
  isPinned: boolean;
  isLocked: boolean;
  replyCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface CreateForumTopicInput {
  locale: string;
  categoryId: string;
  title: string;
  slug?: string;
  body: RichTextDocument;
  metadata?: Record<string, unknown>;
  tags: string[];
  channelSlugs?: string[];
}

export interface UpdateForumTopicInput {
  locale: string;
  title?: string;
  body?: RichTextDocument;
  metadata?: Record<string, unknown>;
  tags?: string[];
  channelSlugs?: string[];
}

export interface CreateForumReplyInput {
  locale: string;
  content: RichTextDocument;
  parentReplyId?: string;
}

const FORUM_TOPIC_FIELDS = `
  id
  requestedLocale
  locale
  effectiveLocale
  availableLocales
  categoryId
  authorId
  title
  slug
  body { document html }
  bodyPlainText
  status
  tags
  isPinned
  isLocked
  replyCount
  createdAt
  updatedAt
`;

export async function listForumCategories(
  opts: GqlOpts = {},
  input: { locale?: string; first?: number } = {}
): Promise<ForumCategoryOption[]> {
  const query = `
    query ForumCategories($tenantId: UUID, $locale: String, $pagination: PaginationInput!) {
      forumCategories(tenantId: $tenantId, locale: $locale, pagination: $pagination) {
        items {
          id
          name
          effectiveLocale
        }
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId?: string | null;
      locale?: string;
      pagination: { first: number };
    },
    { forumCategories: { items: ForumCategoryOption[] } }
  >(
    query,
    {
      tenantId: opts.tenantId,
      locale: input.locale,
      pagination: { first: input.first ?? 100 }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.forumCategories.items;
}

export async function getForumTopic(
  id: string,
  opts: GqlOpts = {},
  locale?: string
): Promise<ForumTopicDetail | null> {
  const query = `
    query ForumTopic($tenantId: UUID, $id: UUID!, $locale: String) {
      forumTopic(tenantId: $tenantId, id: $id, locale: $locale) {
        ${FORUM_TOPIC_FIELDS}
      }
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string; locale?: string },
    { forumTopic: ForumTopicDetail | null }
  >(
    query,
    { tenantId: opts.tenantId, id, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.forumTopic;
}

export async function createForumTopic(
  input: CreateForumTopicInput,
  opts: GqlOpts = {}
): Promise<ForumTopicDetail> {
  const mutation = `
    mutation CreateForumTopic($tenantId: UUID, $input: CreateForumTopicInput!) {
      createForumTopic(tenantId: $tenantId, input: $input) {
        ${FORUM_TOPIC_FIELDS}
      }
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; input: CreateForumTopicInput },
    { createForumTopic: ForumTopicDetail }
  >(mutation, { tenantId: opts.tenantId, input }, opts.token, opts.tenantSlug);

  return data.createForumTopic;
}

export async function updateForumTopic(
  id: string,
  input: UpdateForumTopicInput,
  opts: GqlOpts = {}
): Promise<ForumTopicDetail> {
  const mutation = `
    mutation UpdateForumTopic($tenantId: UUID, $id: UUID!, $input: UpdateForumTopicInput!) {
      updateForumTopic(tenantId: $tenantId, id: $id, input: $input) {
        ${FORUM_TOPIC_FIELDS}
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId?: string | null;
      id: string;
      input: UpdateForumTopicInput;
    },
    { updateForumTopic: ForumTopicDetail }
  >(
    mutation,
    { tenantId: opts.tenantId, id, input },
    opts.token,
    opts.tenantSlug
  );

  return data.updateForumTopic;
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
          effectiveLocale
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
    mutation CreateForumReply($tenantId: UUID, $topicId: UUID!, $input: CreateForumReplyInput!) {
      createForumReply(tenantId: $tenantId, topicId: $topicId, input: $input) {
        id
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId?: string | null;
      topicId: string;
      input: CreateForumReplyInput;
    },
    { createForumReply: { id: string } }
  >(
    mutation,
    { tenantId: opts.tenantId, topicId, input },
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
