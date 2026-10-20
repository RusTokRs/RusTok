/** Interactive navigation is bounded per click, unlike a background scan of a whole thread. */
export const MAX_THREAD_NAVIGATION_PAGES = 20;
export type ThreadPostTarget = number | 'last';
export type ThreadJump = { post: number; complete: boolean };
export type ThreadReplyPage<T> = { items: T[]; nextCursor: string | null };

/** Keep the loaded prefix in owner order; the opaque cursor, not replyCount, proves the end. */
export async function loadThreadReplyPages<T extends { id: string }>({
  items,
  nextCursor,
  target,
  loadPage,
  isCurrent,
}: ThreadReplyPage<T> & {
  target?: ThreadPostTarget;
  loadPage: (after: string) => Promise<ThreadReplyPage<T>>;
  isCurrent: () => boolean;
}): Promise<ThreadReplyPage<T> | null> {
  if (
    target !== undefined &&
    target !== 'last' &&
    (!Number.isSafeInteger(target) || target < 1)
  ) {
    throw new Error('Invalid thread post number');
  }
  const merged = new Map(items.map((item) => [item.id, item]));
  const seen = new Set<string>();
  let cursor = nextCursor;
  const budget = target === undefined ? 1 : MAX_THREAD_NAVIGATION_PAGES;
  for (let pages = 0; cursor && pages < budget; pages++) {
    if (target !== undefined && target !== 'last' && merged.size + 1 >= target)
      break;
    if (!isCurrent()) return null;
    seen.add(cursor);
    const page = await loadPage(cursor);
    if (!isCurrent()) return null;
    if (page.nextCursor !== null && seen.has(page.nextCursor)) {
      throw new Error('Forum reply cursor did not advance');
    }
    for (const item of page.items) merged.set(item.id, item);
    cursor = page.nextCursor;
  }
  return isCurrent()
    ? { items: Array.from(merged.values()), nextCursor: cursor }
    : null;
}

export function threadJumpResult<T>(
  page: ThreadReplyPage<T>,
  target: ThreadPostTarget,
): ThreadJump {
  const lastLoadedPost = page.items.length + 1;
  return {
    post: target === 'last' ? lastLoadedPost : Math.min(target, lastLoadedPost),
    complete:
      page.nextCursor === null ||
      (target !== 'last' && target <= lastLoadedPost),
  };
}

/** Cursor exhaustion is the exact visible total; before then, owner counts are only an estimate. */
export function threadPostTotal(
  replies: number,
  ownerTotal: number,
  hasMore: boolean,
): number {
  return hasMore ? Math.max(replies + 2, ownerTotal + 1) : replies + 1;
}

/** Find the post crossing the reading line in O(log n), including tall posts. */
export function findThreadPostIndex(
  count: number,
  topAt: (index: number) => number,
  line: number,
): number {
  if (count === 0) return -1;
  let low = 0;
  let high = count;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (topAt(middle) <= line) low = middle + 1;
    else high = middle;
  }
  return Math.max(0, low - 1);
}
