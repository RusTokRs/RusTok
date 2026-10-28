import type { storefrontGraphql } from "@/shared/lib/graphql";
import type { RichTextDocument, RichTextView } from "@rustok/richtext";

export type BlogGraphqlExecutor = typeof storefrontGraphql;

export interface BlogPostAuthorProfile {
  userId: string;
  handle: string;
  displayName: string;
  tags: string[];
  avatarMediaId: string | null;
}

export interface BlogPostSummary {
  id: string;
  title: string;
  slug: string | null;
  excerpt: string | null;
  featuredImageUrl: string | null;
  authorId: string | null;
  authorProfile?: BlogPostAuthorProfile | null;
  categoryId?: string | null;
  categoryName?: string | null;
  tags: string[];
  publishedAt: string | null;
}

export interface BlogPostListResponse {
  items: BlogPostSummary[];
  /** Opaque cursor for the next page; null when this is the last page. */
  nextCursor: string | null;
}

export interface BlogPublicComment {
  id: string;
  effectiveLocale: string;
  authorId: string | null;
  contentPreview: string;
  parentCommentId: string | null;
  createdAt: string;
}

export interface BlogPostDetail extends BlogPostSummary {
  effectiveLocale: string;
  version?: number;
  seoTitle: string | null;
  seoDescription: string | null;
  updatedAt: string;
  content: RichTextView;
  contentPlainText: string;
  publicComments: {
    availability: "AVAILABLE" | "DISABLED" | "READ_ONLY" | "UNAVAILABLE" | "TIMEOUT";
    cachedSnapshot: boolean;
    items: BlogPublicComment[];
    total: number;
  };
}

export interface BlogCommentDetail {
  id: string;
  requestedLocale: string;
  effectiveLocale: string;
  postId: string;
  authorId: string | null;
  content: RichTextView;
  contentPlainText: string;
  status: string;
  parentCommentId: string | null;
  createdAt: string;
  updatedAt: string;
}

type PostsQueryResponse = {
  posts: {
    items: Array<{
      id: string;
      title: string;
      slug: string | null;
      excerpt: string | null;
      featuredImageUrl: string | null;
      authorId: string | null;
      authorProfile?: BlogPostAuthorProfile | null;
      categoryId?: string | null;
      categoryName?: string | null;
      tags: string[];
      publishedAt: string | null;
    }>;
    nextCursor?: string | null;
  };
};

const PUBLISHED_POSTS_QUERY = `
  query PublishedPosts($tenantId: UUID!, $filter: PublicPostsFilter) {
    posts: publicBlogPosts(tenantId: $tenantId, filter: $filter) {
      items {
        id title slug excerpt featuredImageUrl authorId categoryId categoryName tags publishedAt
        authorProfile {
          userId handle displayName tags avatarMediaId
        }
      }
      nextCursor
    }
  }
`;

const PUBLISHED_POST_QUERY = `
  query PublishedPost($tenantId: UUID!, $slug: String!, $locale: String, $commentsPage: Int, $commentsPerPage: Int) {
    postBySlug(tenantId: $tenantId, slug: $slug, locale: $locale) {
      id title slug excerpt featuredImageUrl authorId categoryId categoryName tags publishedAt effectiveLocale version
      seoTitle seoDescription updatedAt
      authorProfile {
        userId handle displayName tags avatarMediaId
      }
      content { document html }
      contentPlainText
      publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage) {
        availability cachedSnapshot total
        items { id effectiveLocale authorId contentPreview parentCommentId createdAt }
      }
    }
  }
`;

const CREATE_BLOG_COMMENT_MUTATION = `
  mutation CreateBlogComment($tenantId: UUID!, $postId: UUID!, $input: CreateBlogCommentInput!) {
    createBlogComment(tenantId: $tenantId, postId: $postId, input: $input) {
      id requestedLocale effectiveLocale postId authorId
      content { document html }
      contentPlainText status parentCommentId createdAt updatedAt
    }
  }
`;

export async function fetchPublishedPosts(
  graphql: BlogGraphqlExecutor,
  tenantId: string,
  tenantSlug: string | null,
  after: string | null = null,
  perPage = 6,
  tag?: string,
  categoryId?: string,
  locale?: string,
): Promise<BlogPostListResponse> {
  const filter: {
    perPage: number;
    after?: string;
    tag?: string;
    categoryId?: string;
    locale?: string;
  } = { perPage };
  if (after) filter.after = after;
  if (tag) filter.tag = tag;
  if (categoryId) filter.categoryId = categoryId;
  // Requested locale drives the read; the backend applies the canonical
  // fallback chain (requested -> tenant default -> first available).
  if (locale) filter.locale = locale;

  const response = await graphql<PostsQueryResponse, {
    tenantId: string;
    filter: typeof filter;
  }>({
    query: PUBLISHED_POSTS_QUERY,
    variables: { tenantId, filter },
    tenant: tenantSlug ?? undefined,
  });

  if (response.errors?.length || !response.data) {
    throw new Error(response.errors?.[0]?.message ?? "Blog posts payload is missing");
  }

  return {
    items: response.data.posts.items,
    nextCursor: response.data.posts.nextCursor ?? null,
  };
}

