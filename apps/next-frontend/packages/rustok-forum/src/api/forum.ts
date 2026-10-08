import { storefrontGraphql } from "@/shared/lib/graphql";
import type { RichTextDocument, RichTextView } from "@rustok/richtext";

export interface ForumCategoryListItem {
  id: string;
  effectiveLocale: string;
  name: string;
  slug: string;
  description: string | null;
  icon: string | null;
  color: string | null;
  parentId?: string | null;
  topicCount: number;
  replyCount: number;
}

export interface ForumTopicListItem {
  id: string;
  effectiveLocale: string;
  categoryId: string;
  authorId: string | null;
  title: string;
  slug: string;
  status: string;
  isPinned: boolean;
  isLocked: boolean;
  replyCount: number;
  createdAt: string;
  isUnread?: boolean;
  unreadCount?: number;
  solutionReplyId?: string | null;
  voteScore?: number;
}

export interface ForumTopicDetail {
  id: string;
  effectiveLocale: string;
  availableLocales: string[];
  categoryId: string;
  authorId: string | null;
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

export interface ForumReplyDetail {
  id: string;
  effectiveLocale: string;
  topicId: string;
  authorId: string | null;
  content: RichTextView;
  contentPlainText: string;
  status: string;
  parentReplyId: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ForumAuthorProfile {
  userId: string;
  handle: string;
  displayName: string;
  tags: string[];
  avatarMediaId: string | null;
  preferredLocale?: string | null;
}

export interface ForumUserStats {
  topicCount: number;
  replyCount: number;
  solutionCount: number;
}

export interface ForumMemberCard {
  userId: string;
  profile: ForumAuthorProfile;
  forumStats: ForumUserStats;
}

export interface ForumQuoteReference {
  targetKind: "TOPIC" | "REPLY";
  targetId: string;
  revisionId: number;
  authorHandle?: string;
  snippet?: string;
}

export interface CreateForumReplyWithQuotesInput {
  locale: string;
  content: RichTextDocument;
  parentReplyId?: string | null;
  quotes?: Array<{
    targetKind: "TOPIC" | "REPLY";
    targetId: string;
    revisionId: number;
  }>;
}

export interface CreateForumTopicWithQuotesInput {
  locale: string;
  categoryId: string;
  title: string;
  slug?: string | null;
  body: RichTextDocument;
  metadata?: Record<string, unknown>;
  tags: string[];
  channelSlugs?: string[];
  quotes?: Array<{
    targetKind: "TOPIC" | "REPLY";
    targetId: string;
    revisionId: number;
  }>;
}

const STOREFRONT_CATEGORIES_QUERY = `
  query StorefrontForumCategories($tenantId: UUID, $locale: String, $pagination: PaginationInput) {
    forumStorefrontCategories(tenantId: $tenantId, locale: $locale, pagination: $pagination) {
      total
      items {
        id effectiveLocale name slug description icon color parentId topicCount replyCount
      }
    }
  }
`;

const STOREFRONT_TOPICS_QUERY = `
  query StorefrontForumAudienceTopics($tenantId: UUID, $categoryId: UUID, $locale: String, $after: String, $perPage: Int) {
    forumStorefrontAudienceTopics(tenantId: $tenantId, categoryId: $categoryId, locale: $locale, after: $after, perPage: $perPage) {
      nextCursor
      items {
        id effectiveLocale categoryId authorId title slug status isPinned isLocked replyCount createdAt solutionReplyId voteScore
      }
    }
  }
`;

const STOREFRONT_UNREAD_TOPICS_QUERY = `
  query StorefrontForumUnreadTopics($tenantId: UUID, $categoryId: UUID, $locale: String, $after: String, $limit: Int) {
    forumStorefrontUnreadTopics(tenantId: $tenantId, categoryId: $categoryId, locale: $locale, after: $after, limit: $limit) {
      nextCursor
      items {
        id effectiveLocale categoryId authorId title slug status isPinned isLocked replyCount createdAt
        readStateExplicit lastReadPosition lastReadRevision unreadCount hasUnreadTopicRevision isUnread
      }
    }
  }
`;

const STOREFRONT_TOPIC_QUERY = `
  query StorefrontForumTopic($tenantId: UUID, $id: UUID!, $locale: String) {
    forumStorefrontAudienceTopic(tenantId: $tenantId, id: $id, locale: $locale) {
      id effectiveLocale availableLocales categoryId authorId title slug
      body { document html }
      bodyPlainText status tags isPinned isLocked replyCount createdAt updatedAt
    }
  }
`;

const STOREFRONT_REPLIES_QUERY = `
  query StorefrontForumReplies($tenantId: UUID, $topicId: UUID!, $locale: String, $after: String, $perPage: Int) {
    forumStorefrontReplies(tenantId: $tenantId, topicId: $topicId, locale: $locale, after: $after, perPage: $perPage) {
      nextCursor
      items {
        id effectiveLocale topicId authorId
        content { document html }
        contentPlainText status parentReplyId createdAt updatedAt
      }
    }
  }
`;

const STOREFRONT_MEMBER_CARDS_QUERY = `
  query StorefrontForumMemberCards($userIds: [UUID!]!, $locale: String) {
    forumMemberCards(userIds: $userIds, locale: $locale) {
      userId
      profile {
        userId handle displayName tags avatarMediaId preferredLocale
      }
      forumStats {
        topicCount replyCount solutionCount
      }
    }
  }
`;

const MARK_TOPIC_READ_MUTATION = `
  mutation MarkStorefrontForumTopicRead($tenantId: UUID, $topicId: UUID!, $locale: String) {
    markForumStorefrontTopicRead(tenantId: $tenantId, topicId: $topicId, locale: $locale) {
      topicId
    }
  }
`;

const CREATE_REPLY_MUTATION = `
  mutation CreateForumReplyWithQuotes($tenantId: UUID, $topicId: UUID!, $input: CreateForumReplyWithQuotesInput!) {
    createForumReplyWithQuotes(tenantId: $tenantId, topicId: $topicId, input: $input) {
      id effectiveLocale topicId authorId
      content { document html }
      contentPlainText status parentReplyId createdAt updatedAt
    }
  }
`;

const CREATE_TOPIC_MUTATION = `
  mutation CreateForumTopicWithQuotes($tenantId: UUID, $input: CreateForumTopicWithQuotesInput!) {
    createForumTopicWithQuotes(tenantId: $tenantId, input: $input) {
      id effectiveLocale categoryId authorId title slug status isPinned isLocked replyCount createdAt updatedAt
    }
  }
`;

export async function fetchStorefrontCategories(options: {
  tenantId?: string;
  tenantSlug?: string;
  locale?: string;
  limit?: number;
  offset?: number;
}): Promise<{ items: ForumCategoryListItem[]; total: number }> {
  const result = await storefrontGraphql<{
    forumStorefrontCategories: { items: ForumCategoryListItem[]; total: number };
  }>({
    query: STOREFRONT_CATEGORIES_QUERY,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      locale: options.locale,
      pagination: {
        offset: options.offset ?? 0,
        limit: options.limit ?? 50,
      },
    },
  });

