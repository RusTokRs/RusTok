import { graphqlRequest } from '@/lib/graphql';

// ---------- Types (matches GraphQL schema, camelCase) ----------

export interface PublicPostSummary {
  id: string;
  title: string;
  slug: string | null;
  excerpt: string | null;
  featuredImageUrl: string | null;
  authorId: string | null;
  tags: string[];
  publishedAt: string | null;
}

export interface PublicPostListResponse {
  items: PublicPostSummary[];
  total: number;
}

// ---------- GraphQL ----------

const PUBLISHED_POSTS_QUERY = `
query PublishedPosts($tenantId: UUID!, $filter: PostsFilter) {
  posts(tenantId: $tenantId, filter: $filter) {
    items {
      id
      title
      slug
      excerpt
      status
      authorId
      createdAt
      publishedAt
    }
    total
  }
}
`;

interface PostsQueryResponse {
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
}

// ---------- Env-based tenant resolution ----------

const TENANT_ID = process.env.NEXT_PUBLIC_TENANT_ID ?? '';
const TENANT_SLUG = process.env.NEXT_PUBLIC_TENANT_SLUG ?? '';

// ---------- API ----------

export async function fetchPublishedPosts(
  page = 1,
  perPage = 6
): Promise<PublicPostListResponse> {
  const data = await graphqlRequest<
    { tenantId: string; filter: { status: string; page: number; perPage: number } },
    PostsQueryResponse
  >(
    PUBLISHED_POSTS_QUERY,
    {
      tenantId: TENANT_ID,
      filter: {
        status: 'PUBLISHED',
        page,
        perPage
      }
    },
    undefined,
    TENANT_SLUG || undefined
  );

  return {
    items: data.posts.items.map((item) => ({
      id: item.id,
      title: item.title,
      slug: item.slug,
      excerpt: item.excerpt,
      featuredImageUrl: null,
      authorId: item.authorId,
      tags: [],
      publishedAt: item.publishedAt
    })),
    total: data.posts.total
  };
}
