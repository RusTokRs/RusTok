const API_URL = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:5150';

export interface PublicPostSummary {
  id: string;
  title: string;
  slug: string;
  excerpt: string | null;
  featured_image_url: string | null;
  author_name: string | null;
  tags: string[];
  published_at: string | null;
}

export interface PublicPostListResponse {
  items: PublicPostSummary[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export async function fetchPublishedPosts(
  page = 1,
  perPage = 6,
  tenantSlug?: string
): Promise<PublicPostListResponse> {
  const params = new URLSearchParams({
    status: 'Published',
    page: String(page),
    per_page: String(perPage),
    sort_by: 'published_at',
    sort_order: 'desc'
  });

  const headers: Record<string, string> = {};
  if (tenantSlug) headers['X-Tenant-Slug'] = tenantSlug;

  const res = await fetch(`${API_URL}/api/blog/posts?${params}`, {
    headers,
    next: { revalidate: 60 }
  });
  if (!res.ok) throw new Error(`fetchPublishedPosts failed: ${res.status}`);
  return res.json();
}
