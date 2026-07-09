import { graphqlRequest } from '@/lib/graphql';
import type { GqlOpts } from './posts';

export interface PageBlock {
  id: string;
  blockType: string;
  position: number;
  data: Record<string, unknown>;
}

export interface PageSummary {
  id: string;
  status: string;
  template: string;
  title: string | null;
  slug: string | null;
  updatedAt: string;
}

export interface PageBody {
  locale: string;
  content: string;
  format: string;
  contentJson?: Record<string, unknown> | null;
  updatedAt: string;
}

export interface PageDetail {
  id: string;
  status: string;
  template: string;
  blocks: PageBlock[];
  translation?: {
    locale: string;
    title: string | null;
    slug: string | null;
  } | null;
  body?: PageBody | null;
}

export async function listPages(
  opts: GqlOpts = {},
  filter: { locale?: string; page?: number; perPage?: number } = {}
): Promise<PageSummary[]> {
  const query = `
    query Pages($tenantId: UUID!, $filter: ListGqlPagesFilter) {
      pages(tenantId: $tenantId, filter: $filter) {
        items {
          id
          status
          template
          title
          slug
          updatedAt
        }
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId: string;
      filter?: { locale?: string; page?: number; perPage?: number };
    },
    { pages: { items: PageSummary[] } }
  >(
    query,
    {
      tenantId: opts.tenantId!,
      filter: {
        locale: filter.locale,
        page: filter.page ?? 1,
        perPage: filter.perPage ?? 50
      }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.pages.items;
}

export async function getPage(
  pageId: string,
  opts: GqlOpts = {},
  locale?: string
): Promise<PageDetail | null> {
  const query = `
    query Page($tenantId: UUID!, $id: UUID!, $locale: String) {
      page(tenantId: $tenantId, id: $id, locale: $locale) {
        id
        status
        template
        translation {
          locale
          title
          slug
        }
        body {
          locale
          content
          format
          contentJson
          updatedAt
        }
        blocks {
          id
          blockType
          position
          data
        }
      }
    }
  `;

  const data = await graphqlRequest<
    { tenantId: string; id: string; locale?: string },
    { page: PageDetail | null }
  >(
    query,
    { tenantId: opts.tenantId!, id: pageId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.page;
}

export async function updatePageBody(
  pageId: string,
  body: {
    locale: string;
    format: string;
    content?: string;
    contentJson: Record<string, unknown>;
  },
  opts: GqlOpts = {}
): Promise<PageBody> {
  const mutation = `
    mutation UpdatePageBody($tenantId: UUID!, $id: UUID!, $input: UpdateGqlPageInput!) {
      updatePage(tenantId: $tenantId, id: $id, input: $input) {
        body {
          locale
          content
          format
          contentJson
          updatedAt
        }
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId: string;
      id: string;
      input: {
        body: {
          locale: string;
          format: string;
          content: string;
          contentJson: Record<string, unknown>;
        };
      };
    },
    { updatePage: { body: PageBody | null } }
  >(
    mutation,
    {
      tenantId: opts.tenantId!,
      id: pageId,
      input: {
        body: {
          locale: body.locale,
          format: body.format,
          content: body.content ?? '',
          contentJson: body.contentJson
        }
      }
    },
    opts.token,
    opts.tenantSlug
  );

  if (!data.updatePage.body) {
    throw new Error('Page body was not returned after update');
  }

  return data.updatePage.body;
}

export async function addPageBlock(
  pageId: string,
  input: { blockType: string; position: number; data: Record<string, unknown> },
  opts: GqlOpts = {}
): Promise<PageBlock> {
  const mutation = `
    mutation AddBlock($tenantId: UUID!, $pageId: UUID!, $input: CreateGqlBlockInput!) {
      addBlock(tenantId: $tenantId, pageId: $pageId, input: $input) {
        id
        blockType
        position
        data
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId: string;
      pageId: string;
      input: {
        blockType: string;
        position: number;
        data: Record<string, unknown>;
      };
    },
    { addBlock: PageBlock }
  >(
    mutation,
    { tenantId: opts.tenantId!, pageId, input },
    opts.token,
    opts.tenantSlug
  );

  return data.addBlock;
}

export async function updatePageBlock(
  blockId: string,
  input: { position?: number; data?: Record<string, unknown> },
  opts: GqlOpts = {}
): Promise<PageBlock> {
  const mutation = `
    mutation UpdateBlock($tenantId: UUID!, $blockId: UUID!, $input: UpdateGqlBlockInput!) {
      updateBlock(tenantId: $tenantId, blockId: $blockId, input: $input) {
        id
        blockType
        position
        data
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId: string;
      blockId: string;
      input: { position?: number; data?: Record<string, unknown> };
    },
    { updateBlock: PageBlock }
  >(
    mutation,
    { tenantId: opts.tenantId!, blockId, input },
    opts.token,
    opts.tenantSlug
  );

  return data.updateBlock;
}

export async function reorderPageBlocks(
  pageId: string,
  blockIds: string[],
  opts: GqlOpts = {}
): Promise<boolean> {
  const mutation = `
    mutation ReorderBlocks($tenantId: UUID!, $pageId: UUID!, $input: ReorderBlocksInput!) {
      reorderBlocks(tenantId: $tenantId, pageId: $pageId, input: $input)
    }
  `;

  const data = await graphqlRequest<
    { tenantId: string; pageId: string; input: { blockIds: string[] } },
    { reorderBlocks: boolean }
  >(
    mutation,
    { tenantId: opts.tenantId!, pageId, input: { blockIds } },
    opts.token,
    opts.tenantSlug
  );

  return data.reorderBlocks;
}
