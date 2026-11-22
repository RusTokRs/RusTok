import { graphqlRequest } from '@/lib/graphql';

// ---------- GqlOpts ----------

export interface StatsGqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

// ---------- Types ----------

export interface BlogStats {
  total: number;
  draft: number;
  published: number;
  archived: number;
}

interface BlogStatsQueryResponse {
  blogStats: BlogStats;
}

// ---------- API functions ----------

export const BLOG_STATS_QUERY = `
query BlogAdminStats($tenantId: UUID!) {
  blogStats(tenantId: $tenantId) {
    total
    draft
    published
    archived
  }
}
`;

export async function fetchBlogStats(
  opts: StatsGqlOpts = {}
): Promise<BlogStats> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to view blog statistics.');
  }

  const data = await graphqlRequest<
    { tenantId: string },
    BlogStatsQueryResponse
  >(
    BLOG_STATS_QUERY,
    { tenantId: opts.tenantId },
    opts.token,
    opts.tenantSlug
  );

  return data.blogStats;
}
