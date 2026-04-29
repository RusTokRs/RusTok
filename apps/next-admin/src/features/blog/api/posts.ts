import { graphqlRequest } from '@/lib/graphql';

// ---------- GqlOpts ----------

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

// ---------- Types (matches GraphQL schema, camelCase) ----------

export type BlogPostStatus = 'DRAFT' | 'PUBLISHED' | 'ARCHIVED';

export interface PostSummary {
  id: string;
  title: string;
  slug: string | null;
  excerpt: string | null;
  status: BlogPostStatus;
  authorId: string | null;
  createdAt: string;
  publishedAt: string | null;
}

export interface PostResponse {
  id: string;
  title: string;
  slug: string | null;
  excerpt: string | null;
  body: string | null;
  contentJson?: Record<string, unknown> | null;
  status: BlogPostStatus;
  authorId: string | null;
  createdAt: string;
  publishedAt: string | null;
  tags: string[];
  featuredImageUrl: string | null;
  seoTitle: string | null;
  seoDescription: string | null;
}

export interface PostListResponse {
  items: PostSummary[];
  total: number;
}

export interface PostListQuery {
  status?: BlogPostStatus;
  authorId?: string;
  locale?: string;
  page?: number;
  perPage?: number;
}

export interface CreatePostInput {
  locale: string;
  title: string;
  body: string;
  bodyFormat?: 'markdown' | 'rt_json_v1';
  contentJson?: Record<string, unknown>;
  excerpt?: string;
  slug?: string;
  publish: boolean;
  tags: string[];
  categoryId?: string;
  featuredImageUrl?: string;
  seoTitle?: string;
  seoDescription?: string;
}

export interface UpdatePostInput {
  locale?: string;
  title?: string;
  body?: string;
  bodyFormat?: 'markdown' | 'rt_json_v1';
  contentJson?: Record<string, unknown>;
  excerpt?: string;
  slug?: string;
  status?: BlogPostStatus;
  tags?: string[];
  categoryId?: string;
  featuredImageUrl?: string;
  seoTitle?: string;
  seoDescription?: string;
}

// ---------- GraphQL queries & mutations ----------

const POSTS_QUERY = `
query Posts($tenantId: UUID!, $filter: PostsFilter) {
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

const POST_QUERY = `
query Post($tenantId: UUID!, $id: UUID!) {
  post(tenantId: $tenantId, id: $id) {
    id
    title
    slug
    excerpt
    body
    contentJson
    status
    authorId
    createdAt
    publishedAt
    tags
    featuredImageUrl
    seoTitle
    seoDescription
  }
}
`;

const CREATE_POST_MUTATION = `
mutation CreatePost($tenantId: UUID!, $input: CreatePostInput!) {
  createPost(tenantId: $tenantId, input: $input)
}
`;

const UPDATE_POST_MUTATION = `
mutation UpdatePost($id: UUID!, $tenantId: UUID!, $input: UpdatePostInput!) {
  updatePost(id: $id, tenantId: $tenantId, input: $input)
}
`;

const DELETE_POST_MUTATION = `
mutation DeletePost($id: UUID!, $tenantId: UUID!) {
  deletePost(id: $id, tenantId: $tenantId)
}
`;

const PUBLISH_POST_MUTATION = `
mutation PublishPost($id: UUID!, $tenantId: UUID!) {
  publishPost(id: $id, tenantId: $tenantId)
}
`;

const UNPUBLISH_POST_MUTATION = `
mutation UnpublishPost($id: UUID!, $tenantId: UUID!) {
  unpublishPost(id: $id, tenantId: $tenantId)
}
`;

// ---------- Response types ----------

interface PostsQueryResponse {
  posts: PostListResponse;
}

interface PostQueryResponse {
  post: PostResponse | null;
}

interface CreatePostResponse {
  createPost: string;
}

interface UpdatePostResponse {
  updatePost: boolean;
}

interface DeletePostResponse {
  deletePost: boolean;
}

interface PublishPostResponse {
  publishPost: boolean;
}

interface UnpublishPostResponse {
  unpublishPost: boolean;
}

// ---------- API functions ----------

export async function listPosts(
  query: PostListQuery,
  opts: GqlOpts = {}
): Promise<PostListResponse> {
  const filter: Record<string, unknown> = {};
  if (query.status) filter.status = query.status;
  if (query.authorId) filter.authorId = query.authorId;
  if (query.locale) filter.locale = query.locale;
  if (query.page) filter.page = query.page;
  if (query.perPage) filter.perPage = query.perPage;

  const data = await graphqlRequest<
    { tenantId: string; filter?: Record<string, unknown> },
    PostsQueryResponse
  >(
    POSTS_QUERY,
    {
      tenantId: opts.tenantId!,
      filter: Object.keys(filter).length > 0 ? filter : undefined
    },
    opts.token,
    opts.tenantSlug
  );
  return data.posts;
}

export async function getPost(
  id: string,
  opts: GqlOpts = {}
): Promise<PostResponse | null> {
  const data = await graphqlRequest<
    { tenantId: string; id: string },
    PostQueryResponse
  >(POST_QUERY, { tenantId: opts.tenantId!, id }, opts.token, opts.tenantSlug);
  return data.post;
}

/**
 * Example (legacy): { bodyFormat: "markdown", body: "# Title" }
 * Example (rich): { bodyFormat: "rt_json_v1", contentJson: { version: "rt_json_v1", locale: "en", doc: { type: "doc", content: [] } }, body: "" }
 */
export async function createPost(
  input: CreatePostInput,
  opts: GqlOpts = {}
): Promise<string> {
  const data = await graphqlRequest<
    { tenantId: string; input: CreatePostInput },
    CreatePostResponse
  >(
    CREATE_POST_MUTATION,
    { tenantId: opts.tenantId!, input },
    opts.token,
    opts.tenantSlug
  );
  return data.createPost;
}

/**
 * Example (legacy): { bodyFormat: "markdown", body: "Updated markdown" }
 * Example (rich): { bodyFormat: "rt_json_v1", contentJson: { version: "rt_json_v1", locale: "en", doc: { type: "doc", content: [] } } }
 */
export async function updatePost(
  id: string,
  input: UpdatePostInput,
  opts: GqlOpts = {}
): Promise<void> {
  await graphqlRequest<
    { id: string; tenantId: string; input: UpdatePostInput },
    UpdatePostResponse
  >(
    UPDATE_POST_MUTATION,
    { id, tenantId: opts.tenantId!, input },
    opts.token,
    opts.tenantSlug
  );
}

export async function deletePost(
  id: string,
  opts: GqlOpts = {}
): Promise<void> {
  await graphqlRequest<{ id: string; tenantId: string }, DeletePostResponse>(
    DELETE_POST_MUTATION,
    { id, tenantId: opts.tenantId! },
    opts.token,
    opts.tenantSlug
  );
}

export async function publishPost(
  id: string,
  opts: GqlOpts = {}
): Promise<void> {
  await graphqlRequest<{ id: string; tenantId: string }, PublishPostResponse>(
    PUBLISH_POST_MUTATION,
    { id, tenantId: opts.tenantId! },
    opts.token,
    opts.tenantSlug
  );
}

export async function unpublishPost(
  id: string,
  opts: GqlOpts = {}
): Promise<void> {
  await graphqlRequest<{ id: string; tenantId: string }, UnpublishPostResponse>(
    UNPUBLISH_POST_MUTATION,
    { id, tenantId: opts.tenantId! },
    opts.token,
    opts.tenantSlug
  );
}
