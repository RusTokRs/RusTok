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
  isDeleted: boolean;
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
  isDeleted
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

export interface AdminCategoryTreeNode {
  id: string;
  parentId: string | null;
  depth: number;
  position: number;
  requestedLocale: string;
  effectiveLocale: string;
  name: string;
  slug: string;
  description: string | null;
  icon: string | null;
  color: string | null;
  moderated: boolean;
  allowsTopics: boolean;
  archivedAt: string | null;
  isArchived: boolean;
  topicCount: number;
  replyCount: number;
  children: AdminCategoryTreeNode[];
}

export interface CreateAdminCategoryInput {
  locale: string;
  name: string;
  slug: string;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
  parentId?: string | null;
  position?: number | null;
  moderated: boolean;
  allowsTopics?: boolean;
}

export interface UpdateAdminCategoryInput {
  locale: string;
  name?: string | null;
  slug?: string | null;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
  position?: number | null;
  moderated?: boolean | null;
  allowsTopics?: boolean | null;
}

function buildCategoryTreeFields(remainingDepth: number): string {
  const fields =
    'id parentId depth position requestedLocale effectiveLocale name slug description icon color moderated allowsTopics archivedAt isArchived topicCount replyCount';
  if (remainingDepth <= 0) return fields;
  return `${fields} children { ${buildCategoryTreeFields(remainingDepth - 1)} }`;
}

export async function fetchAdminCategoryTree(
  opts: GqlOpts = {},
  locale?: string
): Promise<AdminCategoryTreeNode[]> {
  const query = `
    query ForumCategoryTree($locale: String) {
      forumCategoryTree(locale: $locale) {
        totalNodes
        maxDepth
        roots {
          ${buildCategoryTreeFields(6)}
        }
      }
    }
  `;

  const data = await graphqlRequest<
    { locale?: string },
    { forumCategoryTree: { roots: AdminCategoryTreeNode[] } }
  >(query, { locale }, opts.token, opts.tenantSlug);

  return data.forumCategoryTree?.roots ?? [];
}

export function flattenCategoryTree(
  nodes: AdminCategoryTreeNode[]
): AdminCategoryTreeNode[] {
  const result: AdminCategoryTreeNode[] = [];
  function traverse(list: AdminCategoryTreeNode[]) {
    for (const item of list) {
      result.push(item);
      if (item.children && item.children.length > 0) {
        traverse(item.children);
      }
    }
  }
  traverse(nodes);
  return result;
}

export async function createAdminCategory(
  input: CreateAdminCategoryInput,
  opts: GqlOpts = {}
): Promise<ForumCategoryOption> {
  const mutation = `
    mutation CreateForumCategory($input: CreateForumCategoryInput!) {
      createForumCategory(input: $input) {
        id
        name
        effectiveLocale
      }
    }
  `;

  const data = await graphqlRequest<
    { input: Omit<CreateAdminCategoryInput, 'allowsTopics'> },
    { createForumCategory: ForumCategoryOption }
  >(
    mutation,
    {
      input: {
        locale: input.locale,
        name: input.name,
        slug: input.slug,
        description: input.description,
        icon: input.icon,
        color: input.color,
        parentId: input.parentId,
        position: input.position,
        moderated: input.moderated
      }
    },
    opts.token,
    opts.tenantSlug
  );

  const created = data.createForumCategory;
  if (input.allowsTopics !== undefined) {
    try {
      await setAdminCategoryTopicPolicy(created.id, input.allowsTopics, opts);
    } catch {
      // Best effort for topic policy initial flag
    }
  }

  return created;
}

export async function updateAdminCategory(
  id: string,
  input: UpdateAdminCategoryInput,
  opts: GqlOpts = {}
): Promise<ForumCategoryOption> {
  const mutation = `
    mutation UpdateForumCategory($id: UUID!, $input: UpdateForumCategoryInput!) {
      updateForumCategory(id: $id, input: $input) {
        id
        name
        effectiveLocale
      }
    }
  `;

  const data = await graphqlRequest<
    { id: string; input: Omit<UpdateAdminCategoryInput, 'allowsTopics'> },
    { updateForumCategory: ForumCategoryOption }
  >(
    mutation,
    {
      id,
      input: {
        locale: input.locale,
        name: input.name,
        slug: input.slug,
        description: input.description,
        icon: input.icon,
        color: input.color,
        position: input.position,
        moderated: input.moderated
      }
    },
    opts.token,
    opts.tenantSlug
  );

  if (input.allowsTopics !== undefined && input.allowsTopics !== null) {
    try {
      await setAdminCategoryTopicPolicy(id, input.allowsTopics, opts);
    } catch {
      // Best effort
    }
  }

  return data.updateForumCategory;
}

export async function moveAdminCategory(
  categoryId: string,
  input: { parentId?: string | null; position: number },
  opts: GqlOpts = {}
): Promise<void> {
  const mutation = `
    mutation MoveForumCategory($categoryId: UUID!, $input: MoveForumCategoryInput!) {
      moveForumCategory(categoryId: $categoryId, input: $input) {
        moved { id }
      }
    }
  `;

  await graphqlRequest<
    { categoryId: string; input: { parent_id: string | null; position: number } },
    { moveForumCategory: { moved: { id: string } } }
  >(
    mutation,
    {
      categoryId,
      input: {
        parent_id: input.parentId ?? null,
        position: input.position
      }
    },
    opts.token,
    opts.tenantSlug
  );
}

