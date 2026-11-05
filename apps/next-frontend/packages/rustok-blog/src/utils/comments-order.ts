import type { BlogPublicComment } from '../api/posts';

export interface DisplayedComment {
  comment: BlogPublicComment;
  depth: number;
}

/**
 * Public comments can be paginated independently of their parents.
 * Always render every fetched comment, even when the parent is on a different
 * page; preserve all reachable nesting within the currently fetched page.
 */
export function orderThreadedComments(items: BlogPublicComment[]): DisplayedComment[] {
  const ids = new Set(items.map((item) => item.id));
  const children = new Map<string, BlogPublicComment[]>();
  const roots: BlogPublicComment[] = [];

  for (const comment of items) {
    const parent = comment.parentCommentId;
    if (!parent || !ids.has(parent) || parent === comment.id) {
      roots.push(comment);
    } else {
      const replies = children.get(parent) ?? [];
      replies.push(comment);
      children.set(parent, replies);
    }
  }

  const result: DisplayedComment[] = [];
  const seen = new Set<string>();
  const pending = roots.slice().reverse().map((comment) => ({ comment, depth: 0 }));
  let remaining = 0;
  // An explicit stack keeps deep projections out of the JavaScript call stack.
  // Fetched-page order also determines cycle roots and bounded indentation.
  while (pending.length > 0 || remaining < items.length) {
    const { comment, depth } = pending.pop() ?? { comment: items[remaining++], depth: 0 };
    if (seen.has(comment.id)) continue;
    seen.add(comment.id);
    result.push({ comment, depth: Math.min(depth, 3) });
    const replies = children.get(comment.id) ?? [];
    for (let index = replies.length - 1; index >= 0; index--) {
      pending.push({ comment: replies[index], depth: depth + 1 });
    }
  }
  return result;
}
