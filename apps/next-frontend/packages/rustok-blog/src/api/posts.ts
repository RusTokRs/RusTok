import type { storefrontGraphql } from "@/shared/lib/graphql";
import type { RichTextDocument, RichTextView } from "@rustok/richtext";

export type BlogGraphqlExecutor = typeof storefrontGraphql;

export interface BlogPostSummary {
  id: string;
  title: string;
  slug: string | null;
  excerpt: string | null;
  featuredImageUrl: string | null;
  authorId: string | null;
  tags: string[];
  publishedAt: string | null;
}

export interface BlogPostListResponse {
  items: BlogPostSummary[];
  total: number;
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
  content: RichTextView;
  contentPlainText: string;
  publicComments: {
    availability: "AVAILABLE" | "UNAVAILABLE" | "TIMEOUT";
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
      authorId: string | null;
      publishedAt: string | null;
    }>;
    total: number;
  };
};

const PUBLISHED_POSTS_QUERY = `
  query PublishedPosts($tenantId: UUID!, $filter: PostsFilter) {
    posts(tenantId: $tenantId, filter: $filter) {
      items { id title slug excerpt authorId publishedAt }
      total
    }
  }
`;

const PUBLISHED_POST_QUERY = `
  query PublishedPost($tenantId: UUID!, $slug: String!, $locale: String) {
    postBySlug(tenantId: $tenantId, slug: $slug, locale: $locale) {
      id title slug excerpt featuredImageUrl authorId tags publishedAt effectiveLocale
      content { document html }
      contentPlainText
      publicComments(locale: $locale, page: 1, perPage: 20) {
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
  page = 1,
  perPage = 6,
): Promise<BlogPostListResponse> {
  const response = await graphql<PostsQueryResponse, {
    tenantId: string;
    filter: { status: string; page: number; perPage: number };
  }>({
    query: PUBLISHED_POSTS_QUERY,
    variables: { tenantId, filter: { status: "PUBLISHED", page, perPage } },
    tenant: tenantSlug ?? undefined,
  });

  if (response.errors?.length || !response.data) {
    throw new Error(response.errors?.[0]?.message ?? "Blog posts payload is missing");
  }

  return {
    items: response.data.posts.items.map((item) => ({ ...item, featuredImageUrl: null, tags: [] })),
    total: response.data.posts.total,
  };
}

export async function fetchPublishedPost(
  graphql: BlogGraphqlExecutor,
  tenantId: string,
  tenantSlug: string | null,
  slug: string,
  locale: string,
): Promise<BlogPostDetail | null> {
  const response = await graphql<{ postBySlug: BlogPostDetail | null }, {
    tenantId: string;
    slug: string;
    locale: string;
  }>({
    query: PUBLISHED_POST_QUERY,
    variables: { tenantId, slug, locale },
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
): Promise<BlogCommentDetail> {
  const response = await graphql<{ createBlogComment: BlogCommentDetail }, {
    tenantId: string;
    postId: string;
    input: { locale: string; content: RichTextDocument; parentCommentId: null };
  }>({
    query: CREATE_BLOG_COMMENT_MUTATION,
    variables: {
      tenantId,
      postId,
      input: { locale, content, parentCommentId: null },
    },
    token,
    tenant: tenantSlug,
  });
  if (response.errors?.length || !response.data) {
    throw new Error(response.errors?.[0]?.message ?? "Comment payload is missing");
  }
  return response.data.createBlogComment;
}
