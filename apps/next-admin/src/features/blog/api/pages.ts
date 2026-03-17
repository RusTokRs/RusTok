import { graphqlRequest } from '@/lib/graphql';
import type { GqlOpts } from './posts';

export interface PageBlock {
  id: string;
  blockType: string;
  position: number;
  data: Record<string, unknown>;
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
    { tenantId: string; pageId: string; input: { blockType: string; position: number; data: Record<string, unknown> } },
    { addBlock: PageBlock }
  >(mutation, { tenantId: opts.tenantId!, pageId, input }, opts.token, opts.tenantSlug);

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
    { tenantId: string; blockId: string; input: { position?: number; data?: Record<string, unknown> } },
    { updateBlock: PageBlock }
  >(mutation, { tenantId: opts.tenantId!, blockId, input }, opts.token, opts.tenantSlug);

  return data.updateBlock;
}

export async function reorderPageBlocks(pageId: string, blockIds: string[], opts: GqlOpts = {}): Promise<boolean> {
  const mutation = `
    mutation ReorderBlocks($tenantId: UUID!, $pageId: UUID!, $input: ReorderBlocksInput!) {
      reorderBlocks(tenantId: $tenantId, pageId: $pageId, input: $input)
    }
  `;

  const data = await graphqlRequest<
    { tenantId: string; pageId: string; input: { blockIds: string[] } },
    { reorderBlocks: boolean }
  >(mutation, { tenantId: opts.tenantId!, pageId, input: { blockIds } }, opts.token, opts.tenantSlug);

  return data.reorderBlocks;
}
