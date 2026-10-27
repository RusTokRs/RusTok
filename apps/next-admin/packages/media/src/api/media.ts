import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

// Types matching the rustok-media GraphQL schema (camelCase).
export interface MediaItem {
  id: string;
  tenantId: string;
  ownerModule: string;
  filename: string;
  originalName: string;
  mimeType: string;
  size: number;
  publicUrl: string;
  width: number | null;
  height: number | null;
  createdAt: string;
}

export interface MediaListResponse {
  items: MediaItem[];
  total: number;
}

export interface MediaUsage {
  fileCount: number;
  totalBytes: number;
}

function requireTenantId(opts: GqlOpts): string {
  if (!opts.tenantId) {
    throw new Error('Media queries require a tenant id in the session');
  }
  return opts.tenantId;
}

export async function listMedia(
  opts: GqlOpts = {},
  pagination: { page?: number; perPage?: number } = {}
): Promise<MediaListResponse> {
  const tenantId = requireTenantId(opts);
  const page = Math.max(1, pagination.page ?? 1);
  const limit = Math.min(100, Math.max(1, pagination.perPage ?? 24));
  const data = await graphqlRequest<
    { tenantId: string; pagination: { offset: number; limit: number } },
    { media: MediaListResponse }
  >(
    `
      query MediaList($tenantId: UUID!, $pagination: PaginationInput!) {
        media(tenantId: $tenantId, pagination: $pagination) {
          items {
            id tenantId ownerModule filename originalName mimeType size
            publicUrl width height createdAt
          }
          total
        }
      }
    `,
    { tenantId, pagination: { offset: (page - 1) * limit, limit } },
    opts.token,
    opts.tenantSlug
  );
  return data.media;
}

export async function mediaUsage(opts: GqlOpts = {}): Promise<MediaUsage> {
  const tenantId = requireTenantId(opts);
  const data = await graphqlRequest<
    { tenantId: string },
    { mediaUsage: MediaUsage }
  >(
    `
      query MediaUsage($tenantId: UUID!) {
        mediaUsage(tenantId: $tenantId) { fileCount totalBytes }
      }
    `,
    { tenantId },
    opts.token,
    opts.tenantSlug
  );
  return data.mediaUsage;
}

export async function deleteMedia(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const tenantId = requireTenantId(opts);
  const data = await graphqlRequest<
    { tenantId: string; id: string },
    { deleteMedia: boolean }
  >(
    `
      mutation DeleteMedia($tenantId: UUID!, $id: UUID!) {
        deleteMedia(tenantId: $tenantId, id: $id)
      }
    `,
    { tenantId, id },
    opts.token,
    opts.tenantSlug
  );
  return data.deleteMedia;
}
