import { graphqlRequest } from '@/lib/graphql';
import type { RichTextDocument, RichTextView } from '@rustok/richtext';

// ---------- GqlOpts ----------

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

// ---------- Types (matches GraphQL schema, camelCase) ----------

export type BlogPostStatus = 'DRAFT' | 'PUBLISHED' | 'ARCHIVED';

/**
 * Sort columns the owner keyset query indexes. The GraphQL enum (`BlogPostSortField`) and the
 * owner query use exactly these spellings, so the wire contract has a single owner here.
 */
export const BLOG_POST_SORT_FIELDS = [
  'CREATED_AT',
  'UPDATED_AT',
  'PUBLISHED_AT'
] as const;

export type BlogPostSortField = (typeof BLOG_POST_SORT_FIELDS)[number];

/** Sort direction of the owner keyset order (`BlogPostSortOrder`). */
export const BLOG_POST_SORT_ORDERS = ['DESC', 'ASC'] as const;

export type BlogPostSortOrder = (typeof BLOG_POST_SORT_ORDERS)[number];

/** Owner defaults, mirrored so the UI shows the order the list is actually returned in. */
export const DEFAULT_POST_SORT_FIELD: BlogPostSortField = 'CREATED_AT';
export const DEFAULT_POST_SORT_ORDER: BlogPostSortOrder = 'DESC';

/**
 * Narrows an untrusted route value to a sort column the owner accepts.
 *
 * The GraphQL enum rejects an unknown column, and the owner refuses a page whose cursor was
 * issued for a different sort, so a bad query parameter falls back to the default instead of
 * turning into a failed list request.
 */
export function parsePostSortField(
  value: string | undefined
): BlogPostSortField {
  return (
    BLOG_POST_SORT_FIELDS.find((field) => field === value) ??
    DEFAULT_POST_SORT_FIELD
  );
}

/** Narrows an untrusted route value to a sort direction the owner accepts. */
export function parsePostSortOrder(
  value: string | undefined
): BlogPostSortOrder {
  return (
    BLOG_POST_SORT_ORDERS.find((order) => order === value) ??
    DEFAULT_POST_SORT_ORDER
  );
}

export interface PostSummary {
  id: string;
  title: string;
  slug: string | null;
  excerpt: string | null;
  status: BlogPostStatus;
  authorId: string | null;
  createdAt: string;
  publishedAt: string | null;
  scheduledAt: string | null;
}

export interface PostResponse {
  id: string;
  requestedLocale: string;
  effectiveLocale: string;
  title: string;
  slug: string | null;
  excerpt: string | null;
  content?: RichTextView | null;
  contentPlainText?: string | null;
  status: BlogPostStatus;
  authorId: string | null;
  categoryId?: string | null;
  categoryName?: string | null;
  channelSlugs?: string[];
  version: number;
  createdAt: string;
  publishedAt: string | null;
  scheduledAt: string | null;
  tags: string[];
  featuredImageUrl: string | null;
  seoTitle: string | null;
  seoDescription: string | null;
}

export interface PostListResponse {
  items: PostSummary[];
  /** Present only when another page exists. Lists never count rows. */
  nextCursor: string | null;
}

export interface PostListQuery {
  status?: BlogPostStatus;
  authorId?: string;
  locale?: string;
  /** Opaque cursor: `nextCursor` of the previous page. */
  after?: string;
  perPage?: number;
  /** Column of the keyset order. Defaults to `CREATED_AT` on the owner side. */
  sortBy?: BlogPostSortField;
  /** Direction of the keyset order. Defaults to `DESC` on the owner side. */
  sortOrder?: BlogPostSortOrder;
}

export interface CreatePostInput {
  locale: string;
  title: string;
  content?: RichTextDocument;
  excerpt?: string;
  slug?: string;
  publish: boolean;
  tags: string[];
  categoryId?: string;
  featuredImageUrl?: string;
  seoTitle?: string;
  seoDescription?: string;
  channelSlugs?: string[];
  scheduledAt?: string;
}

export interface UpdatePostInput {
  locale?: string;
  title?: string;
  content?: RichTextDocument;
  excerpt?: string;
  slug?: string;
  status?: BlogPostStatus;
  tags?: string[];
  categoryId?: string | null;
  featuredImageUrl?: string;
  seoTitle?: string;
  seoDescription?: string;
  channelSlugs?: string[];
  version: number;
  scheduledAt?: string | null;
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
      scheduledAt
    }
    nextCursor
  }
}
`;

const POST_QUERY = `
query Post($tenantId: UUID!, $id: UUID!) {
  post(tenantId: $tenantId, id: $id) {
    id
    requestedLocale
    effectiveLocale
    title
    slug
    excerpt
    content {
      document
      html
    }
    contentPlainText
    status
    authorId
    categoryId
    categoryName
    channelSlugs
    version
    createdAt
    publishedAt
    scheduledAt
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
  if (query.after) filter.after = query.after;
  if (query.perPage) filter.perPage = query.perPage;
  // A cursor is bound to the sort it was issued for; the owner rejects a mismatch, so the
  // caller must drop `after` when it changes either half of the sort.
  if (query.sortBy) filter.sortBy = query.sortBy;
  if (query.sortOrder) filter.sortOrder = query.sortOrder;

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
