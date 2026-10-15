import { storefrontGraphql, type GraphqlResponse } from "@/shared/lib/graphql";
import type { RichTextDocument, RichTextView } from "@rustok/richtext";
import { collectForumCategoryPages, readForumCategoryConnection, type ForumCategoryConnection } from './category-pages';

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
      pageInfo { totalCount }
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
    forumStorefrontUnreadTopics(tenantId: $tenantId, categoryId: $categoryId, locale: $locale, after: $after, limit: $limit, unreadOnly: true) {
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

const STOREFRONT_TOPIC_QUOTE_REVISION_QUERY = `
  query StorefrontForumTopicQuoteRevision($tenantId: UUID, $id: UUID!, $locale: String) {
    forumStorefrontTopicQuoteRevision(tenantId: $tenantId, id: $id, locale: $locale)
  }
`;

const STOREFRONT_REPLY_QUOTE_REVISION_QUERY = `
  query StorefrontForumReplyQuoteRevision($tenantId: UUID, $id: UUID!, $locale: String) {
    forumStorefrontReplyQuoteRevision(tenantId: $tenantId, id: $id, locale: $locale)
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

/**
 * `storefrontGraphql` resolves with `{ data, errors }` under Apollo's `errorPolicy: "all"`,
 * so a denied or failed field is indistinguishable from an empty list unless `errors` is
 * checked. Every forum read fails loudly here and each surface decides how to present the
 * failure; a null field without errors stays a legitimate "nothing here" result.
 */
function assertGraphqlResult(operation: string, result: GraphqlResponse<unknown>): void {
  const message = result.errors?.[0]?.message;
  if (message) {
    throw new Error(`${operation}: ${message}`);
  }
}

export async function fetchStorefrontCategories(options: {
  tenantId?: string;
  tenantSlug?: string;
  token?: string;
  locale?: string;
  limit?: number;
  offset?: number;
}): Promise<{ items: ForumCategoryListItem[]; total: number }> {
  const result = await storefrontGraphql<{
    forumStorefrontCategories: ForumCategoryConnection | null;
  }>({
    query: STOREFRONT_CATEGORIES_QUERY,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      locale: options.locale,
      pagination: {
        offset: options.offset ?? 0,
        // No owner setting governs the category rail, so the widget keeps its own bound.
        limit: options.limit ?? 50,
      },
    },
  });

  assertGraphqlResult("forum categories", result);
  return readForumCategoryConnection(result.data?.forumStorefrontCategories);
}

/** Load the complete bounded, audience-visible category tree for navigation and posting. */
export async function fetchAllStorefrontCategories(options: {
  tenantId?: string;
  tenantSlug?: string;
  token?: string;
  locale?: string;
}): Promise<{ items: ForumCategoryListItem[]; total: number }> {
  return collectForumCategoryPages((offset, limit) =>
    fetchStorefrontCategories({ ...options, offset, limit })
  );
}

export async function fetchStorefrontTopics(options: {
  tenantId?: string;
  tenantSlug?: string;
  token?: string;
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
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      categoryId: options.categoryId,
      locale: options.locale,
      after: options.after,
      // Omitted page sizes resolve through the tenant's `topics_per_page` setting.
      perPage: options.limit,
    },
  });

  assertGraphqlResult("forum topic page", result);
  return (
    result.data?.forumStorefrontAudienceTopics ?? {
      items: [],
      nextCursor: null,
    }
  );
}

/** Read-state is an authenticated owner page, not a filter of the public feed. */
export async function fetchStorefrontUnreadTopics(options: {
  tenantId?: string;
  tenantSlug?: string;
  token: string;
  categoryId?: string;
  locale?: string;
  limit?: number;
  after?: string;
}): Promise<{ items: ForumTopicListItem[]; nextCursor: string | null }> {
  const result = await storefrontGraphql<{
    forumStorefrontUnreadTopics: { items: ForumTopicListItem[]; nextCursor: string | null };
  }>({
    query: STOREFRONT_UNREAD_TOPICS_QUERY,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      categoryId: options.categoryId,
      locale: options.locale,
      after: options.after,
      limit: options.limit,
    },
  });

  assertGraphqlResult("forum unread topic page", result);
  return result.data?.forumStorefrontUnreadTopics ?? { items: [], nextCursor: null };
}

export async function fetchStorefrontTopic(options: {
  tenantId?: string;
  tenantSlug?: string;
  token?: string;
  topicId: string;
  locale?: string;
}): Promise<ForumTopicDetail | null> {
  const result = await storefrontGraphql<{
    forumStorefrontAudienceTopic: ForumTopicDetail | null;
  }>({
    query: STOREFRONT_TOPIC_QUERY,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      id: options.topicId,
      locale: options.locale,
    },
  });

  assertGraphqlResult("forum topic", result);
  return result.data?.forumStorefrontAudienceTopic ?? null;
}

export async function fetchStorefrontReplies(options: {
  tenantId?: string;
  tenantSlug?: string;
  token?: string;
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
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      topicId: options.topicId,
      locale: options.locale,
      after: options.after ?? null,
      // Omitted page sizes resolve through the tenant's `replies_per_page` setting.
      perPage: options.perPage,
    },
  });

  assertGraphqlResult("forum reply page", result);
  return (
    result.data?.forumStorefrontReplies ?? {
      items: [],
      nextCursor: null,
    }
  );
}

export async function fetchStorefrontQuoteRevision(options: {
  tenantId?: string;
  tenantSlug?: string;
  token?: string;
  targetKind: "TOPIC" | "REPLY";
  targetId: string;
  locale?: string;
}): Promise<number | null> {
  const isTopic = options.targetKind === "TOPIC";
  const result = await storefrontGraphql<{
    forumStorefrontTopicQuoteRevision?: string | null;
    forumStorefrontReplyQuoteRevision?: string | null;
  }>({
    query: isTopic ? STOREFRONT_TOPIC_QUOTE_REVISION_QUERY : STOREFRONT_REPLY_QUOTE_REVISION_QUERY,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      id: options.targetId,
      locale: options.locale,
    },
  });

  assertGraphqlResult("forum quote revision", result);
  const id = isTopic
    ? result.data?.forumStorefrontTopicQuoteRevision
    : result.data?.forumStorefrontReplyQuoteRevision;
  // GraphQL returns the global BIGINT as a string. JS numbers cannot safely
  // pin values above MAX_SAFE_INTEGER; fall back to an unlinked text quote.
  if (!id || !/^[1-9]\d*$/.test(id)) return null;
  const revision = Number(id);
  return Number.isSafeInteger(revision) ? revision : null;
}

/** Authenticated member cards are supplementary. The owner accepts at most 100
 * distinct IDs per request; chunk deep-linked threads rather than failing the
 * whole profile batch after following several reply pages. */
export async function fetchForumMemberCards(options: {
  userIds: string[];
  locale?: string;
  tenantSlug?: string;
  token: string;
}): Promise<ForumMemberCard[]> {
  const ids = Array.from(new Set(options.userIds));
  const cards: ForumMemberCard[] = [];
  for (let offset = 0; offset < ids.length; offset += 100) {
    const result = await storefrontGraphql<{
      forumMemberCards: ForumMemberCard[];
    }>({
      query: STOREFRONT_MEMBER_CARDS_QUERY,
      tenant: options.tenantSlug,
      token: options.token,
      variables: { userIds: ids.slice(offset, offset + 100), locale: options.locale },
    });
    assertGraphqlResult("forum member cards", result);
    cards.push(...(result.data?.forumMemberCards ?? []));
  }
  return cards;
}

export async function markForumTopicRead(options: {
  tenantId?: string;
  tenantSlug?: string;
  token: string;
  topicId: string;
  locale?: string;
}): Promise<void> {
  const result = await storefrontGraphql({
    query: MARK_TOPIC_READ_MUTATION,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      topicId: options.topicId,
      locale: options.locale,
    },
  });
  assertGraphqlResult("mark forum topic read", result);
}

export async function createForumReply(options: {
  tenantId?: string;
  tenantSlug?: string;
  token: string;
  topicId: string;
  input: CreateForumReplyWithQuotesInput;
}): Promise<ForumReplyDetail | null> {
  const result = await storefrontGraphql<{
    createForumReplyWithQuotes: ForumReplyDetail;
  }>({
    query: CREATE_REPLY_MUTATION,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      topicId: options.topicId,
      input: options.input,
    },
  });

  assertGraphqlResult("forum reply creation", result);
  return result.data?.createForumReplyWithQuotes ?? null;
}

export async function createForumTopic(options: {
  tenantId?: string;
  tenantSlug?: string;
  token: string;
  input: CreateForumTopicWithQuotesInput;
}): Promise<ForumTopicListItem | null> {
  const result = await storefrontGraphql<{
    createForumTopicWithQuotes: ForumTopicListItem;
  }>({
    query: CREATE_TOPIC_MUTATION,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      input: options.input,
    },
  });

  assertGraphqlResult("forum topic creation", result);
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
  token?: string;
  locale: string;
  shortId: string;
  slug: string;
}): Promise<StorefrontForumTopicRouteResolution | null> {
  const result = await storefrontGraphql<{
    forumStorefrontTopicRouteDecision: StorefrontForumTopicRouteResolution | null;
  }>({
    query: STOREFRONT_FORUM_TOPIC_ROUTE_QUERY,
    tenant: options.tenantSlug,
    token: options.token,
    variables: {
      tenantId: options.tenantId,
      locale: options.locale,
      shortId: options.shortId,
      slug: options.slug,
    },
  });

  assertGraphqlResult("forum topic route", result);
  return result.data?.forumStorefrontTopicRouteDecision ?? null;
}

