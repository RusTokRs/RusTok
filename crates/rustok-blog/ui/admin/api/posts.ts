const API_URL = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:5150';

interface FetchOptions {
  token?: string | null;
  tenantSlug?: string | null;
}

function buildHeaders(opts: FetchOptions): Record<string, string> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (opts.token) headers['Authorization'] = `Bearer ${opts.token}`;
  if (opts.tenantSlug) headers['X-Tenant-Slug'] = opts.tenantSlug;
  return headers;
}

// ---------- Types (mirrors Rust DTOs) ----------

export type BlogPostStatus = 'Draft' | 'Published' | 'Archived';

export interface PostSummary {
  id: string;
  title: string;
  slug: string;
  locale: string;
  effective_locale: string;
  excerpt: string | null;
  status: BlogPostStatus;
  author_id: string;
  author_name: string | null;
  category_id: string | null;
  category_name: string | null;
  tags: string[];
  featured_image_url: string | null;
  comment_count: number;
  published_at: string | null;
  created_at: string;
}

export interface PostResponse {
  id: string;
  tenant_id: string;
  author_id: string;
  title: string;
  slug: string;
  locale: string;
  effective_locale: string;
  available_locales: string[];
  body: string;
  body_format: string;
  excerpt: string | null;
  status: BlogPostStatus;
  category_id: string | null;
  category_name: string | null;
  tags: string[];
  featured_image_url: string | null;
  seo_title: string | null;
  seo_description: string | null;
  metadata: unknown;
  comment_count: number;
  view_count: number;
  created_at: string;
  updated_at: string;
  published_at: string | null;
  version: number;
}

export interface PostListResponse {
  items: PostSummary[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface PostListQuery {
  status?: BlogPostStatus;
  category_id?: string;
  tag?: string;
  author_id?: string;
  search?: string;
  locale?: string;
  page?: number;
  per_page?: number;
  sort_by?: string;
  sort_order?: 'asc' | 'desc';
}

export interface CreatePostInput {
  locale: string;
  title: string;
  body: string;
  excerpt?: string;
  slug?: string;
  publish: boolean;
  tags: string[];
  category_id?: string;
  featured_image_url?: string;
  seo_title?: string;
  seo_description?: string;
  metadata?: unknown;
}

export interface UpdatePostInput {
  locale?: string;
  title?: string;
  body?: string;
  excerpt?: string;
  slug?: string;
  tags?: string[];
  category_id?: string;
  featured_image_url?: string;
  seo_title?: string;
  seo_description?: string;
  metadata?: unknown;
  version?: number;
}

// ---------- API functions ----------

export async function listPosts(
  query: PostListQuery,
  opts: FetchOptions = {}
): Promise<PostListResponse> {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== null) params.set(key, String(value));
  }
  const res = await fetch(`${API_URL}/api/blog/posts?${params}`, {
    headers: buildHeaders(opts),
    cache: 'no-store'
  });
  if (!res.ok) throw new Error(`listPosts failed: ${res.status}`);
  return res.json();
}

export async function getPost(
  id: string,
  locale: string = 'en',
  opts: FetchOptions = {}
): Promise<PostResponse> {
  const res = await fetch(`${API_URL}/api/blog/posts/${id}?locale=${locale}`, {
    headers: buildHeaders(opts),
    cache: 'no-store'
  });
  if (!res.ok) throw new Error(`getPost failed: ${res.status}`);
  return res.json();
}

export async function createPost(
  input: CreatePostInput,
  opts: FetchOptions = {}
): Promise<string> {
  const res = await fetch(`${API_URL}/api/blog/posts`, {
    method: 'POST',
    headers: buildHeaders(opts),
    body: JSON.stringify(input)
  });
  if (!res.ok) throw new Error(`createPost failed: ${res.status}`);
  return res.json();
}

export async function updatePost(
  id: string,
  input: UpdatePostInput,
  opts: FetchOptions = {}
): Promise<void> {
  const res = await fetch(`${API_URL}/api/blog/posts/${id}`, {
    method: 'PUT',
    headers: buildHeaders(opts),
    body: JSON.stringify(input)
  });
  if (!res.ok) throw new Error(`updatePost failed: ${res.status}`);
}

export async function deletePost(
  id: string,
  opts: FetchOptions = {}
): Promise<void> {
  const res = await fetch(`${API_URL}/api/blog/posts/${id}`, {
    method: 'DELETE',
    headers: buildHeaders(opts)
  });
  if (!res.ok) throw new Error(`deletePost failed: ${res.status}`);
}

export async function publishPost(
  id: string,
  opts: FetchOptions = {}
): Promise<void> {
  const res = await fetch(`${API_URL}/api/blog/posts/${id}/publish`, {
    method: 'POST',
    headers: buildHeaders(opts)
  });
  if (!res.ok) throw new Error(`publishPost failed: ${res.status}`);
}

export async function unpublishPost(
  id: string,
  opts: FetchOptions = {}
): Promise<void> {
  const res = await fetch(`${API_URL}/api/blog/posts/${id}/unpublish`, {
    method: 'POST',
    headers: buildHeaders(opts)
  });
  if (!res.ok) throw new Error(`unpublishPost failed: ${res.status}`);
}
