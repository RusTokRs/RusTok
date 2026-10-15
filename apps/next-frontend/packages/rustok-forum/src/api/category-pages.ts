import type { ForumCategoryListItem } from './forum';

// The Forum owner bounds the category tree at 512 nodes; GraphQL PaginationInput
// clamps each request to 100. The rail, overview and topic composer need the
// whole audience-visible tree, not an arbitrary first page of it.
const CATEGORY_PAGE_SIZE = 100;
const MAX_FORUM_CATEGORIES = 512;

interface ForumCategoryPage {
  items: ForumCategoryListItem[];
  total: number;
}

export interface ForumCategoryConnection {
  items: ForumCategoryListItem[];
  pageInfo: { totalCount: number };
}

export function readForumCategoryConnection(
  connection: ForumCategoryConnection | null | undefined,
): ForumCategoryPage {
  return {
    items: connection?.items ?? [],
    total: connection?.pageInfo.totalCount ?? 0,
  };
}

export async function collectForumCategoryPages(
  fetchPage: (offset: number, limit: number) => Promise<ForumCategoryPage>,
): Promise<ForumCategoryPage> {
  const items: ForumCategoryListItem[] = [];
  const seen = new Set<string>();
  let total: number | null = null;

  for (let offset = 0; offset < MAX_FORUM_CATEGORIES; offset += CATEGORY_PAGE_SIZE) {
    const page = await fetchPage(offset, CATEGORY_PAGE_SIZE);
    if (!Number.isSafeInteger(page.total) || page.total < 0 || page.total > MAX_FORUM_CATEGORIES ||
        (total !== null && page.total !== total)) {
      throw new Error('Forum category total changed during pagination');
    }
    total = page.total;
    const expectedLength = Math.min(CATEGORY_PAGE_SIZE, Math.max(0, total - offset));
    if (!Array.isArray(page.items) || page.items.length !== expectedLength) {
      throw new Error('Forum category page is incomplete');
    }
    for (const item of page.items) {
      if (seen.has(item.id)) throw new Error('Forum category page contains a duplicate');
      seen.add(item.id);
      items.push(item);
    }
    if (items.length === total) return { items, total };
  }

  throw new Error('Forum category pagination exceeded the owner limit');
}
