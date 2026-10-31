import { graphqlRequest } from '@/lib/graphql';

// ---------- GqlOpts ----------

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

// ---------- Types (matches the rustok-pages GraphQL schema, camelCase) ----------

export type PageStatus = 'draft' | 'published' | 'archived';

export interface PageListItem {
  id: string;
  status: PageStatus;
  template: string;
  title: string | null;
  slug: string | null;
  channelSlugs: string[];
  updatedAt: string;
}

export interface PageListResponse {
  items: PageListItem[];
  total: number;
}

export interface PageResponse {
  id: string;
  version: number;
  status: PageStatus;
  template: string;
  translation: {
    locale: string;
    title: string | null;
    slug: string | null;
  } | null;
}

export interface CreatePageInput {
  locale: string;
  title: string;
  slug?: string;
  template?: string;
  channelSlugs?: string[];
}

// ---------- Queries & mutations ----------

export const PAGE_LIST_FIELDS = `
  id
  status
  template
  title
  slug
  channelSlugs
  updatedAt
`;

export const PAGES_LIST_QUERY = `
  query PagesList($filter: ListGqlPagesFilter, $tenantId: UUID) {
    pages(filter: $filter, tenantId: $tenantId) {
      items { ${PAGE_LIST_FIELDS} }
      total
    }
  }
`;

export const CREATE_PAGE_MUTATION = `
  mutation CreatePage($input: CreateGqlPageInput!, $tenantId: UUID) {
    createPage(input: $input, tenantId: $tenantId) {
      id
      version
      status
      template
      translation { locale title slug }
    }
  }
`;

export const UNPUBLISH_PAGE_MUTATION = `
  mutation UnpublishPage($id: UUID!, $expectedVersion: Int, $tenantId: UUID) {
    unpublishPage(id: $id, expectedVersion: $expectedVersion, tenantId: $tenantId) {
      id
      version
      status
      template
      translation { locale title slug }
    }
  }
`;

export const DELETE_PAGE_MUTATION = `
  mutation DeletePage($id: UUID!, $tenantId: UUID) {
    deletePage(id: $id, tenantId: $tenantId)
  }
`;

export async function listPages(
  filter: {
    locale?: string;
    template?: string;
    page?: number;
    perPage?: number;
  } = {},
  opts: GqlOpts = {}
): Promise<PageListResponse> {
  const normalizedFilter = {
    ...filter,
    ...(filter.page !== undefined ? { page: Math.max(1, filter.page) } : {}),
    ...(filter.perPage !== undefined
      ? { perPage: Math.min(100, Math.max(1, filter.perPage)) }
      : {})
  };
  const data = await graphqlRequest<
    { filter?: Record<string, unknown>; tenantId?: string },
    { pages: PageListResponse }
  >(
    PAGES_LIST_QUERY,
    { filter: normalizedFilter, tenantId: opts.tenantId ?? undefined },
    opts.token,
    opts.tenantSlug
  );
  return data.pages;
}

export async function createPage(
  input: CreatePageInput,
  opts: GqlOpts = {}
): Promise<PageResponse> {
  if (!input.title || !input.title.trim()) {
    throw new Error('Page title cannot be empty');
  }
  if (!input.locale || !input.locale.trim()) {
    throw new Error('Page locale cannot be empty');
  }

  const data = await graphqlRequest<
    { input: Record<string, unknown>; tenantId?: string },
    { createPage: PageResponse }
  >(
    CREATE_PAGE_MUTATION,
    {
      input: {
        translations: [
          {
            locale: input.locale.trim(),
            title: input.title.trim(),
            slug: input.slug ?? null,
            metaTitle: null,
            metaDescription: null
          }
        ],
        template: input.template ?? null,
        body: null,
        channelSlugs: input.channelSlugs ?? null,
        // Publishing from a list surface is deliberately impossible: publish uses the
        // builder's reviewed runtime flow, and the server rejects publish=true here.
        publish: false
      },
      tenantId: opts.tenantId ?? undefined
    },
    opts.token,
    opts.tenantSlug
  );
  return data.createPage;
}

export async function unpublishPage(
  id: string,
  expectedVersion?: number,
  opts: GqlOpts = {}
): Promise<PageResponse> {
  if (!id || !id.trim()) {
    throw new Error('Page id cannot be empty');
  }

  const data = await graphqlRequest<
    { id: string; expectedVersion: number | null; tenantId?: string },
    { unpublishPage: PageResponse }
  >(
    UNPUBLISH_PAGE_MUTATION,
    {
      id: id.trim(),
      expectedVersion: expectedVersion ?? null,
      tenantId: opts.tenantId ?? undefined
    },
    opts.token,
    opts.tenantSlug
  );
  return data.unpublishPage;
}

export async function deletePage(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  if (!id || !id.trim()) {
    throw new Error('Page id cannot be empty');
  }

  const data = await graphqlRequest<
    { id: string; tenantId?: string },
    { deletePage: boolean }
  >(
    DELETE_PAGE_MUTATION,
    { id: id.trim(), tenantId: opts.tenantId ?? undefined },
    opts.token,
    opts.tenantSlug
  );
  return data.deletePage;
}

// ---------- Builder & Storefront URLs ----------

export { builderUrlForPage, storefrontUrlForPage } from './urls';