export async function fetchPublishedPost(
  graphql: BlogGraphqlExecutor,
  tenantId: string,
  tenantSlug: string | null,
  slug: string,
  locale: string,
  commentsPage = 1,
  commentsPerPage = 20,
): Promise<BlogPostDetail | null> {
  const response = await graphql<{ postBySlug: BlogPostDetail | null }, {
    tenantId: string;
    slug: string;
    locale: string;
    commentsPage: number;
    commentsPerPage: number;
  }>({
    query: PUBLISHED_POST_QUERY,
    variables: { tenantId, slug, locale, commentsPage, commentsPerPage },
    tenant: tenantSlug ?? undefined,
  });
  if (response.errors?.length || !response.data) {
    throw new Error(response.errors?.[0]?.message ?? "Blog post payload is missing");
  }
  return response.data.postBySlug;
}

export async function createBlogComment(
  graphql: BlogGraphqlExecutor,
  tenantId: string,
  tenantSlug: string,
  token: string,
  postId: string,
  locale: string,
  content: RichTextDocument,
  commandId: string,
  parentCommentId?: string | null,
): Promise<BlogCommentDetail> {
  const response = await graphql<{ createBlogComment: BlogCommentDetail }, {
    tenantId: string;
    postId: string;
    input: {
      commandId: string;
      locale: string;
      content: RichTextDocument;
      parentCommentId?: string | null;
    };
  }>({
    query: CREATE_BLOG_COMMENT_MUTATION,
    variables: {
      tenantId,
      postId,
      input: {
        commandId,
        locale,
        content,
        parentCommentId: parentCommentId ?? null,
      },
    },
    token,
    tenant: tenantSlug,
  });
  if (response.errors?.length || !response.data) {
    throw new Error(response.errors?.[0]?.message ?? "Comment payload is missing");
  }
  return response.data.createBlogComment;
}

export interface BlogCategorySummary {
  id: string;
  name: string;
  slug: string;
  description: string | null;
}

const BLOG_CATEGORIES_QUERY = `
  query BlogCategories($tenantId: UUID, $filter: BlogCategoriesFilter) {
    blogCategories(tenantId: $tenantId, filter: $filter) {
      items {
        id name slug description
      }
      total
    }
  }
`;

/** Load the full category dictionary, not just the first GraphQL page. */
export async function fetchBlogCategories(
  graphql: BlogGraphqlExecutor,
  tenantId?: string | null,
  tenantSlug?: string | null,
  locale?: string,
): Promise<BlogCategorySummary[]> {
  const items: BlogCategorySummary[] = [];
  const pageSize = 100;
  for (let page = 1; ; page += 1) {
    const response = await graphql<{
      blogCategories: { items: BlogCategorySummary[]; total: number };
    }, {
      tenantId?: string | null;
      filter: { locale?: string; page: number; perPage: number };
    }>({
      query: BLOG_CATEGORIES_QUERY,
      variables: {
        tenantId,
        filter: { locale, page, perPage: pageSize },
      },
      tenant: tenantSlug ?? undefined,
    });
    if (response.errors?.length || !response.data) {
      throw new Error(response.errors?.[0]?.message ?? "Blog categories payload is missing");
    }

    const batch = response.data.blogCategories;
    items.push(...batch.items);
    if (items.length >= batch.total) return items;
    // Do not spin forever on a broken or inconsistent server pagination response.
    if (batch.items.length === 0) {
      throw new Error("Blog category pagination returned an incomplete result");
    }
  }
}

/** Public taxonomy tags are not limited to the current post page. */
export async function fetchBlogTags(
  graphql: BlogGraphqlExecutor,
  tenantId?: string | null,
  tenantSlug?: string | null,
  locale?: string,
): Promise<string[]> {
  const query = `
    query BlogTags($tenantId: UUID, $filter: BlogTagsFilter) {
      blogTags(tenantId: $tenantId, filter: $filter) {
        items { name }
        total
      }
    }
  `;
  const names = new Set<string>();
  const pageSize = 100;
  for (let page = 1; ; page += 1) {
    const response = await graphql<{
      blogTags: { items: Array<{ name: string }>; total: number };
    }, {
      tenantId?: string | null;
      filter: { locale?: string; page: number; perPage: number };
    }>({
      query,
      variables: { tenantId, filter: { locale, page, perPage: pageSize } },
      tenant: tenantSlug ?? undefined,
    });
    if (response.errors?.length || !response.data) {
      throw new Error(response.errors?.[0]?.message ?? "Blog tags payload is missing");
    }
    const batch = response.data.blogTags;
    for (const item of batch.items) names.add(item.name);
    // Page boundaries are defined by the Blog taxonomy owner, independently of
    // duplicate localized labels or orphaned derived usage rows.
    if (page * pageSize >= batch.total) return [...names];
    if (batch.items.length === 0) {
      throw new Error("Blog tag pagination returned an incomplete result");
    }
  }
}