  return (
    result.data?.forumStorefrontCategories ?? {
      items: [],
      total: 0,
    }
  );
}

export async function fetchStorefrontTopics(options: {
  tenantId?: string;
  tenantSlug?: string;
  categoryId?: string;
  locale?: string;
  limit?: number;
  /** Opaque cursor returned as `nextCursor` by the previous page. */
  after?: string;
}): Promise<{ items: ForumTopicListItem[]; nextCursor: string | null }> {
  const result = await storefrontGraphql<{
    forumStorefrontAudienceTopics: { items: ForumTopicListItem[]; nextCursor: string | null };
  }>({
    query: STOREFRONT_TOPICS_QUERY,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      categoryId: options.categoryId,
      locale: options.locale,
      after: options.after,
      perPage: options.limit ?? 50,
    },
  });

  return (
    result.data?.forumStorefrontAudienceTopics ?? {
      items: [],
      nextCursor: null,
    }
  );
}

export async function fetchStorefrontTopic(options: {
  tenantId?: string;
  tenantSlug?: string;
  topicId: string;
  locale?: string;
}): Promise<ForumTopicDetail | null> {
  const result = await storefrontGraphql<{
    forumStorefrontAudienceTopic: ForumTopicDetail | null;
  }>({
    query: STOREFRONT_TOPIC_QUERY,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      id: options.topicId,
      locale: options.locale,
    },
  });

  return result.data?.forumStorefrontAudienceTopic ?? null;
}