export async function setAdminCategoryTopicPolicy(
  categoryId: string,
  allowsTopics: boolean,
  opts: GqlOpts = {}
): Promise<void> {
  const mutation = `
    mutation SetForumCategoryTopicPolicy($categoryId: UUID!, $input: UpdateForumCategoryTopicPolicyInput!) {
      setForumCategoryTopicPolicy(categoryId: $categoryId, input: $input) {
        category_id
        allows_topics
      }
    }
  `;

  await graphqlRequest<
    { categoryId: string; input: { allows_topics: boolean } },
    { setForumCategoryTopicPolicy: { category_id: string; allows_topics: boolean } }
  >(
    mutation,
    {
      categoryId,
      input: { allows_topics: allowsTopics }
    },
    opts.token,
    opts.tenantSlug
  );
}

export async function archiveAdminCategorySubtree(
  categoryId: string,
  opts: GqlOpts = {}
): Promise<void> {
  const mutation = `
    mutation ArchiveForumCategorySubtree($categoryId: UUID!) {
      archiveForumCategorySubtree(categoryId: $categoryId) {
        root_id
        archived
      }
    }
  `;

  await graphqlRequest<{ categoryId: string }, unknown>(
    mutation,
    { categoryId },
    opts.token,
    opts.tenantSlug
  );
}

export async function restoreAdminCategorySubtree(
  categoryId: string,
  opts: GqlOpts = {}
): Promise<void> {
  const mutation = `
    mutation RestoreForumCategorySubtree($categoryId: UUID!) {
      restoreForumCategorySubtree(categoryId: $categoryId) {
        root_id
        archived
      }
    }
  `;

  await graphqlRequest<{ categoryId: string }, unknown>(
    mutation,
    { categoryId },
    opts.token,
    opts.tenantSlug
  );
}

export async function deleteAdminCategory(
  id: string,
  opts: GqlOpts = {}
): Promise<void> {
  const mutation = `
    mutation DeleteForumCategory($id: UUID!) {
      deleteForumCategory(id: $id)
    }
  `;

  await graphqlRequest<{ id: string }, { deleteForumCategory: boolean }>(
    mutation,
    { id },
    opts.token,
    opts.tenantSlug
  );
}

export async function pinForumTopic(
  id: string,
  pinned: boolean,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation PinForumTopic($tenantId: UUID, $id: UUID!, $pinned: Boolean!) {
      pinForumTopic(tenantId: $tenantId, id: $id, pinned: $pinned)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string; pinned: boolean },
    { pinForumTopic: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id, pinned },
    opts.token,
    opts.tenantSlug
  );

  return data.pinForumTopic;
}

export async function lockForumTopic(
  id: string,
  locked: boolean,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation LockForumTopic($tenantId: UUID, $id: UUID!, $locked: Boolean!) {
      lockForumTopic(tenantId: $tenantId, id: $id, locked: $locked)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string; locked: boolean },
    { lockForumTopic: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id, locked },
    opts.token,
    opts.tenantSlug
  );

  return data.lockForumTopic;
}

export async function closeForumTopic(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation CloseForumTopic($tenantId: UUID, $id: UUID!) {
      closeForumTopic(tenantId: $tenantId, id: $id)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string },
    { closeForumTopic: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return data.closeForumTopic;
}

export async function reopenForumTopic(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation ReopenForumTopic($tenantId: UUID, $id: UUID!) {
      reopenForumTopic(tenantId: $tenantId, id: $id)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string },
    { reopenForumTopic: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return data.reopenForumTopic;
}

export async function deleteForumTopic(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation DeleteForumTopic($tenantId: UUID, $id: UUID!) {
      deleteForumTopic(tenantId: $tenantId, id: $id)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string },
    { deleteForumTopic: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return data.deleteForumTopic;
}

export async function restoreForumTopic(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation RestoreForumTopic($tenantId: UUID, $id: UUID!) {
      restoreForumTopic(tenantId: $tenantId, id: $id)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string },
    { restoreForumTopic: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return data.restoreForumTopic;
}

export async function deleteForumReply(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation DeleteForumReply($tenantId: UUID, $id: UUID!) {
      deleteForumReply(tenantId: $tenantId, id: $id)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string },
    { deleteForumReply: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return data.deleteForumReply;
}

export async function restoreForumReply(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation RestoreForumReply($tenantId: UUID, $id: UUID!) {
      restoreForumReply(tenantId: $tenantId, id: $id)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; id: string },
    { restoreForumReply: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return data.restoreForumReply;
}

export async function approveForumReply(
  replyId: string,
  topicId: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation ApproveForumReply($tenantId: UUID, $replyId: UUID!, $topicId: UUID!) {
      approveForumReply(tenantId: $tenantId, replyId: $replyId, topicId: $topicId)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; replyId: string; topicId: string },
    { approveForumReply: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, replyId, topicId },
    opts.token,
    opts.tenantSlug
  );

  return data.approveForumReply;
}

export async function rejectForumReply(
  replyId: string,
  topicId: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation RejectForumReply($tenantId: UUID, $replyId: UUID!, $topicId: UUID!) {
      rejectForumReply(tenantId: $tenantId, replyId: $replyId, topicId: $topicId)
    }
  `;

  const data = await graphqlRequest<
    { tenantId?: string | null; replyId: string; topicId: string },
    { rejectForumReply: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId, replyId, topicId },
    opts.token,
    opts.tenantSlug
  );

  return data.rejectForumReply;
}
