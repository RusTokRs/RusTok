import type { storefrontGraphql } from "@/shared/lib/graphql";

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