export async function fetchStorefrontReplies(options: {
  tenantId?: string;
  tenantSlug?: string;
  topicId: string;
  locale?: string;
  /** Opaque cursor returned as `nextCursor` by the previous page. */
  after?: string | null;
  perPage?: number;
}): Promise<{ items: ForumReplyDetail[]; nextCursor: string | null }> {
  const result = await storefrontGraphql<{
    forumStorefrontReplies: { items: ForumReplyDetail[]; nextCursor: string | null };
  }>({
    query: STOREFRONT_REPLIES_QUERY,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      topicId: options.topicId,
      locale: options.locale,
      after: options.after ?? null,
      perPage: options.perPage ?? 100,
    },
  });

  return (
    result.data?.forumStorefrontReplies ?? {
      items: [],
      nextCursor: null,
    }
  );
}

export async function fetchForumMemberCards(options: {
  userIds: string[];
  locale?: string;
  tenantSlug?: string;
}): Promise<ForumMemberCard[]> {
  if (options.userIds.length === 0) return [];
  const result = await storefrontGraphql<{
    forumMemberCards: ForumMemberCard[];
  }>({
    query: STOREFRONT_MEMBER_CARDS_QUERY,
    tenant: options.tenantSlug,
    variables: {
      userIds: options.userIds,
      locale: options.locale,
    },
  });

  return result.data?.forumMemberCards ?? [];
}

export async function markForumTopicRead(options: {
  tenantId?: string;
  tenantSlug?: string;
  topicId: string;
  locale?: string;
}): Promise<void> {
  await storefrontGraphql({
    query: MARK_TOPIC_READ_MUTATION,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      topicId: options.topicId,
      locale: options.locale,
    },
  });
}

export async function createForumReply(options: {
  tenantId?: string;
  tenantSlug?: string;
  topicId: string;
  input: CreateForumReplyWithQuotesInput;
}): Promise<ForumReplyDetail | null> {
  const result = await storefrontGraphql<{
    createForumReplyWithQuotes: ForumReplyDetail;
  }>({
    query: CREATE_REPLY_MUTATION,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      topicId: options.topicId,
      input: options.input,
    },
  });

  return result.data?.createForumReplyWithQuotes ?? null;
}

export async function createForumTopic(options: {
  tenantId?: string;
  tenantSlug?: string;
  input: CreateForumTopicWithQuotesInput;
}): Promise<ForumTopicListItem | null> {
  const result = await storefrontGraphql<{
    createForumTopicWithQuotes: ForumTopicListItem;
  }>({
    query: CREATE_TOPIC_MUTATION,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      input: options.input,
    },
  });

  return result.data?.createForumTopicWithQuotes ?? null;
}

export interface StorefrontForumTopicRouteResolution {
  requestedLocale: string;
  requestedShortId: string;
  requestedSlug: string;
  disposition: 'CANONICAL' | 'REDIRECT' | 'GONE';
  canonical: {
    topicId: string;
    locale: string;
    shortId: string;
    slug: string;
    path: string;
  } | null;
}

const STOREFRONT_FORUM_TOPIC_ROUTE_QUERY = `
  query StorefrontForumTopicRouteDecision($tenantId: UUID, $locale: String!, $shortId: String!, $slug: String!) {
    forumStorefrontTopicRouteDecision(tenantId: $tenantId, locale: $locale, shortId: $shortId, slug: $slug) {
      requestedLocale
      requestedShortId
      requestedSlug
      disposition
      canonical {
        topicId
        locale
        shortId
        slug
        path
      }
    }
  }
`;

export async function resolveStorefrontTopicRoute(options: {
  tenantId?: string;
  tenantSlug?: string;
  locale: string;
  shortId: string;
  slug: string;
}): Promise<StorefrontForumTopicRouteResolution | null> {
  const result = await storefrontGraphql<{
    forumStorefrontTopicRouteDecision: StorefrontForumTopicRouteResolution | null;
  }>({
    query: STOREFRONT_FORUM_TOPIC_ROUTE_QUERY,
    tenant: options.tenantSlug,
    variables: {
      tenantId: options.tenantId,
      locale: options.locale,
      shortId: options.shortId,
      slug: options.slug,
    },
  });

  return result.data?.forumStorefrontTopicRouteDecision ?? null;
}

