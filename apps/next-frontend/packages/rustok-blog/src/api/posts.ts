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
    total: number;
  };
};

const PUBLISHED_POSTS_QUERY = `
  query PublishedPosts($tenantId: UUID!, $filter: PostsFilter) {
    posts(tenantId: $tenantId, filter: $filter) {
      items {
        id title slug excerpt featuredImageUrl authorId categoryId categoryName tags publishedAt
        authorProfile {
          userId handle displayName tags avatarMediaId
        }
      }
      total
    }
  }
`;

const PUBLISHED_POST_QUERY = `
  query PublishedPost($tenantId: UUID!, $slug: String!, $locale: String, $commentsPage: Int, $commentsPerPage: Int) {
    postBySlug(tenantId: $tenantId, slug: $slug, locale: $locale) {
      id title slug excerpt featuredImageUrl authorId categoryId categoryName tags publishedAt effectiveLocale
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
  page = 1,
  perPage = 6,
  tag?: string,
  categoryId?: string,
): Promise<BlogPostListResponse> {
  const filter: {
    status: string;
    page: number;
    perPage: number;
    tag?: string;
    categoryId?: string;
  } = {
    status: "PUBLISHED",
    page,
    perPage,
  };
  if (tag) filter.tag = tag;
  if (categoryId) filter.categoryId = categoryId;

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
    total: response.data.posts.total,
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
): Promise<BlogCommentDetail> {
  const response = await graphql<{ createBlogComment: BlogCommentDetail }, {
    tenantId: string;
    postId: string;
    input: {
      commandId: string;
      locale: string;
      content: RichTextDocument;
      parentCommentId: null;
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
        parentCommentId: null,
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
