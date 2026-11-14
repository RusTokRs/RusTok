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

export interface PageStarterSummary {
  key: string;
  name: string;
  category: string;
  description: string | null;
  defaultTemplateKey: string;
  revision: string | null;
}

export interface PageStarterMetadataUpdate {
  key: string;
  name: string;
  category: string;
  description: string | null;
  defaultTemplateKey: string;
  expectedRevision: string;
}

export interface SaveStarterFromPageInput {
  pageId: string;
  locale: string;
  key: string;
  name: string;
  category: string;
  description?: string;
}

export interface SavedStarterSnapshot {
  sourceLocale: string;
  sourceState: string;
  sourceRevision: string;
}

export interface CreatePageInput {
  locale: string;
  title: string;
  slug?: string;
  template?: string;
  channelSlugs?: string[];
  document?: Record<string, unknown>;
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

export const PAGE_LAYOUT_KEYS_QUERY = `
  query PageTemplateKeys($locale: String!, $tenantId: UUID) {
    pageTemplateKeys(locale: $locale, tenantId: $tenantId)
  }
`;

export const PAGE_STARTERS_QUERY = `
  query PageStarters($tenantId: UUID) {
    pageStarters(tenantId: $tenantId) {
      key name category description defaultTemplateKey revision
    }
  }
`;

export const UPDATE_STARTER_METADATA_MUTATION = `
  mutation UpdatePageStarterMetadata($input: UpdateGqlPageStarterMetadataInput!, $tenantId: UUID) {
    updatePageStarterMetadata(input: $input, tenantId: $tenantId)
  }
`;

export const DELETE_STARTER_MUTATION = `
  mutation DeletePageStarter($key: String!, $expectedRevision: UUID!, $tenantId: UUID) {
    deletePageStarter(key: $key, expectedRevision: $expectedRevision, tenantId: $tenantId)
  }
`;

export const SAVE_STARTER_FROM_PAGE_MUTATION = `
  mutation SaveStarterFromPage($input: SavePageStarterFromPageGqlInput!, $tenantId: UUID) {
    savePageStarterFromPage(input: $input, tenantId: $tenantId) { sourceLocale sourceState sourceRevision }
  }
`;

export const INSTANTIATE_PAGE_STARTER_MUTATION = `
  mutation InstantiatePageStarter($key: String!, $expectedRevision: UUID, $locale: String!, $tenantId: UUID) {
    instantiatePageStarter(key: $key, expectedRevision: $expectedRevision, locale: $locale, tenantId: $tenantId)
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

export async function listPageLayoutKeys(locale: string, opts: GqlOpts = {}): Promise<string[]> {
  if (!locale.trim()) throw new Error('Page locale cannot be empty');
  const data = await graphqlRequest<
    { locale: string; tenantId?: string },
    { pageTemplateKeys: unknown }
  >(PAGE_LAYOUT_KEYS_QUERY, { locale, tenantId: opts.tenantId ?? undefined }, opts.token, opts.tenantSlug);
  if (!Array.isArray(data.pageTemplateKeys) || !data.pageTemplateKeys.every((key) => typeof key === 'string' && key.length > 0)) {
    throw new Error('Invalid layout catalog response');
  }
  return data.pageTemplateKeys as string[];
}

export async function listPageStarters(opts: GqlOpts = {}): Promise<PageStarterSummary[]> {
  const data = await graphqlRequest<
    { tenantId?: string },
    { pageStarters: PageStarterSummary[] }
  >(PAGE_STARTERS_QUERY, { tenantId: opts.tenantId ?? undefined }, opts.token, opts.tenantSlug);
  return data.pageStarters;
}

export async function updateStarterMetadata(
  input: PageStarterMetadataUpdate,
  opts: GqlOpts = {}
): Promise<string> {
  if (!input.expectedRevision) throw new Error('An observed starter revision is required');
  const data = await graphqlRequest<
    { input: PageStarterMetadataUpdate; tenantId?: string },
    { updatePageStarterMetadata: string }
  >(UPDATE_STARTER_METADATA_MUTATION, { input, tenantId: opts.tenantId ?? undefined }, opts.token, opts.tenantSlug);
  return data.updatePageStarterMetadata;
}

export async function deleteStarter(
  key: string,
  expectedRevision: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  if (!expectedRevision) throw new Error('An observed starter revision is required');
  const data = await graphqlRequest<
    { key: string; expectedRevision: string; tenantId?: string },
    { deletePageStarter: boolean }
  >(DELETE_STARTER_MUTATION, { key, expectedRevision, tenantId: opts.tenantId ?? undefined }, opts.token, opts.tenantSlug);
  return data.deletePageStarter;
}

export async function saveStarterFromPage(
  input: SaveStarterFromPageInput,
  opts: GqlOpts = {}
): Promise<SavedStarterSnapshot> {
  const data = await graphqlRequest<
    { input: SaveStarterFromPageInput; tenantId?: string },
    { savePageStarterFromPage: SavedStarterSnapshot }
  >(SAVE_STARTER_FROM_PAGE_MUTATION, { input, tenantId: opts.tenantId ?? undefined }, opts.token, opts.tenantSlug);
  return data.savePageStarterFromPage;
}

export async function instantiatePageStarter(
  starter: PageStarterSummary,
  locale: string,
  opts: GqlOpts = {}
): Promise<Record<string, unknown>> {
  const data = await graphqlRequest<
    { key: string; expectedRevision: string | null; locale: string; tenantId?: string },
    { instantiatePageStarter: string }
  >(
    INSTANTIATE_PAGE_STARTER_MUTATION,
    { key: starter.key, expectedRevision: starter.revision, locale, tenantId: opts.tenantId ?? undefined },
    opts.token,
    opts.tenantSlug
  );
  const document: unknown = JSON.parse(data.instantiatePageStarter);
  if (!document || typeof document !== 'object' || Array.isArray(document)) {
    throw new Error('The starter returned an invalid document');
  }
  return document as Record<string, unknown>;
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
        checkedLayout: true,
        body: input.document ? { locale: input.locale.trim(), document: input.document } : null,
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
